//! 多人会话：连接与房间状态 + 整屏分页导航 + 跨模块流程串联。
//!
//! 会话是**唯一持有状态的对象**，它活在 [`crate::mp::MP_SESSION`] 静态里；场景
//! （[`super::scene::MultiplayerScene`]）只是它的一层视图壳。因此：
//! - 进游玩 / 预览 / 观战子场景再返回时，房间与连接状态不会丢失；
//! - 退出多人场景回主菜单时（用 `NextScene::Pop`），会话仍然保留，
//!   下次进来直接回到「房间页」（或在谱面列表里选谱后自动回到多人场景）。
//!
//! 界面没有任何浮层：所有信息都在整屏页面里，页面之间用一次整页淡入切换；
//! 只有引擎的全局输入框（输入房间号 / 密码 / 聊天）与全局提示条（`show_message`）
//! 是游戏既有的公共系统。

use anyhow::{Context, Result};
use inputbox::InputBox;
use macroquad::prelude::*;
use phira_mp_common::{ClientRoomState, RoomId, RoomState};
use prpr::{
    config::Mods,
    ext::{poll_future, LocalTask, SafeTexture},
    scene::{request_input, return_input, show_error, show_message, take_input, GameMode, NextScene},
    time::TimeManager,
    ui::{Ui, UI_AUDIO},
};
use sasa::{AudioClip, Music};

use tracing::warn;

use super::{
    actions::DownloadStep,
    messages::{MessageLog, RoomNotice},
    page::{connect, lobby, players, results, room, spectate as spectate_page, Page, Pages},
    preview::{Preview, PreviewReturn},
    spectate::Spectate,
    state::{DownloadIntent, MpState, PublicRoom},
};
use crate::{
    client::UserManager,
    get_data,
    mp::L10N_LOCAL,
    page::SFader,
    scene::SongScene,
};

/// 整页切换的淡入时长。
const PAGE_TRANSIT: f32 = 0.22;

/// 多人场景的背景音乐音量（用户要求：10%）。
const BGM_VOLUME: f32 = 0.1;

pub struct MpSession {
    state: MpState,
    msgs: MessageLog,
    preview: Preview,
    spectate: Spectate,
    pages: Pages,

    /// 当前页面
    page: Page,
    /// 当前页面的进入时刻（整页淡入用）
    page_time: f32,

    background: SafeTexture,
    icon_user: SafeTexture,
    last_screen_size: (u32, u32),

    /// 多人场景的背景音乐（`bgm/mp_bgm.mp3`，进入场景时以 10% 音量播放）
    bgm_clip: Option<AudioClip>,
    bgm: Option<Music>,

    /// 场景整体淡入淡出（进入 / 退出 / 压栈子场景）
    sf: SFader,
    /// 待压栈的子场景（游玩 / 预览 / 观战）
    pending_scene: Option<NextScene>,
    scene_task: LocalTask<Result<NextScene>>,
}

/// 房间页需要的只读展示状态（把会话的多个字段汇总成一份拥有所有权的数据）。
fn room_view(state: &MpState, spectate: &Spectate) -> room::RoomView {
    room::RoomView {
        spectating: spectate.is_spectating(),
        local_chart: state.local_chart.clone(),
        local_ready: state.local_ready,
        host_started: state.host_started,
        pending_download: state.pending_download.is_some(),
        syncing: state.syncing.is_some(),
        chart_id: state.chart_id,
        chart_name: state.chart_name.clone(),
    }
}

/// 自己是否已就绪（协议只提供自己的就绪状态，别人不可观测）。
fn me_ready(state: &MpState, view: &room::RoomView) -> bool {
    match state.room_state() {
        Some(RoomState::WaitingForReady) => state.client.as_ref().and_then(|c| c.blocking_is_ready()).unwrap_or(false),
        Some(RoomState::LocalChart) => {
            if state.client.as_ref().and_then(|c| c.blocking_is_host()).unwrap_or(false) {
                view.host_started
            } else {
                view.local_ready
            }
        }
        _ => false,
    }
}

