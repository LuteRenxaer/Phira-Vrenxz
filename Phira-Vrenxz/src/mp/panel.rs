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
use phira_mp_common::{ClientRoomState, RoomId, RoomState};
use prpr::{
    config::Mods,
    core::{Smooth, Tweenable},
    ext::{poll_future, semi_black, semi_white, LocalTask, RectExt, SafeTexture},
    info::ChartInfo,
    scene::{request_input, return_input, show_error, show_message, take_input, GameMode, NextScene},
    task::Task,
    time::TimeManager,
    ui::{Dialog, DRectButton, DrawText},
    ui::{Scroll, Ui},
};
use std::{
    fs::File,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
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

/// 底部操作条按钮的种类（渲染与触摸共用，保证两侧按钮集合一致）。
#[derive(Clone, Copy, PartialEq, Eq)]
enum RoomAction {
    Start,
    Lock,
    Cycle,
    Pwd,
    Ready,
    Cancel,
    Preview,
}

struct ActItem {
    act: RoomAction,
    label: String,
}

/// 带透明度修改的 Color 便捷函数。
#[inline]
fn color_alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

/// 以「自己优先、其余按 id 升序」排出的用户 id 列表（渲染与触摸共用同一排序）。
fn sorted_user_ids(room: &ClientRoomState, me: Option<i32>) -> Vec<i32> {
    let mut ids: Vec<i32> = room.users.keys().copied().collect();
    ids.sort_unstable();
    if let Some(m) = me {
        if let Some(pos) = ids.iter().position(|&x| x == m) {
            let me = ids.remove(pos);
            ids.insert(0, me);
        }
    }
    ids
}

/// 在 (x, y) 起始、宽度 avail 内按行自动换行排布按钮，返回总高度与每个按钮的矩形。
fn flow_rects(ui: &mut Ui, labels: &[String], x: f32, y: f32, avail: f32, row_h: f32, gap: f32, col_gap: f32) -> (f32, Vec<Rect>) {
    const TEXT_SIZE: f32 = 0.42;
    let mut rects = Vec::with_capacity(labels.len());
    let mut cx = x;
    let mut cy = y;
    let mut rows = 1usize;
    for label in labels {
        let w = ui.text(label.as_str()).size(TEXT_SIZE).measure().w + 0.11;
        let w = w.max(0.17);
        if cx + w > x + avail && cx > x {
            cx = x;
            cy += row_h + gap;
            rows += 1;
        }
        rects.push(Rect::new(cx, cy, w, row_h));
        cx += w + col_gap;
    }
    if rects.is_empty() {
        return (0., rects);
    }
    (rows as f32 * row_h + (rows - 1) as f32 * gap, rects)
}

/// 画出玩家卡片行内容：头像 + 名字 + 右侧状态徽标（房主/我/观战/已就绪）。
/// 注意：本函数不画底色，底色由调用方（按钮路径）提供。
#[allow(clippy::too_many_arguments)]
fn draw_player_row(
    ui: &mut Ui,
    r: Rect,
    t: f32,
    icon: &SafeTexture,
    id: i32,
    name: &str,
    is_me: bool,
    host: bool,
    watching: bool,
    me_ready: bool,
    accent: Color,
) {
    let cy = r.center().y;
    let avr = (r.h * 0.42).min(0.042);
    let cx = r.x + 0.045 + avr;
    // 头像
    ui.avatar(cx, cy, avr, t, UserManager::opt_avatar(id, icon));
    // 状态徽标从右往左排
    let mut tags_right = r.right() - 0.035;
    let mut tag = |ui: &mut Ui, text: &str, bg: Color, fg: Color, size: f32| {
        let w = ui.text(text).size(size).measure().w + 0.045;
        let x = tags_right - w;
        if x < r.x + 0.14 {
            return;
        }
        let pr = Rect::new(x, cy - 0.016, w, 0.032);
        ui.fill_path(&pr.rounded(0.016), bg);
        ui.text(text)
            .pos(pr.center().x, cy)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(size)
            .color(fg)
            .draw();
        tags_right = x - 0.02;
    };
    if watching {
        let watching_tag = mtl!("mp-watching");
        tag(ui, watching_tag.as_ref(), semi_white(0.1), semi_white(0.6), 0.3);
    }
    if is_me && me_ready {
        let ready_tag = mtl!("mp-ready-tag");
        tag(ui, ready_tag.as_ref(), color_alpha(accent, 0.28), WHITE, 0.3);
    }
    if is_me {
        let me_tag = if host { mtl!("mp-host") } else { mtl!("mp-you") };
        tag(
            ui,
            me_tag.as_ref(),
            if host { color_alpha(accent, 0.3) } else { semi_white(0.12) },
            if host { WHITE } else { semi_white(0.85) },
            0.3,
        );
    }
    // 名字（左对齐，扣除右侧徽标区）
    let name_x = r.x + 0.13;
    let name_max = (tags_right - name_x - 0.02).max(0.05);
    ui.text(name)
        .pos(name_x, cy)
        .anchor(0., 0.5)
        .no_baseline()
        .max_width(name_max)
        .size(0.4)
        .color(semi_white(0.92))
        .draw();
}

/// 玩家行底色（供不可点击的展示行使用）。
fn draw_player_row_bg(ui: &mut Ui, r: Rect) {
    ui.fill_path(&r.rounded(0.008), semi_black(0.22));
}

/// 画一个小圆角胶囊文本（用于标题栏房间标签等）。
fn pill_text(ui: &mut Ui, r: Rect, text: &str, size: f32, bg: Color, fg: Color) {
    ui.fill_path(&r.rounded((r.h * 0.5).min(0.02)), bg);
    ui.text(text)
        .pos(r.center().x, r.center().y)
        .anchor(0.5, 0.5)
        .no_baseline()
        .size(size)
        .color(fg)
        .draw();
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
    // —— 预览期间监控房主开始并打断预览 ——
    // 打断信号：GameScene 每帧读取；后台轮询发现房主开始（WaitingForReady/Playing）后置位
    preview_interrupt: Option<Arc<AtomicBool>>,
    // 后台轮询停止信号：预览场景结束（被打断/自然结束/中途退出）回到面板后置位，结束轮询任务
    preview_watch_stop: Option<Arc<AtomicBool>>,
    // “房主要开始游戏啦”确认框的“准备”按钮：点击置位，update 里消费并走与“就绪”相同的流程
    preview_ready_confirm: Arc<AtomicBool>,
    // 记录"加入需要密码"的房间 id，用于二次请求密码（一次有效）
    join_pwd_pending: Option<String>,

    // 快速进房：公共房间列表
    room_list_btn: DRectButton,
    room_list_p: Smooth<f32>,
    room_list_scroll: Scroll,
    room_list: Option<Vec<PublicRoom>>,
    room_list_task: Option<Task<Result<Vec<PublicRoom>>>>,
    // 公共房间浮层行命中（渲染时同步记录，坐标与 abs 一致）
    room_rows_hits: Vec<(String, Rect)>,

    // 对局结算排名弹层
    results: Option<Vec<phira_mp_common::RoomResultEntry>>,
    results_p: Smooth<f32>,
    results_scroll: Scroll,
    results_btn: DRectButton,

    // —— 新版布局（房内右侧玩家列 + 房主管理弹层）——
    // 玩家列滚动与行按钮（行索引与 sorted_user_ids 顺序一致）
    player_scroll: Scroll,
    player_rows: Vec<DRectButton>,
    // 已请求过头像的用户 id（避免每帧重复请求）
    avatar_req: Vec<i32>,
    // 房主点击玩家行弹出的操作菜单
    manage_p: Smooth<f32>,
    manage_target: Option<i32>,
    manage_cancel_btn: DRectButton,
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
            preview_interrupt: None,
            preview_watch_stop: None,
            preview_ready_confirm: Arc::new(AtomicBool::new(false)),
            join_pwd_pending: None,

            room_list_btn: DRectButton::new(),
            room_list_p: Smooth::default(),
            room_list_scroll: Scroll::new(),
            room_list: None,
            room_list_task: None,
            room_rows_hits: Vec::new(),

            results: None,
            results_p: Smooth::default(),
            results_scroll: Scroll::new(),
            results_btn: DRectButton::new(),

            player_scroll: Scroll::new(),
            player_rows: Vec::new(),
            avatar_req: Vec::new(),
            manage_p: Smooth::default(),
            manage_target: None,
            manage_cancel_btn: DRectButton::new(),
        }
    }

    fn clone_client(&self) -> Arc<Client> {
        Arc::clone(self.client.as_ref().unwrap())
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
        self.room_rows_hits.clear();
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

    /// 以 autoplay 方式进入谱面预览（client 传 None：不参与 live/上报，不影响房间）。
    /// 同时启动一个后台轮询任务：一旦房间状态离开选谱/本地谱阶段（如房主点了开始进入
    /// WaitingForReady，或已进入 Playing），就置位 interrupt 打断预览；GameScene 检测到后
    /// 直接结束预览（不结算）并弹回房间面板。
    fn launch_preview(&mut self, path: String, id: Option<i32>) -> Result<()> {
        use crate::scene::SongScene;
        use prpr::config::Mods;
        use prpr::scene::GameMode;
        let interrupt = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        self.scene_task = SongScene::global_launch_preview(
            id,
            &path,
            Mods::AUTOPLAY,
            GameMode::NoRetry,
            None,
            None,
            None,
            false,
            false,
            true,
            Some(Arc::clone(&interrupt)),
        )?;
        // 后台轮询房间状态（预览期间 MPPanel 自身不更新，只能靠独立任务监视）
        if let Some(client) = self.client.clone() {
            let flag = Arc::clone(&interrupt);
            let stop_flag = Arc::clone(&stop);
            Task::new(async move {
                Self::preview_watch_loop(client, flag, stop_flag).await;
            });
        }
        self.preview_interrupt = Some(interrupt);
        self.preview_watch_stop = Some(stop);
        Ok(())
    }

    /// 预览期间的房间状态轮询：开始时处于选谱/本地谱阶段 → 变为 WaitingForReady / Playing
    /// 即视为“房主开始”，置位 interrupt 打断预览；若是在 WaitingForReady（等待准备、自己尚未
    /// 就绪）时就开始预览谱面，则只有真正开局（Playing）才打断。stop 置位（预览场景结束回到
    /// 面板）或离开房间后停止轮询。
    async fn preview_watch_loop(client: Arc<Client>, interrupt: Arc<AtomicBool>, stop: Arc<AtomicBool>) {
        let initial = client.blocking_room_state();
        loop {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let Some(state) = client.blocking_room_state() else {
                // 已离开房间/被移出：不再需要打断
                break;
            };
            let started = match initial {
                Some(RoomState::WaitingForReady) => matches!(state, RoomState::Playing),
                _ => !matches!(state, RoomState::SelectChart(_) | RoomState::LocalChart),
            };
            if started {
                interrupt.store(true, Ordering::Relaxed);
                break;
            }
        }
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

    // ---------- 新版 UI 辅助 ----------

    /// 让所有可点击控件这一帧失效；仅被实际绘制（render_shadow/build）的按钮会重建命中区，
    /// 避免隐藏按钮残留旧命中区误触发。
    fn invalidate_all_buttons(&mut self) {
        self.connect_btn.invalidate();
        self.create_room_btn.invalidate();
        self.join_room_btn.invalidate();
        self.leave_room_btn.invalidate();
        self.disconnect_btn.invalidate();
        self.request_start_btn.invalidate();
        self.lock_room_btn.invalidate();
        self.cycle_room_btn.invalidate();
        self.ready_btn.invalidate();
        self.cancel_ready_btn.invalidate();
        self.chat_btn.invalidate();
        self.chat_send_btn.invalidate();
        self.user_list_btn.invalidate();
        self.password_btn.invalidate();
        self.kick_user_btn.invalidate();
        self.transfer_host_btn.invalidate();
        self.preview_btn.invalidate();
        self.room_list_btn.invalidate();
        self.results_btn.invalidate();
        self.manage_cancel_btn.invalidate();
    }

    /// 房主在当前房间状态下可否管理玩家（与旧版「踢出/移交」按钮可见阶段一致）。
    fn manage_allowed(&self, room: &ClientRoomState) -> bool {
        room.is_host && matches!(room.state, RoomState::SelectChart(_) | RoomState::LocalChart)
    }

    /// 当前可预览谱面
    fn is_previewable(&self, room: &ClientRoomState) -> bool {
        let local_uuid_ready = match (&room.state, &self.local_chart) {
            (RoomState::LocalChart, Some((uuid, _))) => {
                Path::new(&format!("{}/download/{uuid}/info.yml", dir::charts().unwrap_or_default())).exists()
            }
            _ => false,
        };
        match (&room.state, &self.local_chart) {
            (RoomState::SelectChart(Some(_)), _) => true,
            (RoomState::LocalChart, Some(_)) => local_uuid_ready,
            (RoomState::WaitingForReady, _) => self.chart_id.is_some(),
            _ => false,
        }
    }

    /// 依据房间状态推导底部操作条按钮（渲染与触摸共用同一集合）。
    fn room_action_items(&self, room: &ClientRoomState) -> Vec<ActItem> {
        let mut items = Vec::new();
        let state = room.state;
        let is_host = room.is_host;
        match state {
            RoomState::SelectChart(_) => {
                if is_host {
                    items.push(ActItem { act: RoomAction::Start, label: mtl!("request-start").into_owned() });
                    items.push(ActItem { act: RoomAction::Lock, label: mtl!("lock-room", "current" => room.locked.to_string()) });
                    items.push(ActItem { act: RoomAction::Cycle, label: mtl!("cycle-room", "current" => room.cycle.to_string()) });
                    items.push(ActItem { act: RoomAction::Pwd, label: mtl!("set-password").into_owned() });
                }
            }
            RoomState::LocalChart => {
                if is_host {
                    if self.host_started {
                        items.push(ActItem { act: RoomAction::Cancel, label: mtl!("cancel-ready").into_owned() });
                    } else {
                        items.push(ActItem { act: RoomAction::Start, label: mtl!("request-start").into_owned() });
                    }
                    items.push(ActItem { act: RoomAction::Lock, label: mtl!("lock-room", "current" => room.locked.to_string()) });
                    items.push(ActItem { act: RoomAction::Cycle, label: mtl!("cycle-room", "current" => room.cycle.to_string()) });
                    items.push(ActItem { act: RoomAction::Pwd, label: mtl!("set-password").into_owned() });
                } else if self.local_ready {
                    items.push(ActItem { act: RoomAction::Cancel, label: mtl!("cancel-ready").into_owned() });
                } else if self.pending_download.is_some() && self.syncing.is_none() {
                    items.push(ActItem { act: RoomAction::Ready, label: mtl!("ready").into_owned() });
                }
            }
            RoomState::WaitingForReady => {
                if room.is_ready {
                    items.push(ActItem { act: RoomAction::Cancel, label: mtl!("cancel-ready").into_owned() });
                } else {
                    items.push(ActItem { act: RoomAction::Ready, label: mtl!("ready").into_owned() });
                }
            }
            _ => {}
        }
        if self.is_previewable(room) {
            items.push(ActItem { act: RoomAction::Preview, label: mtl!("preview").into_owned() });
        }
        items
    }

    /// 请求新出现的用户头像（避免每帧重复请求）。
    fn sync_avatar_requests(&mut self, ids: &[i32]) {
        for &id in ids {
            if !self.avatar_req.contains(&id) {
                self.avatar_req.push(id);
                UserManager::request(id);
            }
        }
    }

    /// 房主对某玩家执行踢出（沿用原 kick_user 协议调用）。
    fn kick_user_managed(&mut self, id: i32) {
        let client = self.clone_client();
        self.task = Some(Task::new(async move {
            client.kick_user(id).await.with_context(|| mtl!("kick-user-failed"))
        }));
    }

    /// 房主移交房主给某玩家（沿用原 transfer_host 协议调用）。
    fn transfer_host_managed(&mut self, id: i32) {
        let client = self.clone_client();
        self.task = Some(Task::new(async move {
            client.transfer_host(id).await.with_context(|| mtl!("transfer-host-failed"))
        }));
    }

    fn open_manage(&mut self, id: i32, t: f32) {
        self.manage_target = Some(id);
        self.manage_p.goto(1., t, USER_LIST_TRANSIT);
    }

    fn close_manage(&mut self, t: f32) {
        self.manage_target = None;
        self.manage_p.goto(0., t, USER_LIST_TRANSIT);
    }

    // ---------- 渲染：左侧消息列表 ----------

    /// 消息列表（含布局测量与滚动），r 为面板局部坐标下的消息区矩形。
    fn render_msg_list(&mut self, ui: &mut Ui, r: Rect) {
        ui.scope(|ui| {
            ui.dx(r.x);
            ui.dy(r.y);
            let mut y = if self.msgs_dirty_from == 0 {
                0.
            } else {
                self.msgs.get(self.msgs_dirty_from - 1).map_or(0., |it| it.bottom)
            };
            let old_dirty = self.msgs_dirty_from != self.msgs.len();
            for msg in &mut self.msgs[self.msgs_dirty_from..] {
                msg.y = y + 0.02;
                msg.bottom = msg.text(ui, r.w).measure().bottom();
                y = msg.bottom;
            }
            if old_dirty {
                let o = y - r.h;
                if o >= 0. {
                    self.msg_scroll.y_scroller.goto = Some(o);
                }
            }
            self.msgs_dirty_from = self.msgs.len();
            self.msg_scroll.size((r.w, r.h));
            let offset = self.msg_scroll.y_scroller.offset;
            self.msg_scroll.render(ui, |ui| {
                if self.msgs.is_empty() {
                    ui.text(mtl!("mp-msg-none"))
                        .pos(r.w / 2., 0.04)
                        .anchor(0.5, 0.)
                        .size(0.36)
                        .color(semi_white(0.4))
                        .draw();
                    return (r.w, r.h);
                }
                for msg in &self.msgs {
                    if msg.bottom < offset {
                        continue;
                    }
                    if msg.y > offset + r.h {
                        break;
                    }
                    msg.text(ui, r.w).draw();
                }
                (r.w, self.msgs.last().map(|it| it.bottom).unwrap_or_default() + 0.03)
            });
        });
    }

    // ---------- 渲染：房间内主体 ----------

    fn render_room_body(&mut self, ui: &mut Ui, t: f32, room: &ClientRoomState, me: Option<i32>) {
        let accent = ui.accent();
        let pw = WIDTH;
        let pb = ui.top * 2.;
        let pad = 0.05;

        // 头像 icon（局部 clone，避免与按钮等自借用冲突）
        let icon = self.icon_user.clone();

        // 底部操作条内容（先测量布局，再据此预留空间）
        let items = self.room_action_items(room);
        let labels: Vec<String> = items.iter().map(|it| it.label.clone()).collect();
        let row_h = 0.105;
        let bar_gap = 0.028;
        let col_gap = 0.04;
        let bar_x = pad;
        let bar_w = pw - pad * 2.;
        let (bar_h, bar_rects) = flow_rects(ui, &labels, bar_x, 0., bar_w, row_h, bar_gap, 0.03);
        let bar_top = pb - pad - bar_h;
        let col_top = 0.17;
        let col_bottom = if bar_h > 0. { bar_top - 0.045 } else { pb - pad };

        // 两栏几何
        let avail_w = pw - pad * 2. - col_gap;
        let lw = avail_w * 0.62;
        let rw = avail_w - lw;
        let lx = pad;
        let rx = lx + lw + col_gap;
        let col_h = (col_bottom - col_top).max(0.05);

        // 左栏背景
        let lrect = Rect::new(lx, col_top, lw, col_h);
        ui.fill_path(&lrect.rounded(0.02), semi_black(0.18));

        // 状态卡
        let m = 0.03;
        let st_h = 0.2;
        let st = Rect::new(lx + m, col_top + m, lw - m * 2., st_h);
        ui.fill_path(&st.rounded(0.014), semi_black(0.24));
        // 状态卡左侧竖向高光条
        ui.fill_rect(Rect::new(st.x, st.y, 0.012, st.h), color_alpha(accent, 0.55));

        // 状态标题
        let (title, sub): (String, Option<String>) = match room.state {
            RoomState::SelectChart(None) => (mtl!("mp-state-choose").into_owned(), None),
            RoomState::SelectChart(Some(id)) => (mtl!("mp-state-chosen", "id" => id as u64), None),
            RoomState::LocalChart => {
                let name = self.local_chart.as_ref().map(|it| it.1.clone()).filter(|s| !s.is_empty());
                (mtl!("mp-state-local").into_owned(), name)
            }
            RoomState::WaitingForReady => (mtl!("mp-state-wait").into_owned(), None),
            RoomState::Playing => (mtl!("mp-state-playing").into_owned(), None),
        };
        ui.text(&title)
            .pos(st.x + 0.03, st.center().y - 0.045)
            .anchor(0., 0.5)
            .no_baseline()
            .max_width(st.w - 0.08)
            .size(0.46)
            .color(semi_white(0.95))
            .draw();
        // 第二行：谱面名 / 房间状态（锁定、循环、人数）
        let mut parts: Vec<String> = Vec::new();
        if let Some(name) = sub {
            parts.push(name);
        }
        if room.locked {
            parts.push(mtl!("mp-locked-tag").into_owned());
        }
        if room.cycle {
            parts.push(mtl!("mp-cycle-tag").into_owned());
        }
        parts.push(mtl!("mp-n-players", "n" => room.users.len() as u64));
        let sub_text = parts.join("  ·  ");
        ui.text(&sub_text)
            .pos(st.x + 0.03, st.center().y + 0.05)
            .anchor(0., 0.5)
            .no_baseline()
            .max_width(st.w - 0.08)
            .size(0.32)
            .color(semi_white(0.5))
            .draw();

        // 消息标题行 + 消息滚动区
        let caption_h = 0.09;
        let chat_h = if CHAT_ENABLED { 0.1 + 0.03 } else { 0. };
        let msg_top = st.bottom() + 0.005;
        let msg_area = Rect::new(lx + m, msg_top, lw - m * 2., (col_bottom - m - chat_h) - msg_top);
        if msg_area.h > 0.03 {
            ui.text(mtl!("mp-chat-caption"))
                .pos(msg_area.x, msg_area.y)
                .anchor(0., 0.)
                .size(0.32)
                .color(semi_white(0.45))
                .draw();
            let list = Rect::new(msg_area.x, msg_area.y + caption_h, msg_area.w, msg_area.h - caption_h);
            if list.h > 0.03 {
                self.render_msg_list(ui, list);
            }
        }
        // 聊天输入行（CHAT_ENABLED）
        if CHAT_ENABLED {
            let y = col_bottom - m - 0.1;
            let br = Rect::new(lx + m, y, lw - m * 2. - 0.15, 0.1);
            ui.fill_path(&br.rounded(0.006), semi_black(0.15));
            self.chat_btn.render_input(ui, br.feather(-0.005), t, &self.chat_text, mtl!("chat-placeholder"), 0.5);
            let sbr = Rect::new(br.right() + 0.01, y, 0.14, 0.1);
            let send_bg = color_alpha(accent, 0.8);
            self.chat_send_btn.render_shadow(ui, sbr, t, |ui, path| {
                ui.fill_path(&path, send_bg);
                ui.text(mtl!("chat-send"))
                    .pos(sbr.center().x, sbr.center().y)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(0.4)
                    .color(WHITE)
                    .draw();
            });
        }

        // 右栏：玩家列表
        let rrect = Rect::new(rx, col_top, rw, col_h);
        ui.fill_path(&rrect.rounded(0.02), semi_black(0.18));
        let ids = sorted_user_ids(room, me);
        let user_count = ids.len();
        self.sync_avatar_requests(&ids);

        // 栏头：玩家按钮（点击打开全屏玩家浮层）
        let hdr_h = 0.09;
        let hdr = Rect::new(rx + m, col_top + m, rw - m * 2., hdr_h);
        self.user_list_btn.render_shadow(ui, hdr, t, |ui, path| {
            ui.fill_path(&path, semi_black(0.25));
            ui.text(mtl!("mp-player-count", "n" => user_count as u64))
                .pos(hdr.center().x, hdr.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.42)
                .color(semi_white(0.92))
                .draw();
        });

        // 行滚动区
        let rows_top = col_top + m + hdr_h + 0.035;
        let rows_h = (col_bottom - m - rows_top).max(0.05);
        let rows_w = rw - m * 2.;
        let row_step = 0.13;
        let view_h = (user_count as f32 * row_step).max(0.);
        let pool = &mut self.player_rows;
        pool.resize_with(ids.len(), DRectButton::new);
        let my_state_ready = room.is_ready;
        let local_ready = self.local_ready;
        let host_started = self.host_started;
        ui.scope(|ui| {
            ui.dx(rx + m);
            ui.dy(rows_top);
            self.player_scroll.size((rows_w, rows_h));
            self.player_scroll.render(ui, |ui| {
                for (i, &id) in ids.iter().enumerate() {
                    let rr = Rect::new(0., i as f32 * row_step, rows_w, row_step - 0.015);
                    let Some(user) = room.users.get(&id) else { continue };
                    let is_me = user.id == me.unwrap_or(i32::MIN);
                    let row_bg = if is_me { color_alpha(accent, 0.12) } else { semi_black(0.17) };
                    pool[i].build(ui, t, rr, |ui, path| {
                        ui.fill_path(&path, row_bg);
                    });
                    let watching = user.monitor;
                    // 就绪徽标仅对自己可观测（协议未提供他人就绪状态）
                    let me_ready = is_me
                        && match room.state {
                            RoomState::WaitingForReady => my_state_ready,
                            RoomState::LocalChart => {
                                if room.is_host {
                                    host_started
                                } else {
                                    local_ready
                                }
                            }
                            _ => false,
                        };
                    let crown = is_me && room.is_host;
                    draw_player_row(ui, rr, t, &icon, user.id, &user.name, is_me, crown, watching, me_ready, accent);
                }
                (rows_w, view_h)
            });
        });

        // 底部操作条
        if !bar_rects.is_empty() {
            for (i, item) in items.iter().enumerate() {
                let br0 = bar_rects[i];
                let r = Rect::new(br0.x, bar_top + br0.y, br0.w, row_h);
                let (fill, fg): (Color, Color) = match item.act {
                    RoomAction::Start | RoomAction::Ready => (accent, WHITE),
                    _ => (semi_black(0.34), semi_white(0.92)),
                };
                match item.act {
                    RoomAction::Start => self.request_start_btn.render_shadow(ui, r, t, |ui, path| {
                        ui.fill_path(&path, fill);
                        ui.text(&item.label)
                            .pos(r.center().x, r.center().y)
                            .anchor(0.5, 0.5)
                            .no_baseline()
                            .size(0.42)
                            .color(fg)
                            .max_width(r.w)
                            .draw();
                    }),
                    RoomAction::Lock => self.lock_room_btn.render_shadow(ui, r, t, |ui, path| {
                        ui.fill_path(&path, fill);
                        ui.text(&item.label)
                            .pos(r.center().x, r.center().y)
                            .anchor(0.5, 0.5)
                            .no_baseline()
                            .size(0.42)
                            .color(fg)
                            .max_width(r.w)
                            .draw();
                    }),
                    RoomAction::Cycle => self.cycle_room_btn.render_shadow(ui, r, t, |ui, path| {
                        ui.fill_path(&path, fill);
                        ui.text(&item.label)
                            .pos(r.center().x, r.center().y)
                            .anchor(0.5, 0.5)
                            .no_baseline()
                            .size(0.42)
                            .color(fg)
                            .max_width(r.w)
                            .draw();
                    }),
                    RoomAction::Pwd => self.password_btn.render_shadow(ui, r, t, |ui, path| {
                        ui.fill_path(&path, fill);
                        ui.text(&item.label)
                            .pos(r.center().x, r.center().y)
                            .anchor(0.5, 0.5)
                            .no_baseline()
                            .size(0.42)
                            .color(fg)
                            .max_width(r.w)
                            .draw();
                    }),
                    RoomAction::Ready => self.ready_btn.render_shadow(ui, r, t, |ui, path| {
                        ui.fill_path(&path, fill);
                        ui.text(&item.label)
                            .pos(r.center().x, r.center().y)
                            .anchor(0.5, 0.5)
                            .no_baseline()
                            .size(0.42)
                            .color(fg)
                            .max_width(r.w)
                            .draw();
                    }),
                    RoomAction::Cancel => self.cancel_ready_btn.render_shadow(ui, r, t, |ui, path| {
                        ui.fill_path(&path, fill);
                        ui.text(&item.label)
                            .pos(r.center().x, r.center().y)
                            .anchor(0.5, 0.5)
                            .no_baseline()
                            .size(0.42)
                            .color(fg)
                            .max_width(r.w)
                            .draw();
                    }),
                    RoomAction::Preview => self.preview_btn.render_shadow(ui, r, t, |ui, path| {
                        ui.fill_path(&path, fill);
                        ui.text(&item.label)
                            .pos(r.center().x, r.center().y)
                            .anchor(0.5, 0.5)
                            .no_baseline()
                            .size(0.42)
                            .color(fg)
                            .max_width(r.w)
                            .draw();
                    }),
                }
            }
        }
    }

    // ---------- 渲染：大厅 / 未连接 ----------

    /// 已连接、未进房：居中卡片（创建/加入/公共房间）+ 底部断开连接。
    fn render_lobby_body(&mut self, ui: &mut Ui, t: f32) {
        let accent = ui.accent();
        let pw = WIDTH;
        let pb = ui.top * 2.;
        let pad = 0.05;
        let avail = pw - pad * 2.;
        let btn_h = 0.19;
        let yc = (0.2 + (pb - 0.2) * 0.42).max(0.2);
        let gap = 0.045;
        let bw = (avail - gap * 2.) / 3.;
        let x0 = pad;
        let by = yc;

        // 卡片底
        let card = Rect::new(pad, yc - 0.03, avail, btn_h + 0.3);
        ui.fill_path(&card.rounded(0.02), semi_black(0.16));

        // 三个大按钮
        let create_r = Rect::new(x0, by, bw, btn_h);
        let join_r = Rect::new(x0 + (bw + gap), by, bw, btn_h);
        let list_r = Rect::new(x0 + (bw + gap) * 2., by, bw, btn_h);
        self.create_room_btn.render_shadow(ui, create_r, t, |ui, path| {
            ui.fill_path(&path, semi_black(0.32));
            ui.text(mtl!("create-room"))
                .pos(create_r.center().x, create_r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.48)
                .color(semi_white(0.95))
                .max_width(create_r.w)
                .draw();
        });
        self.join_room_btn.render_shadow(ui, join_r, t, |ui, path| {
            ui.fill_path(&path, semi_black(0.32));
            ui.text(mtl!("join-room"))
                .pos(join_r.center().x, join_r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.48)
                .color(semi_white(0.95))
                .max_width(join_r.w)
                .draw();
        });
        self.room_list_btn.render_shadow(ui, list_r, t, |ui, path| {
            ui.fill_path(&path, color_alpha(accent, 0.55));
            ui.text(mtl!("room-list"))
                .pos(list_r.center().x, list_r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.48)
                .color(WHITE)
                .max_width(list_r.w)
                .draw();
        });

        // 状态小字
        ui.text(mtl!("mp-lobby-connected"))
            .pos(pw / 2., by + btn_h + 0.06)
            .anchor(0.5, 0.)
            .size(0.36)
            .color(accent)
            .draw();
        ui.text(mtl!("mp-lobby-not-room"))
            .pos(pw / 2., by + btn_h + 0.11)
            .anchor(0.5, 0.)
            .size(0.32)
            .color(semi_white(0.45))
            .draw();

        // 底部小字断开连接
        let w = ui.text(mtl!("disconnect")).size(0.34).measure().w + 0.1;
        let dr = Rect::new(pw / 2. - w / 2., pb - pad - 0.075, w, 0.06);
        self.disconnect_btn.render_shadow(ui, dr, t, |ui, path| {
            ui.fill_path(&path, semi_black(0.3));
            ui.text(mtl!("disconnect"))
                .pos(dr.center().x, dr.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.36)
                .color(semi_white(0.7))
                .draw();
        });
    }

    /// 未连接：居中连接按钮。
    fn render_connect_body(&mut self, ui: &mut Ui, t: f32) {
        let accent = ui.accent();
        let pw = WIDTH;
        let pb = ui.top * 2.;
        let yc = (0.2 + (pb - 0.2) * 0.44).max(0.2);
        ui.text(mtl!("mp-connect-hint"))
            .pos(pw / 2., yc - 0.16)
            .anchor(0.5, 0.)
            .size(0.38)
            .color(semi_white(0.55))
            .max_width(pw - 0.2)
            .draw();
        let btn_r = Rect::new(pw / 2. - 0.22, yc - 0.09, 0.44, 0.14);
        self.connect_btn.render_shadow(ui, btn_r, t, |ui, path| {
            ui.fill_path(&path, accent);
            ui.text(mtl!("connect"))
                .pos(btn_r.center().x, btn_r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.52)
                .color(WHITE)
                .draw();
        });
        // 面板关闭小提示
        ui.text(mtl!("mp-close-hint"))
            .pos(pw / 2., pb - 0.05)
            .anchor(0.5, 1.)
            .size(0.3)
            .color(semi_white(0.3))
            .draw();
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
        // 谱面预览场景结束（弹回 MainScene 时 enter 会被调用）：停止后台轮询；
        // 若刚才是被“房主开始”打断，且房间仍停在 WaitingForReady、自己还未就绪，
        // 弹醒目确认框询问是否现在准备（点“准备”走与“就绪”相同的下载→ready 流程）。
        if let Some(stop) = self.preview_watch_stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        let Some(interrupt) = self.preview_interrupt.take() else {
            return;
        };
        if !interrupt.load(Ordering::Relaxed) {
            // 预览自然结束/中途退出：无需询问
            return;
        }
        // 已被房主开始打断：只有房间仍在 WaitingForReady 且自己未就绪才询问；
        // 若已进入 Playing（可能已开局），不在这里准备，交给 update 里的正式开局逻辑接走
        let ask_ready = self.client.as_ref().is_some_and(|client| {
            matches!(client.blocking_room_state(), Some(RoomState::WaitingForReady))
                && !client.blocking_is_ready().unwrap_or(false)
        });
        if !ask_ready {
            return;
        }
        let confirm = Arc::clone(&self.preview_ready_confirm);
        Dialog::plain(mtl!("preview-interrupted-title"), mtl!("preview-interrupted-content"))
            .buttons(vec![mtl!("preview-not-now").into_owned(), mtl!("preview-ready").into_owned()])
            .listener(move |_dialog, id| {
                if id == 1 {
                    confirm.store(true, Ordering::Relaxed);
                }
                false
            })
            .show();
    }

    pub fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> bool {
        let t = tm.now() as f32;
        if self.side_enter_time.is_infinite() {
            return false;
        }
        // 房主操作菜单（最上层）
        if self.manage_p.transiting(t) {
            return true;
        }
        if *self.manage_p.to() > 0.5 {
            if self.manage_target.is_some() {
                if self.transfer_host_btn.touch(touch, t) {
                    let id = self.manage_target.take().unwrap();
                    self.manage_p.goto(0., t, USER_LIST_TRANSIT);
                    self.transfer_host_managed(id);
                    return true;
                }
                if self.kick_user_btn.touch(touch, t) {
                    let id = self.manage_target.take().unwrap();
                    self.manage_p.goto(0., t, USER_LIST_TRANSIT);
                    self.kick_user_managed(id);
                    return true;
                }
                if self.manage_cancel_btn.touch(touch, t) {
                    self.close_manage(t);
                    return true;
                }
                if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                    self.close_manage(t);
                }
            } else {
                self.manage_p.goto(0., t, USER_LIST_TRANSIT);
            }
            return true;
        }
        // 玩家列表浮层
        if self.user_list_p.transiting(t) {
            return true;
        }
        if *self.user_list_p.to() > 0.5 {
            if self.user_list_scroll.touch(touch, t) {
                return true;
            }
            if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                self.user_list_scroll.y_scroller.halt();
                self.user_list_p.goto(0., t, USER_LIST_TRANSIT);
            }
            return true;
        }
        // 公共房间列表浮层
        if self.room_list_p.transiting(t) {
            return true;
        }
        if *self.room_list_p.to() > 0.5 {
            if matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                let hit = self.room_rows_hits.iter().find(|(_, r)| r.contains(touch.position));
                if let Some((room_id, _)) = hit {
                    self.room_list_p.goto(0., t, USER_LIST_TRANSIT);
                    let client = self.clone_client();
                    if let Ok(id) = room_id.clone().try_into() {
                        self.join_room_task = Some(Task::new(async move {
                            client.join_room(id, false).await?;
                            client.room_state().await.ok_or_else(|| anyhow!("expected room state"))
                        }));
                    }
                } else {
                    self.room_list_p.goto(0., t, USER_LIST_TRANSIT);
                }
            }
            return true;
        }
        // 结算浮层
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

        if self.client.is_none() {
            if self.connect_btn.touch(touch, t) {
                self.connect();
                return true;
            }
            return true;
        }

        let client = Arc::clone(self.client.as_ref().unwrap());
        let room = client.blocking_state();
        let in_room = room.is_some();
        if in_room {
            // 消息滚动区
            if self.msg_scroll.contains(touch) && self.msg_scroll.touch(touch, t) {
                return true;
            }
            // 玩家列滚动
            let in_players = self.player_scroll.contains(touch);
            if in_players && self.player_scroll.touch(touch, t) {
                return true;
            }
            // 聊天输入 / 发送（房间内渲染）
            if CHAT_ENABLED {
                if self.chat_btn.touch(touch, t) {
                    request_input("chat", InputBox::new().default_text(&self.chat_text));
                    return true;
                }
                if self.chat_send_btn.touch(touch, t) {
                    if self.chat_text.is_empty() {
                        show_message(mtl!("chat-empty")).error();
                    } else {
                        let client = Arc::clone(&client);
                        let text = self.chat_text.clone();
                        self.chat_task = Some(Task::new(async move { client.chat(text).await }));
                    }
                    return true;
                }
            }
            // 右上角：离开房间
            if self.leave_room_btn.touch(touch, t) {
                let client = self.clone_client();
                self.task = Some(Task::new(async move { client.leave_room().await }));
                return true;
            }
            // 玩家列表浮层开关（栏头）
            if self.user_list_btn.touch(touch, t) {
                self.user_list_scroll.y_scroller.reset();
                self.user_list_p.goto(1., t, USER_LIST_TRANSIT);
                if let Some(users) = room.as_ref() {
                    let ids = sorted_user_ids(users, client.me().map(|it| it.id));
                    self.sync_avatar_requests(&ids);
                }
                return true;
            }
            // 房主点击玩家行 → 管理菜单（取代输入玩家 ID）
            if let Some(r) = &room {
                if self.manage_allowed(r) {
                    let me = client.me().map(|it| it.id);
                    let ids = sorted_user_ids(r, me);
                    for (i, btn) in self.player_rows.iter_mut().enumerate() {
                        if btn.touch(touch, t) {
                            if let Some(&id) = ids.get(i) {
                                if me != Some(id) {
                                    self.open_manage(id, t);
                                }
                            }
                            return true;
                        }
                    }
                }
            }
            // 底部操作条
            if let Some(r) = &room {
                let items = self.room_action_items(r);
                let has = |act: RoomAction| items.iter().any(|it| it.act == act);
                match r.state {
                    RoomState::SelectChart(_) => {
                        if r.is_host {
                            if has(RoomAction::Start) && self.request_start_btn.touch(touch, t) {
                                self.request_start();
                                return true;
                            }
                            if has(RoomAction::Lock) && self.lock_room_btn.touch(touch, t) {
                                let to = !r.locked;
                                let client = self.clone_client();
                                self.task = Some(Task::new(async move { client.lock_room(to).await.with_context(|| mtl!("lock-room-failed")) }));
                                return true;
                            }
                            if has(RoomAction::Pwd) && self.password_btn.touch(touch, t) {
                                request_input("set_pwd", InputBox::new());
                                return true;
                            }
                            if has(RoomAction::Cycle) && self.cycle_room_btn.touch(touch, t) {
                                let to = !r.cycle;
                                let client = self.clone_client();
                                self.task = Some(Task::new(async move { client.cycle_room(to).await.with_context(|| mtl!("cycle-room-failed")) }));
                                return true;
                            }
                        }
                    }
                    RoomState::LocalChart => {
                        if r.is_host {
                            if self.host_started {
                                if has(RoomAction::Cancel) && self.cancel_ready_btn.touch(touch, t) {
                                    self.cancel_local_chart();
                                    return true;
                                }
                            } else if has(RoomAction::Start) && self.request_start_btn.touch(touch, t) {
                                self.request_start();
                                return true;
                            }
                            if has(RoomAction::Lock) && self.lock_room_btn.touch(touch, t) {
                                let to = !r.locked;
                                let client = self.clone_client();
                                self.task = Some(Task::new(async move { client.lock_room(to).await.with_context(|| mtl!("lock-room-failed")) }));
                                return true;
                            }
                            if has(RoomAction::Pwd) && self.password_btn.touch(touch, t) {
                                request_input("set_pwd", InputBox::new());
                                return true;
                            }
                            if has(RoomAction::Cycle) && self.cycle_room_btn.touch(touch, t) {
                                let to = !r.cycle;
                                let client = self.clone_client();
                                self.task = Some(Task::new(async move { client.cycle_room(to).await.with_context(|| mtl!("cycle-room-failed")) }));
                                return true;
                            }
                        } else if self.local_ready {
                            if has(RoomAction::Cancel) && self.cancel_ready_btn.touch(touch, t) {
                                self.cancel_local_download();
                                return true;
                            }
                        } else if self.syncing.is_none() && has(RoomAction::Ready) && self.ready_btn.touch(touch, t) {
                            self.local_ready = true;
                            self.start_pending_download();
                            return true;
                        }
                    }
                    RoomState::WaitingForReady => {
                        if client.blocking_is_ready().unwrap() {
                            if has(RoomAction::Cancel) && self.cancel_ready_btn.touch(touch, t) {
                                let client = self.clone_client();
                                self.task = Some(Task::new(async move { client.cancel_ready().await }));
                                return true;
                            }
                        } else if has(RoomAction::Ready) && self.ready_btn.touch(touch, t) {
                            self.check_download(false);
                            return true;
                        }
                    }
                    _ => {}
                }
                if has(RoomAction::Preview) && self.preview_btn.touch(touch, t) {
                    self.start_preview();
                    return true;
                }
            }
        } else {
            // 未进房（大厅）：创建 / 加入 / 公共房间 / 断开
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
                self.player_rows.clear();
                return true;
            }
        }
        if client.ping_fail_count() >= 2 && self.connect_task.is_none() {
            warn!("lost connection, reconnecting…");
            show_message(mtl!("reconnect")).warn();
            self.connect();
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
        if self.player_scroll.matrix().is_some() && self.client.as_ref().is_some_and(|it| it.blocking_room_id().is_some()) {
            self.player_scroll.update(t);
        }
        if self.manage_p.now(t) > 1e-4 {
            // 操作菜单无滚动内容
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
                        // user==0 为服务器系统消息（如进房欢迎提示），不显示发送者前缀
                        content: if user == 0 {
                            content
                        } else {
                            format!("{}：{content}", client.user_name(user))
                        },
                        y: 0.,
                        bottom: 0.,
                        color: if user == 0 { semi_white(0.7) } else { WHITE },
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
                        show_message(mtl!("server-welcome")).ok(); // 进服提示
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
        // 预览被打断后弹窗点了“准备”：与 WaitingForReady 状态点“就绪”相同的流程（下载→ready）。
        // 若房间已不在 WaitingForReady（如已进入 Playing/已就绪），不再执行 ready，交给正式开局逻辑。
        if self.preview_ready_confirm.swap(false, Ordering::Relaxed) {
            let waiting = self
                .client
                .as_ref()
                .is_some_and(|client| {
                    matches!(client.blocking_room_state(), Some(RoomState::WaitingForReady))
                        && !client.blocking_is_ready().unwrap_or(false)
                });
            if waiting {
                self.check_download(false);
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
                        // 预览场景启动失败：停掉后台轮询并清理打断标志（避免残留任务/误判）
                        if let Some(stop) = self.preview_watch_stop.take() {
                            stop.store(true, Ordering::Relaxed);
                        }
                        self.preview_interrupt = None;
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
                self.render_main(tm, ui, r);
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
        let _ = r;
        let t = tm.now() as f32;
        // 每帧先使所有按钮失效；只有本帧绘制到的按钮会拥有命中区
        self.invalidate_all_buttons();

        let client_room = self.client.as_ref().and_then(|it| it.blocking_state());
        let in_room = client_room.is_some();
        let me = self.client.as_ref().and_then(|c| c.me()).map(|it| it.id);

        // —— 顶部标题栏 ——
        ui.text(mtl!("multiplayer"))
            .pos(0.05, 0.052)
            .size(0.58)
            .color(semi_white(0.95))
            .draw();
        if in_room {
            if let Some(rid) = self.client.as_ref().and_then(|it| it.blocking_room_id()) {
                let tag = mtl!("mp-room-tag", "id" => rid.to_string());
                let tw = (ui.text(&tag).size(0.34).measure().w + 0.09).min(0.62);
                let tr = Rect::new(0.3, 0.052, tw, 0.075);
                pill_text(ui, tr, &tag, 0.34, semi_white(0.09), semi_white(0.85));
            }
        }
        // 退出按钮：房内 = 右上角「离开房间」；未进房 = 大厅底部「断开连接」
        if in_room {
            let er = Rect::new(WIDTH - 0.05 - 0.17, 0.04, 0.17, 0.1);
            let fill = Color::from_rgba(120, 40, 40, 235);
            self.leave_room_btn.render_shadow(ui, er, t, |ui, path| {
                ui.fill_path(&path, fill);
                ui.text(mtl!("leave-room"))
                    .pos(er.center().x, er.center().y)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(0.4)
                    .color(WHITE)
                    .max_width(er.w - 0.02)
                    .draw();
            });
        }

        if self.client.is_none() {
            self.render_connect_body(ui, t);
        } else if in_room {
            if let Some(room) = client_room {
                self.render_room_body(ui, t, &room, me);
            }
        } else {
            self.render_lobby_body(ui, t);
        }

        // —— 居中浮层（重排版）：玩家列表 / 公共房间 / 结算 / 房主操作菜单 ——
        self.render_user_overlay(ui, t);
        self.render_room_list_overlay(ui, t);
        self.render_results_overlay(ui, t);
        self.render_manage_overlay(ui, t);
    }

    // ---------- 居中浮层 ----------

    /// 玩家列表居中浮层（点击空白处关闭）。
    fn render_user_overlay(&mut self, ui: &mut Ui, t: f32) {
        let p = self.user_list_p.now(t);
        if p <= 1e-4 {
            return;
        }
        let Some(client) = self.client.clone() else {
            self.user_list_p.goto(0., t, USER_LIST_TRANSIT);
            return;
        };
        let Some(room) = client.blocking_state() else {
            self.user_list_p.goto(0., t, USER_LIST_TRANSIT);
            return;
        };
        let accent = ui.accent();
        let icon = self.icon_user.clone();
        let me = client.me().map(|it| it.id);
        let ids = sorted_user_ids(&room, me);
        let n = ids.len();
        let max_rows = 12usize;
        let row_h = 0.16;
        let panel_w = 0.95;
        let panel_h = (0.32 + (n.min(max_rows) as f32) * (row_h + 0.02) + if n > max_rows { 0.06 } else { 0. })
            .min(ui.top * 2. - 0.1)
            .max(0.4);
        ui.abs_scope(|ui| {
            ui.alpha(p, |ui| {
                ui.fill_rect(ui.screen_rect(), semi_black(p * 0.5));
                let panel = Rect::new(-panel_w / 2., -panel_h / 2., panel_w, panel_h);
                ui.fill_path(&panel.rounded(0.018), semi_black(0.4));
                // 标题
                ui.text(mtl!("mp-player-count", "n" => n as u64))
                    .pos(panel.x + 0.035, panel.y + 0.025)
                    .size(0.44)
                    .color(semi_white(0.92))
                    .draw();
                ui.text(mtl!("user-list-hint"))
                    .pos(panel.right() - 0.035, panel.y + 0.035)
                    .anchor(1., 0.)
                    .size(0.3)
                    .color(semi_white(0.45))
                    .draw();
                // 行滚动
                let top = panel.y + 0.13;
                let vh = panel_h - 0.17;
                ui.scope(|ui| {
                    ui.dx(panel.x + 0.03);
                    ui.dy(top);
                    self.user_list_scroll.size((panel_w - 0.06, vh));
                    self.user_list_scroll.render(ui, |ui| {
                        for (i, &id) in ids.iter().enumerate().take(max_rows) {
                            let rr = Rect::new(0., i as f32 * (row_h + 0.02), panel_w - 0.06, row_h);
                            let Some(user) = room.users.get(&id) else { continue };
                            let is_me = user.id == me.unwrap_or(i32::MIN);
                            let crown = is_me && room.is_host;
                            // 浮层行仅展示（管理请点击右侧列表行）
                            draw_player_row_bg(ui, rr);
                            draw_player_row(ui, rr, t, &icon, user.id, &user.name, is_me, crown, user.monitor, false, accent);
                        }
                        (panel_w - 0.06, (n.min(max_rows) as f32 * (row_h + 0.02)).max(0.))
                    });
                });
                if n > max_rows {
                    ui.text(mtl!("room-list-more"))
                        .pos(panel.x + 0.035, panel.bottom() - 0.06)
                        .size(0.3)
                        .color(semi_white(0.5))
                        .draw();
                }
            });
        });
    }

    /// 公共房间浮层（点击行快速加入，点击空白处关闭）。
    fn render_room_list_overlay(&mut self, ui: &mut Ui, t: f32) {
        let p = self.room_list_p.now(t);
        if p <= 1e-4 {
            return;
        }
        ui.abs_scope(|ui| {
            ui.alpha(p, |ui| {
                ui.fill_rect(ui.screen_rect(), semi_black(p * 0.5));
                let panel_w = 0.95;
                let max_rows = 10usize;
                let row_h = 0.15;
                let rooms = self.room_list.as_deref().unwrap_or(&[]);
                let n = rooms.len().min(max_rows);
                let panel_h = (0.42 + n as f32 * (row_h + 0.02)).min(ui.top * 2. - 0.1);
                let panel = Rect::new(-panel_w / 2., -panel_h / 2., panel_w, panel_h);
                ui.fill_path(&panel.rounded(0.018), semi_black(0.4));
                let left = panel.x + 0.035;
                ui.text(mtl!("room-list-title"))
                    .pos(left, panel.y + 0.03)
                    .size(0.48)
                    .color(semi_white(0.95))
                    .draw_using(&prpr::core::BOLD_FONT);
                self.room_rows_hits.clear();
                if self.room_list.is_none() && self.room_list_task.is_some() {
                    ui.text(mtl!("room-list-loading")).pos(left, panel.y + 0.16).size(0.38).color(semi_white(0.6)).draw();
                }
                let empty = rooms.is_empty() && self.room_list.is_some();
                if empty {
                    ui.text(mtl!("room-list-empty")).pos(left, panel.y + 0.16).size(0.38).color(semi_white(0.6)).draw();
                }
                let mut y = panel.y + 0.13;
                for room in rooms.iter().take(max_rows) {
                    let rr = Rect::new(panel.x + 0.03, y, panel_w - 0.06, row_h);
                    ui.fill_path(&rr.rounded(0.01), semi_black(0.22));
                    let label = format!("#{}  ·  {}  ·  {}", room.id, room.state, mtl!("mp-room-counts", "players" => room.player_count as u64, "spectators" => room.spectator_count as u64));
                    ui.text(&label)
                        .pos(rr.x + 0.03, rr.center().y)
                        .anchor(0., 0.5)
                        .size(0.38)
                        .max_width(rr.w - (if room.locked { 0.4 } else { 0.2 }))
                        .color(if room.locked { semi_white(0.55) } else { semi_white(0.95) })
                        .draw();
                    if room.locked {
                        let locked_tag = mtl!("mp-room-locked");
                        pill_text(
                            ui,
                            Rect::new(rr.right() - 0.2, rr.y + rr.h * 0.25, 0.17, rr.h * 0.5),
                            locked_tag.as_ref(),
                            0.3,
                            semi_white(0.08),
                            semi_white(0.6),
                        );
                    }
                    // 点击整行可加入
                    self.room_rows_hits.push((room.id.clone(), rr));
                    y += row_h + 0.02;
                }
                if rooms.len() > max_rows {
                    ui.text(mtl!("room-list-more")).pos(left, y + 0.01).size(0.3).color(semi_white(0.5)).draw();
                }
                // 底部提示
                ui.text(mtl!("room-list-tap-hint"))
                    .pos(panel.center().x, panel.bottom() - 0.035)
                    .anchor(0.5, 0.)
                    .size(0.3)
                    .color(semi_white(0.4))
                    .draw();
            });
        });
    }

    /// 对局结算排名浮层。
    fn render_results_overlay(&mut self, ui: &mut Ui, t: f32) {
        let p = self.results_p.now(t);
        if p <= 1e-4 {
            return;
        }
        let results = self.results.clone().unwrap_or_default();
        ui.abs_scope(|ui| {
            ui.alpha(p, |ui| {
                ui.fill_rect(ui.screen_rect(), semi_black(p * 0.55));
                let panel_w = 0.96;
                let row_h = 0.15;
                let n = results.len();
                let panel_h = (0.3 + n as f32 * (row_h + 0.02)).min(ui.top * 2. - 0.1);
                let panel = Rect::new(-panel_w / 2., -panel_h / 2., panel_w, panel_h);
                ui.fill_path(&panel.rounded(0.018), semi_black(0.42));
                ui.text(mtl!("results-title"))
                    .pos(panel.x + 0.04, panel.y + 0.035)
                    .size(0.52)
                    .color(WHITE)
                    .draw_using(&prpr::core::BOLD_FONT);
                let vh = panel_h - 0.16;
                ui.scope(|ui| {
                    ui.dx(panel.x + 0.04);
                    ui.dy(panel.y + 0.13);
                    self.results_scroll.size((panel_w - 0.08, vh));
                    self.results_scroll.render(ui, |ui| {
                        for (i, r) in results.iter().enumerate() {
                            let rr = Rect::new(0., i as f32 * (row_h + 0.02), panel_w - 0.08, row_h);
                            ui.fill_path(&rr.rounded(0.01), semi_black(0.22));
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
                                .size(0.4)
                                .color(if r.aborted { semi_white(0.5) } else { WHITE })
                                .draw();
                        }
                        (panel_w - 0.08, n as f32 * (row_h + 0.02))
                    });
                });
            });
        });
    }

    /// 房主对某玩家的操作菜单（设为房主 / 踢出 / 取消）。
    fn render_manage_overlay(&mut self, ui: &mut Ui, t: f32) {
        let p = self.manage_p.now(t);
        if p <= 1e-4 {
            return;
        }
        let Some(client) = self.client.clone() else {
            self.manage_p.goto(0., t, USER_LIST_TRANSIT);
            return;
        };
        // 目标消失 / 自己不再是房主 → 自动关闭
        let target_name = self.manage_target.and_then(|id| {
            client.blocking_state().and_then(|r| {
                if !r.is_host || !r.users.contains_key(&id) {
                    None
                } else {
                    r.users.get(&id).map(|u| u.name.clone())
                }
            })
        });
        if self.manage_target.is_some() && target_name.is_none() {
            self.manage_p.goto(0., t, USER_LIST_TRANSIT);
            return;
        }
        let accent = ui.accent();
        ui.abs_scope(|ui| {
            ui.alpha(p, |ui| {
                ui.fill_rect(ui.screen_rect(), semi_black(p * 0.5));
                let pw = 0.72;
                let btn_h = 0.13;
                let gap = 0.035;
                let ph = 0.2 + btn_h * 2. + gap + 0.09;
                let panel = Rect::new(-pw / 2., -ph / 2., pw, ph);
                ui.fill_path(&panel.rounded(0.018), semi_black(0.44));
                ui.text(mtl!("mp-manage-title", "name" => target_name.as_deref().unwrap_or("")))
                    .pos(panel.x + 0.04, panel.y + 0.035)
                    .size(0.44)
                    .max_width(pw - 0.08)
                    .color(semi_white(0.92))
                    .draw();
                let x = panel.x + 0.05;
                let w = pw - 0.1;
                let mut y = panel.y + 0.16;
                let tr = Rect::new(x, y, w, btn_h);
                self.transfer_host_btn.render_shadow(ui, tr, t, |ui, path| {
                    ui.fill_path(&path, accent);
                    ui.text(mtl!("mp-manage-transfer"))
                        .pos(tr.center().x, tr.center().y)
                        .anchor(0.5, 0.5)
                        .no_baseline()
                        .size(0.46)
                        .color(WHITE)
                        .draw();
                });
                y += btn_h + gap;
                let kr = Rect::new(x, y, w, btn_h);
                let danger = Color::from_rgba(200, 70, 70, 235);
                self.kick_user_btn.render_shadow(ui, kr, t, |ui, path| {
                    ui.fill_path(&path, danger);
                    ui.text(mtl!("mp-manage-kick"))
                        .pos(kr.center().x, kr.center().y)
                        .anchor(0.5, 0.5)
                        .no_baseline()
                        .size(0.46)
                        .color(WHITE)
                        .draw();
                });
                y += btn_h + gap;
                let cr = Rect::new(x, y, w, 0.09);
                self.manage_cancel_btn.render_shadow(ui, cr, t, |ui, path| {
                    ui.fill_path(&path, semi_black(0.3));
                    ui.text(mtl!("mp-manage-cancel"))
                        .pos(cr.center().x, cr.center().y)
                        .anchor(0.5, 0.5)
                        .no_baseline()
                        .size(0.4)
                        .color(semi_white(0.85))
                        .draw();
                });
            });
        });
    }

    #[inline]
    pub fn next_scene(&mut self) -> Option<NextScene> {
        self.next_scene.take()
    }
}
