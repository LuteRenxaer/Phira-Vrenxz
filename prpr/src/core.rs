
pub use macroquad::color::Color;

pub const NOTE_WIDTH_RATIO_BASE: f64 = 0.13175016;
pub const HEIGHT_RATIO: f64 = 0.83175;

pub const EPS: f64 = 1e-5;

pub const LOW_RES_NOTE_THRESHOLD: usize = 100;

pub const HIT_FX_DENSITY_THRESHOLD: usize = 20;

pub type Point = nalgebra::Point2<f32>;
pub type Vector = nalgebra::Vector2<f32>;
pub type Matrix = nalgebra::Matrix3<f32>;

mod anim;
pub use anim::{Anim, AnimFloat, AnimVector, Keyframe};

mod block;
pub use block::{block_touch_blocked, touch_inset_world, BlockArea, BlockMoveEvent, BlockPhase, BlockRotateEvent, BlockScaleEvent, BlockTransform};

mod block_shader;
pub(crate) use block_shader::prepare_block_effects;
pub(crate) use block_shader::{prepare_block_geometry, clear_prepared_block_geometry};
pub(crate) use block_shader::zone_on_screen;
pub(crate) use block_shader::zone_on_screen_lim;
mod block_audio;
pub(crate) use block_audio::BlockAudio;
pub(crate) use block_shader::reset_block_effects;
pub use block_shader::{draw_disabled_zones, draw_zones, draw_zones_with_touches, Zone};
pub mod rpe_next_block;

mod chart;
mod block_timeline;
pub use chart::{Chart, ChartExtra, ChartSettings, HitSoundMap};

mod dynamic_background;
pub use dynamic_background::DynamicBackground;

mod effect;
pub use effect::{Effect, Uniform};

mod line;
pub use line::{GifFrames, JudgeLine, JudgeLineCache, JudgeLineKind, UIElement};

mod note;
use macroquad::prelude::set_pc_assets_folder;
pub use note::{BadNote, HitSound, Note, NoteKind, RenderConfig};

mod object;
pub use object::{CtrlObject, Object};

mod render;
pub use render::{copy_fbo, internal_id, MSRenderTarget};

mod resource;
pub use resource::{NoteStyle, ParticleEmitter, ResPackInfo, Resource, ResourcePack, BUFFER_SIZE, DPI_VALUE};

mod smooth;
pub use smooth::Smooth;

mod tween;
pub use tween::{
    easing_from, BezierTween, ClampedTween, GeneralIntTween, IntClampedTween, IntStaticTween, StaticTween, TweenFunction, TweenId, TweenMajor,
    TweenMinor, Tweenable, TWEEN_FUNCTIONS,
};

#[cfg(feature = "video")]
mod video;
#[cfg(feature = "video")]
pub use prpr_avc::demux_audio;
#[cfg(feature = "video")]
pub use video::{Video, VideoAttach};

use crate::ui::TextPainter;
use std::cell::RefCell;

thread_local! {
    pub static PGR_FONT: RefCell<Option<TextPainter>> = RefCell::default();
    pub static BOLD_FONT: RefCell<Option<TextPainter>> = RefCell::default();
    pub static MAIN_FONT: RefCell<Option<TextPainter>> = RefCell::default();
}

pub fn init_assets() {
    #[cfg(not(target_env = "ohos"))]
    if let Ok(mut exe) = std::env::current_exe() {
        while exe.pop() {
            if exe.join("assets").exists() {
                std::env::set_current_dir(exe).unwrap();
                break;
            }
        }
    }
    #[cfg(target_env = "ohos")]
    let _ = std::env::set_current_dir("/data/storage/el1/bundle/entry/resources/resfile/");
    set_pc_assets_folder("assets");
}

#[derive(serde::Deserialize)]

pub struct Triple(i32, u32, u32);
impl Default for Triple {
    fn default() -> Self {
        Self(0, 0, 1)
    }
}

impl Triple {
    pub fn beats(&self) -> f64 {
        self.0 as f64 + self.1 as f64 / self.2 as f64
    }
}

#[derive(Default)]
pub struct BpmList {

    elements: Vec<(f64, f64, f64)>,

    cursor: usize,
}

impl BpmList {

    pub fn new(ranges: Vec<(f64, f64)>) -> Self {
        let mut elements = Vec::new();
        let mut time = 0.0;
        let mut last_beats = 0.0;
        let mut last_bpm: Option<f64> = None;
        for (now_beats, bpm) in ranges {
            if let Some(bpm) = last_bpm {
                time += (now_beats - last_beats) * (60. / bpm);
            }
            last_beats = now_beats;
            last_bpm = Some(bpm);
            elements.push((now_beats, time, bpm));
        }
        BpmList { elements, cursor: 0 }
    }

    pub fn time_beats(&mut self, beats: f64) -> f64 {
        while let Some(kf) = self.elements.get(self.cursor + 1) {
            if kf.0 > beats {
                break;
            }
            self.cursor += 1;
        }
        while self.cursor != 0 && self.elements[self.cursor].0 > beats {
            self.cursor -= 1;
        }
        let (start_beats, time, bpm) = &self.elements[self.cursor];
        time + (beats - start_beats) * (60. / bpm)
    }

    pub fn time(&mut self, triple: &Triple) -> f64 {
        self.time_beats(triple.beats())
    }

    pub fn beat(&mut self, time: f64) -> f64 {
        while let Some(kf) = self.elements.get(self.cursor + 1) {
            if kf.1 > time {
                break;
            }
            self.cursor += 1;
        }
        while self.cursor != 0 && self.elements[self.cursor].1 > time {
            self.cursor -= 1;
        }
        let (beats, start_time, bpm) = &self.elements[self.cursor];
        beats + (time - start_time) / (60. / bpm)
    }

    pub(crate) fn time_beats_at(&self, beats: f64) -> f64 {
        let mut cursor = 0usize;
        while let Some(kf) = self.elements.get(cursor + 1) {
            if kf.0 > beats {
                break;
            }
            cursor += 1;
        }
        while cursor != 0 && self.elements[cursor].0 > beats {
            cursor -= 1;
        }
        let (start_beats, time, bpm) = &self.elements[cursor];
        time + (beats - start_beats) * (60. / bpm)
    }

    pub(crate) fn beat_at(&self, time: f64) -> f64 {
        let mut cursor = 0usize;
        while let Some(kf) = self.elements.get(cursor + 1) {
            if kf.1 > time {
                break;
            }
            cursor += 1;
        }
        while cursor != 0 && self.elements[cursor].1 > time {
            cursor -= 1;
        }
        let (beats, start_time, bpm) = &self.elements[cursor];
        beats + (time - start_time) / (60. / bpm)
    }
}