/// 公共房间列表里“正在游戏”的房间：无法以玩家身份加入，应改为观战。
fn is_playing_room(room: &PublicRoom) -> bool {
    room.state.contains("游戏") || room.state.eq_ignore_ascii_case("playing")
}

impl MpSession {
    /// `background` 为多人场景专用背景（`backgrounds/mp_bg.png`），
    /// `bgm_clip` 为多人场景专用 BGM（`bgm/mp_bgm.mp3`，已解码；可能加载失败）。
    pub fn new(icon_user: SafeTexture, background: SafeTexture, bgm_clip: Option<AudioClip>) -> Self {
        Self {
            state: MpState::new(),
            msgs: MessageLog::new(),
            preview: Preview::new(),
            spectate: Spectate::new(),
            pages: Pages::new(),

            page: Page::Connect,
            page_time: 0.,

            background,
            icon_user,
            last_screen_size: super::theme::screen_size(),

            bgm_clip,
            bgm: None,

            sf: SFader::new(),
            pending_scene: None,
            scene_task: None,
        }
    }

    /// 进入多人场景 / 从子场景（游玩、预览、观战）回来时播放背景音乐。
    pub fn play_bgm(&mut self) {
        if self.bgm.is_none() {
            let Some(clip) = self.bgm_clip.clone() else { return };
            let music = UI_AUDIO.with(|it| {
                it.borrow_mut().create_music(
                    clip,
                    sasa::MusicParams {
                        amplifier: BGM_VOLUME,
                        loop_mix_time: 2.0,
                        command_buffer_size: 64,
                        ..Default::default()
                    },
                )
            });
            match music {
                Ok(music) => self.bgm = Some(music),
                Err(err) => {
                    warn!("failed to create multiplayer bgm: {err}");
                    self.bgm_clip = None;
                    return;
                }
            }
        }
        if let Some(bgm) = &mut self.bgm {
            if let Err(err) = bgm.play() {
                warn!("failed to play multiplayer bgm: {err}");
            }
        }
    }

    /// 压栈子场景（游玩 / 预览 / 观战）或离开多人场景时停掉背景音乐。
    pub fn pause_bgm(&mut self) {
        if let Some(bgm) = &mut self.bgm {
            if let Err(err) = bgm.pause() {
                warn!("failed to pause multiplayer bgm: {err}");
            }
        }
    }

    // ---------- 对外 API ----------

    #[inline]
    pub fn in_room(&self) -> bool {
        self.state.in_room()
    }

    /// 接收深链接（phira://）房间动作：自动连接并加入/创建房间。
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

    /// 场景进入 / 子场景返回。
    pub fn enter(&mut self, t: f32) {
        self.state.entered = true;
        self.sf.enter(t);
        // 观战场景结束：停止后台喂数据；若观战者在暂停面板里点了「退出」→ 真正退出观战（离开房间）
        self.spectate.on_scene_return();
        if self.spectate.take_quit_request() {
            self.exit_spectate();
        }
        // 谱面预览场景结束（弹回本场景时 enter 会被调用）：按房间状态决定是否显示内联确认条
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
        self.sync_page(t);
    }

    /// 取出待执行的场景切换（由 [`super::scene::MultiplayerScene`] 转发给引擎）。
    pub fn next_scene(&mut self, t: f32) -> Option<NextScene> {
        self.sf.next_scene(t)
    }

    // ---------- 页面导航 ----------

    /// 当前状态下的「根页面」：未连接 → 连接页；已连接未进房 → 大厅页；在房内 → 房间页。
    fn root_page(&self) -> Page {
        if self.state.client.is_none() {
            Page::Connect
        } else if self.state.in_room() {
            Page::Room
        } else {
            Page::Lobby
        }
    }

