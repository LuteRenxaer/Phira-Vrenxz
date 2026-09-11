//! 多人游戏面板（对外 API 薄壳）。
//!
//! 面板本身只负责三件事：
//! 1. 组织进出动画与整体布局层级；
//! 2. 把一次触摸/一帧更新分派给对应的子模块；
//! 3. 维护场景切换（`next_scene`）与跨模块的流程串联
//!    （下载→就绪/开始/预览/观战、开局→游玩场景、结算上报、深链接）。
//!
//! 各子模块职责：
//! - [`state`]：连接与房间状态、任务句柄（无 UI）
//! - [`actions`]：所有协议调用（连接/建房/进房/就绪/开始/管理/谱面/上报）
//! - [`messages`]：房间消息流
//! - [`lobby`]：未连接 / 未进房视图
//! - [`room`]：房内主体 + 底部操作条（渲染与触摸同一来源）
//! - [`overlays`]：玩家列表 / 公共房间 / 结算 / 房主菜单
//! - [`preview`]：谱面预览会话
//! - [`spectate`]：观战会话 + 观战浮层
//! - [`widgets`]：公共绘制与几何工具

mod actions;
mod lobby;
mod messages;
mod overlays;
mod preview;
mod room;
mod spectate;
mod state;
mod widgets;

use anyhow::{Context, Result};
use inputbox::InputBox;
use macroquad::prelude::*;
use phira_mp_common::{ClientRoomState, RoomId, RoomState};
use prpr::{
    config::Mods,
    core::Tweenable,
    ext::{poll_future, semi_black, semi_white, LocalTask, RectExt, SafeTexture},
    scene::{request_input, return_input, show_error, show_message, take_input, GameMode, NextScene},
    time::TimeManager,
    ui::Ui,
};

use self::{
    actions::DownloadStep,
    lobby::{LobbyAction, LobbyUi},
    messages::{MessageLog, RoomNotice},
    overlays::{OverlayAction, OverlayTouch, Overlays},
    preview::{Preview, PreviewReturn},
    room::{RoomTouch, RoomUi, RoomView},
    spectate::{Spectate, SpectateAction, SpectateTouch},
    state::{DownloadIntent, MpState, PublicRoom},
    widgets::{pill_text, screen_size, sorted_user_ids, ENTER_TRANSIT, PANEL_WIDTH},
};
use crate::{
    get_data,
    mp::L10N_LOCAL,
    scene::SongScene,
};

pub struct MPPanel {
    state: MpState,
    msgs: MessageLog,
    lobby: LobbyUi,
    room: RoomUi,
    overlays: Overlays,
    preview: Preview,
    spectate: Spectate,

    /// 面板进出的起始时刻（负数表示正在关闭，正数表示正在打开，∞ 表示已关闭）
    side_enter_time: f32,
    last_screen_size: (u32, u32),

    next_scene: Option<NextScene>,
    scene_task: LocalTask<Result<NextScene>>,
    icon_user: SafeTexture,
}

/// 房间主体需要的只读展示状态（把面板的多个字段汇总成一份）。
fn room_view<'a>(state: &'a MpState, spectate: &Spectate) -> RoomView<'a> {
    RoomView {
        spectating: spectate.is_spectating(),
        local_chart: state.local_chart.as_ref().map(|(uuid, name)| (uuid.as_str(), name.as_str())),
        local_ready: state.local_ready,
        host_started: state.host_started,
        pending_download: state.pending_download.is_some(),
        syncing: state.syncing.is_some(),
        chart_id: state.chart_id,
    }
}

/// 公共房间列表里“正在游戏”的房间：无法以玩家身份加入，应改为观战。
fn is_playing_room(room: &PublicRoom) -> bool {
    room.state.contains("游戏") || room.state.eq_ignore_ascii_case("playing")
}

impl MPPanel {
    pub fn new(icon_user: SafeTexture) -> Self {
        Self {
            state: MpState::new(),
            msgs: MessageLog::new(),
            lobby: LobbyUi::new(),
            room: RoomUi::new(),
            overlays: Overlays::new(),
            preview: Preview::new(),
            spectate: Spectate::new(),

            side_enter_time: f32::INFINITY,
            last_screen_size: screen_size(),

            next_scene: None,
            scene_task: None,
            icon_user,
        }
    }

