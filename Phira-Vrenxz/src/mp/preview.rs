
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::Result;
use phira_mp_client::Client;
use phira_mp_common::RoomState;
use prpr::{
    config::Mods,
    ext::LocalTask,
    scene::{GameMode, NextScene},
    task::Task,
};

use crate::scene::SongScene;

pub enum PreviewReturn {

    Ignore,

    AlreadyStarted,

    AskReady,
}

#[derive(Default)]
pub struct Preview {

    stop: Option<Arc<AtomicBool>>,

    interrupt: Option<Arc<AtomicBool>>,

    armed: bool,
}

impl Preview {
    pub fn new() -> Self {
        Self {
            stop: None,
            interrupt: None,
            armed: false,
        }
    }

    pub fn begin(&mut self, client: Option<Arc<Client>>, id: Option<i32>, path: &str) -> Result<LocalTask<Result<NextScene>>> {
        let interrupt = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let task = SongScene::global_launch_preview(
            id,
            path,
            Mods::AUTOPLAY,
            prpr::config::HpMode::Off,
            GameMode::NoRetry,
            None,
            None,
            None,
            false,
            false,
            true,
            Some(Arc::clone(&interrupt)),
            false,
            None,
        )?;
        if let Some(client) = client {
            let (interrupt, stop) = (Arc::clone(&interrupt), Arc::clone(&stop));

            Task::new(async move { Self::watch_loop(client, interrupt, stop).await });
        }
        self.interrupt = Some(interrupt);
        self.stop = Some(stop);
        Ok(task)
    }

    pub fn abort(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        self.interrupt = None;
    }

    pub fn on_return(&mut self, room: Option<&RoomState>, is_ready: bool) -> PreviewReturn {

        if self.stop.take().is_none() {
            return PreviewReturn::Ignore;
        }
        self.interrupt = None;
        match room {

            Some(RoomState::Playing) => PreviewReturn::AlreadyStarted,

            Some(RoomState::WaitingForReady) if is_ready => PreviewReturn::Ignore,

            _ => PreviewReturn::AskReady,
        }
    }

    #[inline]
    pub fn ask_ready(&mut self) {
        self.armed = true;
    }

    #[inline]
    pub fn prompt(&self, room: Option<&RoomState>, is_ready: bool) -> bool {
        self.armed && !is_ready && matches!(room, Some(RoomState::WaitingForReady))
    }

    #[inline]
    pub fn dismiss_prompt(&mut self) {
        self.armed = false;
    }

    async fn watch_loop(client: Arc<Client>, interrupt: Arc<AtomicBool>, stop: Arc<AtomicBool>) {

        let initial = client.room_state().await;
        loop {
            tokio::time::sleep(Duration::from_millis(150)).await;
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let Some(state) = client.room_state().await else {

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
}
