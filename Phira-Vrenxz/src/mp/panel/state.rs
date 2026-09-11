//! 多人面板的连接与房间状态：所有在途任务句柄、谱面下载流程、本地谱面分享
//! 状态、对局流程标记都集中在这里。本模块**不做任何绘制**，只负责“状态 +
//! 生命周期”，协议调用见 [`super::actions`]。

use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

use phira_mp_client::Client;
use phira_mp_common::{ClientRoomState, RoomId, RoomState};
use prpr::task::Task;

use crate::{client::Chart, dir, get_data, scene::Downloading};

/// 服务器公共房间列表 JSON（GET /api/rooms 返回字段的子集）。
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PublicRoom {
    pub id: String,
    #[serde(rename = "player_count")]
    pub player_count: usize,
    pub state: String,
    pub locked: bool,
    #[serde(rename = "spectator_count", default)]
    pub spectator_count: usize,
}

/// “先下载谱面、下载完再做某件事”的去向。
///
/// 旧实现用 `download_next` / `preview_pending` / `spectate_pending` 三个
/// bool 表达同一件事，且优先级散落在 `post_download` 的 if 链里；这里统一成
/// 一个枚举，同一时刻只可能有一个去向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadIntent {
    /// 下载完成后点“就绪”
    Ready,
    /// 下载完成后由房主发起开始
    RequestStart,
    /// 下载完成后以 autoplay 预览
    Preview,
    /// 下载完成后进入同步观战
    Spectate,
}

/// 谱面下载流程的内部状态（渲染需要的 `Downloading` 也放在这里）。
#[derive(Default)]
pub struct ChartDownload {
    /// 正在拉取谱面元数据的任务
    pub task: Option<Task<anyhow::Result<Arc<Chart>>>>,
    /// 正在下载谱面包的界面
    pub ui: Option<Downloading>,
    /// 下载完成后的去向
    pub intent: Option<DownloadIntent>,
    /// 本次下载的谱面 id（预览/观战的目标谱面可能不同于房间当前选谱）
    pub chart_id: i32,
}

/// 面板持有的全部协议状态与任务句柄。
pub struct MpState {
    pub client: Option<Arc<Client>>,

    // —— 连接 / 大厅 ——
    pub connect_task: Option<Task<anyhow::Result<Client>>>,
    pub create_room_task: Option<Task<anyhow::Result<()>>>,
    pub join_room_task: Option<Task<anyhow::Result<RoomState>>>,
    pub room_list_task: Option<Task<anyhow::Result<Vec<PublicRoom>>>>,
    pub room_list: Option<Vec<PublicRoom>>,
    /// 加入带密码的房间时暂存房间号，失败提示需要密码时用于一次重试
    pub join_pwd_pending: Option<String>,

    // —— 通用协议任务 ——
    /// 一次性协议调用（踢人 / 移交 / 锁房 / 改模式 / 离开 / 上传成绩 …）
    pub task: Option<Task<anyhow::Result<()>>>,
    pub chat_task: Option<Task<anyhow::Result<()>>>,
    pub chat_text: String,

    // —— 谱面 ——
    /// 房间当前选中的在线谱面
    pub chart_id: Option<i32>,
    pub download: ChartDownload,

    // —— 本地谱面分享 ——
    /// 当前分享/同步中的本地谱面 (uuid, 谱面名)
    pub local_chart: Option<(String, String)>,
    /// 谱面同步进度（浮层提示用）
    pub syncing: Option<Arc<crate::mp::serve::ChartSyncing>>,
    /// 待下载的本地谱面 (chart_id, chart_name)
    pub pending_download: Option<(String, String)>,
    pub local_chart_task: Option<Task<anyhow::Result<()>>>,
    /// 房主已把自己的谱面上传给服务端（房主侧“已开始”标记）
    pub host_started: bool,
    /// 自己已就绪（本地谱面同步流程中，服务端不提供他人的就绪状态）
    pub local_ready: bool,
    /// 下载被取消时的信号（取消后就不要再发 download_ready）
    pub local_download_cancel: Option<Arc<AtomicBool>>,

    // —— 对局流程 ——
    /// 本局 Playing 是否已被消费（避免重复进入游玩场景）
    pub game_start_consumed: bool,
    /// 从游玩场景回来时需要上报成绩
    pub need_upload: bool,
    /// 是否已经进过游玩场景（配合 `need_upload` 判断“是打完还是中途退出”）
    pub entered: bool,

    // —— 深链接（phira://）——
    pub deep_link: Option<crate::mp::PendingRoomLink>,
    pub deep_link_auto_connected: bool,
}

impl Default for MpState {
    fn default() -> Self {
        Self::new()
    }
}

