//! 多人协议调用的唯一出口：连接 / 建房 / 进房 / 就绪 / 开始 / 管理 / 聊天 /
//! 离开 / 谱面分享 / 成绩上报。这里只做“发命令 + 记录任务句柄 + 错误提示”，
//! 不含任何 UI 布局。

use std::{
    fs::File,
    path::Path,
    sync::{atomic::Ordering, Arc},
};

use anyhow::{anyhow, Context};
use phira_mp_client::{Client, LocalChartEvent};
use phira_mp_common::{RoomId, RoomState};
use prpr::{
    info::ChartInfo,
    scene::{show_error, show_message},
    task::Task,
};
use tokio::net::TcpStream;
use tracing::warn;

use super::state::{DownloadIntent, MpState};
use crate::{
    client::{Chart, Ptr},
    dir, get_data,
    mp::{serve, L10N_LOCAL},
    scene::{SongScene, LAST_MP_FINISH},
};

/// 谱面下载流程的推进结果。
pub enum DownloadStep {
    /// 还没有结果
    Pending,
    /// 谱面已在本地，按 intent 继续后续动作
    Continue(DownloadIntent),
    /// 用户取消了下载，流程终止（intent 一并丢弃）
    Cancelled,
}

impl MpState {
    // ---------- 连接 / 大厅 ----------

    /// 连接服务器并鉴权。深链接可携带服务器地址（`phira://...?server=xxx`），优先使用。
    pub fn connect(&mut self) {
        let Some(token) = get_data().tokens.as_ref().map(|it| it.0.clone()) else {
            show_message(mtl!("connect-must-login")).error();
            return;
        };
        let addr = self
            .deep_link
            .as_ref()
            .and_then(|l| l.server.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| get_data().config.mp_address.clone());
        self.connect_task = Some(Task::new(async move {
            let client = Client::new(TcpStream::connect(addr).await?).await?;
            client
                .authenticate(token)
                .await
                .with_context(|| anyhow!(mtl!("connect-authenticate-failed")))?;
            Ok(client)
        }));
    }

    pub fn create_room(&mut self, id: RoomId) {
        let client = self.connected();
        self.create_room_task = Some(Task::new(async move {
            client.create_room(id).await?;
            Ok(())
        }));
    }

    /// 以玩家身份加入房间。
    pub fn join_room(&mut self, id: RoomId) {
        let client = self.connected();
        self.join_room_task = Some(Task::new(async move {
            client.join_room(id, false).await?;
            client.room_state().await.ok_or_else(|| anyhow!("expected room state"))
        }));
    }

    /// 带密码加入房间（“加入失败且提示需要密码”后的一次重试）。
    pub fn join_room_with_password(&mut self, id: RoomId, password: String) {
        let client = self.connected();
        self.join_room_task = Some(Task::new(async move {
            client.join_room_with_password(id, false, password).await?;
            client.room_state().await.ok_or_else(|| anyhow!("expected room state"))
        }));
    }

    /// 以观战者身份（monitor）加入房间：只读旁观，不占玩家位、不参与就绪。
    /// 返回 false 表示房间号非法（已提示）。
    pub fn join_room_as_spectator(&mut self, room_id: &str) -> bool {
        let Ok(id) = room_id.to_owned().try_into() else {
            show_message(mtl!("join-room-invalid-id")).error();
            return false;
        };
        let client = self.connected();
        self.join_room_task = Some(Task::new(async move {
            client.join_room(id, true).await?;
            client.room_state().await.ok_or_else(|| anyhow!("expected room state"))
        }));
        true
    }

    pub fn leave_room(&mut self) {
        let client = self.connected();
        self.task = Some(Task::new(async move { client.leave_room().await }));
    }

    /// 拉取公共房间列表（GET {web}/api/rooms）。
    pub fn load_room_list(&mut self) {
        if self.room_list_task.is_some() {
            return;
        }
        let Some(base) = Self::web_base() else {
            show_message(mtl!("room-list-failed")).error();
            return;
        };
        self.room_list_task = Some(Task::new(async move {
            let url = format!("{base}/api/rooms");
            let resp = reqwest::get(&url).await?.error_for_status()?;
            let rooms: Vec<super::state::PublicRoom> = resp.json().await?;
            Ok(rooms)
        }));
    }

    // ---------- 房间管理 ----------

    pub fn lock_room(&mut self, to: bool) {
        let client = self.connected();
        self.task = Some(Task::new(async move { client.lock_room(to).await.with_context(|| mtl!("lock-room-failed")) }));
    }