    // ---------- 对外 API ----------

    #[inline]
    pub fn in_room(&self) -> bool {
        self.state.in_room()
    }

    #[inline]
    pub fn show(&mut self, rt: f32) {
        self.side_enter_time = rt;
    }

    /// 接收深链接（phira://）房间动作：打开面板、自动连接并加入/创建房间。
    pub fn set_deep_link(&mut self, link: crate::mp::PendingRoomLink) {
        self.state.deep_link_auto_connected = false;
        self.state.deep_link = Some(link);
    }

    /// 从谱面库选择在线谱面（房主）。
    pub fn select_chart(&mut self, id: i32) {
        self.state.select_chart(id);
    }

    /// 从谱面库选择本地谱面进行分享（房主）。
    pub fn select_local_chart(&mut self, local_path: String, name: String) {
        self.state.select_local_chart(local_path, name);
    }

    pub fn enter(&mut self) {
        self.state.entered = true;
        // 观战场景结束：停止后台喂数据；若观战者在暂停面板里点了「退出」→ 真正退出观战（离开房间）
        self.spectate.on_scene_return();
        if self.spectate.take_quit_request() {
            self.exit_spectate();
        }
        // 谱面预览场景结束（弹回 MainScene 时 enter 会被调用）：按房间状态决定是否询问准备
        let room_state = self.state.room_state();
        let is_ready = self.state.client.as_ref().and_then(|c| c.blocking_is_ready()).unwrap_or(false);
        match self.preview.on_return(room_state.as_ref(), is_ready, self.spectate.joined()) {
            PreviewReturn::Ignore => {}
            // 已经开局：给一条轻提示，不打断对局流程
            PreviewReturn::AlreadyStarted => {
                show_message(mtl!("preview-started-title")).warn();
            }
            PreviewReturn::AskReady => self.preview.ask_ready(),
        }
    }

    pub fn next_scene(&mut self) -> Option<NextScene> {
        self.next_scene.take()
    }

    #[inline]
    fn busy(&self) -> bool {
        self.state.busy() || self.scene_task.is_some()
    }

    // ---------- 深链接 ----------

    /// 已连上服务器时执行深链接动作（加入/创建房间），只执行一次。
    fn run_deep_link(&mut self) {
        let Some(link) = self.state.deep_link.take() else { return };
        if let Some(join) = link.join {
            match join.try_into() {
                Ok(id) => {
                    // 深链接为正常游玩进房：清理观战状态
                    self.spectate.cancel(0.);
                    self.state.join_room(id);
                }
                Err(_) => {
                    show_message(mtl!("join-room-invalid-id")).error();
                }
            }
            return;
        }
        if let Some(id) = link.create {
            match id.try_into() {
                Ok(room_id) => {
                    self.spectate.cancel(0.);
                    self.state.create_room(room_id);
                }
                Err(_) => {
                    show_message(mtl!("create-invalid-id")).error();
                }
            }
        }
    }

    // ---------- 触摸 ----------

    pub fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> bool {
        let t = tm.now() as f32;
        if self.side_enter_time.is_infinite() {
            return false;
        }
        // 浮层（最上层优先：房主菜单 > 玩家列表 > 公共房间 > 结算）
        let room = self.state.room();
        match self.overlays.touch(touch, t, room.as_ref()) {
            OverlayTouch::Consumed => return true,
            OverlayTouch::Action(action) => {
                self.apply_overlay_action(action, t);
                return true;
            }
            OverlayTouch::Pass => {}
        }
        // 观战浮层
        match self.spectate.touch(touch, t) {
            SpectateTouch::Consumed => return true,
            SpectateTouch::Action(action) => {
                self.apply_spectate_action(action);
                return true;
            }
            SpectateTouch::Pass => {}
        }
        // 面板打开动画期间吞掉触摸
        if !(self.side_enter_time > 0. && tm.real_time() as f32 > self.side_enter_time + ENTER_TRANSIT) {
            return true;
        }
        if self.busy() {
            return true;
        }
        // 谱面下载浮层的取消按钮
        if let Some(dl) = &mut self.state.download.ui {
            if dl.touch(touch, t) {
                self.state.download.ui = None;
                self.state.download.intent = None;
                return true;
            }
        }
        // 点面板外关闭：附带收起全部浮层，避免下次打开面板时浮层还停在打开态
        // （`close_all` 会清空 manage 目标；观战浮层单独收起）
        if touch.position.x + 1. > PANEL_WIDTH {
            self.overlays.close_all(t);
            self.spectate.close(t);
            self.side_enter_time = -tm.real_time() as f32;
            return true;
        }

        if self.state.client.is_none() {
            if self.lobby.touch_connect(touch, t) {
                self.state.connect();
                return true;
            }
            return true;
        }

        match &room {
            Some(room) => {
                self.apply_room_touch(touch, t, room);
            }
            None => {
                if let Some(action) = self.lobby.touch(touch, t) {
                    self.apply_lobby_action(action, t);
                }
            }
        }

        // 心跳连续失败：提示并自动重连一次
        self.state.reconnect_if_lost();
        true
    }

