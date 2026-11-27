
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
    page::{connect, lobby, players, room, Page, Pages},
    preview::{Preview, PreviewReturn},
    state::{DownloadIntent, MpState},
};
use crate::{
    client::UserManager,
    get_data,
    mp::L10N_LOCAL,
    page::SFader,
    scene::SongScene,
};

const PAGE_TRANSIT: f32 = 0.22;

#[inline]
pub fn scene_time(tm: &TimeManager) -> f32 {
    tm.real_time() as f32
}

const BGM_VOLUME: f32 = 0.1;

pub struct MpSession {
    state: MpState,
    msgs: MessageLog,
    /// 系统日志（与聊天分开一条流）
    log: MessageLog,
    preview: Preview,
    pages: Pages,

    page: Page,

    page_time: f32,

    background: SafeTexture,
    icon_user: SafeTexture,
    last_screen_size: (u32, u32),

    bgm_clip: Option<AudioClip>,
    bgm: Option<Music>,

    dev_spec: Option<String>,
    dev_connected: bool,
    dev_room_started: bool,
    dev_chart_done: bool,

    sf: SFader,

    pending_scene: Option<NextScene>,
    scene_task: LocalTask<Result<NextScene>>,
}

fn room_view(state: &MpState) -> room::RoomView {
    room::RoomView {
        local_chart: state.local_chart.clone(),
        local_ready: state.local_ready,
        host_started: state.host_started,
        pending_download: state.pending_download.is_some(),
        syncing: state.syncing.is_some(),
        chart_id: state.chart_id,
        chart_name: state.chart_name.clone(),
    }
}

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

impl MpSession {

    pub fn new(icon_user: SafeTexture, background: SafeTexture, bgm_clip: Option<AudioClip>) -> Self {
        Self {
            state: MpState::new(),
            msgs: MessageLog::new(),
        log: MessageLog::new(),
            preview: Preview::new(),
            pages: Pages::new(),

            page: Page::Connect,
            page_time: 0.,

            background,
            icon_user,
            last_screen_size: super::theme::screen_size(),

            bgm_clip,
            bgm: None,

            dev_spec: std::env::var("PHIRA_MP_DEV").ok(),
            dev_connected: false,
            dev_room_started: false,
            dev_chart_done: false,

            sf: SFader::new(),
            pending_scene: None,
            scene_task: None,
        }
    }

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

    pub fn pause_bgm(&mut self) {
        if let Some(bgm) = &mut self.bgm {
            if let Err(err) = bgm.pause() {
                warn!("failed to pause multiplayer bgm: {err}");
            }
        }
    }

    #[inline]
    pub fn in_room(&self) -> bool {
        self.state.in_room()
    }

    pub fn set_deep_link(&mut self, link: crate::mp::PendingRoomLink) {
        self.state.deep_link_auto_connected = false;
        self.state.deep_link = Some(link);
    }

    pub fn select_chart(&mut self, id: i32) {
        self.state.select_chart(id);
    }

    pub fn select_local_chart(&mut self, local_path: String, name: String) {
        self.state.select_local_chart(local_path, name);
    }

    pub fn enter(&mut self, t: f32) {
        self.state.entered = true;
        self.sf.enter(t);

        let room_state = self.state.room_state();
        let is_ready = self.state.client.as_ref().and_then(|c| c.blocking_is_ready()).unwrap_or(false);
        match self.preview.on_return(room_state.as_ref(), is_ready) {
            PreviewReturn::Ignore => {}

            PreviewReturn::AlreadyStarted => {
                show_message(mtl!("preview-started-title")).warn();
            }
            PreviewReturn::AskReady => self.preview.ask_ready(),
        }
        self.sync_page(t);
    }

    pub fn next_scene(&mut self, t: f32) -> Option<NextScene> {
        self.sf.next_scene(t)
    }