    pub fn cycle_room(&mut self, to: bool) {
        let client = self.connected();
        self.task = Some(Task::new(async move { client.cycle_room(to).await.with_context(|| mtl!("cycle-room-failed")) }));
    }

    /// 设置房间密码（空串 = 清除密码）。
    pub fn set_room_password(&mut self, password: String) {
        let client = self.connected();
        self.task = Some(Task::new(async move {
            client.set_room_password(password).await.with_context(|| mtl!("set-password-failed"))
        }));
    }

    pub fn kick_user(&mut self, id: i32) {
        let client = self.connected();
        self.task = Some(Task::new(async move {
            client.kick_user(id).await.with_context(|| mtl!("kick-user-failed"))
        }));
    }

    pub fn transfer_host(&mut self, id: i32) {
        let client = self.connected();
        self.task = Some(Task::new(async move {
            client.transfer_host(id).await.with_context(|| mtl!("transfer-host-failed"))
        }));
    }

    pub fn send_chat(&mut self, text: String) {
        let client = self.connected();
        self.chat_task = Some(Task::new(async move { client.chat(text).await }));
    }

    // ---------- 谱面选择 ----------

    /// 选择在线谱面（仅房主，且必须在选谱/本地谱阶段）。
    pub fn select_chart(&mut self, id: i32) {
        let Some(client) = self.client() else { return };
        if !client.blocking_is_host().unwrap_or(false) {
            show_message(mtl!("select-chart-host-only")).error();
            return;
        }
        if !matches!(client.blocking_room_state(), Some(RoomState::SelectChart(_) | RoomState::LocalChart)) {
            show_message(mtl!("select-chart-not-now")).error();
            return;
        }
        // 切换到在线谱面：清除之前选择的本地谱面
        self.local_chart = None;
        self.reset_local_chart();
        self.task = Some(Task::new(async move {
            client.select_online_chart(id).await.with_context(|| mtl!("select-chart-failed"))?;
            Ok(())
        }));
    }

    /// 从谱面库中选择本地谱面进行分享（仅房主，且必须在选谱阶段）。
    pub fn select_local_chart(&mut self, local_path: String, name: String) {
        if !Self::server_allows_local_chart() {
            show_message(mtl!("mp-server-no-local-chart")).error();
            return;
        }
        let Some(client) = self.client() else { return };
        if !client.blocking_is_host().unwrap_or(false) {
            show_message(mtl!("select-chart-host-only")).error();
            return;
        }
        if !matches!(client.blocking_room_state(), Some(RoomState::SelectChart(_))) {
            show_message(mtl!("select-chart-not-now")).error();
            return;
        }
        self.local_chart_task = Some(Task::new(async move {
            let uuid = uuid::Uuid::new_v4().to_string();
            // 把本地谱面复制到 download/{uuid}
            serve::stage_local_chart(&local_path, &uuid)?;
            client.select_local_chart(uuid, name).await.with_context(|| mtl!("select-chart-failed"))?;
            Ok(())
        }));
    }

    // ---------- 就绪 / 开始 ----------

    /// 房主请求开始。
    pub fn request_start(&mut self) {
        let Some(client) = self.client() else { return };
        let Some(state) = client.blocking_room_state() else { return };
        // LocalChart 状态下房主已选择本地谱面：直接请求开始（服务端会通知房主启动上传）
        if matches!(state, RoomState::LocalChart) {
            if !Self::server_allows_local_chart() {
                show_message(mtl!("mp-server-no-local-chart")).error();
                return;
            }
            self.host_started = true;
            self.task = Some(Task::new(async move {
                client.request_start().await.with_context(|| mtl!("request-start-failed"))?;
                Ok(())
            }));
            return;
        }
        if matches!(state, RoomState::SelectChart(None)) {
            show_message(mtl!("request-start-no-chart")).error();
            return;
        }
        if let Some(id) = self.chart_id {
            self.fetch_chart(id, DownloadIntent::RequestStart);
        }
    }

    /// 玩家点“准备”：先确保谱面在本地，再上报就绪。
    pub fn ready(&mut self) {
        if let Some(id) = self.chart_id {
            self.fetch_chart(id, DownloadIntent::Ready);
        }
    }

    /// 谱面已就绪：房主真正发起开始。
    pub fn start_game(&mut self) {
        let client = self.connected();
        self.task = Some(Task::new(async move {
            client.request_start().await.with_context(|| mtl!("request-start-failed"))?;
            Ok(())
        }));
    }

    /// 谱面已就绪：真正上报就绪。
    pub fn set_ready(&mut self) {
        let client = self.connected();
        self.task = Some(Task::new(async move {
            client.ready().await.with_context(|| mtl!("ready-failed"))?;
            Ok(())
        }));
    }

