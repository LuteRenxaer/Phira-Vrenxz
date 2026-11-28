#[cfg(feature = "cpal")]
pub mod cpal;

#[cfg(feature = "oboe")]
pub mod oboe;

#[cfg(feature = "ohos")]
pub mod ohos;

use std::sync::mpsc;

use crate::{
    mixer::{Mixer, MixerCommand},
    LatencyRecorder,
};
use anyhow::Result;

pub struct BackendSetup {
    pub(crate) mixer_rx: mpsc::Receiver<MixerCommand>,
    pub(crate) latency_rec: LatencyRecorder,
}

pub trait Backend {
    fn setup(&mut self, setup: BackendSetup) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn consume_broken(&self) -> bool;
}

struct State {
    pub mixer: Mixer,
    pub recorder: LatencyRecorder,
}

impl From<BackendSetup> for State {
    fn from(value: BackendSetup) -> Self {
        Self {
            mixer: Mixer::new(0, value.mixer_rx),
            recorder: value.latency_rec,
        }
    }
}