    /// 页面与当前状态是否仍匹配；不匹配（离房 / 断开 / 被踢 / 结算数据没了）则回到根页面。
    fn sync_page(&mut self, t: f32) {
        let root = self.root_page();
        let valid = match self.page {
            Page::Connect => root == Page::Connect,
            Page::Lobby => root == Page::Lobby,
            Page::Room => root == Page::Room,
            Page::Manage(id) => self
                .state
                .room()
                .is_some_and(|r| r.is_host && r.users.contains_key(&id)),
            Page::Results => self.pages.results.has_data(),
            Page::Spectate => self.state.in_room(),
        };
        if !valid {
            self.goto(root, t);
        }
    }

    /// 切换页面（瞬时；视觉上由整页淡入完成过渡）。
    fn goto(&mut self, page: Page, t: f32) {
        if self.page == page {
            return;
        }
        if self.page == Page::Results {
            // 离开结算页后清掉上一局数据，避免下次进来看到旧排名
            self.pages.results.clear();
        }
        self.page = page;
        self.page_time = t;
    }

    /// 整页过渡进度（1 表示完全显示）。
    fn page_progress(&self, t: f32) -> f32 {
        if get_data().prefer_reduced_motion {
            return 1.;
        }
        ((t - self.page_time) / PAGE_TRANSIT).clamp(0., 1.)
    }

    /// 根页面的返回 = 退出多人场景回到主菜单（会话保留，连接与房间不丢）。
    fn exit_scene(&mut self, t: f32) {
        self.pending_scene = None;
        self.sf.next(t, NextScene::Pop);
    }

    /// 页面内返回。
    fn back(&mut self, t: f32) {
        match self.page {
            Page::Connect | Page::Lobby | Page::Room => self.exit_scene(t),
            Page::Manage(_) => self.goto(Page::Room, t),
            Page::Results | Page::Spectate => self.goto(self.root_page(), t),
        }
    }

    // ---------- 深链接 ----------

    /// 已连上服务器时执行深链接动作（加入/创建房间），只执行一次。
    fn run_deep_link(&mut self) {
        let Some(link) = self.state.deep_link.take() else { return };
        if let Some(join) = link.join {
            match join.try_into() {
                Ok(id) => {
                    // 深链接为正常游玩进房：清理观战状态
                    self.spectate.cancel();
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
                    self.spectate.cancel();
                    self.state.create_room(room_id);
                }
                Err(_) => {
                    show_message(mtl!("create-invalid-id")).error();
                }
            }
        }
    }

    // ---------- 触摸 ----------

    /// 处理一次触摸；多人场景是全屏场景，始终消费触摸。
    pub fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> bool {
        let t = tm.now() as f32;
        if self.sf.transiting() {
            return true;
        }
        match self.page {
            Page::Connect => {
                if let Some(a) = self.pages.connect.touch(touch, t) {
                    self.apply_connect(a, t);
                }
            }
            Page::Lobby => {
                if let Some(a) = self.pages.lobby.touch(touch, t) {
                    self.apply_lobby(a, t);
                }
            }
            Page::Room => {
                // 谱面下载状态行上的「取消」
                if let Some(dl) = &mut self.state.download.ui {
                    if dl.touch(touch, t) {
                        self.state.download.ui = None;
                        self.state.download.intent = None;
                        return true;
                    }
                }
                let room = self.state.room();
                if let Some(room) = room {
                    let view = room_view(&self.state, &self.spectate);
                    if let Some(a) = self.pages.room.touch(touch, t, &room, &view, &mut self.msgs) {
                        self.apply_room(a, t, &room);
                    }
                } else {
                    self.sync_page(t);
                }
            }
            Page::Manage(id) => {
                if let Some(a) = self.pages.manage.touch(touch, t) {
                    self.apply_manage(a, t, id);
                }
            }
            Page::Results => {
                if let Some(a) = self.pages.results.touch(touch, t) {
                    self.apply_results(a, t);
                }
            }
            Page::Spectate => {
                if let Some(a) = self.pages.spectate.touch(touch, t) {
                    self.apply_spectate(a, t);
                }
            }
        }
        // 心跳连续失败：提示并自动重连一次
        self.state.reconnect_if_lost();
        true
    }

    fn apply_connect(&mut self, a: connect::Action, t: f32) {
        match a {
            connect::Action::Connect => self.state.connect(),
            connect::Action::Back => self.back(t),
        }
    }