    pub fn cancel_ready(&mut self) {
        let client = self.connected();
        self.task = Some(Task::new(async move { client.cancel_ready().await }));
    }

    /// 房主取消本地谱面分享。
    pub fn cancel_local_chart(&mut self) {
        self.reset_local_chart();
        let client = self.connected();
        self.task = Some(Task::new(async move {
            client.cancel_local_chart().await?;
            Ok(())
        }));
    }

    /// 玩家取消“已就绪”。
    pub fn cancel_download_ready(&mut self) {
        self.local_ready = false;
        self.syncing = None;
        if let Some(cancel) = self.local_download_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        let client = self.connected();
        self.task = Some(Task::new(async move {
            client.cancel_download_ready().await?;
            Ok(())
        }));
    }

    // ---------- 谱面下载流程 ----------

    /// 开始拉取谱面元数据，并记下“下载完之后要做什么”。
    pub fn fetch_chart(&mut self, id: i32, intent: DownloadIntent) {
        self.download.intent = Some(intent);
        self.download.chart_id = id;
        self.download.task = Some(Task::new(async move { Ptr::new(id).fetch().await }));
    }

    /// 推进谱面下载流程（每帧调用）。
    pub fn poll_download(&mut self) -> anyhow::Result<DownloadStep> {
        if let Some(task) = &mut self.download.task {
            if let Some(res) = task.take() {
                self.download.task = None;
                match res {
                    Ok(entity) => {
                        let path = format!("download/{}", entity.id);
                        let info_path = format!("{}/{path}/info.yml", dir::charts()?);
                        let should_download = if Path::new(&info_path).exists() {
                            let local_info: ChartInfo = serde_yaml::from_reader(File::open(info_path)?)?;
                            local_info
                                .updated
                                .map_or(entity.updated != entity.created, |local_updated| local_updated != entity.updated)
                        } else {
                            true
                        };
                        if should_download {
                            let info = entity.to_info();
                            self.download.ui = Some(SongScene::global_start_download(info, Chart::clone(&entity), {
                                if Path::new(&format!("{}/{path}", dir::charts()?)).exists() {
                                    Some(path)
                                } else {
                                    None
                                }
                            })?);
                        } else {
                            return Ok(self.take_intent());
                        }
                    }
                    Err(err) => {
                        self.download.intent = None;
                        show_error(err.context(mtl!("download-failed")));
                    }
                }
            }
        }
        if let Some(dl) = &mut self.download.ui {
            if let Some(res) = dl.check()? {
                self.download.ui = None;
                // res 为 None 表示玩家取消了下载：流程终止，不再继续 intent
                return Ok(if res.is_some() { self.take_intent() } else { self.discard_intent() });
            }
        }
        Ok(DownloadStep::Pending)
    }

    fn take_intent(&mut self) -> DownloadStep {
        match self.download.intent.take() {
            Some(intent) => DownloadStep::Continue(intent),
            None => DownloadStep::Pending,
        }
    }

    fn discard_intent(&mut self) -> DownloadStep {
        self.download.intent = None;
        DownloadStep::Cancelled
    }

    // ---------- 本地谱面分享 ----------

    /// 房主开始上传本地谱面到服务端（服务端下发 StartServing 后调用）。
    pub fn start_serving(&mut self, chart_id: String) {
        let client = self.connected();
        let syncing = Arc::new(serve::ChartSyncing::new());
        self.syncing = Some(Arc::clone(&syncing));
        self.local_chart_task = Some(Task::new(async move {
            // 把本地谱面包经 game 连接上传到服务端
            serve::upload_chart(&client, &chart_id).await?;
            // 通知服务端开始分享；玩家下载地址由服务端下发
            client.send_chart(String::new(), 0).await?;
            Ok::<_, anyhow::Error>(())
        }));
    }

    /// 玩家点“准备”后开始下载房主分享的本地谱面。
    pub fn download_pending_local_chart(&mut self) {
        let Some((chart_id, _chart_name)) = self.pending_download.take() else {
            // 没有待下载内容：直接上报就绪
            let client = self.connected();
            self.local_chart_task = Some(Task::new(async move {
                client.download_ready().await?;
                Ok::<_, anyhow::Error>(())
            }));
            return;
        };
        let syncing = Arc::new(serve::ChartSyncing::new());
        syncing.mark_started();
        self.syncing = Some(Arc::clone(&syncing));
        let client = self.connected();
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_task = Arc::clone(&cancel);
        self.local_download_cancel = Some(cancel);
        self.local_chart_task = Some(Task::new(async move {
            serve::download_chart(&client, &chart_id, Arc::clone(&syncing)).await?;
            if !cancel_task.load(Ordering::Relaxed) {
                client.download_ready().await?;
            }
            Ok::<_, anyhow::Error>(())
        }));
    }

