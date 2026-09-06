use crate::{
    client::{Chart, Ptr, UserManager},
    dir, get_data,
    mp::L10N_LOCAL,
    scene::{Downloading, SongScene, RECORD_ID},
};
use anyhow::{anyhow, Context, Result};
use inputbox::InputBox;
use macroquad::prelude::*;
use phira_mp_client::Client;
use phira_mp_common::{RoomId, RoomState};
use prpr::{
    config::Mods,
    core::{Smooth, Tweenable},
    ext::{poll_future, semi_black, semi_white, LocalTask, RectExt, SafeTexture},
    info::ChartInfo,
    scene::{request_input, return_input, show_error, show_message, take_input, GameMode, NextScene},
    task::Task,
    time::TimeManager,
    ui::{DRectButton, DrawText},
    ui::{Scroll, Ui},
};
use smallvec::SmallVec;
use std::{
    fs::File,
    path::Path,
    sync::{atomic::Ordering, Arc},
};
use tokio::net::TcpStream;
use tracing::warn;

const ENTER_TRANSIT: f32 = 0.5;
const USER_LIST_TRANSIT: f32 = 0.4;
const WIDTH: f32 = 1.6;

// 服务器公共房间列表 JSON（GET /api/rooms 返回字段的子集）
#[derive(Debug, Clone, serde::Deserialize)]
struct PublicRoom {
    id: String,
    #[serde(rename = "player_count")]
    player_count: usize,
    state: String,
    locked: bool,
    #[serde(default)]
    mode: String,
    #[serde(rename = "spectator_count", default)]
    spectator_count: usize,
}

const CHAT_ENABLED: bool = cfg!(feature = "chat");

fn screen_size() -> (u32, u32) {
    (screen_width() as u32, screen_height() as u32)
}

struct Message {
    content: String,
    y: f32,
    bottom: f32,
    color: Color,
}

impl Message {
    pub fn text<'a, 's, 'ui>(&'s self, ui: &'ui mut Ui<'a>, mw: f32) -> DrawText<'a, 's, 'ui> {
        ui.text(&self.content)
            .pos(0., self.y)
            .size(0.4)
            .color(self.color)
            .max_width(mw)
            .multiline()
    }
}

pub struct MPPanel {
    pub client: Option<Arc<Client>>,

    side_enter_time: f32,

    // 深链接（phira://）待处理的多人房间动作
    deep_link: Option<crate::mp::PendingRoomLink>,
    deep_link_auto_connected: bool,

    msg_scroll: Scroll,
    msgs: Vec<Message>,
    msgs_dirty_from: usize,
    last_screen_size: (u32, u32),

    connect_btn: DRectButton,
    connect_task: Option<Task<Result<Client>>>,

    create_room_btn: DRectButton,
    create_room_task: Option<Task<Result<()>>>,
    join_room_btn: DRectButton,
    join_room_task: Option<Task<Result<RoomState>>>,
    leave_room_btn: DRectButton,

    disconnect_btn: DRectButton,

    request_start_btn: DRectButton,
    lock_room_btn: DRectButton,
    cycle_room_btn: DRectButton,

    ready_btn: DRectButton,
    cancel_ready_btn: DRectButton,

    chat_text: String,
    chat_btn: DRectButton,
    chat_send_btn: DRectButton,
    chat_task: Option<Task<Result<()>>>,

    download_task: Option<Task<Result<Arc<Chart>>>>,
    downloading: Option<Downloading>,
    download_next: bool,

    // LocalChart 本地谱面同步
    local_chart: Option<(String, String)>,
    syncing: Option<Arc<crate::mp::serve::ChartSyncing>>,
    pending_download: Option<(String, u16, String, String)>,
    local_chart_task: Option<Task<Result<()>>>,
    host_started: bool,
    local_ready: bool,
    local_download_cancel: Option<Arc<std::sync::atomic::AtomicBool>>,

    chart_id: Option<i32>,
    game_start_consumed: bool,
    need_upload: bool,
    entered: bool,

    next_scene: Option<NextScene>,

    task: Option<Task<Result<()>>>,

    scene_task: LocalTask<Result<NextScene>>,

    user_list_btn: DRectButton,
    user_list_p: Smooth<f32>,
    user_list_scroll: Scroll,
    icon_user: SafeTexture,

    // 房间管理（房主视图）
    password_btn: DRectButton,
    kick_user_btn: DRectButton,
    transfer_host_btn: DRectButton,
    // 谱面预览（autoplay）
    preview_btn: DRectButton,
    // 预览前需要先下载谱面（下载完成后自动开始预览）
    preview_pending: bool,
    // 记录"加入需要密码"的房间 id，用于二次请求密码（一次有效）
    join_pwd_pending: Option<String>,

    // 快速进房：公共房间列表
    room_list_btn: DRectButton,
    room_list_p: Smooth<f32>,
    room_list_scroll: Scroll,
    room_list: Option<Vec<PublicRoom>>,
    room_list_task: Option<Task<Result<Vec<PublicRoom>>>>,

    // 对局结算排名弹层
    results: Option<Vec<phira_mp_common::RoomResultEntry>>,
    results_p: Smooth<f32>,
    results_scroll: Scroll,
    results_btn: DRectButton,
}

impl MPPanel {
    pub fn new(icon_user: SafeTexture) -> Self {
        Self {
            client: None,

            side_enter_time: f32::INFINITY,

            deep_link: None,
            deep_link_auto_connected: false,

            msg_scroll: Scroll::new(),
            msgs: Vec::new(),
            msgs_dirty_from: 0,
            last_screen_size: screen_size(),

            connect_btn: DRectButton::new(),
            connect_task: None,

            create_room_btn: DRectButton::new(),
            create_room_task: None,
            join_room_btn: DRectButton::new(),
            join_room_task: None,
            leave_room_btn: DRectButton::new(),

            disconnect_btn: DRectButton::new(),

            request_start_btn: DRectButton::new(),
            lock_room_btn: DRectButton::new(),
            cycle_room_btn: DRectButton::new(),

            ready_btn: DRectButton::new(),
            cancel_ready_btn: DRectButton::new(),

            chat_text: String::new(),
            chat_btn: DRectButton::new().with_delta(-0.002),
            chat_send_btn: DRectButton::new(),
            chat_task: None,

            download_task: None,
            downloading: None,
            download_next: false,

            local_chart: None,
            syncing: None,
            pending_download: None,
            local_chart_task: None,
            host_started: false,
            local_ready: false,
            local_download_cancel: None,

            chart_id: None,
            game_start_consumed: false,
            need_upload: false,
            entered: false,

            next_scene: None,

            task: None,

            scene_task: None,

            user_list_btn: DRectButton::new(),
            user_list_p: Smooth::default(),
            user_list_scroll: Scroll::new(),
            icon_user,

            password_btn: DRectButton::new(),
            kick_user_btn: DRectButton::new(),
            transfer_host_btn: DRectButton::new(),
            preview_btn: DRectButton::new(),
            preview_pending: false,
            join_pwd_pending: None,

            room_list_btn: DRectButton::new(),
            room_list_p: Smooth::default(),
            room_list_scroll: Scroll::new(),
            room_list: None,
            room_list_task: None,

            results: None,
            results_p: Smooth::default(),
            results_scroll: Scroll::new(),
            results_btn: DRectButton::new(),
        }
    }

    fn clone_client(&self) -> Arc<Client> {
        Arc::clone(self.client.as_ref().unwrap())
    }

