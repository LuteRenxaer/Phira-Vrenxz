use anyhow::{Context, Result};
use image::{codecs::gif, AnimationDecoder, DynamicImage, ImageError};
use macroquad::prelude::{Color, WHITE};
use sasa::AudioClip;
use serde::{Deserialize, Deserializer};
use std::{any::Any, cell::RefCell, collections::HashMap, future::IntoFuture, io::Cursor, str::FromStr, sync::Arc, time::Duration};
use tracing::debug;

use super::{process_lines, L10N_LOCAL, RPE_TWEEN_MAP};
use super::pgr::{parse_block_areas, PgrBlockArea};
use crate::{
    core::{
        Anim, AnimFloat, AnimVector, BezierTween, BlockArea, BlockMoveEvent, BlockRotateEvent, BlockScaleEvent, BpmList, Chart, ChartExtra,
        ChartSettings, ClampedTween, CtrlObject, GeneralIntTween, GifFrames, HitSoundMap, IntClampedTween, IntStaticTween, JudgeLine,
        JudgeLineCache, JudgeLineKind, Keyframe, Note, NoteKind, Object, StaticTween, Triple, TweenFunction, Tweenable, UIElement, Vector,
        EPS, HEIGHT_RATIO,
    },
    ext::{NotNanExt, SafeTexture},
    fs::FileSystem,
    judge::{HitSound, JudgeStatus},
    parse::ParseWarnings,
};

pub const RPE_WIDTH: f32 = 1350.;
pub const RPE_HEIGHT: f32 = 900.;
const SPEED_RATIO: f64 = 10. / 45. / HEIGHT_RATIO;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEBpmItem {
    bpm: f64,
    start_time: Triple,
}

fn f32_zero() -> f32 {
    0.
}

fn f32_one() -> f32 {
    1.
}

fn i32_one() -> i32 {
    1
}

fn rpe_version_default() -> i32 {
    160
}