impl MpState {
    pub fn new() -> Self {
        Self {
            client: None,
            connect_task: None,
            create_room_task: None,
            join_room_task: None,
            room_list_task: None,
            room_list: None,
            join_pwd_pending: None,
            task: None,
            chat_task: None,
            chat_text: String::new(),
            chart_id: None,
            download: ChartDownload::default(),
            local_chart: None,
            syncing: None,
            pending_download: None,
            local_chart_task: None,
            host_started: false,
            local_ready: false,
            local_download_cancel: None,
            game_start_consumed: false,
            need_upload: false,
            entered: false,
            deep_link: None,
            deep_link_auto_connected: false,
        }
    }

    /// 当前连接（未连接时为 None）。
    #[inline]
    pub fn client(&self) -> Option<Arc<Client>> {
        self.client.clone()
    }

    /// 当前连接；所有调用点都已确认在房间/已连接，未连接属于逻辑错误。
    #[inline]
    pub fn connected(&self) -> Arc<Client> {
        Arc::clone(self.client.as_ref().expect("multiplayer client not connected"))
    }

    /// 当前房间状态（不在房间时为 None）。
    #[inline]
    pub fn room(&self) -> Option<ClientRoomState> {
        self.client.as_ref().and_then(|it| it.blocking_state())
    }

    #[inline]
    pub fn room_id(&self) -> Option<RoomId> {
        self.client.as_ref().and_then(|it| it.blocking_room_id())
    }

    #[inline]
    pub fn in_room(&self) -> bool {
        self.room_id().is_some()
    }

    #[inline]
    pub fn me_id(&self) -> Option<i32> {
        self.client.as_ref().and_then(|c| c.me()).map(|it| it.id)
    }

    #[inline]
    pub fn room_state(&self) -> Option<RoomState> {
        self.room().map(|it| it.state)
    }

    /// 是否有面板级任务在跑（决定是否显示全屏 loading）。
    pub fn busy(&self) -> bool {
        self.connect_task.is_some()
            || self.create_room_task.is_some()
            || self.chat_task.is_some()
            || self.download.task.is_some()
            || self.local_chart_task.is_some()
            || self.task.is_some()
    }

    /// 谱面是否已在本地（download/{id} 或 download/{uuid}）。
    pub fn local_chart_ready(&self, id: Option<i32>, uuid: Option<&str>) -> bool {
        let id = match (id, uuid) {
            (Some(id), _) => id.to_string(),
            (None, Some(uuid)) => uuid.to_string(),
            _ => return false,
        };
        Path::new(&format!("{}/download/{id}/info.yml", dir::charts().ok().unwrap_or_default())).exists()
    }

    /// 当前服务器是否允许上传本地谱面。
    pub fn server_allows_local_chart() -> bool {
        const ALLOWED: &[&str] = &["mp.tianstudio.top", "mp.ratzen.top"];
        let addr = get_data().config.mp_address.as_str();
        // 去掉 scheme（如 tcp:// 等）
        let addr = addr.rsplit_once("://").map(|(_, host)| host).unwrap_or(addr);
        // 若包含端口（最后一个 ':'），取 ':' 之前作为主机名
        let host = addr.rsplit_once(':').map(|(h, _)| h).unwrap_or(addr);
        let host = host.trim().trim_matches(|c| c == '[' || c == ']');
        ALLOWED.contains(&host)
    }

    /// 从 mp_address（如 `mp2.phira.cn:12345`）推导 Web API 地址（游戏端口 + 1）。
    pub fn web_base() -> Option<String> {
        let addr = get_data().config.mp_address.clone();
        let (host, port) = addr.rsplit_once(':')?;
        let port: u16 = port.parse().ok()?;
        let host = host.trim_end_matches('.').to_owned();
        Some(format!("http://{host}:{}", port + 1))
    }

    /// 重置本地谱面分享相关的全部状态。
    pub fn reset_local_chart(&mut self) {
        self.host_started = false;
        self.local_ready = false;
        self.pending_download = None;
        self.syncing = None;
        if let Some(cancel) = self.local_download_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// 断开连接（保留面板其余状态，交由面板决定是否清空消息）。
    pub fn disconnect(&mut self) {
        self.client = None;
        self.connect_task = None;
        self.create_room_task = None;
        self.join_room_task = None;
        self.chat_task = None;
        self.task = None;
        self.local_chart_task = None;
        self.room_list = None;
        self.room_list_task = None;
        self.join_pwd_pending = None;
        self.chart_id = None;
        self.download = ChartDownload::default();
        self.reset_local_chart();
        self.local_chart = None;
        self.game_start_consumed = false;
        self.need_upload = false;
        self.entered = false;
    }
}