    /// 房间列表浮层：命中第几行（与 render 中几何一致）。点不到返回 None。
    fn hit_room_list_row(&self, pos: macroquad::prelude::Vec2) -> Option<usize> {
        const PANEL_W: f32 = 0.92;
        const MAX_ROWS: usize = 10;
        const ROW_H: f32 = 0.13;
        const GAP: f32 = 0.015;
        let rooms = self.room_list.as_deref().unwrap_or(&[]);
        let n = rooms.len().min(MAX_ROWS);
        if n == 0 {
            return None;
        }
        let header = 0.3 + n as f32 * (ROW_H + GAP);
        // panel_h 与 render 相同（受 ui.top*2 限制；本命中直接忽略截断情形）
        let panel_h = header;
        let panel_x = -PANEL_W / 2.;
        let panel_y = -panel_h / 2.;
        let x0 = panel_x + 0.03;
        let y0 = panel_y + 0.03 + 0.085 + 0.02;
        if pos.x < x0 || pos.x > x0 + PANEL_W - 0.06 {
            return None;
        }
        for (i, _) in rooms.iter().take(MAX_ROWS).enumerate() {
            let y = y0 + i as f32 * (ROW_H + GAP);
            if pos.y >= y && pos.y <= y + ROW_H {
                return Some(i);
            }
        }
        None
    }

    /// 从 mp_address（如 `mp2.phira.cn:12345`）推导 Web API 地址（游戏端口 + 1）
    fn web_base() -> Option<String> {
        let addr = get_data().config.mp_address.clone();
        let (host, port) = addr.rsplit_once(':')?;
        let port: u16 = port.parse().ok()?;
        let host = host.trim_end_matches('.').to_owned();
        Some(format!("http://{host}:{}", port + 1))
    }

