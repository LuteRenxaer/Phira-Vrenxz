
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

pub enum DownloadStep {

    Pending,

    Continue(DownloadIntent),

    Cancelled,
}

impl MpState {

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

    pub fn join_room(&mut self, id: RoomId) {
        let client = self.connected();
        self.join_room_task = Some(Task::new(async move {
            client.join_room(id, false).await?;
            client.room_state().await.ok_or_else(|| anyhow!("expected room state"))
        }));
    }

    pub fn join_room_with_password(&mut self, id: RoomId, password: String) {
        let client = self.connected();
        self.join_room_task = Some(Task::new(async move {
            client.join_room_with_password(id, false, password).await?;
            client.room_state().await.ok_or_else(|| anyhow!("expected room state"))
        }));
    }

    pub fn leave_room(&mut self) {
        let client = self.connected();
        self.task = Some(Task::new(async move { client.leave_room().await }));
    }

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

    pub fn lock_room(&mut self, to: bool) {
        let client = self.connected();
        self.task = Some(Task::new(async move { client.lock_room(to).await.with_context(|| mtl!("lock-room-failed")) }));
    }

    pub fn cycle_room(&mut self, to: bool) {
        let client = self.connected();
        self.task = Some(Task::new(async move { client.cycle_room(to).await.with_context(|| mtl!("cycle-room-failed")) }));
    }

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

        self.local_chart = None;
        self.reset_local_chart();
        self.task = Some(Task::new(async move {
            client.select_online_chart(id).await.with_context(|| mtl!("select-chart-failed"))?;
            Ok(())
        }));
    }

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

            serve::stage_local_chart(&local_path, &uuid)?;
            client.select_local_chart(uuid, name).await.with_context(|| mtl!("select-chart-failed"))?;
            Ok(())
        }));
    }

    pub fn request_start(&mut self) {
        let Some(client) = self.client() else { return };
        let Some(state) = client.blocking_room_state() else { return };

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

    pub fn ready(&mut self) {
        if let Some(id) = self.chart_id {
            self.fetch_chart(id, DownloadIntent::Ready);
        }
    }

    pub fn start_game(&mut self) {
        let client = self.connected();
        self.task = Some(Task::new(async move {
            client.request_start().await.with_context(|| mtl!("request-start-failed"))?;
            Ok(())
        }));
    }

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

    pub fn cancel_local_chart(&mut self) {
        self.reset_local_chart();
        let client = self.connected();
        self.task = Some(Task::new(async move {
            client.cancel_local_chart().await?;
            Ok(())
        }));
    }

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

    pub fn fetch_chart(&mut self, id: i32, intent: DownloadIntent) {

        if self.download.chart_id == id
            && self.download.intent == Some(DownloadIntent::Cache)
            && (self.download.task.is_some() || self.download.ui.is_some())
        {
            self.download.intent = Some(intent);
            return;
        }
        self.download.intent = Some(intent);
        self.download.chart_id = id;
        self.download.task = Some(Task::new(async move { Ptr::new(id).fetch().await }));
    }

    pub fn prefetch_chart(&mut self, id: i32) {
        if self.prefetch_done == Some(id) {
            return;
        }
        if self.download.chart_id == id && (self.download.task.is_some() || self.download.ui.is_some()) {
            return;
        }
        self.prefetch_done = Some(id);
        if self.local_chart_ready(Some(id), None) {
            return;
        }
        self.fetch_chart(id, DownloadIntent::Cache);
    }

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

    pub fn start_serving(&mut self, chart_id: String) {
        let client = self.connected();
        let syncing = Arc::new(serve::ChartSyncing::new());
        self.syncing = Some(Arc::clone(&syncing));
        self.local_chart_task = Some(Task::new(async move {

            serve::upload_chart(&client, &chart_id).await?;

            client.send_chart(String::new(), 0).await?;
            Ok::<_, anyhow::Error>(())
        }));
    }

    pub fn download_pending_local_chart(&mut self) {
        let Some((chart_id, _chart_name)) = self.pending_download.take() else {

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

    pub fn sync_local_chart_stage(&mut self, state: Option<RoomState>) {
        if matches!(state, Some(RoomState::LocalChart)) {
            if self.local_chart.is_some() {
                self.chart_id = None;
            }
        } else {

            self.reset_local_chart();
            self.local_chart = None;
        }
    }

    pub fn begin_playing(&mut self) {
        use std::sync::atomic::Ordering as O;
        self.game_start_consumed = true;
        crate::scene::RECORD_ID.store(-1, O::Relaxed);

        *LAST_MP_FINISH.lock().unwrap() = None;
        self.need_upload = true;
        self.entered = false;

        if let Some(client) = self.client.clone() {
            *prpr::scene::PAUSE_NOTIFY.lock().unwrap() = Some(Arc::new(move |paused: bool| {

                let _ = client.blocking_send(phira_mp_common::ClientCommand::PauseState { paused });
            }));
        }
    }

    pub fn report_finish(&mut self) {
        if !self.need_upload {
            return;
        }
        self.need_upload = false;
        let Some(client) = self.client() else { return };

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

    pub fn reconnect_if_lost(&mut self) {
        let Some(client) = &self.client else { return };
        if client.ping_fail_count() >= 2 && self.connect_task.is_none() {
            warn!("lost connection, reconnecting…");
            show_message(mtl!("reconnect")).warn();
            self.connect();
        }
    }
}