    fn apply_room_touch(&mut self, touch: &Touch, t: f32, room: &ClientRoomState) {
        let view = room_view(&self.state, &self.spectate);
        let touched = self.room.touch(touch, t, room, &view, &mut self.msgs);
        let Some(touched) = touched else { return };
        match touched {
            RoomTouch::Consumed => {}
            RoomTouch::ChatInput => request_input("chat", InputBox::new().default_text(&self.state.chat_text)),
            RoomTouch::ChatSend => {
                if self.state.chat_text.is_empty() {
                    show_message(mtl!("chat-empty")).error();
                } else {
                    let text = self.state.chat_text.clone();
                    self.state.send_chat(text);
                }
            }
            RoomTouch::Leave => self.leave_room(t),
            RoomTouch::OpenUserList => {
                self.overlays.open_user_list(t);
                let ids = sorted_user_ids(room, self.state.me_id());
                self.room.request_avatars(&ids);
            }
            RoomTouch::PlayerRow(index) => {
                let me = self.state.me_id();
                let ids = sorted_user_ids(room, me);
                if let Some(&id) = ids.get(index) {
                    if me != Some(id) {
                        self.overlays.open_manage(id, t);
                    }
                }
            }
            RoomTouch::Action(action) => self.apply_room_action(action, room, t),
        }
    }

    /// 底部操作条的动作分派：按钮集合由 `room::action_items` 唯一决定，
    /// 这里只按动作语义调用协议，不再重新判断房间状态。
    fn apply_room_action(&mut self, action: room::RoomAction, room: &ClientRoomState, t: f32) {
        use room::RoomAction as A;
        match action {
            A::Start => self.state.request_start(),
            A::LockRoom => self.state.lock_room(!room.locked),
            A::CycleRoom => self.state.cycle_room(!room.cycle),
            A::Password => request_input("set_pwd", InputBox::new()),
            A::Ready => match room.state {
                // 本地谱面分享：先标记已就绪，再开始下载
                RoomState::LocalChart => {
                    self.state.local_ready = true;
                    self.state.download_pending_local_chart();
                }
                _ => self.state.ready(),
            },
            A::CancelReady => self.state.cancel_ready(),
            A::CancelLocalShare => self.state.cancel_local_chart(),
            A::CancelDownload => self.state.cancel_download_ready(),
            A::Preview => self.start_preview(),
            A::Spectate => {
                self.spectate.open(t);
            }
        }
    }

    fn apply_lobby_action(&mut self, action: LobbyAction, t: f32) {
        match action {
            LobbyAction::CreateRoom => request_input("room_id", InputBox::new()),
            LobbyAction::JoinRoom => request_input("join_room", InputBox::new()),
            LobbyAction::OpenRoomList => {
                self.overlays.open_room_list(t);
                self.state.load_room_list();
            }
            LobbyAction::Disconnect => {
                self.state.disconnect();
                self.msgs.clear();
                self.room.reset_player_cache();
                self.overlays.close_all(t);
                self.spectate.cancel(t);
            }
        }
    }