    fn root_page(&self) -> Page {
        if self.state.client.is_none() {
            Page::Connect
        } else if self.state.in_room() {
            Page::Room
        } else {
            Page::Lobby
        }
    }

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
        };
        if !valid {
            self.goto(root, t);
        }
    }

    fn goto(&mut self, page: Page, t: f32) {
        if self.page == page {
            return;
        }
        self.page = page;
        self.page_time = t;
    }

    fn page_progress(&self, t: f32) -> f32 {
        if get_data().prefer_reduced_motion {
            return 1.;
        }
        ((t - self.page_time) / PAGE_TRANSIT).clamp(0., 1.)
    }

    fn exit_scene(&mut self, t: f32) {
        self.pending_scene = None;
        self.sf.next(t, NextScene::Pop);
    }

    fn back(&mut self, t: f32) {
        match self.page {
            Page::Connect | Page::Lobby | Page::Room => self.exit_scene(t),
            Page::Manage(_) => self.goto(Page::Room, t),
        }
    }

    fn run_deep_link(&mut self) {
        let Some(link) = self.state.deep_link.take() else { return };
        if let Some(join) = link.join {
            match join.try_into() {
                Ok(id) => self.state.join_room(id),
                Err(_) => {
                    show_message(mtl!("join-room-invalid-id")).error();
                }
            }
            return;
        }
        if let Some(id) = link.create {
            match id.try_into() {
                Ok(room_id) => self.state.create_room(room_id),
                Err(_) => {
                    show_message(mtl!("create-invalid-id")).error();
                }
            }
        }
    }

    pub fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> bool {
        let t = scene_time(tm);
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

                if let Some(dl) = &mut self.state.download.ui {
                    if dl.touch(touch, t) {
                        self.state.download.ui = None;
                        self.state.download.intent = None;
                        return true;
                    }
                }
                let room = self.state.room();
                if let Some(room) = room {
                    let view = room_view(&self.state);
                    if let Some(a) = self.pages.room.touch(touch, t, &room, &view, &mut self.msgs, &mut self.log) {
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
        }

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
                self.log.clear();
                self.goto(Page::Connect, t);
            }
            lobby::Action::Back => self.back(t),
            lobby::Action::Join(room_id) => {

                if let Ok(id) = room_id.clone().try_into() {
                    self.state.join_room(id);
                } else {
                    show_message(mtl!("join-room-invalid-id")).error();
                }
            }
        }
    }

    fn apply_room(&mut self, a: room::Action, t: f32, room: &ClientRoomState) {
        match a {

            room::Action::Back => {
                self.pages.room.chat_focus = false;
                self.back(t)
            }
            room::Action::Manage(id) => {
                self.pages.room.chat_focus = false;
                self.goto(Page::Manage(id), t)
            }
            room::Action::ChatInput => self.pages.room.chat_focus = true,
            room::Action::ChatSend => {
                self.pages.room.chat_focus = false;
                if self.state.chat_text.is_empty() {
                    show_message(mtl!("chat-empty")).error();
                } else {
                    let text = self.state.chat_text.clone();
                    self.state.send_chat(text);
                }
            }

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

    fn apply_room_action(&mut self, action: room::RoomAction, room: &ClientRoomState, t: f32) {
        use room::RoomAction as A;
        match action {
            A::Start => {
                self.pages.room.chat_focus = false;
                self.state.request_start()
            }
            A::LockRoom => self.state.lock_room(!room.locked),
            A::CycleRoom => self.state.cycle_room(!room.cycle),
            A::Password => request_input("set_pwd", InputBox::new()),
            A::Ready => match room.state {

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

            A::Library => {
                crate::mp::request_open_library();
                self.exit_scene(t);
            }
            A::LeaveRoom => {
                self.state.leave_room();
                self.goto(Page::Lobby, t);
            }
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

    fn start_preview(&mut self) {
        let Some(client) = self.state.client() else { return };
        let Some(room) = client.blocking_state() else { return };

        let (id, uuid, path) = match (&room.state, &self.state.local_chart) {
            (RoomState::SelectChart(Some(id)), _) => (Some(*id), None, Some(format!("download/{id}"))),
            (RoomState::LocalChart, Some((uuid, _))) => (None, Some(uuid.clone()), Some(format!("download/{uuid}"))),
            _ => (None, None, None),
        };
        let Some(path) = path else {
            show_message(mtl!("preview-unavailable")).error();
            return;
        };

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

    fn dev_tick(&mut self) {
        let Some(spec) = self.dev_spec.clone() else { return };
        if self.state.client.is_none() {
            if self.state.connect_task.is_none() && !self.dev_connected {
                self.dev_connected = true;
                self.state.connect();
            }
            return;
        }
        if let Some(rest) = spec.strip_prefix("room:") {

            let mut it = rest.split(':');
            let room_id = it.next().unwrap_or("12345");
            let chart = match (it.next(), it.next()) {
                (Some("chart"), Some(id)) => id.parse::<i32>().ok(),
                _ => None,
            };

            if !self.state.in_room() && self.state.create_room_task.is_none() && self.state.join_room_task.is_none() && !self.dev_room_started {
                self.dev_room_started = true;
                if let Ok(room_id) = room_id.to_owned().try_into() {
                    self.state.create_room(room_id);
                }
            } else if self.state.in_room() && !self.dev_chart_done {
                self.dev_chart_done = true;
                if let Some(id) = chart {
                    self.select_chart(id);
                }
            }
        } else if self.state.in_room() && !self.dev_room_started {

            self.dev_room_started = true;
            self.state.leave_room();
        }
    }

    pub fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = scene_time(tm);
        self.dev_tick();

        if let Some(link) = crate::mp::take_pending_room_link() {
            self.set_deep_link(link);
        }
        let new_size = super::theme::screen_size();
        if self.last_screen_size != new_size {
            self.last_screen_size = new_size;
            self.msgs.invalidate_layout();
            self.log.invalidate_layout();
        }
        self.msgs.update(t);
        self.log.update(t);
        self.pages.update(t);

        // 聊天框在编辑态时：回车 = 发送，系统输入点已拿走键盘，这里直接问系统键状态
        #[cfg(target_os = "windows")]
        if self.page == Page::Room && self.pages.room.chat_focus && crate::native_edit::enter_pressed() {
            self.pages.room.chat_focus = false;
            if self.state.chat_text.is_empty() {
                show_message(mtl!("chat-empty")).error();
            } else {
                let text = self.state.chat_text.clone();
                self.state.send_chat(text);
            }
        }

        if let Some(client) = self.state.client() {
            let pending = client.blocking_take_messages();
            // 聊天和服务端日志分两条流：房间页中间是「聊天栏」，下面是「日志」。
            let mut chats = Vec::new();
            let mut systems = Vec::new();
            for msg in pending {
                match msg {
                    phira_mp_common::Message::Chat { .. } => chats.push(msg),
                    _ => systems.push(msg),
                }
            }
            for notice in self.log.ingest(&client, systems) {

                match &notice {
                    RoomNotice::OnlineChart { name } => self.state.chart_name = Some(name.clone()),
                    RoomNotice::LocalChart { name } => self.state.chart_name = Some(name.clone()),
                }
            }
            self.msgs.ingest(&client, chats);
            self.update_room_stage(client.blocking_room_state())?;
        }

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

        match self.state.poll_download()? {
            DownloadStep::Pending | DownloadStep::Cancelled => {}
            DownloadStep::Continue(intent) => self.continue_download(intent)?,
        }

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

        if let Some(task) = &mut self.state.task {
            if let Some(res) = task.take() {
                if let Err(err) = res {
                    show_error(err);
                }
                self.state.task = None;
            }
        }

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

        if let Some(task) = &mut self.state.join_room_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {

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
                    }
                }
                self.state.join_room_task = None;
            }
        }

        if let Some((id, text)) = take_input() {
            self.handle_input(&id, text)?;
        }

        if let Some(task) = &mut self.scene_task {
            if let Some(res) = poll_future(task.as_mut()) {
                match res {
                    Err(err) => {
                        show_error(err);

                        self.preview.abort();
                    }
                    Ok(scene) => self.pending_scene = Some(scene),
                }
                self.scene_task = None;
            }
        }
        if let Some(scene) = self.pending_scene.take() {
            if self.sf.transiting() {

                self.pending_scene = Some(scene);
            } else {
                self.sf.next(t, scene);
            }
        }

        if let Some(client) = self.state.client() {
            for ev in client.blocking_take_local_chart_events() {
                self.state.apply_local_chart_event(ev);
            }
        }

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

        if self.state.need_upload && self.state.entered {
            self.state.report_finish();
        }

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

    fn update_room_stage(&mut self, state: Option<RoomState>) -> Result<()> {
        if matches!(state, Some(RoomState::Playing)) {

            self.preview.dismiss_prompt();
            if !self.state.game_start_consumed {
                self.state.begin_playing();
                self.launch_play_scene()?;
            }
        } else {
            self.state.game_start_consumed = false;
        }
        if let Some(RoomState::SelectChart(chart)) = state {
            self.state.chart_id = chart;

            if let Some(id) = chart {
                self.state.prefetch_chart(id);
            }
        }
        self.state.sync_local_chart_stage(state);
        Ok(())
    }

    fn launch_play_scene(&mut self) -> Result<()> {
        let client = self.state.client();
        if let Some((uuid, _)) = self.state.local_chart.clone() {
            self.scene_task = SongScene::global_launch(
                None,
                &format!("download/{uuid}"),
                Mods::default(),
                get_data().config.hp, // [HP 血量模式] 多人无谱面级 HP 面板，沿用全局档位
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
            get_data().config.hp, // [HP 血量模式] 多人无谱面级 HP 面板，沿用全局档位
            GameMode::NoRetry,
            client,
            None,
            None,
            false,
            false,
        )?;
        Ok(())
    }

    fn continue_download(&mut self, intent: DownloadIntent) -> Result<()> {
        match intent {

            DownloadIntent::Cache => {}
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
        }
        Ok(())
    }

    fn handle_input(&mut self, id: &str, text: String) -> Result<()> {
        match id {
            "chat" => self.state.chat_text = text,
            "room_id" => {
                let room_id: RoomId = text.try_into().with_context(|| mtl!("create-invalid-id"))?;
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

    pub fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) {
        let t = scene_time(tm);

        set_camera(&ui.bg_camera());
        let r = ui.screen_rect();
        ui.fill_rect(r, (*self.background, r));
        set_camera(&ui.camera());

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

                    if self.state.room_list.is_none() && self.state.room_list_task.is_none() {
                        self.state.load_room_list();
                    }
                    let v = lobby::View {
                        address: &get_data().config.mp_address,
                        rooms: self.state.room_list.as_deref(),
                        loading: self.state.room_list_task.is_some(),
                        busy: self.state.create_room_task.is_some() || self.state.room_list_task.is_some(),
                    };
                    self.pages.lobby.render(ui, t, &v);
                }
                Page::Room => self.render_room(ui, t),
                Page::Manage(id) => self.render_manage(ui, t, id),
            }
        });

        self.sf.render(ui, t);
    }

    fn render_room(&mut self, ui: &mut Ui, t: f32) {
        let Some(room) = self.state.room() else { return };
        let view = room_view(&self.state);

        let room_id = self.state.room_id().map(|it| it.to_string()).or_else(|| Some(room.id.to_string()));
        let syncing = self.state.syncing.is_some();
        let busy = self.state.busy();
        let me = self.state.me_id();
        let me_ready = me_ready(&self.state, &view);
        let prompt = self.preview.prompt(self.state.room_state().as_ref(), me_ready);
        let mut chat_text = std::mem::take(&mut self.state.chat_text);
        let download = self.state.download.ui.as_mut();

        for id in super::state::sorted_user_ids(&room, me) {
            UserManager::request(id);
        }
        #[cfg(target_os = "windows")]
        crate::native_edit::begin_frame();
        let ctx = room::Render {
            room: &room,
            room_id: room_id.as_deref(),
            view: &view,
            messages: &mut self.msgs,
            log: &mut self.log,
            chat_text: &mut chat_text,
            download,
            syncing,
            busy,
            prompt,
            icon: &self.icon_user,
            me,
            me_ready,
        };
        self.pages.room.render(ui, t, ctx);
        #[cfg(target_os = "windows")]
        crate::native_edit::end_frame();
        self.state.chat_text = chat_text;
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

}