    /// 消费服务端下发的本地谱面分享事件。
    /// 返回 true 表示房主需要上传谱面（已在此启动上传任务）。
    pub fn apply_local_chart_event(&mut self, ev: LocalChartEvent) {
        let is_host = self.client.as_ref().and_then(|it| it.blocking_is_host()).unwrap_or(false);
        match ev {
            LocalChartEvent::ChangeLocalChart { local, chart_id } => {
                if local {
                    self.local_chart = Some((chart_id, String::new()));
                } else {
                    self.local_chart = None;
                    self.pending_download = None;
                    self.syncing = None;
                }
                self.reset_local_chart();
            }
            LocalChartEvent::StartServing { chart_id, chart_name } => {
                if !is_host {
                    return;
                }
                self.local_chart = Some((chart_id.clone(), chart_name));
                self.start_serving(chart_id);
            }
            LocalChartEvent::StartDownload { chart_id, chart_name, .. } => {
                if is_host {
                    return;
                }
                self.local_chart = Some((chart_id.clone(), chart_name.clone()));
                self.pending_download = Some((chart_id, chart_name));
            }
            LocalChartEvent::HostReady => {
                self.reset_local_chart();
            }
            LocalChartEvent::Canceled => {
                self.pending_download = None;
                self.syncing = None;
                self.reset_local_chart();
            }
        }
    }

    /// 按房间阶段同步本地谱面分享状态（每帧调用）。
    pub fn sync_local_chart_stage(&mut self, state: Option<RoomState>) {
        if matches!(state, Some(RoomState::LocalChart)) {
            if self.local_chart.is_some() {
                self.chart_id = None;
            }
        } else {
            // 离开本地谱面分享阶段：重置状态
            self.reset_local_chart();
            self.local_chart = None;
        }
    }

    // ---------- 对局 ----------

    /// 房间进入 Playing：准备好成绩上报所需的全局状态与暂停上报钩子。
    /// 实际启动游玩场景由面板负责（需要 scene_task）。
    pub fn begin_playing(&mut self) {
        use std::sync::atomic::Ordering as O;
        self.game_start_consumed = true;
        crate::scene::RECORD_ID.store(-1, O::Relaxed);
        // 开局清空上一局结算记录，避免残留导致误判“完成”
        *LAST_MP_FINISH.lock().unwrap() = None;
        self.need_upload = true;
        self.entered = false;
        // 多人正式游玩：接入“暂停/继续上报服务器”的钩子（GameScene 构造时取走）。
        // 观战场景刻意不取用它（见 SongScene::global_launch_spectate），
        // 保证观战者本地的暂停不会去暂停被观战者。
        if let Some(client) = self.client.clone() {
            *prpr::scene::PAUSE_NOTIFY.lock().unwrap() = Some(Arc::new(move |paused: bool| {
                // 设置/发送失败也不 panic（例如连接已断开）
                let _ = client.blocking_send(phira_mp_common::ClientCommand::PauseState { paused });
            }));
        }
    }

    /// 从游玩场景回来：以真实成绩上报完成，未产生有效结算则按放弃处理。
    pub fn report_finish(&mut self) {
        if !self.need_upload {
            return;
        }
        self.need_upload = false;
        let Some(client) = self.client() else { return };
        // 谱面自然打完（引擎在 record 有效时才写入 LAST_MP_FINISH）→ 上报真实成绩；
        // 中途退出/跳过/失败未产生有效结算 → 仍按放弃(abort)处理。
        if let Some(stats) = LAST_MP_FINISH.lock().unwrap().take() {
            self.task = Some(Task::new(async move {
                client
                    .played(
                        0,
                        stats.score,
                        stats.accuracy,
                        stats.full_combo,
                        stats.max_combo,
                        stats.perfect,
                        stats.good,
                        stats.bad,
                        stats.miss,
                    )
                    .await
            }));
        } else {
            self.task = Some(Task::new(async move { client.abort().await }));
        }
    }

    // ---------- 断线重连 ----------

    /// 心跳连续失败时的自动重连（提示 + 重连一次）。
    pub fn reconnect_if_lost(&mut self) {
        let Some(client) = &self.client else { return };
        if client.ping_fail_count() >= 2 && self.connect_task.is_none() {
            warn!("lost connection, reconnecting…");
            show_message(mtl!("reconnect")).warn();
            self.connect();
        }
    }
}