    fn apply_overlay_action(&mut self, action: OverlayAction, t: f32) {
        match action {
            OverlayAction::JoinRoom(room_id) => {
                // 正在游戏中的房间无法以玩家身份加入：直接进入观战
                let playing = self
                    .state
                    .room_list
                    .as_deref()
                    .and_then(|rooms| rooms.iter().find(|r| r.id == room_id))
                    .is_some_and(is_playing_room);
                if playing {
                    self.join_as_spectator(&room_id);
                } else if let Ok(id) = room_id.clone().try_into() {
                    self.spectate.cancel(t);
                    self.state.join_room(id);
                } else {
                    show_message(mtl!("join-room-invalid-id")).error();
                }
            }
            OverlayAction::SpectateRoom(room_id) => self.join_as_spectator(&room_id),
            OverlayAction::TransferHost(id) => self.state.transfer_host(id),
            OverlayAction::KickUser(id) => self.state.kick_user(id),
        }
    }

    fn apply_spectate_action(&mut self, action: SpectateAction) {
        match action {
            SpectateAction::Watch => self.start_spectate_watch(),
            SpectateAction::Exit => self.exit_spectate(),
            SpectateAction::SelectTarget(id) => self.spectate.set_target(id),
        }
    }

    /// 以观战者身份（monitor）进入房间：只读旁观，不占玩家位、不参与就绪。
    fn join_as_spectator(&mut self, room_id: &str) {
        self.spectate.begin();
        if !self.state.join_room_as_spectator(room_id) {
            // 房间号非法：回滚观战意图
            self.spectate.cancel(0.);
        }
    }

    fn leave_room(&mut self, t: f32) {
        self.state.leave_room();
        // 离开房间同时结束观战状态（观战者离开即退出观战）
        self.spectate.cancel(t);
    }

    fn exit_spectate(&mut self) {
        self.spectate.finish(0.);
        self.state.leave_room();
    }

    // ---------- 预览 / 观战会话 ----------

    /// 预览当前房主选定的谱面（autoplay；结束时自动回到房间，不影响正式开局）。
    fn start_preview(&mut self) {
        let Some(client) = self.state.client() else { return };
        let Some(room) = client.blocking_state() else { return };
        // 在线谱 / 本地谱
        let (id, uuid, path) = match (&room.state, &self.state.local_chart) {
            (RoomState::SelectChart(Some(id)), _) => (Some(*id), None, Some(format!("download/{id}"))),
            (RoomState::LocalChart, Some((uuid, _))) => (None, Some(uuid.clone()), Some(format!("download/{uuid}"))),
            _ => (None, None, None),
        };
        let Some(path) = path else {
            show_message(mtl!("preview-unavailable")).error();
            return;
        };
        // 本地没有缓存：先下载（下载完成后自动进入预览）
        if !self.state.local_chart_ready(id, uuid.as_deref()) {
            let Some(chart_id) = id.or(self.state.chart_id) else {
                show_message(mtl!("preview-unavailable")).error();
                return;
            };
            self.state.chart_id = Some(chart_id);
            self.state.fetch_chart(chart_id, DownloadIntent::Preview);
            return;
        }
        if let Err(err) = self.launch_preview_scene(path, id) {
            show_error(err.context(mtl!("preview-failed")));
        }
    }

    fn launch_preview_scene(&mut self, path: String, id: Option<i32>) -> Result<()> {
        let client = self.state.client();
        self.scene_task = self.preview.begin(client, id, &path)?;
        Ok(())
    }

    /// 同步观战：加载对方正在游玩的谱面，并按对方视角同步播放。
    fn start_spectate_watch(&mut self) {
        let Some(client) = self.state.client() else { return };
        let Some(room) = client.blocking_state() else { return };
        // 观战对象：优先已选定的目标，否则取房间里第一个正在游玩的玩家
        let Some(target) = self.spectate.pick_target(&room) else {
            show_message(mtl!("spectate-none")).error();
            return;
        };
        self.spectate.set_target(target);
        // 在线谱：取当前选中/正在游玩的谱面 id
        let (id, path) = match room.state {
            RoomState::SelectChart(Some(id)) => (Some(id), format!("download/{id}")),
            _ => match self.state.chart_id {
                Some(id) => (Some(id), format!("download/{id}")),
                None => {
                    show_message(mtl!("spectate-no-chart")).error();
                    return;
                }
            },
        };
        // 本地没有缓存：先下载，完成后自动进入同步观战
        if !self.state.local_chart_ready(id, None) {
            let Some(chart_id) = id else {
                show_message(mtl!("spectate-no-chart")).error();
                return;
            };
            self.state.chart_id = Some(chart_id);
            self.state.fetch_chart(chart_id, DownloadIntent::Spectate);
            return;
        }
        if let Err(err) = self.launch_spectate_scene(path, id, target) {
            show_error(err.context(mtl!("preview-failed")));
        }
    }