    /// 拉取公共房间列表（GET {web}/api/rooms）
    fn load_room_list(&mut self) {
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
            let rooms: Vec<PublicRoom> = resp.json().await?;
            Ok(rooms)
        }));
    }

    /// 打开房间列表浮层
    fn open_room_list(&mut self, t: f32) {
        self.room_list_scroll.y_scroller.reset();
        self.room_list_p.goto(1., t, USER_LIST_TRANSIT);
        self.load_room_list();
    }

    fn has_task(&self) -> bool {
        self.connect_task.is_some()
            || self.create_room_task.is_some()
            || self.chat_task.is_some()
            || self.download_task.is_some()
            || self.local_chart_task.is_some()
            || self.task.is_some()
            || self.scene_task.is_some()
    }

    fn connect(&mut self) {
        let Some(token) = get_data().tokens.as_ref().map(|it| it.0.clone()) else {
            show_message(mtl!("connect-must-login")).error();
            return;
        };
        // 深链接可携带服务器地址（phira://...?server=xxx），优先使用它
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

    fn create_room(&mut self, id: RoomId) {
        let client = self.clone_client();
        self.create_room_task = Some(Task::new(async move {
            client.create_room(id).await?;
            Ok(())
        }));
    }

    pub fn select_chart(&mut self, id: i32) {
        let client = self.clone_client();
        if !client.blocking_is_host().unwrap() {
            show_message(mtl!("select-chart-host-only")).error();
            return;
        }
        if !matches!(client.blocking_room_state(), Some(RoomState::SelectChart(_) | RoomState::LocalChart)) {
            show_message(mtl!("select-chart-not-now")).error();
            return;
        }
        // 切换到在线谱面：清除之前选择的本地谱面
        self.local_chart = None;
        self.pending_download = None;
        self.syncing = None;
        self.local_download_cancel = None;
        self.host_started = false;
        self.local_ready = false;
        self.task = Some(Task::new(async move {
            client.select_online_chart(id).await.with_context(|| mtl!("select-chart-failed"))?;
            Ok(())
        }));
    }

    /// 判断当前服务器是否允许上传本地谱面
    fn server_allows_local_chart(&self) -> bool {
        const ALLOWED: &[&str] = &["mp.tianstudio.top", "mp.ratzen.top"];
        let addr = get_data().config.mp_address.as_str();
        // 去掉 scheme（如 tcp:// 等）
        let addr = addr.rsplit_once("://").map(|(_, host)| host).unwrap_or(addr);
        // 若包含端口（最后一个 ':'），取 ':' 之前作为主机名
        let host = addr.rsplit_once(':').map(|(h, _)| h).unwrap_or(addr);
        let host = host.trim().trim_matches(|c| c == '[' || c == ']');
        ALLOWED.contains(&host)
    }

    /// 从谱面库中选择本地谱面进行分享
    pub fn select_local_chart(&mut self, local_path: String, name: String) {
        if !self.server_allows_local_chart() {
            show_message(mtl!("mp-server-no-local-chart")).error();
            return;
        }
        let client = self.clone_client();
        if !client.blocking_is_host().unwrap() {
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
            crate::mp::serve::stage_local_chart(&local_path, &uuid)?;
            client.select_local_chart(uuid, name).await.with_context(|| mtl!("select-chart-failed"))?;
            Ok(())
        }));
    }

    fn request_start(&mut self) {
        let client = self.clone_client();
        let state = self.client.as_ref().unwrap().blocking_room_state().unwrap();
        // LocalChart 状态下房主已选择本地谱面：直接请求开始（服务端会通知房主启动上传）
        if matches!(state, RoomState::LocalChart) {
            if !self.server_allows_local_chart() {
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
        self.check_download(true);
    }

    fn check_download(&mut self, next: bool) {
        let id = self.chart_id.unwrap();
        self.download_next = next;
        self.download_task = Some(Task::new(async move { Ptr::new(id).fetch().await }));
    }

    /// 谱面是否已在本地（download/{id} 或 download/{uuid}）
    fn local_chart_ready(&self, id: Option<i32>, uuid: Option<&str>) -> bool {
        let id = match (id, uuid) {
            (Some(id), _) => id.to_string(),
            (None, Some(uuid)) => uuid.to_string(),
            _ => return false,
        };
        Path::new(&format!("{}/download/{id}/info.yml", dir::charts().ok().unwrap_or_default())).exists()
    }

    /// 预览当前房主选定的谱面（autoplay；结束时自动回到房间，不影响正式开局）
    fn start_preview(&mut self) {
        let Some(client) = self.client.clone() else { return };
        let state = match client.blocking_state() {
            Some(s) => s,
            None => return,
        };
        // 在线谱 / 本地谱
        let (id, uuid, path): (Option<i32>, Option<String>, Option<String>) = match (&state.state, &self.local_chart) {
            (RoomState::SelectChart(Some(id)), _) => (Some(*id), None, Some(format!("download/{id}"))),
            (RoomState::LocalChart, Some((uuid, _))) => (None, Some(uuid.clone()), Some(format!("download/{uuid}"))),
            _ => (None, None, None),
        };
        let Some(path) = path else {
            show_message(mtl!("preview-unavailable")).error();
            return;
        };
        // 本地没有缓存：先下载（下载完成后由 post_download 自动进入预览）
        if !self.local_chart_ready(id, uuid.as_deref()) {
            self.chart_id = id.or(self.chart_id);
            self.preview_pending = true;
            if let Some(id) = self.chart_id {
                self.check_download_preview(id);
            } else {
                show_message(mtl!("preview-unavailable")).error();
            }
            return;
        }
        let res = self.launch_preview(path, id);
        if let Err(err) = res {
            show_error(err.context(mtl!("preview-failed")));
        }
    }

    fn check_download_preview(&mut self, id: i32) {
        self.download_task = Some(Task::new(async move { Ptr::new(id).fetch().await }));
    }

    /// 以 autoplay 方式进入谱面预览（client 传 None：不参与 live/上报，不影响房间）
    fn launch_preview(&mut self, path: String, id: Option<i32>) -> Result<()> {
        use crate::scene::SongScene;
        use prpr::config::Mods;
        use prpr::scene::GameMode;
        self.scene_task = SongScene::global_launch(
            id,
            &path,
            Mods::AUTOPLAY,
            GameMode::NoRetry,
            None,
            None,
            None,
            false,
            false,
        )?;
        Ok(())
    }

    fn post_download(&mut self) {
        let client = self.clone_client();
        if self.preview_pending {
            self.preview_pending = false;
            // 谱面下载完成后进入预览
            let Some(id) = self.chart_id else {
                show_message(mtl!("preview-unavailable")).error();
                return;
            };
            let path = format!("download/{id}");
            if let Err(err) = self.launch_preview(path, Some(id)) {
                show_error(err.context(mtl!("preview-failed")));
            }
            return;
        }
        if self.download_next {
            self.task = Some(Task::new(async move {
                client.request_start().await.with_context(|| mtl!("request-start-failed"))?;
                Ok(())
            }));
        } else {
            self.task = Some(Task::new(async move {
                client.ready().await.with_context(|| mtl!("ready-failed"))?;
                Ok(())
            }));
        }
    }

    /// 消费服务端下发的 LocalChart 事件
    fn update_local_chart(&mut self) {
        let Some(client) = self.client.clone() else { return };
        let is_host = client.blocking_is_host().unwrap_or(false);
        let events = client.blocking_take_local_chart_events();
        for ev in events {
            match ev {
                phira_mp_client::LocalChartEvent::ChangeLocalChart { local, chart_id } => {
                    if local {
                        self.local_chart = Some((chart_id, String::new()));
                    } else {
                        self.local_chart = None;
                        self.pending_download = None;
                        self.syncing = None;
                    }
                    self.host_started = false;
                    self.local_ready = false;
                    self.local_download_cancel = None;
                }
                phira_mp_client::LocalChartEvent::StartServing { chart_id, chart_name } => {
                    if !is_host { continue; }
                    self.local_chart = Some((chart_id.clone(), chart_name));
                    self.start_serving(chart_id);
                }
                phira_mp_client::LocalChartEvent::StartDownload {
                    host_id: _,
                    host_name: _,
                    addr: _,
                    port: _,
                    chart_id,
                    chart_name,
                } => {
                    if is_host { continue; }
                    self.local_chart = Some((chart_id.clone(), chart_name.clone()));
                    self.pending_download = Some((String::new(), 0u16, chart_id, chart_name));
                }
                phira_mp_client::LocalChartEvent::HostReady => {
                    self.host_started = false;
                    self.local_ready = false;
                    self.local_download_cancel = None;
                }
                phira_mp_client::LocalChartEvent::Canceled => {
                    self.host_started = false;
                    self.local_ready = false;
                    self.pending_download = None;
                    self.syncing = None;
                    self.local_download_cancel = None;
                }
            }
        }
    }

    /// 玩家点击"准备"后开始下载谱面
    fn start_pending_download(&mut self) {
        let Some((_addr, _port, chart_id, _chart_name)) = self.pending_download.take() else {
            let client = self.clone_client();
            self.local_chart_task = Some(Task::new(async move {
                client.download_ready().await?;
                Ok::<_, anyhow::Error>(())
            }));
            return;
        };
        let syncing = Arc::new(crate::mp::serve::ChartSyncing::new());
        syncing.mark_started();
        self.syncing = Some(Arc::clone(&syncing));
        let client = self.clone_client();
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel_task = Arc::clone(&cancel);
        self.local_download_cancel = Some(cancel);
        self.local_chart_task = Some(Task::new(async move {
            crate::mp::serve::download_chart(&client, &chart_id, Arc::clone(&syncing)).await?;
            if !cancel_task.load(std::sync::atomic::Ordering::Relaxed) {
                client.download_ready().await?;
            }
            Ok::<_, anyhow::Error>(())
        }));
    }

    /// 房主取消本地谱面分享
    fn cancel_local_chart(&mut self) {
        self.host_started = false;
        self.local_ready = false;
        self.pending_download = None;
        self.syncing = None;
        if let Some(cancel) = self.local_download_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let client = self.clone_client();
        self.task = Some(Task::new(async move {
            client.cancel_local_chart().await?;
            Ok(())
        }));
    }

    /// 玩家取消已就绪
    fn cancel_local_download(&mut self) {
        self.local_ready = false;
        self.syncing = None;
        if let Some(cancel) = self.local_download_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let client = self.clone_client();
        self.task = Some(Task::new(async move {
            client.cancel_download_ready().await?;
            Ok(())
        }));
    }

    /// 房主开始上传本地谱面到服务端
    fn start_serving(&mut self, chart_id: String) {
        let client = self.clone_client();
        let syncing = Arc::new(crate::mp::serve::ChartSyncing::new());
        self.syncing = Some(Arc::clone(&syncing));
        self.local_chart_task = Some(Task::new(async move {
            // 把本地谱面包经 game 连接上传到服务端
            crate::mp::serve::upload_chart(&client, &chart_id).await?;
            // 通知服务端开始分享；玩家下载地址由服务端下发
            client.send_chart(String::new(), 0).await?;
            Ok::<_, anyhow::Error>(())
        }));
    }
}

impl MPPanel {
    #[inline]
    pub fn in_room(&self) -> bool {
        self.client.as_ref().is_some_and(|it| it.blocking_room_id().is_some())
    }

    #[inline]
    pub fn show(&mut self, rt: f32) {
        self.side_enter_time = rt;
    }

    /// 接收深链接（phira://）房间动作：打开面板、自动连接并加入/创建房间。
    pub fn set_deep_link(&mut self, link: crate::mp::PendingRoomLink) {
        self.deep_link_auto_connected = false;
        self.deep_link = Some(link);
    }

    /// 已连上服务器时执行深链接动作（加入/创建房间），只执行一次。
    fn run_deep_link(&mut self) {
        let Some(link) = self.deep_link.take() else { return };
        if let Some(join) = link.join {
            match join.try_into() {
                Ok(id) => {
                    let client = self.clone_client();
                    self.join_room_task = Some(Task::new(async move {
                        client.join_room(id, false).await?;
                        client.room_state().await.ok_or_else(|| anyhow!("expected room state"))
                    }));
                }
                Err(_) => {
                    show_message(mtl!("join-room-invalid-id")).error();
                }
            }
            return;
        }
        if let Some(id) = link.create {
            match id.try_into() {
                Ok(room_id) => self.create_room(room_id),
                Err(_) => {
                    show_message(mtl!("create-invalid-id")).error();
                }
            }
        }
    }

    pub fn enter(&mut self) {
        self.entered = true;
    }

    pub fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> bool {
        let t = tm.now() as f32;
        if self.side_enter_time.is_infinite() {
            return false;
        }
        if self.user_list_p.transiting(t) {
            return true;
        }
        if *self.user_list_p.to() > 0.5 {
            if self.user_list_scroll.touch(touch, t) {
                return true;
            }
            if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                self.user_list_p.goto(0., t, USER_LIST_TRANSIT);
            }
            return true;
        }
        if self.room_list_p.transiting(t) {
            return true;
        }
        if *self.room_list_p.to() > 0.5 {
            // 命中房间行 → 快速加入
            if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                let hit = self.hit_room_list_row(touch.position);
                if let Some(room) = hit.and_then(|idx| self.room_list.as_ref().and_then(|r| r.get(idx))) {
                    self.room_list_p.goto(0., t, USER_LIST_TRANSIT);
                    let client = self.clone_client();
                    if let Ok(id) = room.id.clone().try_into() {
                        self.join_room_task = Some(Task::new(async move {
                            client.join_room(id, false).await?;
                            client.room_state().await.ok_or_else(|| anyhow!("expected room state"))
                        }));
                    }
                    return true;
                }
                // 点击空白处关闭
                if hit.is_none() {
                    self.room_list_p.goto(0., t, USER_LIST_TRANSIT);
                }
            }
            return true;
        }
        if self.results_p.transiting(t) {
            return true;
        }
        if *self.results_p.to() > 0.5 {
            if self.results_scroll.touch(touch, t) {
                return true;
            }
            if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                self.results_p.goto(0., t, USER_LIST_TRANSIT);
            }
            return true;
        }
        if !(self.side_enter_time > 0. && tm.real_time() as f32 > self.side_enter_time + ENTER_TRANSIT) {
            return true;
        }
        if self.has_task() {
            return true;
        }
        if let Some(dl) = &mut self.downloading {
            if dl.touch(touch, t) {
                self.downloading = None;
                return true;
            }
        }
        if touch.position.x + 1. > WIDTH {
            self.side_enter_time = -tm.real_time() as f32;
            return true;
        }
        if self.client.is_none() && self.connect_btn.touch(touch, t) {
            self.connect();
            return true;
        }
        if let Some(client) = &self.client {
            if self.msg_scroll.touch(touch, t) {
                return true;
            }
            if let Some(state) = client.blocking_state() {
                if self.chat_btn.touch(touch, t) {
                    request_input("chat", InputBox::new().default_text(&self.chat_text));
                    return true;
                }
                if self.chat_send_btn.touch(touch, t) {
                    if self.chat_text.is_empty() {
                        show_message(mtl!("chat-empty")).error();
                    } else {
                        let client = Arc::clone(client);
                        let text = self.chat_text.clone();
                        self.chat_task = Some(Task::new(async move { client.chat(text).await }));
                    }
                    return true;
                }
                let is_host = state.is_host;
                match state.state {
                    RoomState::SelectChart(_) => {
                        if is_host {
                            if self.request_start_btn.touch(touch, t) {
                                self.request_start();
                                return true;
                            }
                            if self.lock_room_btn.touch(touch, t) {
                                let to = !state.locked;
                                let client = self.clone_client();
                                self.task = Some(Task::new(async move { client.lock_room(to).await.with_context(|| mtl!("lock-room-failed")) }));
                                return true;
                            }
                            if self.password_btn.touch(touch, t) {
                                request_input("set_pwd", InputBox::new());
                                return true;
                            }
                            if self.kick_user_btn.touch(touch, t) {
                                request_input("kick_user", InputBox::new());
                                return true;
                            }
                            if self.transfer_host_btn.touch(touch, t) {
                                request_input("transfer_host", InputBox::new());
                                return true;
                            }
                            if self.cycle_room_btn.touch(touch, t) {
                                let to = !state.cycle;
                                let client = self.clone_client();
                                self.task = Some(Task::new(async move { client.cycle_room(to).await.with_context(|| mtl!("cycle-room-failed")) }));
                                return true;
                            }
                        }
                        if self.leave_room_btn.touch(touch, t) {
                            let client = self.clone_client();
                            self.task = Some(Task::new(async move { client.leave_room().await }));
                            return true;
                        }
                    }
                    RoomState::LocalChart => {
                        if is_host {
                            if self.host_started {
                                if self.cancel_ready_btn.touch(touch, t) {
                                    self.cancel_local_chart();
                                    return true;
                                }
                            } else if self.request_start_btn.touch(touch, t) {
                                self.request_start();
                                return true;
                            }
                            if self.lock_room_btn.touch(touch, t) {
                                let to = !state.locked;
                                let client = self.clone_client();
                                self.task = Some(Task::new(async move { client.lock_room(to).await.with_context(|| mtl!("lock-room-failed")) }));
                                return true;
                            }
                            if self.password_btn.touch(touch, t) {
                                request_input("set_pwd", InputBox::new());
                                return true;
                            }
                            if self.kick_user_btn.touch(touch, t) {
                                request_input("kick_user", InputBox::new());
                                return true;
                            }
                            if self.transfer_host_btn.touch(touch, t) {
                                request_input("transfer_host", InputBox::new());
                                return true;
                            }
                            if self.cycle_room_btn.touch(touch, t) {
                                let to = !state.cycle;
                                let client = self.clone_client();
                                self.task = Some(Task::new(async move { client.cycle_room(to).await.with_context(|| mtl!("cycle-room-failed")) }));
                                return true;
                            }
                        } else {
                            if self.local_ready {
                                if self.cancel_ready_btn.touch(touch, t) {
                                    self.cancel_local_download();
                                    return true;
                                }
                            } else if self.syncing.is_none() && self.ready_btn.touch(touch, t) {
                                self.local_ready = true;
                                self.start_pending_download();
                                return true;
                            }
                        }
                        if self.leave_room_btn.touch(touch, t) {
                            let client = self.clone_client();
                            self.task = Some(Task::new(async move { client.leave_room().await }));
                            return true;
                        }
                    }
                    RoomState::WaitingForReady => {
                        if client.blocking_is_ready().unwrap() {
                            if self.cancel_ready_btn.touch(touch, t) {
                                let client = self.clone_client();
                                self.task = Some(Task::new(async move { client.cancel_ready().await }));
                                return true;
                            }
                        } else if self.ready_btn.touch(touch, t) {
                            self.check_download(false);
                            return true;
                        }
                    }
                    _ => {}
                }
                if self.preview_btn.touch(touch, t) {
                    self.start_preview();
                    return true;
                }
                if self.user_list_btn.touch(touch, t) {
                    self.user_list_scroll.y_scroller.reset();
                    self.user_list_p.goto(1., t, USER_LIST_TRANSIT);
                    client.blocking_state().unwrap().users.keys().copied().for_each(UserManager::request);
                }
            } else {
                if self.create_room_btn.touch(touch, t) {
                    request_input("room_id", InputBox::new());
                    return true;
                }
                if self.join_room_btn.touch(touch, t) {
                    request_input("join_room", InputBox::new());
                    return true;
                }
                if self.room_list_btn.touch(touch, t) {
                    self.open_room_list(t);
                    return true;
                }
                if self.disconnect_btn.touch(touch, t) {
                    self.client = None;
                    self.msgs.clear();
                    self.msgs_dirty_from = 0;
                    return true;
                }
            }
            if client.ping_fail_count() >= 2 && self.connect_task.is_none() {
                warn!("lost connection, reconnecting…");
                show_message(mtl!("reconnect")).warn();
                self.connect();
            }
        }
        true
    }

    pub fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = tm.now() as f32;
        if self.side_enter_time < 0. && -tm.real_time() as f32 + ENTER_TRANSIT < self.side_enter_time {
            self.side_enter_time = f32::INFINITY;
        }
        let new_size = screen_size();
        if self.last_screen_size != new_size {
            self.last_screen_size = new_size;
            self.msgs_dirty_from = 0;
        }
        self.msg_scroll.update(t);
        if self.user_list_p.now(t) > 1e-4 {
            self.user_list_scroll.update(t);
        }
        if self.room_list_p.now(t) > 1e-4 {
            self.room_list_scroll.update(t);
        }
        if self.results_p.now(t) > 1e-4 {
            self.results_scroll.update(t);
        }
        if let Some(client) = &self.client {
            for res in client.blocking_take_room_results() {
                // 收到结算后自动弹出排名
                self.results = Some(res);
                self.results_p.goto(1., t, USER_LIST_TRANSIT);
            }
        }
        if let Some(client) = &self.client {
            self.msgs.extend(client.blocking_take_messages().into_iter().map(|msg| {
                use phira_mp_common::Message as M;
                match msg {
                    M::Chat { user, content, .. } => Message {
                        content: format!("{}：{content}", client.user_name(user)),
                        y: 0.,
                        bottom: 0.,
                        color: WHITE,
                    },
                    msg => {
                        let content = match msg {
                            M::Chat { .. } => unreachable!(),
                            M::CreateRoom { user, .. } => {
                                mtl!("msg-create-room", "user" => client.user_name(user))
                            }
                            M::JoinRoom { name, .. } => {
                                mtl!("msg-join-room", "user" => name)
                            }
                            M::LeaveRoom { name, .. } => {
                                mtl!("msg-leave-room", "user" => name)
                            }
                            M::NewHost { user, .. } => {
                                mtl!("msg-new-host", "user" => client.user_name(user))
                            }
                            M::SelectChart { user, name, id } => {
                                mtl!("msg-select-chart", "user" => client.user_name(user), "chart" => name, "id" => id)
                            }
                            M::GameStart { user, .. } => {
                                mtl!("msg-game-start", "user" => client.user_name(user))
                            }
                            M::Ready { user, .. } => {
                                mtl!("msg-ready", "user" => client.user_name(user))
                            }
                            M::CancelReady { user, .. } => {
                                mtl!("msg-cancel-ready", "user" => client.user_name(user))
                            }
                            M::CancelGame { user, .. } => {
                                mtl!("msg-cancel-game", "user" => client.user_name(user))
                            }
                            M::StartPlaying => mtl!("msg-start-playing").into_owned(),
                            M::Played { user, score, accuracy, full_combo, .. } => {
                                mtl!("msg-played", "user" => client.user_name(user), "score" => format!("{score:07}"), "accuracy" => format!("{:.2}%", accuracy * 100.), "full-combo" => full_combo.to_string())
                            }
                            M::GameEnd => mtl!("msg-game-end").into_owned(),
                            M::Abort { user, .. } => mtl!("msg-abort", "user" => client.user_name(user)),
                            M::LockRoom { lock } => mtl!("msg-room-lock", "lock" => lock.to_string()),
                            M::CycleRoom { cycle } => mtl!("msg-room-cycle", "cycle" => cycle.to_string()),
                            M::SelectLocalChart { user, name, .. } => {
                                format!("{} 选择了本地谱面: {}", client.user_name(user), name)
                            }
                            M::SendChart { user, .. } => {
                                format!("{} 开始分享谱面", client.user_name(user))
                            }
                            M::DownloadReady { user, .. } => {
                                format!("{} 谱面下载完成", client.user_name(user))
                            }
                            M::Kicked { user, name, .. } => {
                                if Some(user) == client.me().as_ref().map(|it| it.id) {
                                    mtl!("msg-kicked-me").into_owned()
                                } else {
                                    mtl!("msg-kicked", "user" => name.as_str())
                                }
                            }
                            // 结算排名经 room_results 队列单独展示，不会出现在消息流
                            M::RoomResults { results } => {
                                mtl!("msg-room-results", "n" => results.len() as u64)
                            }
                        };
                        Message {
                            content,
                            y: 0.,
                            bottom: 0.,
                            color: semi_white(0.7),
                        }
                    }
                }
            }));
            let state = client.blocking_room_state();
            if matches!(state, Some(RoomState::Playing)) {
                if !self.game_start_consumed {
                    self.game_start_consumed = true;
                    RECORD_ID.store(-1, Ordering::Relaxed);
                    self.need_upload = true;
                    self.entered = false;
                    // 本地谱面分享：从本地 download/{uuid} 加载
                    if let Some((uuid, _)) = self.local_chart.clone() {
                        self.scene_task = SongScene::global_launch(
                            None,
                            &format!("download/{uuid}"),
                            Mods::default(),
                            GameMode::NoRetry,
                            self.client.as_ref().map(Arc::clone),
                            None,
                            None,
                            false,
                            false,
                        )?;
                    } else {
                        let id = self.chart_id.unwrap();
                        self.scene_task = SongScene::global_launch(
                            Some(id),
                            &format!("download/{id}"),
                            Mods::default(),
                            GameMode::NoRetry,
                            self.client.as_ref().map(Arc::clone),
                            None,
                            None,
                            false,
                            false,
                        )?;
                    }
                }
            } else {
                self.game_start_consumed = false;
            }
            if let Some(RoomState::SelectChart(chart)) = state {
                self.chart_id = chart;
            }
            if matches!(state, Some(RoomState::LocalChart)) {
                if self.local_chart.is_some() {
                    self.chart_id = None;
                }
            } else {
                // 离开本地谱面分享阶段：重置状态
                self.host_started = false;
                self.local_ready = false;
                self.local_download_cancel = None;
                self.local_chart = None;
                self.pending_download = None;
                self.syncing = None;
            }
        }
        if let Some(task) = &mut self.connect_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(client) => {
                        show_message(mtl!("connect-success")).ok();
                        self.client = Some(client.into());
                    }
                    Err(err) => {
                        show_error(err.context(mtl!("connect-failed")));
                    }
                }
                self.connect_task = None;
            }
        }
        if let Some(task) = &mut self.create_room_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(_) => {
                        show_message(mtl!("create-room-success")).ok();
                    }
                    Err(err) => {
                        show_error(err.context(mtl!("create-room-failed")));
                    }
                }
                self.create_room_task = None;
            }
        }
        if let Some(task) = &mut self.download_task {
            if let Some(res) = task.take() {
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
                            self.downloading = Some(SongScene::global_start_download(info, Chart::clone(&entity), {
                                if Path::new(&format!("{}/{path}", dir::charts()?)).exists() {
                                    Some(path)
                                } else {
                                    None
                                }
                            })?);
                        } else {
                            self.post_download();
                        }
                    }
                    Err(err) => {
                        show_error(err.context(mtl!("download-failed")));
                    }
                }
                self.download_task = None;
            }
        }
        if let Some(dl) = &mut self.downloading {
            if let Some(res) = dl.check()? {
                if res.is_some() {
                    self.post_download();
                }
                self.downloading = None;
            }
        }
        if let Some(task) = &mut self.chat_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(_) => {
                        show_message(mtl!("chat-sent")).ok();
                        self.chat_text.clear();
                    }
                    Err(err) => {
                        show_error(err.context(mtl!("chat-send-failed")));
                    }
                }
                self.chat_task = None;
            }
        }
        if let Some(task) = &mut self.task {
            if let Some(res) = task.take() {
                if let Err(err) = res {
                    show_error(err);
                }
                self.task = None;
            }
        }
        if let Some(task) = &mut self.room_list_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(rooms) => {
                        self.room_list = Some(rooms);
                    }
                    Err(err) => {
                        show_error(err.context(mtl!("room-list-failed")));
                        self.room_list = Some(Vec::new());
                    }
                }
                self.room_list_task = None;
            }
        }
        if let Some(task) = &mut self.join_room_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        // 若房间需要密码，请用户输入密码后重试（仅第一次失败时询问）
                        let need_pwd = self.join_pwd_pending.is_some()
                            && {
                                let msg = format!("{err}");
                                msg.contains("密码") || msg.to_lowercase().contains("password")
                            };
                        if need_pwd {
                            let room_id = self.join_pwd_pending.take().unwrap();
                            self.join_pwd_pending = Some(room_id);
                            request_input(
                                "join_room_pwd",
                                InputBox::new().title(mtl!("join-room-password-title")),
                            );
                        } else {
                            self.join_pwd_pending = None;
                            show_error(err.context(mtl!("join-room-failed")));
                        }
                    }
                    Ok(state) => {
                        self.join_pwd_pending = None;
                        self.chart_id = match state {
                            RoomState::SelectChart(id) => id,
                            _ => None,
                        };
                    }
                }
                self.join_room_task = None;
            }
        }
        if let Some((id, text)) = take_input() {
            match id.as_str() {
                "chat" => {
                    self.chat_text = text;
                }
                "room_id" => {
                    self.create_room(text.try_into().with_context(|| mtl!("create-invalid-id"))?);
                }
                "join_room" => {
                    let client = self.clone_client();
                    if let Ok(id) = <RoomId as TryFrom<String>>::try_from(text.clone()) {
                        self.join_pwd_pending = Some(id.to_string());
                        self.join_room_task = Some(Task::new(async move {
                            client.join_room(id, false).await?;
                            client.room_state().await.ok_or_else(|| anyhow!("expected room state"))
                        }));
                    } else {
                        show_message(mtl!("join-room-invalid-id")).error();
                    }
                }
                // 加入失败且提示需要密码 → 请求输入密码后带密码重试
                "join_room_pwd" => {
                    if let Some(room_id) = self.join_pwd_pending.take() {
                        let client = self.clone_client();
                        let password = text.clone();
                        if let Ok(id) = room_id.try_into() {
                            self.join_room_task = Some(Task::new(async move {
                                client.join_room_with_password(id, false, password).await?;
                                client.room_state().await.ok_or_else(|| anyhow!("expected room state"))
                            }));
                        } else {
                            show_message(mtl!("join-room-invalid-id")).error();
                        }
                    } else {
                        return_input(id, text);
                    }
                }
                "set_pwd" => {
                    let client = self.clone_client();
                    let password = text.clone();
                    self.task = Some(Task::new(async move {
                        client.set_room_password(password).await.with_context(|| mtl!("set-password-failed"))
                    }));
                }
                "kick_user" => {
                    if let Ok(id) = text.trim().parse::<i32>() {
                        let client = self.clone_client();
                        self.task = Some(Task::new(async move {
                            client.kick_user(id).await.with_context(|| mtl!("kick-user-failed"))
                        }));
                    } else {
                        show_message(mtl!("kick-user-invalid-id")).error();
                    }
                }
                "transfer_host" => {
                    if let Ok(id) = text.trim().parse::<i32>() {
                        let client = self.clone_client();
                        self.task = Some(Task::new(async move {
                            client.transfer_host(id).await.with_context(|| mtl!("transfer-host-failed"))
                        }));
                    } else {
                        show_message(mtl!("transfer-host-invalid-id")).error();
                    }
                }
                _ => return_input(id, text),
            }
        }
        if let Some(task) = &mut self.scene_task {
            if let Some(res) = poll_future(task.as_mut()) {
                match res {
                    Err(err) => {
                        show_error(err);
                    }
                    Ok(scene) => self.next_scene = Some(scene),
                }
                self.scene_task = None;
            }
        }
        // 处理本地谱面同步事件
        self.update_local_chart();

        // 本地谱面同步任务完成
        if let Some(task) = &mut self.local_chart_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(()) => {
                        self.syncing = None;
                    }
                    Err(err) => {
                        self.syncing = None;
                        self.host_started = false;
                        self.local_ready = false;
                        self.local_download_cancel = None;
                        show_error(err);
                    }
                }
                self.local_chart_task = None;
            }
        }
        if let Some(syncing) = &self.syncing {
            if let Some(err) = syncing.error() {
                self.syncing = None;
                let err_str = err.to_string();
                let args = prpr_l10n::fluent_args!["err" => err_str.as_str()];
                show_message(mtl!("mp-sync-failed", &args)).error();
            }
        }

        if self.need_upload && self.entered {
            let id = RECORD_ID.load(Ordering::Relaxed);
            if id != -1 {
                let client = self.clone_client();
                self.task = Some(Task::new(async move { client.played(id, 0, 0., false, 0, 0, 0, 0, 0).await }));
            } else {
                let client = self.clone_client();
                self.task = Some(Task::new(async move { client.abort().await }));
            }
            self.need_upload = false;
        }

        // 深链接（phira://）自动流程：
        // 1) 未连接 → 自动连接一次（失败不重试，用户可手动重连后仍会执行 2)）；
        // 2) 已连接且未在房间 → 自动加入/创建房间（仅一次）。
        if self.deep_link.is_some() {
            if self.client.is_none() {
                if !self.deep_link_auto_connected && self.connect_task.is_none() {
                    self.deep_link_auto_connected = true;
                    if get_data().me.is_some() && get_data().tokens.is_some() {
                        self.connect();
                    }
                }
            } else if self.join_room_task.is_none()
                && self.create_room_task.is_none()
                && self.client.as_ref().and_then(|it| it.blocking_room_id()).is_none()
            {
                self.run_deep_link();
            }
        }
        Ok(())
    }

    pub fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) {
        let rt = tm.real_time() as f32;
        let t = tm.now() as f32;
        if self.side_enter_time.is_finite() {
            let p = ((rt - self.side_enter_time.abs()) / ENTER_TRANSIT).min(1.);
            let p = 1. - (1. - p).powi(3);
            let p = if self.side_enter_time < 0. { 1. - p } else { p };
            ui.fill_rect(ui.screen_rect(), semi_black(p * 0.5));
            let w = WIDTH;
            let rt = f32::tween(&-1., &(w - 1.), p);
            ui.scope(|ui| {
                ui.dx(rt - w);
                ui.dy(-ui.top);
                let h = ui.top * 2.;
                let r = Rect::new(0., 0., w, h).feather(-0.02);
                ui.fill_path(&r.rounded(0.015), semi_black(0.25));
                ui.fill_path(&r.rounded(0.015), (semi_white(0.05), (r.x, r.y), Color::default(), (r.right(), r.y)));
                if let Some(id) = self.client.as_ref().and_then(|it| it.blocking_room_id()) {
                    ui.text(mtl!("room-id", "id" => id.to_string()))
                        .pos(r.right() - 0.02, r.y + 0.02)
                        .anchor(1., 0.)
                        .size(0.44)
                        .color(semi_white(0.5))
                        .draw();
                }
                let tr = ui.text(mtl!("multiplayer"))
                    .pos(0.05, 0.05)
                    .size(0.6)
                    .color(semi_white(0.9))
                    .draw();
                let r = Rect::new(r.x, tr.bottom() + 0.02, r.w, r.bottom() - tr.bottom() - 0.02).feather(-0.02);
                if self.client.is_none() {
                    let ct = r.center();
                    let btn_r = Rect::new(ct.x - 0.14, ct.y - 0.04, 0.28, 0.08);
                    self.connect_btn.render_shadow(ui, btn_r, t, |ui, path| {
                        ui.fill_path(&path, semi_black(0.4));
                        ui.text(mtl!("connect"))
                            .pos(ct.x, ct.y)
                            .anchor(0.5, 0.5)
                            .no_baseline()
                            .size(0.5)
                            .color(semi_white(0.9))
                            .draw();
                    });
                } else {
                    self.render_main(tm, ui, r);
                }
            });
        }
        if let Some(dl) = &mut self.downloading {
            dl.render(ui, t);
        }
        if self.syncing.is_some() {
            ui.full_loading(mtl!("mp-syncing-chart"), t);
        } else if self.has_task() {
            ui.full_loading_simple(t);
        }
    }

    fn render_main(&mut self, tm: &mut TimeManager, ui: &mut Ui, r: Rect) {
        let t = tm.now() as f32;
        let client = self.client.as_ref().unwrap();
        let mr = Rect::new(r.x, r.y, r.w * 0.8, r.h - if CHAT_ENABLED { 0.12 } else { 0. });
        ui.fill_path(&mr.rounded(0.01), semi_black(0.2));
        ui.scope(|ui| {
            let mut mr = mr.feather(-0.015);
            mr.y -= 0.015;
            mr.h += 0.015;
            ui.dx(mr.x);
            ui.dy(mr.y);
            let mut y = if self.msgs_dirty_from == 0 {
                0.
            } else {
                self.msgs.get(self.msgs_dirty_from - 1).map_or(0., |it| it.bottom)
            };
            let old_dirty = self.msgs_dirty_from != self.msgs.len();
            for msg in &mut self.msgs[self.msgs_dirty_from..] {
                msg.y = y + 0.02;
                msg.bottom = msg.text(ui, mr.w).measure().bottom();
                y = msg.bottom;
            }
            if old_dirty {
                let o = y - mr.h;
                if o >= 0. {
                    self.msg_scroll.y_scroller.goto = Some(o);
                }
            }
            self.msgs_dirty_from = self.msgs.len();
            self.msg_scroll.size((mr.w, mr.h));
            let offset = self.msg_scroll.y_scroller.offset;
            self.msg_scroll.render(ui, |ui| {
                for msg in &self.msgs {
                    if msg.bottom < offset {
                        continue;
                    }
                    if msg.y > offset + mr.h {
                        break;
                    }
                    msg.text(ui, mr.w).draw();
                }
                (mr.w, self.msgs.last().map(|it| it.bottom).unwrap_or_default() + 0.03)
            });
        });

        if CHAT_ENABLED {
            let lw = 0.16;
            let h = 0.09;
            let br = Rect::new(r.x, r.bottom() - h, mr.w - lw - 0.02, h);
            ui.fill_path(&br.rounded(0.005), semi_black(0.15));
            self.chat_btn.render_input(ui, br.feather(-0.005), t, &self.chat_text, mtl!("chat-placeholder"), 0.5);
            let br = Rect::new(mr.right() - lw, br.y, lw, br.h);
            self.chat_send_btn.render_shadow(ui, br, t, |ui, path| {
                ui.fill_path(&path, semi_black(0.35));
                ui.text(mtl!("chat-send"))
                    .pos(br.center().x, br.center().y)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(0.4)
                    .color(semi_white(0.9))
                    .draw();
            });
        }

        let mut br = Rect::new(mr.right() + 0.02, mr.y, r.right() - mr.right() - 0.02, 0.1);
        let mut btns = SmallVec::<[(&mut DRectButton, String); 10]>::new();
        if let Some(state) = client.blocking_state() {
            match state.state {
                RoomState::SelectChart(_) => {
                    if client.blocking_is_host().unwrap() {
                        btns.push((&mut self.request_start_btn, mtl!("request-start").into_owned()));
                        btns.push((&mut self.lock_room_btn, mtl!("lock-room", "current" => state.locked.to_string())));
                        btns.push((&mut self.cycle_room_btn, mtl!("cycle-room", "current" => state.cycle.to_string())));
                        btns.push((&mut self.password_btn, mtl!("set-password").into_owned()));
                        btns.push((&mut self.kick_user_btn, mtl!("kick-user").into_owned()));
                        btns.push((&mut self.transfer_host_btn, mtl!("transfer-host").into_owned()));
                    }
                    btns.push((&mut self.leave_room_btn, mtl!("leave-room").into_owned()));
                }
                RoomState::LocalChart => {
                    if client.blocking_is_host().unwrap() {
                        if self.host_started {
                            btns.push((&mut self.cancel_ready_btn, mtl!("cancel-ready").into_owned()));
                        } else {
                            btns.push((&mut self.request_start_btn, mtl!("request-start").into_owned()));
                        }
                        btns.push((&mut self.lock_room_btn, mtl!("lock-room", "current" => state.locked.to_string())));
                        btns.push((&mut self.cycle_room_btn, mtl!("cycle-room", "current" => state.cycle.to_string())));
                        btns.push((&mut self.password_btn, mtl!("set-password").into_owned()));
                        btns.push((&mut self.kick_user_btn, mtl!("kick-user").into_owned()));
                        btns.push((&mut self.transfer_host_btn, mtl!("transfer-host").into_owned()));
                    } else if self.local_ready {
                        btns.push((&mut self.cancel_ready_btn, mtl!("cancel-ready").into_owned()));
                    } else if self.pending_download.is_some() {
                        btns.push((&mut self.ready_btn, mtl!("ready").into_owned()));
                    }
                    btns.push((&mut self.leave_room_btn, mtl!("leave-room").into_owned()));
                }
                RoomState::WaitingForReady => {
                    if client.blocking_is_ready().unwrap() {
                        btns.push((&mut self.cancel_ready_btn, mtl!("cancel-ready").into_owned()));
                    } else {
                        btns.push((&mut self.ready_btn, mtl!("ready").into_owned()));
                    }
                }
                _ => {}
            }
            // 谱面预览（autoplay）：选好谱后、正式开始前都可用
            let local_uuid_ready = match (&state.state, &self.local_chart) {
                (RoomState::LocalChart, Some((uuid, _))) => {
                    let ok = Path::new(&format!("{}/download/{uuid}/info.yml", dir::charts().unwrap_or_default())).exists();
                    ok
                }
                _ => false,
            };
            let previewable = match (&state.state, &self.local_chart) {
                (RoomState::SelectChart(Some(_)), _) => true,
                (RoomState::LocalChart, Some(_)) => local_uuid_ready,
                (RoomState::WaitingForReady, _) => self.chart_id.is_some(),
                _ => false,
            };
            if previewable {
                btns.push((&mut self.preview_btn, mtl!("preview").into_owned()));
            }
            btns.push((&mut self.user_list_btn, mtl!("user-list").into_owned()));
        } else {
            btns.push((&mut self.create_room_btn, mtl!("create-room").into_owned()));
            btns.push((&mut self.join_room_btn, mtl!("join-room").into_owned()));
            btns.push((&mut self.room_list_btn, mtl!("room-list").into_owned()));
            btns.push((&mut self.disconnect_btn, mtl!("disconnect").into_owned()));
        }
        // 动态布局：按钮多时自动压缩行高，避免溢出面板
        {
            let n = btns.len();
            if n > 0 {
                let gap = 0.02;
                let avail = (r.bottom() - 0.02) - mr.y;
                let h = ((avail - gap * (n as f32 - 1.)) / n as f32).min(0.1).max(0.045);
                br.h = h;
            }
        }
        for (btn, text) in btns {
            btn.render_shadow(ui, br, t, |ui, path| {
                ui.fill_path(&path, semi_black(0.3));
                ui.text(text)
                    .pos(br.center().x, br.center().y)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(0.42)
                    .color(semi_white(0.9))
                    .draw();
            });
            br.y += br.h + 0.02;
        }

        let p = self.user_list_p.now(t);
        if p > 1e-4 {
            ui.abs_scope(|ui| {
                ui.alpha(p, |ui| {
                    let users: Vec<_> = client.blocking_state().unwrap().users.values().cloned().collect();
                    let n = users.len();
                    let columns = n.clamp(2, 4);
                    let rn = n.div_ceil(columns);
                    ui.fill_rect(ui.screen_rect(), semi_black(p * 0.45));
                    let panel_w = 0.9;
                    let panel_h = (rn as f32 * 0.17 + 0.06).min(ui.top * 2. - 0.1);
                    let panel_r = Rect::new(
                        -panel_w / 2.,
                        -panel_h / 2.,
                        panel_w,
                        panel_h,
                    );
                    ui.fill_path(&panel_r.rounded(0.015), semi_black(0.3));

                    let mut iter = users.into_iter();
                    let h = 0.14;
                    let w = 0.42;
                    let pad = 0.03;
                    let width = w * columns as f32 + pad * (columns - 1) as f32;
                    let viewport_height = panel_h - 0.06;
                    ui.dx(-width / 2.);
                    ui.dy(-viewport_height / 2.);
                    self.user_list_scroll.size((width, viewport_height));
                    self.user_list_scroll.render(ui, |ui| {
                        for i in 0..rn {
                            let cn = (n - i * columns).min(columns);
                            let row_width = w * cn as f32 + pad * (cn - 1) as f32;
                            let row_offset = (width - row_width) / 2.;
                            for j in 0..cn {
                                let r = Rect::new(row_offset + j as f32 * (w + pad), i as f32 * (h + pad), w, h);
                                let Some(user) = iter.next() else { unreachable!() };
                                ui.fill_path(&r.rounded(0.008), semi_black(0.15));
                                ui.avatar(r.x + 0.055, r.center().y, 0.04, t, UserManager::opt_avatar(user.id, &self.icon_user));
                                let label = if client.blocking_is_host().unwrap_or(false) {
                                    format!("{} (#{})", user.name, user.id)
                                } else {
                                    user.name.clone()
                                };
                                ui.text(label)
                                    .pos(r.x + 0.105, r.center().y)
                                    .anchor(0., 0.5)
                                    .no_baseline()
                                    .max_width(0.32)
                                    .size(0.5)
                                    .color(semi_white(0.9))
                                    .draw();
                            }
                        }
                        (width, (rn as f32 * (h + pad) - pad).max(0.))
                    });
                });
            });
        }

        // 公共房间列表浮层（快速进房）
        let p = self.room_list_p.now(t);
        if p > 1e-4 {
            ui.abs_scope(|ui| {
                ui.alpha(p, |ui| {
                    ui.fill_rect(ui.screen_rect(), semi_black(p * 0.5));
                    let panel_w = 0.92;
                    let max_rows = 10;
                    let row_h = 0.13;
                    let rooms = self.room_list.as_deref().unwrap_or(&[]);
                    let n = rooms.len().min(max_rows);
                    let panel_h = (0.3 + n as f32 * (row_h + 0.015)).min(ui.top * 2. - 0.1);
                    let panel_r = Rect::new(-panel_w / 2., -panel_h / 2., panel_w, panel_h);
                    ui.fill_path(&panel_r.rounded(0.015), semi_black(0.3));
                    let cx = panel_r.x + 0.03;
                    let mut y = panel_r.y + 0.03;
                    ui.text(mtl!("room-list-title"))
                        .pos(cx, y)
                        .size(0.5)
                        .color(semi_white(0.9))
                        .draw();
                    y += 0.085;
                    if self.room_list.is_none() && self.room_list_task.is_some() {
                        ui.text(mtl!("room-list-loading")).pos(cx, y).size(0.4).color(semi_white(0.6)).draw();
                    }
                    let empty = rooms.is_empty() && self.room_list.is_some();
                    if empty {
                        ui.text(mtl!("room-list-empty")).pos(cx, y + 0.05).size(0.4).color(semi_white(0.6)).draw();
                    }
                    let mut shown = 0;
                    for room in rooms.iter().take(max_rows) {
                        let r = Rect::new(panel_r.x + 0.03, y + 0.02, panel_w - 0.06, row_h);
                        ui.fill_path(&r.rounded(0.008), semi_black(0.2));
                        let label = format!("#{}  ·  {}人/{}观  ·  {}", room.id, room.player_count, room.spectator_count, room.state);
                        ui.text(label)
                            .pos(r.x + 0.03, r.center().y)
                            .anchor(0., 0.5)
                            .size(0.42)
                            .max_width(r.w - 0.2)
                            .color(if room.locked { semi_white(0.5) } else { WHITE })
                            .draw();
                        if room.locked {
                            ui.text(mtl!("room-locked-tag"))
                                .pos(r.right() - 0.05, r.center().y)
                                .anchor(1., 0.5)
                                .size(0.36)
                                .color(semi_white(0.6))
                                .draw();
                        }
                        y += row_h + 0.015;
                        shown += 1;
                    }
                    if rooms.len() > max_rows {
                        ui.text(mtl!("room-list-more"))
                            .pos(cx, y + 0.02)
                            .size(0.35)
                            .color(semi_white(0.5))
                            .draw();
                    }
                });
            });
        }

        // 对局结算排名弹层
        let p = self.results_p.now(t);
        if p > 1e-4 {
            let results = self.results.clone().unwrap_or_default();
            ui.abs_scope(|ui| {
                ui.alpha(p, |ui| {
                    ui.fill_rect(ui.screen_rect(), semi_black(p * 0.55));
                    let panel_w = 0.94;
                    let row_h = 0.135;
                    let n = results.len();
                    let panel_h = (0.28 + n as f32 * (row_h + 0.012)).min(ui.top * 2. - 0.1);
                    let panel_r = Rect::new(-panel_w / 2., -panel_h / 2., panel_w, panel_h);
                    ui.fill_path(&panel_r.rounded(0.015), semi_black(0.35));
                    let cx = panel_r.x + 0.03;
                    let mut y = panel_r.y + 0.03;
                    ui.text(mtl!("results-title"))
                        .pos(cx, y)
                        .size(0.55)
                        .color(WHITE)
                        .draw_using(&prpr::core::BOLD_FONT);
                    y += 0.1;
                    // 行内可滚动（排名多时）
                    let viewport_h = panel_h - 0.13;
                    ui.dx(cx);
                    ui.dy(y);
                    self.results_scroll.size((panel_w - 0.06, viewport_h));
                    self.results_scroll.render(ui, |ui| {
                        for (i, r) in results.iter().enumerate() {
                            let rr = Rect::new(0., i as f32 * (row_h + 0.012), panel_w - 0.06, row_h);
                            ui.fill_path(&rr.rounded(0.008), semi_black(0.2));
                            let medal = if r.aborted { "✕" } else if i == 0 { "🥇" } else if i == 1 { "🥈" } else if i == 2 { "🥉" } else { "" };
                            let line = if r.aborted {
                                format!("{medal}  {}  —  {}", r.user_name, mtl!("results-aborted"))
                            } else {
                                format!("{medal}  {}  ·  {:07}  ·  {:.2}%  {} {}", r.user_name, r.score, r.accuracy * 100., if r.full_combo { "FC" } else { "" }, if r.max_combo > 0 { format!(" · {}combo", r.max_combo) } else { String::new() })
                            };
                            ui.text(line)
                                .pos(rr.x + 0.03, rr.center().y)
                                .anchor(0., 0.5)
                                .max_width(rr.w - 0.06)
                                .size(0.42)
                                .color(if r.aborted { semi_white(0.5) } else { WHITE })
                                .draw();
                        }
                        (panel_w - 0.06, n as f32 * (row_h + 0.012))
                    });
                });
            });
        }
    }

    #[inline]
    pub fn next_scene(&mut self) -> Option<NextScene> {
        self.next_scene.take()
    }
}