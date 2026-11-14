
use std::{
    path::Path,
    sync::{atomic::AtomicBool, Arc},
};

use phira_mp_client::Client;
use phira_mp_common::{ClientRoomState, RoomId, RoomState};
use prpr::task::Task;

use crate::{client::Chart, dir, get_data, scene::Downloading};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PublicRoom {
    pub id: String,
    #[serde(rename = "player_count")]
    pub player_count: usize,
    pub state: String,
    pub locked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadIntent {

    Ready,

    RequestStart,

    Preview,

    Cache,
}

#[derive(Default)]
pub struct ChartDownload {

    pub task: Option<Task<anyhow::Result<Arc<Chart>>>>,

    pub ui: Option<Downloading>,

    pub intent: Option<DownloadIntent>,

    pub chart_id: i32,
}

pub struct MpState {
    pub client: Option<Arc<Client>>,

    pub connect_task: Option<Task<anyhow::Result<Client>>>,
    pub create_room_task: Option<Task<anyhow::Result<()>>>,
    pub join_room_task: Option<Task<anyhow::Result<RoomState>>>,
    pub room_list_task: Option<Task<anyhow::Result<Vec<PublicRoom>>>>,
    pub room_list: Option<Vec<PublicRoom>>,

    pub join_pwd_pending: Option<String>,

    pub task: Option<Task<anyhow::Result<()>>>,
    pub chat_task: Option<Task<anyhow::Result<()>>>,
    pub chat_text: String,

    pub chart_id: Option<i32>,

    pub chart_name: Option<String>,
    pub download: ChartDownload,

    pub prefetch_done: Option<i32>,

    pub local_chart: Option<(String, String)>,

    pub syncing: Option<Arc<crate::mp::serve::ChartSyncing>>,

    pub pending_download: Option<(String, String)>,
    pub local_chart_task: Option<Task<anyhow::Result<()>>>,

    pub host_started: bool,

    pub local_ready: bool,

    pub local_download_cancel: Option<Arc<AtomicBool>>,

    pub game_start_consumed: bool,

    pub need_upload: bool,

    pub entered: bool,

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
            chart_name: None,
            download: ChartDownload::default(),
            prefetch_done: None,
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

    #[inline]
    pub fn client(&self) -> Option<Arc<Client>> {
        self.client.clone()
    }

    #[inline]
    pub fn connected(&self) -> Arc<Client> {
        Arc::clone(self.client.as_ref().expect("multiplayer client not connected"))
    }

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

    pub fn busy(&self) -> bool {
        self.connect_task.is_some()
            || self.create_room_task.is_some()
            || self.chat_task.is_some()
            || self.download.task.is_some()
            || self.local_chart_task.is_some()
            || self.task.is_some()
    }

    pub fn local_chart_ready(&self, id: Option<i32>, uuid: Option<&str>) -> bool {
        let id = match (id, uuid) {
            (Some(id), _) => id.to_string(),
            (None, Some(uuid)) => uuid.to_string(),
            _ => return false,
        };
        Path::new(&format!("{}/download/{id}/info.yml", dir::charts().ok().unwrap_or_default())).exists()
    }

    pub fn server_allows_local_chart() -> bool {
        true
    }

    pub fn web_base() -> Option<String> {
        let addr = get_data().config.mp_address.clone();
        let (host, port) = addr.rsplit_once(':')?;
        let port: u16 = port.parse().ok()?;
        let host = host.trim_end_matches('.').to_owned();
        Some(format!("http://{host}:{}", port + 1))
    }

    pub fn reset_local_chart(&mut self) {
        self.host_started = false;
        self.local_ready = false;
        self.pending_download = None;
        self.syncing = None;
        if let Some(cancel) = self.local_download_cancel.take() {
            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }

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
        self.chart_name = None;
        self.download = ChartDownload::default();
        self.prefetch_done = None;
        self.reset_local_chart();
        self.local_chart = None;
        self.game_start_consumed = false;
        self.need_upload = false;
        self.entered = false;
    }
}

pub const RECORDER_BOT_USER_ID: i32 = -999;

pub const RECORDER_BOT_USER_NAME: &str = "回放录制器";

pub fn is_recorder(id: i32, name: &str) -> bool {
    id == RECORDER_BOT_USER_ID || name == RECORDER_BOT_USER_NAME
}

pub fn sorted_user_ids(room: &ClientRoomState, me: Option<i32>) -> Vec<i32> {
    let mut ids: Vec<i32> = room
        .users
        .iter()
        .filter(|(id, u)| !is_recorder(**id, &u.name))
        .map(|(id, _)| *id)
        .collect();
    ids.sort_unstable();
    if let Some(m) = me {
        if let Some(pos) = ids.iter().position(|&x| x == m) {
            let me = ids.remove(pos);
            ids.insert(0, me);
        }
    }
    ids
}

pub fn user_count(room: &ClientRoomState) -> usize {
    room.users
        .iter()
        .filter(|(id, u)| !is_recorder(**id, &u.name))
        .count()
}