    fn launch_spectate_scene(&mut self, path: String, id: Option<i32>, target: i32) -> Result<()> {
        let client = self.state.client();
        self.scene_task = self.spectate.begin_watch(client, id, &path, target)?;
        Ok(())
    }

    // ---------- 每帧更新 ----------

    pub fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = tm.now() as f32;
        if self.side_enter_time < 0. && -tm.real_time() as f32 + ENTER_TRANSIT < self.side_enter_time {
            self.side_enter_time = f32::INFINITY;
        }
        let new_size = screen_size();
        if self.last_screen_size != new_size {
            self.last_screen_size = new_size;
            self.msgs.invalidate_layout();
        }
        self.msgs.update(t);
        self.overlays.update(t);
        self.room.update(t, self.state.in_room());
        self.spectate.update(t);

        // —— 房间事件：结算 / 消息 / 房间阶段 ——
        if let Some(client) = self.state.client() {
            for results in client.blocking_take_room_results() {
                // 收到结算后自动弹出排名
                self.overlays.show_results(results, t);
            }
            let pending = client.blocking_take_messages();
            for notice in self.msgs.ingest(&client, pending) {
                // 观战：服务端在观战者加入时会补发当前谱面，记下来以便“同步观战”能加载它
                if let RoomNotice::OnlineChart { id, .. } = &notice {
                    if self.spectate.joined() {
                        self.state.chart_id = Some(*id);
                    }
                }
                self.spectate.notice(notice);
            }
            self.update_room_stage(client.blocking_room_state())?;
        }