    fn apply_lobby(&mut self, a: lobby::Action, t: f32) {
        match a {
            lobby::Action::CreateRoom => request_input("room_id", InputBox::new()),
            lobby::Action::JoinRoom => request_input("join_room", InputBox::new()),
            lobby::Action::Refresh => self.state.load_room_list(),
            lobby::Action::Disconnect => {
                self.state.disconnect();
                self.msgs.clear();
                self.pages.results.clear();
                self.spectate.cancel();
                self.goto(Page::Connect, t);
            }
            lobby::Action::Back => self.back(t),
            lobby::Action::Join(room_id) => {
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
                    self.spectate.cancel();
                    self.state.join_room(id);
                } else {
                    show_message(mtl!("join-room-invalid-id")).error();
                }
            }
            lobby::Action::Spectate(room_id) => self.join_as_spectator(&room_id),
        }
    }

    fn apply_room(&mut self, a: room::Action, t: f32, room: &ClientRoomState) {
        match a {
            room::Action::Back => self.back(t),
            room::Action::Leave => {
                self.state.leave_room();
                // 离开房间同时结束观战状态（观战者离开即退出观战）
                self.spectate.cancel();
                self.goto(Page::Lobby, t);
            }
            room::Action::Manage(id) => self.goto(Page::Manage(id), t),
            room::Action::ChatInput => request_input("chat", InputBox::new().default_text(&self.state.chat_text)),
            room::Action::ChatSend => {
                if self.state.chat_text.is_empty() {
                    show_message(mtl!("chat-empty")).error();
                } else {
                    let text = self.state.chat_text.clone();
                    self.state.send_chat(text);
                }
            }
            // 内联确认条：与在房间里点「准备」同一条路径
            room::Action::Prompt(ready) => {
                self.preview.dismiss_prompt();
                if ready {
                    let waiting = matches!(self.state.room_state(), Some(RoomState::WaitingForReady))
                        && !self.state.client.as_ref().is_some_and(|c| c.blocking_is_ready().unwrap_or(false));
                    if waiting {
                        self.state.ready();
                    }
                }
            }
            room::Action::Room(action) => self.apply_room_action(action, room, t),
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
            A::Spectate => self.goto(Page::Spectate, t),
        }
    }

    fn apply_manage(&mut self, a: players::ManageAction, t: f32, id: i32) {
        match a {
            players::ManageAction::Back => self.back(t),
            players::ManageAction::Transfer => {
                self.state.transfer_host(id);
                self.goto(Page::Room, t);
            }
            players::ManageAction::Kick => {
                self.state.kick_user(id);
                self.goto(Page::Room, t);
            }
        }
    }

    fn apply_results(&mut self, a: results::Action, t: f32) {
        match a {
            results::Action::Back => self.back(t),
        }
    }

    fn apply_spectate(&mut self, a: spectate_page::Action, t: f32) {
        match a {
            spectate_page::Action::Back => self.back(t),
            spectate_page::Action::Watch => self.start_spectate_watch(),
            spectate_page::Action::Exit => {
                if self.spectate.joined() {
                    self.exit_spectate();
                } else {
                    // 普通玩家只是来查看观战页：退出观战页而不是离开房间
                    self.goto(self.root_page(), t);
                }
            }
            spectate_page::Action::Select(id) => self.spectate.set_target(id),
        }
    }

    /// 以观战者身份（monitor）进入房间：只读旁观，不占玩家位、不参与就绪。
    fn join_as_spectator(&mut self, room_id: &str) {
        self.spectate.begin();
        if !self.state.join_room_as_spectator(room_id) {
            // 房间号非法：回滚观战意图
            self.spectate.cancel();
        }
    }

    fn exit_spectate(&mut self) {
        self.spectate.finish();
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
        // 深链接（主菜单未取走时由这里兜底处理）
        if let Some(link) = crate::mp::take_pending_room_link() {
            self.set_deep_link(link);
        }
        let new_size = super::theme::screen_size();
        if self.last_screen_size != new_size {
            self.last_screen_size = new_size;
            self.msgs.invalidate_layout();
        }
        self.msgs.update(t);
        self.pages.update(t);

        // —— 房间事件：结算 / 消息 / 房间阶段 ——
        if let Some(client) = self.state.client() {
            for rt in client.blocking_take_room_results() {
                if !rt.is_empty() {
                    // 收到结算后自动切到结算页（带上本局谱面名，结算页概要用）
                    let chart = self
                        .state
                        .chart_name
                        .clone()
                        .or_else(|| self.state.local_chart.as_ref().map(|(_, name)| name.clone()));
                    self.pages.results.show(rt, chart);
                    self.goto(Page::Results, t);
                }
            }
            let pending = client.blocking_take_messages();
            for notice in self.msgs.ingest(&client, pending) {
                // 房间页左上角要显示"当前谱面"，名字只有服务端消息里带（房间状态里没有）
                match &notice {
                    RoomNotice::OnlineChart { id, name } => {
                        self.state.chart_name = Some(name.clone());
                        // 观战：服务端在观战者加入时会补发当前谱面，记下来以便"同步观战"能加载它
                        if self.spectate.joined() {
                            self.state.chart_id = Some(*id);
                        }
                    }
                    RoomNotice::LocalChart { name } => self.state.chart_name = Some(name.clone()),
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
                        // 观战意图：入房成功后进入观战页，直接看到实时进度
                        if self.spectate.joined() {
                            self.goto(Page::Spectate, t);
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
        // —— 子场景（游玩 / 预览 / 观战）——
        if let Some(task) = &mut self.scene_task {
            if let Some(res) = poll_future(task.as_mut()) {
                match res {
                    Err(err) => {
                        show_error(err);
                        // 场景启动失败：停掉后台轮询并清理会话状态（避免残留任务/误判“从预览返回”）
                        self.preview.abort();
                        self.spectate.on_scene_return();
                    }
                    Ok(scene) => self.pending_scene = Some(scene),
                }
                self.scene_task = None;
            }
        }
        if let Some(scene) = self.pending_scene.take() {
            if self.sf.transiting() {
                // 上一次场景过渡还没结束：等下一帧
                self.pending_scene = Some(scene);
            } else {
                self.sf.next(t, scene);
            }
        }
        // —— 本地谱面分享事件 ——
        if let Some(client) = self.state.client() {
            for ev in client.blocking_take_local_chart_events() {
                self.state.apply_local_chart_event(ev);
            }
        }
        // —— 观战：累计实时统计；已不在房间则自动结束观战状态 ——
        // 注意：以观战者身份（monitor）进房时，“已加入”的意图在进房请求返回之前就已置位，
        // 这期间连接上还没有房间号，不能据此判定“已离开房间”而把观战意图清掉
        // （否则观战者进房后不会被识别为观战者，房间页会错误地给出开始/准备等操作）。
        if self.spectate.joined() {
            let client = self.state.client();
            if self.state.join_room_task.is_none() {
                self.spectate.poll_room(client.as_deref());
            }
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

        self.sync_page(t);
        Ok(())
    }

    /// 按房间阶段推进：开局进入游玩场景、同步选谱 id、同步本地谱面分享状态。
    fn update_room_stage(&mut self, state: Option<RoomState>) -> Result<()> {
        if matches!(state, Some(RoomState::Playing)) {
            // 开局即作废"预览回来待确认"的提示，免得下一局又冒出来
            self.preview.dismiss_prompt();
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
                self.spectate.cancel();
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
        let t = tm.now() as f32;

        // 背景（与其余场景一致：原始比例铺满）
        set_camera(&ui.bg_camera());
        let r = ui.screen_rect();
        ui.fill_rect(r, (*self.background, r));
        set_camera(&ui.camera());

        // 每帧先使所有按钮失效；只有本帧绘制到的按钮会拥有命中区
        self.pages.invalidate();

        let p = self.page_progress(t);
        ui.alpha(p, |ui| {
            ui.dy((1. - p) * 0.02);
            match self.page {
                Page::Connect => {
                    let v = connect::View {
                        connecting: self.state.connect_task.is_some(),
                        address: &get_data().config.mp_address,
                    };
                    self.pages.connect.render(ui, t, &v);
                }
                Page::Lobby => {
                    // 主页就是房间大厅：第一次进来（还没拿到过列表）自动拉一次
                    if self.state.room_list.is_none() && self.state.room_list_task.is_none() {
                        self.state.load_room_list();
                    }
                    let joined = self.state.room_id().map(|it| it.to_string());
                    let v = lobby::View {
                        address: &get_data().config.mp_address,
                        rooms: self.state.room_list.as_deref(),
                        loading: self.state.room_list_task.is_some(),
                        joined: joined.as_deref(),
                        spectating: self.spectate.joined(),
                        busy: self.state.create_room_task.is_some() || self.state.room_list_task.is_some(),
                    };
                    self.pages.lobby.render(ui, t, &v);
                }
                Page::Room => self.render_room(ui, t),
                Page::Manage(id) => self.render_manage(ui, t, id),
                Page::Results => self.pages.results.render(ui, t, self.state.me_id()),
                Page::Spectate => self.render_spectate(ui, t),
            }
        });

        // 场景整体淡入/淡出（进入、退出、压栈子场景），代替原来的面板进出动画
        self.sf.render(ui, t);
    }

    fn render_room(&mut self, ui: &mut Ui, t: f32) {
        let Some(room) = self.state.room() else { return };
        let view = room_view(&self.state, &self.spectate);
        // 房间号（房名）：`ClientRoomState` 带房间号，但深链接/大厅进入时更可靠的是连接上的
        let room_id = self.state.room_id().map(|it| it.to_string()).or_else(|| Some(room.id.to_string()));
        let syncing = self.state.syncing.is_some();
        let busy = self.state.busy();
        let me = self.state.me_id();
        let me_ready = me_ready(&self.state, &view);
        let prompt = self.preview.prompt(self.state.room_state().as_ref(), me_ready);
        let chat_text = self.state.chat_text.clone();
        let download = self.state.download.ui.as_mut();
        // 右侧用户列表直接显示头像，先为每个可见用户请求一次
        for id in super::state::sorted_user_ids(&room, me) {
            UserManager::request(id);
        }
        let ctx = room::Render {
            room: &room,
            room_id: room_id.as_deref(),
            view: &view,
            messages: &mut self.msgs,
            chat_text: &chat_text,
            download,
            syncing,
            busy,
            prompt,
            icon: &self.icon_user,
            me,
            me_ready,
        };
        self.pages.room.render(ui, t, ctx);
    }

    fn render_manage(&mut self, ui: &mut Ui, t: f32, id: i32) {
        let Some(room) = self.state.room() else { return };
        let Some(user) = room.users.get(&id) else { return };
        let name = user.name.clone();
        let v = players::ManageView {
            id,
            name: &name,
            icon: &self.icon_user,
        };
        UserManager::request(id);
        self.pages.manage.render(ui, t, &v);
    }

    fn render_spectate(&mut self, ui: &mut Ui, t: f32) {
        let Some(room) = self.state.room() else { return };
        let Some(client) = self.state.client() else { return };
        let me = self.state.me_id();
        let players: Vec<(i32, String)> = super::state::player_ids(&room, me)
            .into_iter()
            .map(|id| (id, client.user_name(id)))
            .collect();
        let chart_name = self.spectate.chart_name().map(|it| it.to_owned());
        let room_id = self.state.room_id().map(|it| it.to_string());
        let playing = matches!(self.state.room_state(), Some(RoomState::Playing));
        // 观战榜也要显示头像
        for (id, _) in &players {
            UserManager::request(*id);
        }
        let v = spectate_page::View {
            room_id: room_id.as_deref(),
            chart_name: chart_name.as_deref(),
            target: self.spectate.target(),
            playing,
            players: &players,
            stats: self.spectate.stats(),
            icon: &self.icon_user,
        };
        self.pages.spectate.render(ui, t, &v);
    }
}