fn deserialize_rpe_version<'de, D>(deserializer: D) -> std::result::Result<i32, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    let parsed = match value {
        Some(serde_json::Value::Number(v)) => v.as_i64().map(|it| it as i32),
        Some(serde_json::Value::String(s)) => s.parse::<i32>().ok(),
        _ => None,
    };
    Ok(parsed.unwrap_or(rpe_version_default()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEEvent<T = f32> {
    #[serde(default = "f32_zero")]
    easing_left: f32,
    #[serde(default = "f32_one")]
    easing_right: f32,
    #[serde(default)]
    bezier: u8,
    #[serde(default)]
    bezier_points: [f32; 4],
    #[serde(default = "i32_one")]
    easing_type: i32,
    start: T,
    end: T,
    start_time: Triple,
    end_time: Triple,
}

impl<T> RPEEvent<T> {
    fn bezier_key(&self) -> (u16, i16, i16) {
        let p = &self.bezier_points;
        let int = |p: f32| (p * 100.).round() as i16;
        ((int(p[0]) * 100 + int(p[1])) as u16, int(p[2]), int(p[3]))
    }

    pub fn tween(&self, bezier_map: &BezierMap) -> Arc<dyn TweenFunction> {
        let tween = RPE_TWEEN_MAP.get(self.easing_type.max(1) as usize).copied().unwrap_or(RPE_TWEEN_MAP[0]);
        let left = self.easing_left.clamp(0., 1.);
        let right = self.easing_right.clamp(0., 1.);
        if self.bezier != 0 {
            Arc::clone(&bezier_map[&self.bezier_key()])
        } else if tween <= 2 || (left.abs() < EPS as f32 && (right - 1.0).abs() < EPS as f32) || left >= right {
            StaticTween::get_rc(tween)
        } else {
            Arc::new(ClampedTween::new(tween, left..right))
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPECtrlEvent {
    easing: u8,
    x: f64,
    #[serde(flatten)]
    value: HashMap<String, f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEEventLayer {
    alpha_events: Option<Vec<RPEEvent>>,
    move_x_events: Option<Vec<RPEEvent>>,
    move_y_events: Option<Vec<RPEEvent>>,
    rotate_events: Option<Vec<RPEEvent>>,
    speed_events: Option<Vec<RPEEvent>>,
}

#[derive(Clone, Deserialize)]
struct RGBColor(u8, u8, u8);
impl From<RGBColor> for Color {
    fn from(RGBColor(r, g, b): RGBColor) -> Self {
        Self::from_rgba(r, g, b, 255)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEExtendedEvents {
    color_events: Option<Vec<RPEEvent<RGBColor>>>,
    text_events: Option<Vec<RPEEvent<String>>>,
    scale_x_events: Option<Vec<RPEEvent>>,
    scale_y_events: Option<Vec<RPEEvent>>,
    incline_events: Option<Vec<RPEEvent>>,
    paint_events: Option<Vec<RPEEvent>>,
    gif_events: Option<Vec<RPEEvent>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPENote {

    #[serde(rename = "type")]
    kind: u8,
    above: u8,
    start_time: Triple,
    end_time: Triple,
    position_x: f32,
    y_offset: f32,
    alpha: u16,
    hitsound: Option<String>,
    size: f32,
    speed: f32,
    is_fake: u8,
    visible_time: f64,
    #[serde(default)]
    tint: Option<[u8; 3]>,
    #[serde(default)]
    tint_hit_effects: Option<[u8; 3]>,
    #[serde(default)]
    judge_area: Option<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEJudgeLine {

    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Texture")]
    texture: String,
    #[serde(rename = "father")]
    parent: Option<isize>,
    rotate_with_father: Option<bool>,
    event_layers: Vec<Option<RPEEventLayer>>,
    extended: Option<RPEExtendedEvents>,
    notes: Option<Vec<RPENote>>,
    is_cover: u8,
    #[serde(default)]
    z_order: i32,
    #[serde(rename = "attachUI")]
    attach_ui: Option<UIElement>,

    #[serde(default)]
    pos_control: Vec<RPECtrlEvent>,
    #[serde(default)]
    size_control: Vec<RPECtrlEvent>,
    #[serde(default)]
    alpha_control: Vec<RPECtrlEvent>,
    #[serde(default)]
    y_control: Vec<RPECtrlEvent>,

    #[serde(default, rename = "bpmfactor")]
    bpm_factor: Option<f32>,

    #[serde(default, rename = "anchor")]
    anchor: Option<[f32; 2]>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEMetadata {
    offset: i32,
    #[serde(rename = "RPEVersion", default = "rpe_version_default", deserialize_with = "deserialize_rpe_version")]
    rpe_version: i32,
}

#[derive(Copy, Clone)]
enum SpeedEasingMode {
    Legacy,
    Modern,
}

struct SpeedIntegralTween {
    tween: Arc<dyn TweenFunction>,
    k: f32,
    b: f32,
    total: f32,
}

impl SpeedIntegralTween {
    fn try_create(tween: Arc<dyn TweenFunction>, k: f32, b: f32) -> Option<(Arc<dyn TweenFunction>, f32)> {
        let mut result = Self { tween, k, b, total: 0. };
        let total = result.partial(1.);
        if !total.is_finite() || total.abs() < EPS as f32 {
            return None;
        }
        result.total = total;
        Some((Arc::new(result), total))
    }

    fn partial(&self, x: f32) -> f32 {
        self.tween.y(x) * self.k + self.b * x
    }
}

impl TweenFunction for SpeedIntegralTween {
    fn y(&self, x: f32) -> f32 {
        if x <= 0. {
            return 0.;
        }
        if x >= 1. {
            return 1.;
        }
        let y = self.partial(x) / self.total;
        if y.is_finite() {
            y
        } else {
            x
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn speed_linear_tween(start_speed: f32, end_speed: f32) -> Arc<dyn TweenFunction> {
    if (start_speed - end_speed).abs() < EPS as f32 {
        StaticTween::get_rc(2)
    } else if start_speed.abs() > end_speed.abs() {
        Arc::new(ClampedTween::new(7 , 0.0..(1. - end_speed / start_speed)))
    } else {
        Arc::new(ClampedTween::new(6 , (start_speed / end_speed)..1.))
    }
}

fn speed_segment_tween(mode: SpeedEasingMode, start_speed: f32, end_speed: f32, tween: Arc<dyn TweenFunction>) -> (Arc<dyn TweenFunction>, f32) {
    let (tween, total) = match mode {
        SpeedEasingMode::Legacy => {
            let df0 = tween.derivative(0.);
            let df1 = tween.derivative(1.);
            let denom = df1 - df0;
            if !denom.is_finite() || denom.abs() < 1e-8 {
                return (speed_linear_tween(start_speed, end_speed), (start_speed + end_speed) / 2.);
            }
            let k = (end_speed - start_speed) / denom;
            let b = start_speed - k * df0;
            SpeedIntegralTween::try_create(tween, k, b)
        }
        SpeedEasingMode::Modern => {
            let int_tween: Arc<dyn TweenFunction> = if let Some(s) = tween.as_any().downcast_ref::<StaticTween>() {
                IntStaticTween::get_rc(s.0)
            } else if let Some(s) = tween.as_any().downcast_ref::<ClampedTween>() {
                Arc::new(IntClampedTween::new(s.0, s.1.clone()))
            } else {
                Arc::new(GeneralIntTween::new(tween))
            };
            SpeedIntegralTween::try_create(int_tween, end_speed - start_speed, start_speed)
        }
    }
    .unwrap_or_else(|| (speed_linear_tween(start_speed, end_speed), (start_speed + end_speed) / 2.));
    (tween, total)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEChart {
    #[serde(rename = "META")]
    meta: RPEMetadata,
    #[serde(rename = "BPMList")]
    bpm_list: Vec<RPEBpmItem>,
    judge_line_list: Vec<RPEJudgeLine>,
    #[serde(default, alias = "BlockAreaList")]
    block_area_list: Vec<serde_json::Value>,
    #[serde(default)]
    noise_area_options: NextNoiseOptions,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NextNoiseOptions {
    #[serde(default)]
    ignore_triple_inversion: bool,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
struct RPEVec2 {
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEActiveInterval {
    start_time: Triple,
    end_time: Triple,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEMoveEvent {
    start_time: Triple,
    end_time: Triple,
    start: f32,
    end: f32,
    #[serde(default)]
    easing_type: i32,
    #[serde(default)]
    inst: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEBlockTransformEvent {
    start_time: Triple,
    end_time: Triple,
    start: f32,
    end: f32,
    #[serde(default)]
    easing_type: i32,
    #[serde(default)]
    inst: i32,
    #[serde(default)]
    anchor: RPEVec2,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEBlockArea {
    #[serde(default)]
    bottom_left: RPEVec2,
    #[serde(default)]
    top_right: RPEVec2,
    #[serde(default)]
    appear_time: Triple,
    #[serde(default)]
    disappear_time: Triple,
    #[serde(default)]
    active_intervals: Vec<RPEActiveInterval>,
    #[serde(default)]
    is_invert: bool,
    #[serde(default)]
    move_x_events: Vec<RPEMoveEvent>,
    #[serde(default)]
    move_y_events: Vec<RPEMoveEvent>,
    #[serde(default)]
    rotate_events: Vec<RPEBlockTransformEvent>,
    #[serde(default)]
    scale_x_events: Vec<RPEBlockTransformEvent>,
    #[serde(default)]
    scale_y_events: Vec<RPEBlockTransformEvent>,
}

type BezierMap = HashMap<(u16, i16, i16), Arc<dyn TweenFunction>>;

fn parse_events<T: Tweenable, V: Clone + Into<T>>(
    r: &mut BpmList,
    rpe: &[RPEEvent<V>],
    default: Option<T>,
    bezier_map: &BezierMap,
) -> Result<Anim<T>> {
    if rpe.is_empty() {
        return Ok(Anim::default());
    }
    let mut kfs = Vec::new();
    if let Some(default) = default {
        if rpe[0].start_time.beats() != 0.0 {
            kfs.push(Keyframe::new(0.0, default, 0));
        }
    }
    for e in rpe {
        kfs.push(Keyframe {
            time: r.time(&e.start_time),
            value: e.start.clone().into(),
            tween: e.tween(bezier_map),
        });
        kfs.push(Keyframe::new(r.time(&e.end_time), e.end.clone().into(), 0));
    }
    Ok(Anim::new(kfs))
}

fn parse_speed_events(r: &mut BpmList, rpe: &[RPEEventLayer], bezier_map: &BezierMap, max_time: f64, mode: SpeedEasingMode, bpmfactor: f32) -> Result<AnimFloat> {
    let layers: Vec<_> = rpe.iter().filter_map(|it| it.speed_events.as_ref()).collect();
    if layers.is_empty() {
        return Ok(AnimFloat::default());
    }
    let mut anis = Vec::new();
    for layer in layers {
        if layer.is_empty() {
            continue;
        }
        let mut events = layer.iter().collect::<Vec<_>>();
        events.sort_by_key(|it| it.start_time.beats().not_nan());

        let mut kfs = vec![Keyframe::new(0.0, 0.0, 2)];
        let mut height = 0f64;
        let mut push_kf = |start_time: f64, end_time: f64, tween: Arc<dyn TweenFunction>, factor: f32| {
            if end_time - start_time <= EPS {
                return;
            }
            if let Some(last) = kfs.last_mut() {
                if (last.time - start_time).abs() < EPS {
                    last.value = height as f32;
                    last.tween = tween;
                } else {
                    kfs.push(Keyframe {
                        time: start_time,
                        value: height as f32,
                        tween,
                    });
                }
            }
            height += factor as f64 * (end_time - start_time);
        };

        let mut cursor = 0.0;
        let mut last_speed = 0.0;
        for event in events {
            let start_time = r.time(&event.start_time).max(cursor);
            let end_time = r.time(&event.end_time).max(start_time);
            let start_speed = event.start * SPEED_RATIO as f32 * bpmfactor;
            let end_speed = event.end * SPEED_RATIO as f32 * bpmfactor;

            push_kf(cursor, start_time, StaticTween::get_rc(2), last_speed);
            if end_time > start_time + EPS {
                if event.easing_type == 0 {
                    push_kf(start_time, end_time, StaticTween::get_rc(2), start_speed);
                } else if event.easing_type <= 1 {
                    if start_speed * end_speed < 0. {
                        let x = start_speed / (start_speed - end_speed);
                        let mid = f64::tween(&start_time, &end_time, x);
                        for (start_time, end_time, start, end) in [(start_time, mid, start_speed, 0.), (mid, end_time, 0., end_speed)] {
                            let factor = start.midpoint(end);
                            let tween = speed_linear_tween(start, end);
                            push_kf(start_time, end_time, tween, factor);
                        }
                    } else {
                        let factor = start_speed.midpoint(end_speed);
                        let tween = speed_linear_tween(start_speed, end_speed);
                        push_kf(start_time, end_time, tween, factor);
                    }
                } else {
                    let (tween, factor) = speed_segment_tween(mode, start_speed, end_speed, event.tween(bezier_map));
                    push_kf(start_time, end_time, tween, factor);
                }
            }
            cursor = end_time;
            last_speed = end_speed;
        }

        push_kf(cursor, max_time, StaticTween::get_rc(2), last_speed);
        if let Some(last) = kfs.last() {
            if (last.time - max_time).abs() > EPS {
                kfs.push(Keyframe::new(max_time, height as f32, 0));
            }
        }
        anis.push(AnimFloat::new(kfs));
    }
    if anis.is_empty() {
        return Ok(AnimFloat::default());
    }
    Ok(AnimFloat::chain(anis))
}

fn parse_speed_events_legacy(r: &mut BpmList, rpe: &[RPEEventLayer], max_time: f64) -> Result<AnimFloat> {
    let rpe: Vec<_> = rpe.iter().filter_map(|it| it.speed_events.as_ref()).collect();
    if rpe.is_empty() {

        return Ok(AnimFloat::default());
    };
    let anis: Vec<_> = rpe
        .into_iter()
        .filter_map(|it| {
            if it.is_empty() {
                return None;
            }
            let mut kfs = Vec::new();
            for e in it {
                kfs.push(Keyframe::new(r.time(&e.start_time), e.start, 2));
                kfs.push(Keyframe::new(r.time(&e.end_time), e.end, 0));
            }
            Some(AnimFloat::new(kfs))
        })
        .collect();
    if anis.is_empty() {
        return Ok(AnimFloat::default());
    }
    let mut pts: Vec<_> = anis.iter().flat_map(|it| it.keyframes.iter().map(|it| it.time.not_nan())).collect();
    pts.push(max_time.not_nan());
    pts.sort();
    pts.dedup();
    let mut sani = AnimFloat::chain(anis);
    sani.map_value(|v| v * SPEED_RATIO as f32);
    for i in 0..(pts.len() - 1) {
        let now_time = *pts[i];
        let end_time = *pts[i + 1];
        sani.set_time(now_time);
        let speed = sani.now();
        sani.set_time(end_time - 1e-4);
        let end_speed = sani.now();
        if speed.signum() * end_speed.signum() < 0. {
            pts.push(f64::tween(&now_time, &end_time, speed / (speed - end_speed)).not_nan());
        }
    }
    pts.sort();
    pts.dedup();
    let mut kfs = Vec::new();
    let mut height = 0f64;
    for i in 0..(pts.len() - 1) {
        let now_time = *pts[i];
        let end_time = *pts[i + 1];
        sani.set_time(now_time);
        let speed = sani.now();

        sani.set_time(end_time - 1e-4);
        let end_speed = sani.now();
        kfs.push(if (speed - end_speed).abs() < EPS as f32 {
            Keyframe::new(now_time, height as f32, 2)
        } else if speed.abs() > end_speed.abs() {
            Keyframe {
                time: now_time,
                value: height as f32,
                tween: Arc::new(ClampedTween::new(7 , 0.0..(1. - end_speed / speed))),
            }
        } else {
            Keyframe {
                time: now_time,
                value: height as f32,
                tween: Arc::new(ClampedTween::new(6 , (speed / end_speed)..1.)),
            }
        });
        height += (speed + end_speed) as f64 * (end_time - now_time) / 2.;
    }
    if kfs.is_empty() {
        return Ok(Anim::default());
    }
    kfs.push(Keyframe::new(max_time, height as f32, 0));
    Ok(AnimFloat::new(kfs))
}

fn parse_gif_events<V: Clone + Into<f32>>(r: &mut BpmList, rpe: &[RPEEvent<V>], bezier_map: &BezierMap, gif: &GifFrames) -> Result<Anim<f32>> {
    let mut kfs = Vec::new();
    kfs.push(Keyframe::new(0.0, 0.0, 2));
    let mut next_rep_time: u128 = 0;
    for e in rpe {
        while r.time(&e.start_time) > next_rep_time as f64 / 1000. {
            kfs.push(Keyframe::new(next_rep_time as f64 / 1000., 1.0, 0));
            kfs.push(Keyframe::new(next_rep_time as f64 / 1000., 0.0, 2));
            next_rep_time += gif.total_time();
        }
        let stop_prog = 1. - (next_rep_time as f64 - r.time(&e.start_time) * 1000.) / gif.total_time() as f64;
        kfs.push(Keyframe::new(r.time(&e.start_time), stop_prog as f32, 0));
        kfs.push(Keyframe {
            time: r.time(&e.start_time),
            value: e.start.clone().into(),
            tween: e.tween(bezier_map),
        });
        kfs.push(Keyframe::new(r.time(&e.end_time), e.end.clone().into(), 2));
        next_rep_time = (r.time(&e.end_time) * 1000. + gif.total_time() as f64 * (1. - e.end.clone().into()) as f64).round() as u128;
    }

    const GIF_MAX_TIME: f64 = 2000.;
    while GIF_MAX_TIME > next_rep_time as f64 / 1000. {
        kfs.push(Keyframe::new(next_rep_time as f64 / 1000., 1.0, 0));
        kfs.push(Keyframe::new(next_rep_time as f64 / 1000., 0.0, 2));
        next_rep_time += gif.total_time();
    }
    if kfs.is_empty() {
        return Ok(Anim::default());
    }
    Ok(Anim::new(kfs))
}

async fn parse_notes(
    r: &mut BpmList,
    rpe: Vec<RPENote>,
    fs: &mut dyn FileSystem,
    height: &mut AnimFloat,
    hitsounds: &mut HitSoundMap,
) -> Result<Vec<Note>> {
    let mut notes = Vec::new();
    for note in rpe {
        let time = r.time(&note.start_time);
        height.set_time(time);
        let note_height = height.now();
        let y_offset = note.y_offset * 2. / RPE_HEIGHT * note.speed;
        let kind = match note.kind {
            1 => NoteKind::Click,
            2 => {
                let end_time = r.time(&note.end_time);
                height.set_time(end_time);
                NoteKind::Hold {
                    end_time,
                    end_height: height.now() as f64,
                }
            }
            3 => NoteKind::Flick,
            4 => NoteKind::Drag,
            _ => ptl!(bail "unknown-note-type", "type" => note.kind),
        };
        let hitsound = match note.hitsound {
            Some(s) => {

                if s == "flick.mp3" {
                    HitSound::Flick
                } else if s == "tap.mp3" {
                    HitSound::Click
                } else if s == "drag.mp3" {
                    HitSound::Drag
                } else {
                    if hitsounds.get(&s).is_none() {
                        let data = fs.load_file(&s).await;
                        if let Ok(data) = data {
                            hitsounds.insert(s.clone(), AudioClip::new(data)?);
                        } else {
                            ptl!(bail "hitsound-missing", "name" => s);
                        }
                    }
                    HitSound::Custom(String::from_str(&s)?)
                }
            }
            None => HitSound::default_from_kind(&kind),
        };
        notes.push(Note {
            object: Object {
                alpha: if note.visible_time >= time {
                    if note.alpha >= 255 {
                        AnimFloat::default()
                    } else {
                        AnimFloat::fixed(note.alpha as f32 / 255.)
                    }
                } else {
                    let alpha = note.alpha.min(255) as f32 / 255.;
                    AnimFloat::new(vec![Keyframe::new(0.0, 0.0, 0), Keyframe::new(time - note.visible_time, alpha, 0)])
                },
                translation: AnimVector(AnimFloat::fixed(note.position_x / (RPE_WIDTH / 2.)), AnimFloat::fixed(y_offset)),
                scale: AnimVector(AnimFloat::fixed(note.size), AnimFloat::fixed(note.size)),
                ..Default::default()
            },
            kind,
            hitsound,
            time,
            height: note_height as f64,
            speed: note.speed as f64,
            color: note.tint.map_or(WHITE, |[r, g, b]| Color::from_rgba(r, g, b, 255)),
            fx_color: note.tint_hit_effects.map(|[r, g, b]| Color::from_rgba(r, g, b, 255)),
            judge_area: note.judge_area.unwrap_or(1.0),
            protected: false,

            above: note.above == 1,
            multiple_hint: false,
            fake: note.is_fake != 0,
            judge: JudgeStatus::NotJudged,
        })
    }
    Ok(notes)
}

fn parse_ctrl_events(rpe: &[RPECtrlEvent], key: &str) -> AnimFloat {
    let vals: Vec<_> = rpe.iter().map(|it| it.value[key]).collect();
    if rpe.is_empty() || (rpe.len() == 2 && rpe[0].easing == 1 && (vals[0] - 1.).abs() < 1e-4) {
        return AnimFloat::default();
    }

    let tweens: Vec<Arc<dyn TweenFunction>> = rpe
        .iter()
        .skip(1)
        .map(|it| StaticTween::get_rc(RPE_TWEEN_MAP.get(it.easing.max(1) as usize).copied().unwrap_or(RPE_TWEEN_MAP[0])))
        .chain(std::iter::once(StaticTween::get_rc(0)))
        .collect();
    AnimFloat::new(
        rpe.iter()
            .zip(vals)
            .zip(tweens)
            .map(|((it, val), tween)| Keyframe {
                time: it.x,
                value: val,
                tween,
            })
            .collect(),
    )
}

#[allow(clippy::too_many_arguments)]
async fn parse_judge_line(
    r: &mut BpmList,
    rpe: RPEJudgeLine,
    max_time: f64,
    speed_mode: SpeedEasingMode,
    fs: &mut dyn FileSystem,
    use_rpe_170_speed: bool,
    bezier_map: &BezierMap,
    hitsounds: &mut HitSoundMap,
    line_texture_map: &mut HashMap<String, SafeTexture>,
) -> Result<JudgeLine> {
    let event_layers: Vec<_> = rpe.event_layers.into_iter().flatten().collect();
    fn events_with_factor(
        r: &mut BpmList,
        event_layers: &[RPEEventLayer],
        get: impl Fn(&RPEEventLayer) -> &Option<Vec<RPEEvent>>,
        factor: f32,
        desc: &str,
        bezier_map: &BezierMap,
    ) -> Result<AnimFloat> {
        let anis: Vec<_> = event_layers
            .iter()
            .filter_map(|it| get(it).as_ref().map(|es| parse_events(r, es, None, bezier_map)))
            .collect::<Result<_>>()
            .with_context(|| ptl!("type-events-parse-failed", "type" => desc))?;
        let mut res = AnimFloat::chain(anis);
        if res.is_default() {
            return Ok(AnimFloat::fixed(0.0));
        }
        res.map_value(|v| v * factor);
        Ok(res)
    }
    let bpmfactor = rpe.bpm_factor.unwrap_or(1.0);
    let mut height = if use_rpe_170_speed {

        let bpmfactor = if bpmfactor > 0.0 { bpmfactor } else { 1.0 };
        parse_speed_events(r, &event_layers, bezier_map, max_time, speed_mode, bpmfactor)?
    } else {
        parse_speed_events_legacy(r, &event_layers, max_time)?
    };
    let mut notes = parse_notes(r, rpe.notes.unwrap_or_default(), fs, &mut height, hitsounds).await?;
    let cache = JudgeLineCache::new(&mut notes);
    Ok(JudgeLine {
        object: Object {
            alpha: events_with_factor(r, &event_layers, |it| &it.alpha_events, 1. / 255., "alpha", bezier_map)?,
            rotation: events_with_factor(r, &event_layers, |it| &it.rotate_events, -1., "rotate", bezier_map)?,
            translation: AnimVector(
                events_with_factor(r, &event_layers, |it| &it.move_x_events, 2. / RPE_WIDTH, "move X", bezier_map)?,
                events_with_factor(r, &event_layers, |it| &it.move_y_events, 2. / RPE_HEIGHT, "move Y", bezier_map)?,
            ),
            scale: {
                fn parse(r: &mut BpmList, opt: &Option<Vec<RPEEvent>>, factor: f32, bezier_map: &BezierMap) -> Result<AnimFloat> {
                    let mut res = opt
                        .as_ref()
                        .map(|it| parse_events(r, it, None, bezier_map))
                        .transpose()?
                        .unwrap_or_default();
                    if res.is_default() {

                        return Ok(AnimFloat::fixed(factor));
                    }
                    res.map_value(|v| v * factor);
                    Ok(res)
                }
                let factor = if rpe.texture == "line.png" { 1. } else { 2. / RPE_WIDTH };
                let scale_x_factor = factor
                    * if rpe.texture == "line.png"
                        && rpe
                            .extended
                            .as_ref()
                            .and_then(|it| it.text_events.as_ref())
                            .is_none_or(|it| it.is_empty())
                        && rpe.attach_ui.is_none()
                    {
                        0.5
                    } else {
                        1.
                    };
                rpe.extended
                    .as_ref()
                    .map(|e| -> Result<_> {
                        Ok(AnimVector(
                            parse(r, &e.scale_x_events, scale_x_factor, bezier_map)?,
                            parse(r, &e.scale_y_events, factor, bezier_map)?,
                        ))
                    })
                    .transpose()?
                    .unwrap_or_else(|| AnimVector(AnimFloat::fixed(scale_x_factor), AnimFloat::fixed(factor)))
            },
        },
        ctrl_obj: RefCell::new(CtrlObject {
            alpha: parse_ctrl_events(&rpe.alpha_control, "alpha"),
            size: parse_ctrl_events(&rpe.size_control, "size"),
            pos: parse_ctrl_events(&rpe.pos_control, "pos"),
            y: parse_ctrl_events(&rpe.y_control, "y"),
        }),
        height,
        incline: if let Some(events) = rpe.extended.as_ref().and_then(|e| e.incline_events.as_ref()) {
            parse_events(r, events, Some(0.), bezier_map).with_context(|| ptl!("incline-events-parse-failed"))?
        } else {
            AnimFloat::default()
        },
        notes,
        kind: {

            if let Some(events) = rpe
                .extended
                .as_ref()
                .and_then(|e| e.text_events.as_ref())
                .filter(|it| !it.is_empty())
            {
                JudgeLineKind::Text(parse_events(r, events, Some(String::new()), bezier_map).with_context(|| ptl!("text-events-parse-failed"))?)
            } else if rpe.texture == "line.png" {
                if let Some(events) = rpe.extended.as_ref().and_then(|e| e.paint_events.as_ref()) {
                    JudgeLineKind::Paint(
                        parse_events(r, events, Some(-1.), bezier_map).with_context(|| ptl!("paint-events-parse-failed"))?,
                        RefCell::default(),
                    )
                } else {
                    JudgeLineKind::Normal
                }
            } else if let Some(extended) = rpe.extended.as_ref() {
                if let Some(events) = extended.gif_events.as_ref() {
                    let data = fs
                        .load_file(&rpe.texture)
                        .await
                        .with_context(|| ptl!("gif-load-failed", "path" => rpe.texture.clone()))?;
                    let frames = GifFrames::new(
                        tokio::spawn(async move {
                            let decoder = gif::GifDecoder::new(Cursor::new(data))?;
                            debug!("decoding gif");
                            Ok::<std::vec::Vec<_>, ImageError>(decoder.into_frames().collect())
                        })
                        .into_future()
                        .await??
                        .into_iter()
                        .map(|frame| -> (u128, SafeTexture) {
                            let frame = frame.unwrap();
                            let delay: Duration = frame.delay().into();
                            (delay.as_millis(), SafeTexture::from(DynamicImage::ImageRgba8(frame.into_buffer())))
                        })
                        .collect(),
                    );
                    debug!("gif decoded");
                    let events = parse_gif_events(r, events, bezier_map, &frames).with_context(|| ptl!("gif-events-parse-failed"))?;
                    JudgeLineKind::TextureGif(events, frames, rpe.texture.clone())
                } else if let Some(texture) = line_texture_map.get(&rpe.texture) {
                    debug!("texture {} reused, id: {}", rpe.texture.clone(), texture.clone().into_inner().raw_miniquad_texture_handle().gl_internal_id());
                    JudgeLineKind::Texture(texture.clone(), rpe.texture.clone())
                } else {
                    let texture = SafeTexture::from(image::load_from_memory(
                        &fs.load_file(&rpe.texture)
                            .await
                            .with_context(|| ptl!("illustration-load-failed", "path" => rpe.texture.clone()))?,
                    )?)
                    .with_mipmap();
                    line_texture_map.insert(rpe.texture.clone(), texture.clone());
                    JudgeLineKind::Texture(texture, rpe.texture.clone())
                }
            } else if let Some(texture) = line_texture_map.get(&rpe.texture) {
                debug!("texture {} reused, id: {}", rpe.texture.clone(), texture.clone().into_inner().raw_miniquad_texture_handle().gl_internal_id());
                JudgeLineKind::Texture(texture.clone(), rpe.texture.clone())
            } else {
                let texture = SafeTexture::from(image::load_from_memory(
                    &fs.load_file(&rpe.texture)
                        .await
                        .with_context(|| ptl!("illustration-load-failed", "path" => rpe.texture.clone()))?,
                )?)
                .with_mipmap();
                line_texture_map.insert(rpe.texture.clone(), texture.clone());
                JudgeLineKind::Texture(texture, rpe.texture.clone())
            }
        },
        color: if let Some(events) = rpe.extended.as_ref().and_then(|e| e.color_events.as_ref()) {
            parse_events(r, events, Some(WHITE), bezier_map).with_context(|| ptl!("color-events-parse-failed"))?
        } else {
            Anim::default()
        },
        parent: {
            let parent = rpe.parent.unwrap_or(-1);
            if parent == -1 {
                None
            } else {
                Some(parent as usize)
            }
        },
        rot_with_parent: rpe.rotate_with_father.unwrap_or(false),
        z_index: rpe.z_order,
        show_below: rpe.is_cover != 1,
        attach_ui: rpe.attach_ui,
        texture_anchor: rpe.anchor.map(|a| (a[0], a[1])).unwrap_or((0.5, 0.5)),

        cache,
    })
}

fn add_bezier<T>(map: &mut BezierMap, event: &RPEEvent<T>) {
    if event.bezier != 0 {
        let p = &event.bezier_points;
        let int = |p: f32| (p * 100.).round() as i16;
        map.entry(((int(p[0]) * 100 + int(p[1])) as u16, int(p[2]), int(p[3])))
            .or_insert_with(|| Arc::new(BezierTween::new((p[0], p[1]), (p[2], p[3]))));
    }
}

macro_rules! process_bezier {
    ($event_layer:expr, $map:expr, $($field:ident),*) => {
        $(
            for event in $event_layer.$field.iter().flatten() {
                add_bezier($map, event);
            }
        )*
    };
}

fn get_bezier_map(rpe: &RPEChart) -> BezierMap {
    let mut map = HashMap::new();
    for line in &rpe.judge_line_list {
        for event_layer in line.event_layers.iter().flatten() {
            process_bezier!(event_layer, &mut map, alpha_events, move_x_events, move_y_events, rotate_events, speed_events);
        }
        if let Some(ext_layer) = &line.extended {
            process_bezier!(ext_layer, &mut map, paint_events, scale_x_events, scale_y_events, gif_events, incline_events, text_events, color_events);
        }
    }
    map
}

fn rpen_beat(v: &serde_json::Value) -> Option<f64> {
    if let Some(n) = v.as_f64() {
        return Some(n);
    }
    let a = v.as_array()?;
    if a.len() != 3 {
        return None;
    }
    Some(a[0].as_f64()? + a[1].as_f64()? / a[2].as_f64()?)
}

fn parse_rpen_effects(raw: &serde_json::Value, bpm: &mut BpmList) -> Vec<crate::core::Effect> {
    let mut events: Vec<&serde_json::Value> = Vec::new();
    for key in ["effects", "shaderEvents"] {
        events.extend(raw[key].as_array().into_iter().flatten());
    }
    events.extend(raw["META"]["effects"].as_array().into_iter().flatten());
    // 线内 paintEvents 需要按判定线作用域渲染；当前全局后处理无法正确承载，
    // 全量转全屏效果会叠加大量 shader 导致帧率暴跌，先跳过。
    let mut out = Vec::new();
    for e in events {
        let Some(name) = e
            .get("shader")
            .or_else(|| e.get("shaderType"))
            .or_else(|| e.get("name"))
            .and_then(|v| v.as_str())
        else {
            continue;
        };
        let base = name
            .replace('\\', "/")
            .rsplit('/')
            .next()
            .unwrap_or(name)
            .trim_end_matches(".glsl")
            .trim_end_matches(".frag")
            .trim_end_matches("_pr")
            .to_ascii_lowercase();
        let source = crate::core::Effect::get_rpe_preset(&format!("{base}_pr"))
            .or_else(|| crate::core::Effect::get_preset(&base));
        let Some(source) = source else {
            continue;
        };
        let start = e
            .get("startTime")
            .or_else(|| e.get("start"))
            .and_then(rpen_beat)
            .unwrap_or(0.);
        let end = e
            .get("endTime")
            .or_else(|| e.get("end"))
            .and_then(rpen_beat)
            .unwrap_or(start);
        if end <= start {
            continue;
        }
        let range = bpm.time_beats(start)..bpm.time_beats(end);
        let global = e
            .get("global")
            .and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|n| n != 0)))
            .unwrap_or(false);
        let mut uniforms: Vec<Box<dyn crate::core::Uniform>> = Vec::new();
        if let Some(vars) = e["vars"].as_object() {
            for (key, value) in vars {
                if ["time", "screenSize", "UVScale"].contains(&key.as_str()) {
                    continue;
                }
                if let Some(n) = value.as_f64() {
                    uniforms.push(Box::new((key.clone(), n as f32)));
                } else if let Some(a) = value.as_array() {
                    if a.first().is_some_and(|v| v.is_object()) {
                        let mut kfs = Vec::new();
                        for item in a {
                            let Some(start) = item.get("startTime").and_then(rpen_beat) else { continue };
                            let Some(end) = item.get("endTime").and_then(rpen_beat) else { continue };
                            let start_v = item.get("start").and_then(|v| v.as_f64()).unwrap_or(0.) as f32;
                            let end_v = item.get("end").and_then(|v| v.as_f64()).unwrap_or(start_v as f64) as f32;
                            kfs.push(crate::core::Keyframe::new(bpm.time_beats(start), start_v, 0));
                            kfs.push(crate::core::Keyframe::new(bpm.time_beats(end), end_v, 0));
                        }
                        if !kfs.is_empty() {
                            uniforms.push(Box::new((key.clone(), crate::core::Anim::new(kfs))));
                        }
                    } else if a.len() == 2 {
                        if let (Some(x), Some(y)) = (a[0].as_f64(), a[1].as_f64()) {
                            uniforms.push(Box::new((key.clone(), macroquad::prelude::vec2(x as f32, y as f32))));
                        }
                    } else if a.len() == 4 {
                        let values: Vec<f32> = a.iter().filter_map(|v| v.as_f64().map(|n| n as f32)).collect();
                        if values.len() == 4 {
                            uniforms.push(Box::new((
                                key.clone(),
                                macroquad::prelude::Color::new(values[0], values[1], values[2], values[3]),
                            )));
                        }
                    }
                } else if value.is_object() {
                    // 简化：取事件终点作为静态 uniform。
                    if let Some(n) = value.get("end").and_then(|v| v.as_f64()) {
                        uniforms.push(Box::new((key.clone(), n as f32)));
                    }
                }
            }
        }
        if let Ok(effect) = crate::core::Effect::new(range, source, uniforms, global) {
            out.push(effect);
        }
    }
    out
}

fn parse_embedded_blocks(
    list: Vec<serde_json::Value>,
    bpm: &[RPEBpmItem],
    layer_order: &[String],
) -> Result<(Vec<BlockArea>, Vec<crate::core::rpe_next_block::Area>)> {
    use crate::core::rpe_next_block::{Area, AreaData, Tempo};
    let mut native = Vec::new();
    let mut next = Vec::new();
    let mut tempo = None;
    let mut layers: Vec<String> = Vec::new();
    for (index, value) in list.into_iter().enumerate() {
        let parse = || -> Result<_> {
            let is_next = value.get("bottomLeft").is_some() || value.get("topRight").is_some();
            let is_native = value.get("bottomLeftPercentage").is_some() || value.get("topRightPercentage").is_some();
            anyhow::ensure!(is_next != is_native, "unknown or ambiguous block-area schema");
            if is_next {
                if tempo.is_none() {
                    tempo = Some(Tempo::new(bpm.iter().map(|b| (b.start_time.beats(), b.bpm)))?);
                }
                let data: AreaData = serde_json::from_value(value)?;
                let layer = data.layer_id.as_ref().map(|id| {
                    layer_order
                        .iter()
                        .position(|s| s == id)
                        .unwrap_or_else(|| {
                            layers.iter().position(|s| s == id).unwrap_or_else(|| {
                                layers.push(id.clone());
                                layers.len() - 1
                            })
                        })
                });
                let mut area = Area::new(data, tempo.as_ref().unwrap())?;
                if let Some(layer) = layer {
                    area.set_layer(layer);
                }
                next.push(area);
            } else {
                native.push(serde_json::from_value::<PgrBlockArea>(value)?);
            }
            Ok(())
        };
        parse().with_context(|| format!("blockAreaList[{index}]"))?;
    }
    Ok((parse_block_areas(native), next))
}

fn rpe_block_axis_pct(v: f32, full: f32) -> f32 {
    if v.abs() <= 1.0 {
        (v + 1.) / 2.
    } else {
        0.5 + v / full
    }
}

fn rpe_block_vec(v: RPEVec2) -> Vector {
    Vector::new(
        rpe_block_axis_pct(v.x, RPE_WIDTH),
        rpe_block_axis_pct(v.y, RPE_HEIGHT),
    )
}

fn rpe_block_base_px(bl: RPEVec2, tr: RPEVec2) -> Vector {
    let span = |a: f32, b: f32, full: f32| {
        if a.abs() <= 1.0 && b.abs() <= 1.0 {
            full
        } else {
            (b - a).abs().max(1.)
        }
    };
    Vector::new(
        span(bl.x, tr.x, RPE_WIDTH),
        span(bl.y, tr.y, RPE_HEIGHT),
    )
}

fn block_axis_events(
    r: &mut BpmList,
    events: Vec<RPEMoveEvent>,
    x: bool,
    appear: f64,
) -> Vec<BlockMoveEvent> {
    let mut out = Vec::new();
    let value = |v: f32| {
        if x {
            rpe_block_axis_pct(v, RPE_WIDTH)
        } else {
            rpe_block_axis_pct(v, RPE_HEIGHT)
        }
    };
    if let Some(e) = events.first() {
        let start = r.time(&e.start_time);
        if start > appear {
            out.push(BlockMoveEvent {
                end: if x {
                    Vector::new(value(e.start), 0.)
                } else {
                    Vector::new(0., value(e.start))
                },
                time: appear,
                ease_x: 0,
                ease_y: 0,
            });
        }
    }
    for e in events {
        let start = r.time(&e.start_time);
        let end = r.time(&e.end_time);
        let ease = if e.inst != 0 { 14 } else { e.easing_type };
        if end > start {
            out.push(BlockMoveEvent {
                end: if x {
                    Vector::new(value(e.start), 0.)
                } else {
                    Vector::new(0., value(e.start))
                },
                time: start,
                ease_x: ease,
                ease_y: ease,
            });
        }
        out.push(BlockMoveEvent {
            end: if x {
                Vector::new(value(e.end), 0.)
            } else {
                Vector::new(0., value(e.end))
            },
            time: end,
            ease_x: ease,
            ease_y: ease,
        });
    }
    out.sort_by(|a, b| a.time.total_cmp(&b.time));
    out
}

fn rpe_scale_raw(
    r: &mut BpmList,
    events: Vec<RPEBlockTransformEvent>,
    _base: f32,
) -> Vec<(f64, f64, f32, f32, i32)> {
    events
        .into_iter()
        .map(|e| {
            (
                r.time(&e.start_time),
                r.time(&e.end_time),
                e.start,
                e.end,
                if e.inst != 0 { 14 } else { e.easing_type },
            )
        })
        .collect()
}

fn sample_scale_raw(raw: &[(f64, f64, f32, f32, i32)], t: f64) -> f32 {
    if raw.is_empty() {
        return 1.;
    }
    if t <= raw[0].0 {
        return raw[0].2;
    }
    let mut previous_end = raw[0].2;
    for e in raw {
        if t < e.0 {
            return previous_end;
        }
        if t <= e.1 && e.1 > e.0 {
            let p = ((t - e.0) / (e.1 - e.0)) as f32;
            return e.2 + p * (e.3 - e.2);
        }
        previous_end = e.3;
    }
    previous_end
}

fn parse_rpe_block_areas(r: &mut BpmList, list: Vec<RPEBlockArea>) -> Vec<BlockArea> {
    let mut out = Vec::new();
    for b in list {
        let top_right = rpe_block_vec(b.top_right);
        let bottom_left = rpe_block_vec(b.bottom_left);
        let base_px = rpe_block_base_px(b.bottom_left, b.top_right);
        let scale_anchor = b
            .scale_x_events
            .first()
            .or_else(|| b.scale_y_events.first())
            .map(|e| rpe_block_vec(e.anchor))
            .unwrap_or_else(|| Vector::new(0.5, 0.5));
        let appear_time = r.time(&b.appear_time);
        let disappear_time = r.time(&b.disappear_time);

        let mut active_intervals = Vec::new();
        for it in &b.active_intervals {
            active_intervals.push((r.time(&it.start_time), r.time(&it.end_time)));
        }
        active_intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
        active_intervals.retain(|&(start, end)| end > start);
        let enable_time = active_intervals.first().map_or(appear_time, |it| it.0);
        let disable_time = active_intervals.last().map_or(disappear_time, |it| it.1);

        let mut rotate_events = Vec::new();
        if let Some(e) = b.rotate_events.first() {
            let start = r.time(&e.start_time);
            if start > appear_time {
                rotate_events.push(BlockRotateEvent {
                    anchor: rpe_block_vec(e.anchor),
                    time: appear_time,
                    ease: 0,
                    rotation: e.start,
                });
            }
        }
        for e in b.rotate_events {
            let start = r.time(&e.start_time);
            let end = r.time(&e.end_time);
            let anchor = rpe_block_vec(e.anchor);
            let ease = if e.inst != 0 { 14 } else { e.easing_type };
            if end > start {
                rotate_events.push(BlockRotateEvent {
                    anchor,
                    time: start,
                    ease,
                    rotation: e.start,
                });
            }
            rotate_events.push(BlockRotateEvent {
                anchor,
                time: end,
                ease,
                rotation: e.end,
            });
        }
        rotate_events.sort_by(|a, b| a.time.total_cmp(&b.time));

        let raw_x = rpe_scale_raw(r, b.scale_x_events, base_px.x);
        let raw_y = rpe_scale_raw(r, b.scale_y_events, base_px.y);
        let mut scale_times = vec![appear_time];
        for (start, end, _, _, _) in raw_x.iter().chain(raw_y.iter()) {
            scale_times.push(*start);
            scale_times.push(*end);
        }
        scale_times.sort_by(f64::total_cmp);
        scale_times.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        let scale_events = scale_times
            .into_iter()
            .map(|time| BlockScaleEvent {
                anchor: scale_anchor,
                time,
                ease_x: 0,
                ease_y: 0,
                scale: Vector::new(sample_scale_raw(&raw_x, time), sample_scale_raw(&raw_y, time)),
            })
            .collect::<Vec<_>>();

        out.push(BlockArea {
            top_right,
            bottom_left,
            appear_time,
            enable_time,
            disable_time,
            disappear_time,
            is_subtract: b.is_invert,
            rotate_events,
            move_events: Vec::new(),
            scale_events,
            active_intervals,
            absolute_scale: true,
            base_px,
            move_x_events: block_axis_events(r, b.move_x_events, true, appear_time),
            move_y_events: block_axis_events(r, b.move_y_events, false, appear_time),
        });
    }
    out
}

pub async fn parse_rpe(source: &str, fs: &mut dyn FileSystem, extra: ChartExtra, use_rpe_170_speed: bool) -> Result<Chart> {
    let rpe: RPEChart = serde_json::from_str(source).with_context(|| ptl!("json-parse-failed"))?;
    let speed_mode = if rpe.meta.rpe_version >= 170 {
        SpeedEasingMode::Modern
    } else {
        SpeedEasingMode::Legacy
    };
    let bezier_map = get_bezier_map(&rpe);
    let raw_effects: serde_json::Value = serde_json::from_str(source).unwrap_or_default();
    let layer_order: Vec<String> = raw_effects["noiseAreaLayers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| l["id"].as_str().map(str::to_owned))
        .collect();
    let (block_areas, next_block_areas) = parse_embedded_blocks(rpe.block_area_list, &rpe.bpm_list, &layer_order)?;
    let mut r = BpmList::new(rpe.bpm_list.into_iter().map(|it| (it.start_time.beats(), it.bpm)).collect());
    let global_effects = parse_rpen_effects(&raw_effects, &mut r);
    fn vec<T>(v: &Option<Vec<T>>) -> impl Iterator<Item = &T> {
        v.iter().flat_map(|it| it.iter())
    }
    let mut hitsounds = HashMap::new();
    #[rustfmt::skip]
    let max_time = *rpe
        .judge_line_list
        .iter()
        .map(|line| {
            line.notes.as_ref().map(|notes| {
                notes
                    .iter()
                    .map(|note| r.time(&note.end_time).not_nan())
                    .max()
                    .unwrap_or_default()
            }).unwrap_or_default().max(
                line.event_layers.iter().filter_map(|it| it.as_ref().map(|layer| {
                    vec(&layer.alpha_events)
                        .chain(vec(&layer.move_x_events))
                        .chain(vec(&layer.move_y_events))
                        .chain(vec(&layer.rotate_events))
                        .map(|it| r.time(&it.end_time).not_nan())
                        .max().unwrap_or_default()
                })).max().unwrap_or_default()
            ).max(
                line.extended.as_ref().map(|e| {
                    vec(&e.scale_x_events)
                        .chain(vec(&e.scale_y_events))
                        .map(|it| r.time(&it.end_time).not_nan())
                        .max().unwrap_or_default()
                        .max(vec(&e.text_events).map(|it| r.time(&it.end_time).not_nan()).max().unwrap_or_default())
                }).unwrap_or_default()
            )
        })
        .max().unwrap_or_default() + 1.;

    let mut lines = Vec::new();
    let mut line_texture_map = HashMap::new();
    for (id, rpe) in rpe.judge_line_list.into_iter().enumerate() {
        let name = rpe.name.clone();
        lines.push(
            parse_judge_line(&mut r, rpe, max_time, speed_mode, fs, use_rpe_170_speed, &bezier_map, &mut hitsounds, &mut line_texture_map)
                .await
                .with_context(move || ptl!("judge-line-location-name", "jlid" => id, "name" => name))?,
        );
    }
    fn has_cycle(line: &JudgeLine, lines: &[JudgeLine], visited: &mut Vec<usize>) -> Option<usize> {
        if let Some(parent_index) = line.parent {
            if visited.contains(&parent_index) {
                return Some(parent_index);
            }
            visited.push(parent_index);
            return has_cycle(&lines[parent_index], lines, visited);
        }
        None
    }
    for (i, line) in lines.iter().enumerate() {
        let mut vec = Vec::new();
        vec.push(i);
        if let Some(line) = has_cycle(line, &lines, &mut vec) {
            ptl!(bail "found infinite recursive parent relations", "line" => line)
        }
    }
    process_lines(&mut lines);
    let mut chart = Chart::new(rpe.meta.offset as f32 / 1000.0, lines, r, ChartSettings::default(), extra, hitsounds, block_areas);
    chart.extra.effects.extend(global_effects);
    // RPEN noise-skin：基础支持 main.glsl 全屏 pass；复杂多 target 皮肤仍按内置渲染。
    if let Some(layers) = raw_effects["noiseAreaLayers"].as_array() {
        for layer in layers {
            let Some(root) = layer["skin"].as_str().filter(|s| !s.is_empty()) else { continue };
            let Ok(bytes) = fs.load_file(&format!("{root}/main.glsl")).await else { continue };
            let Ok(source) = String::from_utf8(bytes) else { continue };
            let mut uniforms: Vec<Box<dyn crate::core::Uniform>> = Vec::new();
            if let Some(params) = layer["parameters"].as_object() {
                for (k, v) in params {
                    if let Some(n) = v.as_f64() {
                        uniforms.push(Box::new((k.clone(), n as f32)));
                    } else if let Some(a) = v.as_array() {
                        let vals: Vec<f32> = a.iter().filter_map(|x| x.as_f64().map(|n| n as f32)).collect();
                        if vals.len() == 2 {
                            uniforms.push(Box::new((k.clone(), macroquad::prelude::vec2(vals[0], vals[1]))));
                        } else if vals.len() == 4 {
                            uniforms.push(Box::new((
                                k.clone(),
                                macroquad::prelude::Color::new(vals[0], vals[1], vals[2], vals[3]),
                            )));
                        }
                    }
                }
            }
            if let Ok(effect) = crate::core::Effect::new(0.0..f64::INFINITY, &source, uniforms, false) {
                chart.extra.effects.push(effect);
            }
        }
    }
    chart.set_next_block_areas(next_block_areas, rpe.noise_area_options.ignore_triple_inversion);
    Ok(chart)
}

pub async fn lint(source: &str) -> Result<ParseWarnings> {
    let rpe: RPEChart = serde_json::from_str(source).with_context(|| ptl!("json-parse-failed"))?;
    let has_new_speed_events = rpe
        .judge_line_list
        .iter()
        .flat_map(|line| line.event_layers.iter().flatten())
        .flat_map(|layer| layer.speed_events.iter())
        .flatten()
        .any(|event| event.easing_type > 1);
    let has_attach_ui = rpe.judge_line_list.iter().any(|line| line.attach_ui.is_some());

    Ok(ParseWarnings {
        has_new_speed_events,
        has_attach_ui,
    })
}