        // —— 连接 ——
        if let Some(task) = &mut self.state.connect_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(client) => {
                        show_message(mtl!("server-welcome")).ok();
                        self.state.client = Some(client.into());
                    }
                    Err(err) => show_error(err.context(mtl!("connect-failed"))),
                }
                self.state.connect_task = None;
            }
        }
        // —— 建房 ——
        if let Some(task) = &mut self.state.create_room_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(_) => {
                        show_message(mtl!("create-room-success")).ok();
                    }
                    Err(err) => show_error(err.context(mtl!("create-room-failed"))),
                }
                self.state.create_room_task = None;
            }
        }
        // —— 谱面下载 ——
        match self.state.poll_download()? {
            DownloadStep::Pending | DownloadStep::Cancelled => {}
            DownloadStep::Continue(intent) => self.continue_download(intent)?,
        }
        // —— 聊天 ——
        if let Some(task) = &mut self.state.chat_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(_) => {
                        show_message(mtl!("chat-sent")).ok();
                        self.state.chat_text.clear();
                    }
                    Err(err) => show_error(err.context(mtl!("chat-send-failed"))),
                }
                self.state.chat_task = None;
            }
        }
        // —— 通用协议任务 ——
        if let Some(task) = &mut self.state.task {
            if let Some(res) = task.take() {
                if let Err(err) = res {
                    show_error(err);
                }
                self.state.task = None;
            }
        }
        // —— 预览被打断后弹窗点了「准备」：与在房间里点「准备」同一条路径 ——
        // 若房间已不在 WaitingForReady（如已进入 Playing/已就绪），不再执行 ready，交给正式开局逻辑。
        if self.preview.take_ready_request() {
            let waiting = matches!(self.state.room_state(), Some(RoomState::WaitingForReady))
                && !self.state.client.as_ref().is_some_and(|c| c.blocking_is_ready().unwrap_or(false));
            if waiting {
                self.state.ready();
            }
        }
        // —— 公共房间列表 ——
        if let Some(task) = &mut self.state.room_list_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(rooms) => self.state.room_list = Some(rooms),
                    Err(err) => {
                        show_error(err.context(mtl!("room-list-failed")));
                        self.state.room_list = Some(Vec::new());
                    }
                }
                self.state.room_list_task = None;
            }
        }
        // —— 进房 ——
        if let Some(task) = &mut self.state.join_room_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        // 若房间需要密码，请用户输入密码后重试（仅第一次失败时询问）
                        let need_pwd = self.state.join_pwd_pending.is_some() && {
                            let msg = format!("{err}");
                            msg.contains("密码") || msg.to_lowercase().contains("password")
                        };
                        if need_pwd {
                            let room_id = self.state.join_pwd_pending.take().unwrap();
                            self.state.join_pwd_pending = Some(room_id);
                            request_input("join_room_pwd", InputBox::new().title(mtl!("join-room-password-title")));
                        } else {
                            self.state.join_pwd_pending = None;
                            show_error(err.context(mtl!("join-room-failed")));
                        }
                    }
                    Ok(state) => {
                        self.state.join_pwd_pending = None;
                        self.state.chart_id = match state {
                            RoomState::SelectChart(id) => id,
                            _ => None,
                        };
                        // 观战意图：入房成功后自动打开观战浮层，直接看到实时进度
                        if self.spectate.joined() {
                            self.spectate.open(0.);
                            show_message(mtl!("spectate-joined")).ok();
                        }
                    }
                }
                self.state.join_room_task = None;
            }
        }
        // —— 输入框 ——
        if let Some((id, text)) = take_input() {
            self.handle_input(&id, text)?;
        }
        // —— 场景 ——
        if let Some(task) = &mut self.scene_task {
            if let Some(res) = poll_future(task.as_mut()) {
                match res {
                    Err(err) => {
                        show_error(err);
                        // 场景启动失败：停掉后台轮询并清理会话状态（避免残留任务/误判“从预览返回”）
                        self.preview.abort();
                        self.spectate.on_scene_return();
                    }
                    Ok(scene) => self.next_scene = Some(scene),
                }
                self.scene_task = None;
            }
        }
        // —— 本地谱面分享事件 ——
        if let Some(client) = self.state.client() {
            for ev in client.blocking_take_local_chart_events() {
                self.state.apply_local_chart_event(ev);
            }
        }
        // —— 观战：累计实时统计；已不在房间则自动结束观战状态 ——
        if self.spectate.joined() {
            let client = self.state.client();
            self.spectate.poll_room(client.as_deref(), t);
            if self.spectate.joined() {
                if let Some(client) = &client {
                    self.spectate.collect_stats(client);
                }
            }
        }
        // —— 本地谱面同步任务 ——
        if let Some(task) = &mut self.state.local_chart_task {
            if let Some(res) = task.take() {
                match res {
                    Ok(()) => self.state.syncing = None,
                    Err(err) => {
                        self.state.syncing = None;
                        self.state.host_started = false;
                        self.state.local_ready = false;
                        self.state.local_download_cancel = None;
                        show_error(err);
                    }
                }
                self.state.local_chart_task = None;
            }
        }
        if let Some(syncing) = &self.state.syncing {
            if let Some(err) = syncing.error() {
                self.state.syncing = None;
                let err_str = err.to_string();
                let args = prpr_l10n::fluent_args!["err" => err_str.as_str()];
                show_message(mtl!("mp-sync-failed", &args)).error();
            }
        }
        // —— 打完一局后上报真实成绩/放弃 ——
        if self.state.need_upload && self.state.entered {
            self.state.report_finish();
        }
        // —— 深链接自动流程 ——
        // 1) 未连接 → 自动连接一次（失败不重试，用户可手动重连后仍会执行 2)）；
        // 2) 已连接且未在房间 → 自动加入/创建房间（仅一次）。
        if self.state.deep_link.is_some() {
            if self.state.client.is_none() {
                if !self.state.deep_link_auto_connected && self.state.connect_task.is_none() {
                    self.state.deep_link_auto_connected = true;
                    if get_data().me.is_some() && get_data().tokens.is_some() {
                        self.state.connect();
                    }
                }
            } else if self.state.join_room_task.is_none() && self.state.create_room_task.is_none() && self.state.room_id().is_none() {
                self.run_deep_link();
            }
        }
        Ok(())
    }

    /// 按房间阶段推进：开局进入游玩场景、同步选谱 id、同步本地谱面分享状态。
    fn update_room_stage(&mut self, state: Option<RoomState>) -> Result<()> {
        if matches!(state, Some(RoomState::Playing)) {
            // 观战者只旁观：不进入游玩场景、不参与成绩上报
            if self.spectate.joined() {
                self.state.game_start_consumed = true;
                self.state.need_upload = false;
            } else if !self.state.game_start_consumed {
                self.state.begin_playing();
                self.launch_play_scene()?;
            }
        } else {
            self.state.game_start_consumed = false;
        }
        if let Some(RoomState::SelectChart(chart)) = state {
            self.state.chart_id = chart;
        }
        self.state.sync_local_chart_stage(state);
        Ok(())
    }

    /// 房间开局：以真实游玩模式进入谱面场景（本地谱面分享时从本地 uuid 加载）。
    fn launch_play_scene(&mut self) -> Result<()> {
        let client = self.state.client();
        if let Some((uuid, _)) = self.state.local_chart.clone() {
            self.scene_task = SongScene::global_launch(
                None,
                &format!("download/{uuid}"),
                Mods::default(),
                GameMode::NoRetry,
                client,
                None,
                None,
                false,
                false,
            )?;
            return Ok(());
        }
        let Some(id) = self.state.chart_id else { return Ok(()) };
        self.scene_task = SongScene::global_launch(
            Some(id),
            &format!("download/{id}"),
            Mods::default(),
            GameMode::NoRetry,
            client,
            None,
            None,
            false,
            false,
        )?;
        Ok(())
    }

    /// 谱面下载完成后按记录的去向继续（就绪 / 开始 / 预览 / 观战）。
    fn continue_download(&mut self, intent: DownloadIntent) -> Result<()> {
        match intent {
            DownloadIntent::RequestStart => self.state.start_game(),
            DownloadIntent::Ready => self.state.set_ready(),
            DownloadIntent::Preview => {
                let Some(id) = self.state.chart_id else {
                    show_message(mtl!("preview-unavailable")).error();
                    return Ok(());
                };
                let path = format!("download/{id}");
                if let Err(err) = self.launch_preview_scene(path, Some(id)) {
                    show_error(err.context(mtl!("preview-failed")));
                }
            }
            DownloadIntent::Spectate => {
                let Some(id) = self.state.chart_id else {
                    show_message(mtl!("spectate-no-chart")).error();
                    return Ok(());
                };
                let Some(target) = self.spectate.target() else {
                    show_message(mtl!("spectate-none")).error();
                    return Ok(());
                };
                let path = format!("download/{id}");
                if let Err(err) = self.launch_spectate_scene(path, Some(id), target) {
                    show_error(err.context(mtl!("preview-failed")));
                }
            }
        }
        Ok(())
    }

    /// 处理输入框返回（聊天 / 建房 / 进房 / 密码 / 踢人 / 移交）。
    fn handle_input(&mut self, id: &str, text: String) -> Result<()> {
        match id {
            "chat" => self.state.chat_text = text,
            "room_id" => {
                let room_id: RoomId = text.try_into().with_context(|| mtl!("create-invalid-id"))?;
                self.spectate.cancel(0.);
                self.state.create_room(room_id);
            }
            "join_room" => {
                if let Ok(id) = <RoomId as TryFrom<String>>::try_from(text.clone()) {
                    self.state.join_pwd_pending = Some(id.to_string());
                    self.state.join_room(id);
                } else {
                    show_message(mtl!("join-room-invalid-id")).error();
                }
            }
            // 加入失败且提示需要密码 → 请求输入密码后带密码重试（仅一次）
            "join_room_pwd" => {
                if let Some(room_id) = self.state.join_pwd_pending.take() {
                    if let Ok(id) = room_id.try_into() {
                        self.state.join_room_with_password(id, text.clone());
                    } else {
                        show_message(mtl!("join-room-invalid-id")).error();
                    }
                } else {
                    return_input(id.to_owned(), text);
                }
            }
            "set_pwd" => self.state.set_room_password(text.clone()),
            "kick_user" => {
                if let Ok(id) = text.trim().parse::<i32>() {
                    self.state.kick_user(id);
                } else {
                    show_message(mtl!("kick-user-invalid-id")).error();
                }
            }
            "transfer_host" => {
                if let Ok(id) = text.trim().parse::<i32>() {
                    self.state.transfer_host(id);
                } else {
                    show_message(mtl!("transfer-host-invalid-id")).error();
                }
            }
            _ => return_input(id.to_owned(), text),
        }
        Ok(())
    }

    // ---------- 渲染 ----------

    pub fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) {
        let rt = tm.real_time() as f32;
        let t = tm.now() as f32;
        if self.side_enter_time.is_finite() {
            let p = ((rt - self.side_enter_time.abs()) / ENTER_TRANSIT).min(1.);
            let p = 1. - (1. - p).powi(3);
            let p = if self.side_enter_time < 0. { 1. - p } else { p };
            ui.fill_rect(ui.screen_rect(), semi_black(p * 0.5));
            let w = PANEL_WIDTH;
            let rt = f32::tween(&-1., &(w - 1.), p);
            ui.scope(|ui| {
                ui.dx(rt - w);
                ui.dy(-ui.top);
                let h = ui.top * 2.;
                let r = Rect::new(0., 0., w, h).feather(-0.02);
                ui.fill_path(&r.rounded(0.015), semi_black(0.25));
                ui.fill_path(&r.rounded(0.015), (semi_white(0.05), (r.x, r.y), Color::default(), (r.right(), r.y)));
                self.render_main(tm, ui);
            });
        }
        if let Some(dl) = &mut self.state.download.ui {
            dl.render(ui, t);
        }
        if self.state.syncing.is_some() {
            ui.full_loading(mtl!("mp-syncing-chart"), t);
        } else if self.busy() {
            ui.full_loading_simple(t);
        }
    }

    fn render_main(&mut self, tm: &mut TimeManager, ui: &mut Ui) {
        let t = tm.now() as f32;
        // 每帧先使所有按钮失效；只有本帧绘制到的按钮会拥有命中区
        self.lobby.invalidate();
        self.room.invalidate();
        self.overlays.invalidate();
        self.spectate.invalidate();

        let client_room = self.state.room();
        let in_room = client_room.is_some();

        // —— 顶部标题栏 ——
        ui.text(mtl!("multiplayer"))
            .pos(0.05, 0.052)
            .size(0.58)
            .color(semi_white(0.95))
            .draw();
        if in_room {
            if let Some(rid) = self.state.room_id() {
                let tag = mtl!("mp-room-tag", "id" => rid.to_string());
                let tw = (ui.text(&tag).size(0.34).measure().w + 0.09).min(0.62);
                let tr = Rect::new(0.3, 0.052, tw, 0.075);
                pill_text(ui, tr, &tag, 0.34, semi_white(0.09), semi_white(0.85));
            }
            self.room.render_leave_button(ui, t);
        }

        if self.state.client.is_none() {
            self.lobby.render_connect(ui, t);
        } else if let Some(room) = client_room {
            let view = room_view(&self.state, &self.spectate);
            self.room.render(
                ui,
                t,
                &room,
                self.state.me_id(),
                &view,
                &mut self.msgs,
                &self.state.chat_text,
                &self.icon_user,
            );
        } else {
            self.lobby.render(ui, t);
        }

        // —— 居中浮层（渲染顺序与触摸优先级各自固定）——
        if let Some(client) = self.state.client() {
            if let Some(room) = self.state.room() {
                self.overlays.render_user_list(ui, t, &client, &room, &self.icon_user);
            }
        }
        self.overlays.render_room_list(
            ui,
            t,
            self.state.room_list.as_deref(),
            self.state.room_list_task.is_some(),
            self.state.room_id().map(|it| it.to_string()),
            self.spectate.joined(),
        );
        self.overlays.render_results(ui, t);
        if let Some(client) = self.state.client() {
            self.overlays.render_manage(ui, t, &client);
        }
        if let (Some(client), Some(room)) = (self.state.client(), self.state.room()) {
            self.spectate.render(ui, t, &client, &room);
        }
    }
}
