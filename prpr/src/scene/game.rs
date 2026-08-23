#![allow(unused)]

prpr_l10n::tl_file!("game");

use super::{
    draw_background,
    ending::RecordUpdateState,
    loading::{BasicPlayer, SaveFn, UpdateFn, UploadFn},
    request_input, return_input, show_message, take_input, EndingScene, NextScene, Scene,
};
use crate::{
    bin::BinaryReader,
    config::{Config, Mods},
    core::{copy_fbo, internal_id, BadNote, Chart, ChartExtra, Effect, Point, Resource, UIElement, Vector, PGR_FONT},
    ext::{format_number, parse_time, screen_aspect, semi_black, semi_white, draw_parallelogram, draw_parallelogram_ex, draw_text_aligned, RectExt, SafeTexture, ScaleType, PARALLELOGRAM_SLOPE},
    fs::FileSystem,
    info::{ChartFormat, ChartInfo},
    judge::{icon_index, Judge},
    parse::{parse_extra, parse_pec, parse_phigros, parse_rpe},
    task::Task,
    time::TimeManager,
    ui::{back_sound, suspend_sound, RectButton, TextPainter, Ui},
};
use anyhow::{bail, Context, Result};
use concat_string::concat_string;
use inputbox::InputBox;
use lyon::path::Path;
use macroquad::{prelude::*, window::InternalGlContext};
use sasa::{Music, MusicParams};
use serde::{Deserialize, Serialize};
use std::{
    any::Any,
    cell::RefCell,
    fs::File,
    io::{Cursor, ErrorKind},
    ops::{Deref, DerefMut, Range},
    path::PathBuf,
    process::{Command, Stdio},
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};
use tracing::{debug, warn};

const PAUSE_CLICK_INTERVAL: f32 = 0.7;

#[rustfmt::skip]
#[cfg(closed)]
mod inner;
#[cfg(closed)]
use inner::*;

const WAIT_TIME: f64 = 0.5;
const AFTER_TIME: f64 = 0.7;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleRecord {
    pub score: i32,
    pub accuracy: f32,
    pub full_combo: bool,
}

impl SimpleRecord {
    pub fn update(&mut self, other: &SimpleRecord) -> bool {
        let mut changed = false;
        if other.score > self.score {
            self.score = other.score;
            changed = true;
        }
        if other.accuracy > self.accuracy {
            self.accuracy = other.accuracy;
            changed = true;
        }
        if other.full_combo & !self.full_combo {
            self.full_combo = other.full_combo;
            changed = true;
        }
        changed
    }
}

fn fmt_time(t: f32) -> String {
    let f = t < 0.;
    let t = t.abs();
    let secs = t % 60.;
    let mut t = (t / 60.) as u64;
    let mins = t % 60;
    t /= 60;
    let hrs = t % 100;
    format!("{}{hrs:02}:{mins:02}:{secs:05.2}", if f { "-" } else { "" })
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    fn on_game_start();
}

#[derive(PartialEq, Eq)]
pub enum GameMode {
    Normal,
    TweakOffset,
    Exercise,
    NoRetry,
    View,
}

#[derive(Clone)]
enum State {
    Starting,
    BeforeMusic,
    Playing,
    Ending,
}

pub struct GameScene {
    should_exit: bool,
    next_scene: Option<NextScene>,

    pub mode: GameMode,
    pub res: Resource,
    pub chart: Chart,
    pub judge: Judge,
    pub gl: InternalGlContext<'static>,
    player: Option<BasicPlayer>,
    chart_bytes: Vec<u8>,
    chart_format: ChartFormat,
    info_offset: f32,
    effects: Vec<Effect>,

    first_in: bool,
    exercise_range: Range<f64>,
    exercise_press: Option<(i8, u64)>,
    exercise_btns: (RectButton, RectButton),

    pub music: Music,

    state: State,
    pub last_update_time: f64,
    pause_rewind: Option<f64>,
    pause_first_time: f32,
    pause_alpha: f32,
    pause_blur_tex: Option<Texture2D>,
    pause_blur_rt: Option<RenderTarget>,
    pause_need_blur: bool,

    pub bad_notes: Vec<BadNote>,

    upload_fn: Option<UploadFn>,
    update_fn: Option<UpdateFn>,
    save_fn: Option<SaveFn>,

    best_record: Option<SimpleRecord>,

    pub touch_points: Vec<(f32, f32)>,
    fps_frame_count: u32,
    fps_total_time: f64,
    fps_last_frame_time: f64,

    dead: bool,
}

macro_rules! reset {
    ($self:ident, $res:expr, $tm:ident) => {{
        $self.bad_notes.clear();
        $self.judge.reset();
        $self.chart.reset();
        $res.judge_line_color = $res.res_pack.info.color_perfect();
        $self.music.pause()?;
        $self.music.seek_to(0.)?;
        $tm.speed = $res.config.speed as _;
        $tm.reset();
        $self.last_update_time = $tm.now();
        $self.state = State::Starting;
        $self.fps_frame_count = 0;
        $self.fps_total_time = 0.0;
        $self.fps_last_frame_time = $tm.real_time();
        $self.dead = false;
    }};
}

impl GameScene {
    pub const BEFORE_TIME: f64 = 0.7;
    pub const FADEOUT_TIME: f64 = WAIT_TIME + AFTER_TIME + 0.3;

    pub async fn load_chart_bytes(fs: &mut dyn FileSystem, info: &ChartInfo) -> Result<Vec<u8>> {
        if let Ok(bytes) = fs.load_file(&info.chart).await {
            return Ok(bytes);
        }
        if let Some(name) = info.chart.strip_suffix(".pec") {
            if let Ok(bytes) = fs.load_file(&concat_string!(name, ".json")).await {
                return Ok(bytes);
            }
        }
        bail!("Cannot find chart file")
    }

    pub fn infer_chart_format(info: &ChartInfo, bytes: &[u8]) -> ChartFormat {
        info.format.clone().unwrap_or_else(|| {
            if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                if text.starts_with('{') {
                    if text.contains("\"META\"") {
                        ChartFormat::Rpe
                    } else {
                        ChartFormat::Pgr
                    }
                } else {
                    ChartFormat::Pec
                }
            } else {
                ChartFormat::Pbc
            }
        })
    }

    pub async fn load_chart(fs: &mut dyn FileSystem, info: &ChartInfo) -> Result<(Chart, Vec<u8>, ChartFormat)> {
        let extra = fs.load_file("extra.json").await.ok().map(String::from_utf8).transpose()?;
        let extra = if let Some(extra) = extra {
            parse_extra(&extra, fs).await.context("Failed to parse extra")?
        } else {
            ChartExtra::default()
        };
        let bytes = Self::load_chart_bytes(fs, info).await.context("Failed to load chart")?;
        let format = Self::infer_chart_format(info, &bytes);
        let mut chart = match format {
            ChartFormat::Rpe => parse_rpe(&String::from_utf8_lossy(&bytes), fs, extra, info.use_rpe_170_speed.unwrap_or_default()).await,
            ChartFormat::Pgr => parse_phigros(&String::from_utf8_lossy(&bytes), extra),
            ChartFormat::Pec => parse_pec(&String::from_utf8_lossy(&bytes), extra),
            ChartFormat::Pbc => {
                let mut r = BinaryReader::new(Cursor::new(&bytes));
                r.read()
            }
        }?;
        chart.load_textures(fs).await?;
        chart.settings.hold_partial_cover = info.hold_partial_cover;
        Ok((chart, bytes, format))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        mode: GameMode,
        info: ChartInfo,
        mut config: Config,
        mut fs: Box<dyn FileSystem>,
        player: Option<BasicPlayer>,
        background: SafeTexture,
        illustration: SafeTexture,
        upload_fn: Option<UploadFn>,
        update_fn: Option<UpdateFn>,
        save_fn: Option<SaveFn>,
    ) -> Result<Self> {
        match mode {
            GameMode::TweakOffset => {
                config.mods.insert(Mods::AUTOPLAY);
            }
            GameMode::Exercise => {
                config.mods.remove(Mods::AUTOPLAY);
            }
            _ => {}
        }

        let (mut chart, chart_bytes, chart_format) = Self::load_chart(fs.deref_mut(), &info).await?;
        chart.arcaea_judgement = info.arcaea_judgement || config.arcaea_judgement;
        chart.fnf_judgement = info.fnf_judgement || config.fnf_judgement;
        if config.mods.contains(Mods::NO_SHADER) {
            chart.extra.effects.clear();
            chart.extra.global_effects.clear();
        }
        let effects = std::mem::take(&mut chart.extra.global_effects);
        if config.fxaa {
            chart
                .extra
                .effects
                .push(Effect::new(0.0..f64::INFINITY, include_str!("fxaa.glsl"), Vec::new(), false).unwrap());
        }

        if config.has_mod(Mods::NIGHTCORE) {
            config.speed *= 1.5;
        }

        if config.has_mod(Mods::RAINBOW) {
            chart
                .extra
                .effects
                .push(Effect::new(0.0..f64::INFINITY, include_str!("rainbow.glsl"), Vec::new(), false).unwrap());
        }

        let info_offset = info.offset;
        let mut res = Resource::new(
            config,
            info,
            fs,
            player.as_ref().and_then(|it| it.avatar.clone()),
            background,
            illustration,
            chart.extra.effects.is_empty() && effects.is_empty(),
        )
        .await
        .context("Failed to load resources")?;


        chart.hitsounds.drain().for_each(|(name, clip)| {
            if let Ok(clip) = res.create_sfx(clip) {
                res.extra_sfxs.insert(name, clip);
            }
        });

        let exercise_range = (chart.offset + info_offset + res.config.offset) as f64..res.track_length;

        let judge = Judge::new(&chart);

        let music = Self::new_music(&mut res)?;
        Ok(Self {
            should_exit: false,
            next_scene: None,

            mode,
            res,
            chart,
            judge,
            gl: unsafe { get_internal_gl() },
            player,
            chart_bytes,
            chart_format,
            effects,
            info_offset,

            first_in: false,
            exercise_range,
            exercise_press: None,
            exercise_btns: (RectButton::new(), RectButton::new()),

            music,

            state: State::Starting,
            last_update_time: 0.,
            pause_rewind: None,
            pause_first_time: f32::NEG_INFINITY,
            pause_alpha: 0.,
            pause_blur_tex: None,
            pause_blur_rt: None,
            pause_need_blur: false,

            bad_notes: Vec::new(),

            upload_fn,
            update_fn,
            save_fn,

            best_record: None,

            touch_points: Vec::new(),

            fps_frame_count: 0,
            fps_total_time: 0.0,
            fps_last_frame_time: 0.0,

            dead: false,
        })
    }

    fn new_music(res: &mut Resource) -> Result<Music> {
        res.audio.create_music(
            res.music.clone(),
            MusicParams {
                amplifier: res.config.volume_music as _,
                playback_rate: res.config.speed as _,
                ..Default::default()
            },
        )
    }

    fn touch_scale(&self) -> f32 {
        (screen_width() / screen_height()) / self.res.aspect_ratio
    }

    fn ui(&mut self, ui: &mut Ui, tm: &mut TimeManager) -> Result<()> {
        let time = tm.now();
        let p = match self.state {
            State::Starting => {
                if time <= Self::BEFORE_TIME {
                    1. - (1. - time / Self::BEFORE_TIME).powi(3)
                } else {
                    1.
                }
            }
            State::BeforeMusic => 1.,
            State::Playing => 1.,
            State::Ending => {
                let t = time - self.res.track_length - WAIT_TIME;
                1. - (t / (AFTER_TIME + 0.3)).min(1.).powi(2)
            }
        } as f32;
        let res = &mut self.res;
        let eps = 2e-2 / res.aspect_ratio;
        let top = -1. / res.aspect_ratio;
        let pause_w = 0.015;
        let pause_h = pause_w * 3.2;
        let pause_center = Point::new(pause_w * 4.0 - 1., top + eps * 3.5 - (1. - p) * 0.4 + pause_h / 2.);
        if res.config.interactive
            && !tm.paused()
            && self.pause_rewind.is_none()
            && Judge::get_touches().iter().any(|touch| {
                touch.phase == TouchPhase::Started && {
                    let p = touch.position;
                    let p = Point::new(p.x, p.y);
                    (pause_center - p).norm() < 0.05
                }
            })
        {
            let t = tm.now() as f32;
            if t - self.pause_first_time > PAUSE_CLICK_INTERVAL && res.config.double_click_to_pause {
                self.pause_first_time = t;
            } else {
                self.pause_first_time = f32::NEG_INFINITY;
                if !self.music.paused() {
                    self.music.pause()?;
                }
                tm.pause();
                suspend_sound();
                self.pause_need_blur = true;
                #[cfg(target_env = "ohos")]
                miniquad::native::set_interceptor_state(false);
            }
        }
        ui.alpha(res.alpha, |ui| {
            ui.text("MAGIC BUGFIX TEXT").color(Color::new(0., 0., 0., 0.)).draw();
            if tm.now() as f32 - self.pause_first_time <= PAUSE_CLICK_INTERVAL {
                ui.fill_circle(pause_center.x, pause_center.y, 0.05, Color::new(1., 1., 1., 0.5));
            }

            ui.text("PHIRLIE v1.3.1")
                .pos(pause_center.x + 0.08, pause_center.y)
                .anchor(0., 0.5)
                .size(0.4)
                .color(semi_white(0.6))
                .draw();

            let margin = 0.03;

            let legacy_aui = !res.info.use_attach_ui_fix.unwrap_or_default();
            let unit_h = if legacy_aui { ui.text("0").measure_using(&PGR_FONT).h } else { 0. };


            let h = 0.07;
            let score_top = top + eps * 2.2 - (1. - p) * 0.4 + res.config.score_offset_y;
            let score_right = 1. - margin + res.config.score_offset_x;
            let score = if res.config.roman_numerals {
                format_number(self.judge.score() as u32, true, false)
            } else if res.config.chinese_numerals {
                format_number(self.judge.score() as u32, false, true)
            } else {
                format!("{:07}", self.judge.score())
            };
            let base_score_size = 0.8;
            let score_size = if res.config.roman_numerals || res.config.chinese_numerals {
                let ref_width = ui.text("0000000").size(base_score_size).measure_using(&PGR_FONT).w;
                let actual_width = ui.text(&score).size(base_score_size).measure_using(&PGR_FONT).w;
                if actual_width > ref_width {
                    base_score_size * ref_width / actual_width
                } else {
                    base_score_size
                }
            } else {
                base_score_size
            };
            let scale_point = legacy_aui.then(|| {
                let ct = ui.text(&score).size(score_size).measure_using(&PGR_FONT).center();
                (score_right - ct.x, score_top + ct.y)
            });
            self.chart
                .with_element(ui, res, UIElement::Score, scale_point, (score_right, score_top), |ui, c| {
                    if res.config.show_score {
                        ui.text(&score)
                            .pos(score_right, score_top)
                            .anchor(1., 0.)
                            .size(score_size)
                            .color(c)
                            .draw_using(&PGR_FONT);
                    }
                    if res.config.show_acc {
                        ui.text(format!("{:05.2}%", self.judge.real_time_accuracy() * 100.))
                            .pos(1. - margin, score_top + h)
                            .anchor(1., 0.)
                            .size(0.4)
                            .color(Color { a: c.a * 0.7, ..c })
                            .draw_using(&PGR_FONT);
                    }
                });

            self.chart.with_element(
                ui,
                res,
                UIElement::Pause,
                legacy_aui.then(|| (pause_center.x, pause_center.y)),
                (pause_center.x - pause_w * 1.5, pause_center.y - pause_h / 2.),
                |ui, c| {
                    let mut r = Rect::new(pause_center.x - pause_w * 1.5, pause_center.y - pause_h / 2., pause_w, pause_h);
                    ui.fill_rect(r, c);
                    r.x += pause_w * 2.;
                    ui.fill_rect(r, c);
                },
            );

            let is_autoplay = res.config.autoplay();

            if (self.judge.combo() >= 3 || is_autoplay) && res.config.show_combo {
                if legacy_aui {
                    let combo_top = top + eps * 2. - (1. - p) * 0.4 + res.config.combo_offset_y;
                    let btm = self
                        .chart
                        .with_element(ui, res, UIElement::ComboNumber, None, (res.config.combo_offset_x, combo_top + unit_h / 2.), |ui, c| {
                            if is_autoplay {
                                let text = &res.config.autoplay_display_text;
                                let size = 0.45;
                                let color = Color::new(1.0, 0.8, 0.2, c.a);
                                ui.text(text)
                                    .pos(0., combo_top)
                                    .anchor(0.5, 0.)
                                    .size(size)
                                    .color(color)
                                    .draw_using(&PGR_FONT)
                                    .bottom()
                            } else {
                                ui.text(format_number(self.judge.combo(), res.config.roman_numerals, res.config.chinese_numerals))
                                    .pos(0., combo_top)
                                    .anchor(0.5, 0.)
                                    .color(c)
                                    .draw_using(&PGR_FONT)
                                    .bottom()
                            }
                        });
                    let combo_top = btm + 0.01;

                    let combo_label = res.config.custom_combo_text();
                    let combo_display = if combo_label.is_empty() { "COMBO" } else { combo_label };

                    self.chart
                        .with_element(ui, res, UIElement::Combo, None, (res.config.combo_offset_x, combo_top + unit_h * 0.2), |ui, c| {
                            ui.text(if is_autoplay { "" } else { combo_display })
                                .pos(res.config.combo_offset_x, combo_top)
                                .anchor(0.5, 0.)
                                .size(0.4)
                                .color(c)
                                .draw_using(&PGR_FONT);
                        });
                } else {
                    if is_autoplay {
                        let text = &res.config.autoplay_display_text;
                        let size = 0.6;
                        let color = Color::new(1.0, 0.8, 0.2, res.alpha);
                        let text_r = ui.text(text).size(size).measure();
                        let y_pos = top + eps * 2. - (1. - p) * 0.4 + text_r.center().y + res.config.combo_offset_y;
                        self.chart.with_element(ui, res, UIElement::ComboNumber, None, (res.config.combo_offset_x, y_pos), |ui, c| {
                            ui.text(text)
                                .pos(res.config.combo_offset_x, y_pos)
                                .anchor(0.5, 0.5)
                                .size(size)
                                .color(color)
                                .draw_using(&PGR_FONT);
                        });
                    } else {
                        let combo = format_number(self.judge.combo(), res.config.roman_numerals, res.config.chinese_numerals);
                        let ct = ui.text(&combo).size(1.0).measure().center();
                        let combo_y = top + eps * 2. - (1. - p) * 0.4 + ct.y + res.config.combo_offset_y;
                        let btm = self.chart.with_element(ui, res, UIElement::ComboNumber, None, (res.config.combo_offset_x, combo_y), |ui, c| {
                            ui.text(&combo)
                                .pos(res.config.combo_offset_x, combo_y)
                                .anchor(0.5, 0.5)
                                .size(1.0)
                                .color(c)
                                .draw_using(&PGR_FONT)
                                .bottom()
                        });
                        let ct = ui.text("COMBO").size(0.4).measure().center();
                        let combo_top = btm + 0.01 + ct.y;

                        let combo_label = res.config.custom_combo_text();
                        let combo_display = if combo_label.is_empty() { "COMBO" } else { combo_label };

                        self.chart.with_element(ui, res, UIElement::Combo, None, (res.config.combo_offset_x, combo_top), |ui, c| {
                            ui.text(combo_display)
                                .pos(res.config.combo_offset_x, combo_top)
                                .anchor(0.5, 0.5)
                                .size(0.4)
                                .color(c)
                                .draw_using(&PGR_FONT);
                        });
                    }
                }
            }
            ui.text("").draw_using(&PGR_FONT);
            let lf = -1. + margin;
            let bt = -top - eps * 2.8 + (1. - p) * 0.4;
            let scale_point = legacy_aui.then(|| {
                let ct = ui.text(&res.info.name).size(0.5).measure().center();
                (lf + ct.x, bt - ct.y)
            });
            self.chart.with_element(ui, res, UIElement::Name, scale_point, (lf, bt), |ui, c| {
                ui.text(&res.info.name)
                    .pos(lf, bt)
                    .anchor(0., 1.)
                    .size(0.5)
                    .color(c)
                    .max_width(0.8)
                    .draw();
            });

            let scale_point = legacy_aui.then(|| {
                let ct = ui.text(&res.info.level).size(0.5).measure().center();
                (-lf - ct.x, bt - ct.y)
            });
            self.chart.with_element(ui, res, UIElement::Level, scale_point, (-lf, bt), |ui, c| {
                ui.text(&res.info.level).pos(-lf, bt).anchor(1., 1.).size(0.5).color(c).draw();
            });

            let hw = 0.003;
            let height = eps * 1.0;
            let dest = (2. * res.time / res.track_length).clamp(0., 2.) as f32;
            self.chart
                .with_element(ui, res, UIElement::Bar, Some((-1., top + height / 2.)), (-1., top + height / 2.), |ui, color| {
                    ui.fill_rect(Rect::new(-1., top, dest, height), semi_white(0.6));
                    ui.fill_rect(Rect::new(-1. + dest - hw, top, hw * 2., height), WHITE);
                });
        });
        Ok(())
    }

    fn overlay_ui(&mut self, ui: &mut Ui, tm: &mut TimeManager) -> Result<()> {
        let c = semi_white(self.res.alpha);
        let res = &mut self.res;
        if self.pause_alpha > 0.001 {
            let h = 1. / res.aspect_ratio;
            // 高斯模糊背景：暂停时抓取画面像素，用 fastblur 做 CPU 端高斯模糊
            if self.pause_need_blur {
                let vp = ui.viewport;
                let bw = (vp.2 as u32 / 8).max(1);
                let bh = (vp.3 as u32 / 8).max(1);
                // 创建/复用低分辨率 RenderTarget
                let need_new = self.pause_blur_rt.as_ref().map_or(true, |t| {
                    t.texture.width() as u32 != bw || t.texture.height() as u32 != bh
                });
                if need_new {
                    let gl = unsafe { get_internal_gl() };
                    let texture = miniquad::Texture::new_render_texture(
                        gl.quad_context,
                        miniquad::TextureParams {
                            width: bw,
                            height: bh,
                            format: miniquad::TextureFormat::RGBA8,
                            ..Default::default()
                        },
                    );
                    let render_pass = miniquad::RenderPass::new(gl.quad_context, texture, None);
                    self.pause_blur_rt = Some(RenderTarget {
                        texture: Texture2D::from_miniquad_texture(texture),
                        render_pass,
                    });
                }
                if let Some(rt) = &self.pause_blur_rt {
                    unsafe {
                        use miniquad::gl::*;
                        let mut current_fbo = 0;
                        glGetIntegerv(GL_FRAMEBUFFER_BINDING, &mut current_fbo);
                        // 从当前 framebuffer 缩放到低分辨率目标
                        glBindFramebuffer(GL_READ_FRAMEBUFFER, current_fbo as u32);
                        glBindFramebuffer(GL_DRAW_FRAMEBUFFER, internal_id(*rt));
                        glBlitFramebuffer(
                            0, 0, vp.2, vp.3,
                            0, 0, bw as i32, bh as i32,
                            GL_COLOR_BUFFER_BIT, GL_LINEAR,
                        );
                        glBindFramebuffer(GL_FRAMEBUFFER, current_fbo as u32);
                        // 读取低分辨率像素
                        let mut pixels = vec![0u8; (bw * bh * 3) as usize];
                        glBindFramebuffer(GL_FRAMEBUFFER, internal_id(*rt));
                        // 关键：设置行对齐为1字节，否则 glReadPixels 默认4字节对齐
                        // 当 bw*3 不是4的倍数时会越界写入，导致堆损坏 (0xc0000374)
                        const GL_PACK_ALIGNMENT: u32 = 0x0D05;
                        let mut old_pack_alignment = 4;
                        glGetIntegerv(GL_PACK_ALIGNMENT, &mut old_pack_alignment);
                        glPixelStorei(GL_PACK_ALIGNMENT, 1);
                        glReadPixels(0, 0, bw as i32, bh as i32, GL_RGB, GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut _);
                        glPixelStorei(GL_PACK_ALIGNMENT, old_pack_alignment);
                        glBindFramebuffer(GL_FRAMEBUFFER, current_fbo as u32);
                        // 上下翻转
                        let mut flipped = vec![0u8; (bw * bh * 3) as usize];
                        let row_size = (bw * 3) as usize;
                        for y in 0..bh as usize {
                            let src = (bh as usize - 1 - y) * row_size;
                            let dst = y * row_size;
                            flipped[dst..dst + row_size].copy_from_slice(&pixels[src..src + row_size]);
                        }
                        // 高斯模糊（低分辨率，半径小一点效果一样）
                        let mut pixel_data: Vec<[u8; 3]> = flipped.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
                        fastblur::gaussian_blur(&mut pixel_data, bw as _, bh as _, 8.0);
                        // 转换为 RGBA 创建纹理
                        let mut rgba = Vec::with_capacity((bw * bh * 4) as usize);
                        for p in &pixel_data {
                            rgba.push(p[0]);
                            rgba.push(p[1]);
                            rgba.push(p[2]);
                            rgba.push(255);
                        }
                        self.pause_blur_tex = Some(Texture2D::from_rgba8(bw as u16, bh as u16, &rgba));
                    }
                }
                self.pause_need_blur = false;
            }
            if let Some(tex) = self.pause_blur_tex {
                let bg_rect = Rect::new(-1., -h, 2., h * 2.);
                ui.fill_rect(bg_rect, (tex, bg_rect, ScaleType::Fit, Color::new(1., 1., 1., 0.7 * self.pause_alpha)));
            } else {
                draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., 0.5 * self.pause_alpha));
            }

            let old_alpha = ui.alpha;
            ui.alpha *= self.pause_alpha;

            // ===== 统计面板（来自 ending.rs 样式）=====
            let result = self.judge.result();
            let max_combo = result.max_combo;
            let counts = result.counts;
            // 实时准确率: (P + G*0.65) / (P+G+B+M)，分母为0时100%
            let judged = counts[0] + counts[1] + counts[2] + counts[3];
            let accuracy = if judged == 0 {
                1.0
            } else {
                (counts[0] as f64 + counts[1] as f64 * 0.65) / judged as f64
            };
            let early = result.early;
            let late = result.late;

            let panel_w = 0.9;
            let panel_h = 0.13;
            let panel_x = -panel_w / 2.;
            let panel_gap = 0.04;

            // 面板1：Max Combo / Accuracy
            let s1_y = 0.16;
            let s1 = Rect::new(panel_x, s1_y, panel_w, panel_h);
            ui.fill_path(&s1.rounded(0.01), Color::new(0., 0., 0., 0.4));
            {
                let dy = 0.02;
                ui.text("Max Combo")
                    .pos(s1.x + 0.04, s1.bottom() - dy)
                    .anchor(0., 1.)
                    .size(0.28)
                    .color(semi_white(0.8))
                    .draw();
                ui.text(&max_combo.to_string())
                    .pos(s1.x + 0.04, s1.y + 0.02)
                    .anchor(0., 0.)
                    .size(0.55)
                    .color(WHITE)
                    .draw();
                ui.text("Accuracy")
                    .pos(s1.right() - 0.04, s1.bottom() - dy)
                    .anchor(1., 1.)
                    .size(0.28)
                    .color(semi_white(0.8))
                    .draw();
                ui.text(&format!("{:.2}%", accuracy * 100.))
                    .pos(s1.right() - 0.04, s1.y + 0.02)
                    .anchor(1., 0.)
                    .size(0.55)
                    .color(WHITE)
                    .draw();
            }

            // 面板2：Perfect/Good/Bad/Miss + Early/Late
            let s2_y = s1_y + panel_h + panel_gap;
            let s2 = Rect::new(panel_x, s2_y, panel_w, panel_h);
            ui.fill_path(&s2.rounded(0.01), Color::new(0., 0., 0., 0.4));
            {
                let dy = 0.02;
                let dy2 = 0.012;
                let big = 0.45;
                let sm = 0.22;
                let draw_count = |ui: &mut Ui, x: f32, name: &str, count: u32| {
                    ui.text(name)
                        .pos(x, s2.bottom() - dy)
                        .anchor(0.5, 1.)
                        .size(sm)
                        .color(semi_white(0.8))
                        .draw();
                    ui.text(&count.to_string())
                        .pos(x, s2.y + dy2)
                        .anchor(0.5, 0.)
                        .size(big)
                        .color(WHITE)
                        .draw();
                };
                draw_count(ui, s2.x + s2.w * 0.12, "Perfect", counts[0]);
                draw_count(ui, s2.x + s2.w * 0.30, "Good", counts[1]);
                draw_count(ui, s2.x + s2.w * 0.44, "Bad", counts[2]);
                draw_count(ui, s2.x + s2.w * 0.58, "Miss", counts[3]);
                // Early / Late
                let l = s2.x + s2.w * 0.72;
                let rt = s2.x + s2.w * 0.94;
                let cy = s2.center().y;
                ui.text("Early")
                    .pos(l, cy - dy2 / 2.)
                    .anchor(0., 1.)
                    .size(0.24)
                    .color(semi_white(0.8))
                    .draw();
                ui.text(&early.to_string())
                    .pos(rt, cy - dy2 / 2.)
                    .anchor(1., 1.)
                    .size(0.24)
                    .color(WHITE)
                    .draw();
                ui.text("Late")
                    .pos(l, cy + dy2 / 2.)
                    .anchor(0., 0.)
                    .size(0.24)
                    .color(semi_white(0.8))
                    .draw();
                ui.text(&late.to_string())
                    .pos(rt, cy + dy2 / 2.)
                    .anchor(1., 0.)
                    .size(0.24)
                    .color(WHITE)
                    .draw();
            }

            let no_retry = self.mode == GameMode::NoRetry;
            let alpha = res.alpha;
            let btn_s = 0.06f32;
            let btn_gap = 0.08f32;
            let btn_y = 0f32;
            let btn_icons_arr = [*res.icon_back, *res.icon_retry, *res.icon_resume];
            let btn_disabled = [false, no_retry, self.dead];
            let mut btn_rects = [Rect::default(); 3];
            let spacing = btn_s * 2. + btn_gap;
            for i in 0..3 {
                let x = (i as f32 - 1.) * spacing;
                let r = Rect::new(x - btn_s, btn_y - btn_s, btn_s * 2., btn_s * 2.);
                btn_rects[i] = r;
                let color = if btn_disabled[i] { semi_white(alpha * 0.3) } else { semi_white(alpha) };
                let icon_r = r.feather(0.012);
                // 手动计算保持纹理比例的绘制区域，确保不拉伸
                let tex = btn_icons_arr[i];
                let tex_ratio = tex.width() / tex.height();
                let draw_r = if tex_ratio > 1. {
                    let h = icon_r.w / tex_ratio;
                    Rect::new(icon_r.x, icon_r.y + (icon_r.h - h) / 2., icon_r.w, h)
                } else {
                    let w = icon_r.h * tex_ratio;
                    Rect::new(icon_r.x + (icon_r.w - w) / 2., icon_r.y, w, icon_r.h)
                };
                ui.fill_rect(draw_r, (tex, draw_r, ScaleType::Fit, color));
            }

            if res.config.interactive {
                let mut clicked = None;
                for touch in Judge::get_touches() {
                    if touch.phase != TouchPhase::Started {
                        continue;
                    }
                    let p = touch.position;
                    let p = Point::new(p.x, p.y);
                    for i in 0..3 {
                        let br = btn_rects[i];
                        if p.x >= br.x && p.x <= br.right() && p.y >= br.y && p.y <= br.bottom() {
                            clicked = Some(i as i32 - 1);
                            break;
                        }
                    }
                }
                if no_retry && clicked == Some(0) || self.dead && clicked == Some(1) {
                    clicked = None;
                }
                let mut pos = self.music.position();
                if self.mode == GameMode::Exercise {
                    pos = tm.now();
                }
                if clicked.is_some_and(|it| it != -1) && (tm.speed - res.config.speed as f64).abs() > 0.01 {
                    debug!("recreating music");
                    self.music = res.audio.create_music(
                        res.music.clone(),
                        MusicParams {
                            amplifier: res.config.volume_music as _,
                            playback_rate: res.config.speed as _,
                            ..Default::default()
                        },
                    )?;
                }
                match clicked {
                    Some(-1) => {
                        back_sound();
                        self.should_exit = true;
                        #[cfg(target_env = "ohos")]
                        miniquad::native::set_interceptor_state(false);
                    }
                    Some(0) => {
                        reset!(self, res, tm);
                        if self.mode == GameMode::Exercise {
                            self.judge.advance_to(&mut self.chart, self.exercise_range.start);
                        }
                        #[cfg(target_env = "ohos")]
                        miniquad::native::set_interceptor_state(true);
                    }
                    Some(1) => {
                        if self.mode == GameMode::Exercise && (tm.now() > self.exercise_range.end || tm.now() < self.exercise_range.start) {
                            tm.seek_to(self.exercise_range.start);
                            self.music.seek_to(self.exercise_range.start)?;
                            pos = self.exercise_range.start;
                        }
                        self.music.play()?;
                        res.time -= 3.;
                        let dst = pos - 3.;
                        if dst < 0. {
                            self.music.pause()?;
                            self.state = State::BeforeMusic;
                        } else {
                            self.music.seek_to(dst)?;
                        }
                        let now = tm.now();
                        tm.speed = res.config.speed as _;
                        tm.resume();
                        tm.seek_to(now - 3.);
                        self.pause_rewind = Some(tm.now() - 0.2);
                        #[cfg(target_env = "ohos")]
                        miniquad::native::set_interceptor_state(true);
                    }
                    _ => {}
                }
            }
            if self.mode == GameMode::Exercise {
                let asp = self.touch_scale();
                for touch in ui.ensure_touches() {
                    touch.position *= asp;
                }
                // 将练习模式控制元素放在播放按钮下方，避免与统计面板和谱面重叠
                ui.dy(-0.22);
                ui.scope(|ui| {
                    ui.dx(0.3);
                    ui.dy(-0.28);
                    ui.slider(tl!("speed"), 0.5..2.0, 0.05, &mut self.res.config.speed, Some(0.5));
                });
                ui.dy(0.06);
                let hw = 0.7;
                let h = 0.06;
                let eh = 0.12;
                let rad = 0.03;
                let sp = self.offset().min(0.) as f64;
                ui.fill_rect(Rect::new(-hw, -h, hw * 2., h * 2.), GRAY);
                let st = -hw + ((self.exercise_range.start - sp) / (self.res.track_length - sp)) as f32 * hw * 2.;
                let en = -hw + ((self.exercise_range.end - sp) / (self.res.track_length - sp)) as f32 * hw * 2.;
                let t = tm.now();
                let cur = -hw + ((t - sp) / (self.res.track_length - sp)) as f32 * hw * 2.;
                ui.fill_rect(Rect::new(st, -h, en - st, h * 2.), WHITE);
                ui.fill_rect(Rect::new(st, -eh, 0., eh + h).feather(0.005), BLUE);
                ui.fill_circle(st, -eh, rad, BLUE);
                if self.exercise_press.is_none() {
                    let r = ui.rect_to_global(Rect::new(st, -eh, 0., 0.).feather(rad));
                    self.exercise_press = Judge::get_touches()
                        .iter()
                        .find(|it| it.phase == TouchPhase::Started && r.contains(it.position))
                        .map(|it| (-1, it.id));
                }
                ui.fill_rect(Rect::new(en, -h, 0., eh + h).feather(0.005), RED);
                ui.fill_circle(en, eh, rad, RED);
                if self.exercise_press.is_none() {
                    let r = ui.rect_to_global(Rect::new(en, eh, 0., 0.).feather(rad));
                    self.exercise_press = Judge::get_touches()
                        .iter()
                        .find(|it| it.phase == TouchPhase::Started && r.contains(it.position))
                        .map(|it| (1, it.id));
                }
                ui.fill_rect(Rect::new(cur, -h, 0., h * 2.).feather(0.005), GREEN);
                ui.fill_circle(cur, 0., rad, GREEN);
                if self.exercise_press.is_none() {
                    let r = ui.rect_to_global(Rect::new(cur, 0., 0., 0.).feather(rad));
                    self.exercise_press = Judge::get_touches()
                        .iter()
                        .find(|it| it.phase == TouchPhase::Started && r.contains(it.position))
                        .map(|it| (0, it.id));
                }
                ui.text(fmt_time(t as f32)).pos(0., 0.14).anchor(0.5, 0.).size(0.8).color(Color::new(1., 1., 1., 0.5)).draw();
                if let Some((ctrl, id)) = &self.exercise_press {
                    if let Some(touch) = Judge::get_touches().iter().rfind(|it| it.id == *id) {
                        let x = touch.position.x;
                        let p = (x + hw) as f64 / (hw * 2.) as f64 * (self.res.track_length - sp) + sp;
                        let p = if self.res.track_length - sp <= 3. || *ctrl == 0 {
                            p.clamp(sp, self.res.track_length)
                        } else {
                            p.clamp(
                                if *ctrl == -1 { sp } else { self.exercise_range.start + 3. },
                                if *ctrl == -1 {
                                    self.exercise_range.end - 3.
                                } else {
                                    self.res.track_length
                                },
                            )
                        };
                        if *ctrl == 0 {
                            tm.seek_to(p);
                            self.music.seek_to(p)?;
                            self.bad_notes.clear();
                            self.judge.reset();
                            self.chart.reset();
                            self.res.judge_line_color = self.res.res_pack.info.color_perfect();
                        } else {
                            *(if *ctrl == -1 {
                                &mut self.exercise_range.start
                            } else {
                                &mut self.exercise_range.end
                            }) = p;
                        }
                        if matches!(touch.phase, TouchPhase::Cancelled | TouchPhase::Ended) {
                            self.exercise_press = None;
                        }
                    }
                }
                ui.dy(-0.14);
                let r = ui.text(tl!("to")).size(0.8).anchor(0.5, 0.).draw();
                let mut tx = ui
                    .text(fmt_time(self.exercise_range.start as f32))
                    .pos(r.x - 0.02, 0.)
                    .anchor(1., 0.)
                    .size(0.8)
                    .color(BLACK);
                let re = tx.measure();
                self.exercise_btns.0.set(tx.ui, re);
                tx.ui
                    .fill_rect(re.feather(0.01), Color::new(1., 1., 1., if self.exercise_btns.0.touching() { 0.5 } else { 1. }));
                tx.draw();

                let mut tx = ui
                    .text(fmt_time(self.exercise_range.end as f32))
                    .pos(r.right() + 0.02, 0.)
                    .size(0.8)
                    .color(BLACK);
                let re = tx.measure();
                self.exercise_btns.1.set(tx.ui, re);
                tx.ui
                    .fill_rect(re.feather(0.01), Color::new(1., 1., 1., if self.exercise_btns.1.touching() { 0.5 } else { 1. }));
                tx.draw();
                for touch in ui.ensure_touches() {
                    touch.position /= asp;
                }
            }
            ui.alpha = old_alpha;
        }
        if let Some(time) = self.pause_rewind {
            let dt = tm.now() - time;
            let t = 3 - dt.floor() as i32;
            if t <= 0 {
                self.pause_rewind = None;
            } else {
                let a = (1. - dt as f32 / 3.) * 1.;
                let h = 1. / self.res.aspect_ratio;
                draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., a));
                ui.text(t.to_string()).anchor(0.5, 0.5).size(1.).color(c).draw();
            }
        }
        if self.res.config.touch_debug {
            for touch in Judge::get_touches() {
                ui.fill_circle(touch.position.x, touch.position.y, 0.04, Color { a: 0.4, ..RED });
            }
        }
        for pos in &self.touch_points {
            ui.fill_circle(pos.0, pos.1, 0.04, Color { a: 0.4, ..BLUE });
        }
        Ok(())
    }

    fn interactive(res: &Resource, state: &State) -> bool {
        res.config.interactive && matches!(state, State::Playing)
    }

    fn offset(&self) -> f32 {
        self.chart.offset + self.res.config.offset + self.info_offset
    }

    fn tweak_offset(&mut self, ui: &mut Ui, ita: bool) {
        ui.scope(|ui| {
            let width = 0.55;
            let height = 0.4;
            ui.dx(1. - width - 0.02);
            ui.dy(ui.top - height - 0.02);
            ui.fill_rect(Rect::new(0., 0., width, height), GRAY);
            ui.dy(0.02);
            ui.text(tl!("adjust-offset")).pos(width / 2., 0.).anchor(0.5, 0.).size(0.7).draw();
            ui.dy(0.16);
            let r = ui
                .text(format!("{}ms", (self.info_offset * 1000.).round() as i32))
                .pos(width / 2., 0.)
                .anchor(0.5, 0.)
                .size(0.6)
                .no_baseline()
                .draw();
            let d = 0.14;
            if ui.button("lg_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.026), "-") && ita {
                self.info_offset -= 0.05;
            }
            if ui.button("lg_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.026), "+") && ita {
                self.info_offset += 0.05;
            }
            let d = 0.08;
            if ui.button("sm_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.022), "-") && ita {
                self.info_offset -= 0.005;
            }
            if ui.button("sm_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.022), "+") && ita {
                self.info_offset += 0.005;
            }
            let d = 0.03;
            if ui.button("ti_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.017), "-") && ita {
                self.info_offset -= 0.001;
            }
            if ui.button("ti_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.017), "+") && ita {
                self.info_offset += 0.001;
            }
            ui.dy(0.14);
            let pad = 0.02;
            let spacing = 0.01;
            let mut r = Rect::new(pad, 0., (width - pad * 2. - spacing * 2.) / 3., 0.06);
            if ui.button("cancel", r, tl!("offset-cancel")) {
                self.next_scene = Some(NextScene::PopWithResult(Box::new(None::<f32>)));
            }
            r.x += r.w + spacing;
            if ui.button("reset", r, tl!("offset-reset")) {
                self.info_offset = 0.;
            }
            r.x += r.w + spacing;
            if ui.button("save", r, tl!("offset-save")) {
                self.next_scene = Some(NextScene::PopWithResult(Box::new(Some(self.info_offset))));
            }
        });
    }

    pub fn get_avg_fps(&self) -> Option<f32> {
        if self.fps_frame_count > 0 && self.fps_total_time > 0.0 {
            Some(self.fps_frame_count as f32 / self.fps_total_time as f32)
        } else {
            None
        }
    }
}

impl Scene for GameScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        on_game_start();
        #[cfg(target_env = "ohos")]
        miniquad::native::set_interceptor_state(true);
        super::set_ime_enabled(false);
        self.music = Self::new_music(&mut self.res)?;
        self.res.camera.render_target = target;
        tm.speed = self.res.config.speed as _;
        tm.adjust_time = self.res.config.adjust_time;
        reset!(self, self.res, tm);
        set_camera(&self.res.camera);
        self.first_in = true;
        Ok(())
    }

    fn pause(&mut self, tm: &mut TimeManager) -> Result<()> {
        if !tm.paused() {
            self.pause_rewind = None;
            self.music.pause()?;
            tm.pause();
            suspend_sound();
        }
        #[cfg(target_env = "ohos")]
        miniquad::native::set_interceptor_state(false);
        Ok(())
    }

    fn resume(&mut self, tm: &mut TimeManager) -> Result<()> {
        if !matches!(self.state, State::Playing) {
            tm.resume();
        }
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.res.audio.recover_if_needed()?;
        // 更新暂停界面渐变
        let target = if tm.paused() { 1. } else { 0. };
        self.pause_alpha += (target - self.pause_alpha) * 0.15;
        if matches!(self.state, State::Playing) {
            tm.update(self.music.position());
        }
        if self.mode == GameMode::Exercise && tm.now() > self.exercise_range.end && !tm.paused() {
            let state = self.state.clone();
            reset!(self, self.res, tm);
            self.state = state;
            tm.seek_to(self.exercise_range.start);
            tm.pause();
            self.music.pause()?;
            #[cfg(target_env = "ohos")]
            miniquad::native::set_interceptor_state(false);
        }
        let offset = self.offset();
        let time = tm.now();
        let time = match self.state {
            State::Starting => {
                if time >= Self::BEFORE_TIME {
                    self.res.alpha = 1.;
                    self.state = State::BeforeMusic;
                    tm.reset();
                    tm.seek_to(if self.mode == GameMode::Exercise {
                        self.exercise_range.start
                    } else {
                        offset.min(0.) as f64
                    });
                    self.last_update_time = tm.real_time();
                    if self.first_in && self.mode == GameMode::Exercise {
                        tm.pause();
                        self.first_in = false;
                    }
                    tm.now()
                } else {
                    #[cfg(target_os = "windows")]
                    {
                        let emitter_config = self.res.emitter.emitter.config.clone();
                        let emitter_square_config = self.res.emitter.emitter_square.config.clone();
                        self.res.emitter.emitter.config.size = 0.0;
                        self.res.emitter.emitter_square.config.size = 0.0;
                        self.res.emitter.emitter.emit(vec2(0.0, 0.0), 1);
                        self.res.emitter.emitter_square.emit(vec2(0.0, 0.0), 1);
                        self.res.emitter.emitter.config = emitter_config;
                        self.res.emitter.emitter_square.config = emitter_square_config;
                    }
                    self.res.alpha = (1. - (1. - time / Self::BEFORE_TIME).powi(3)) as f32;
                    if self.mode == GameMode::Exercise {
                        self.exercise_range.start
                    } else {
                        offset as f64
                    }
                }
            }
            State::BeforeMusic => {
                if time >= 0.0 {
                    self.music.seek_to(time)?;
                    if !tm.paused() {
                        self.music.play()?;
                    }
                    self.state = State::Playing;
                }
                time
            }
            State::Playing => {
                if time > self.res.track_length + WAIT_TIME {
                    self.state = State::Ending;
                    #[cfg(target_env = "ohos")]
                    miniquad::native::set_interceptor_state(false);
                }
                time
            }
            State::Ending => {
                let t = time - self.res.track_length - WAIT_TIME;
                if t >= AFTER_TIME + 0.3 {
                    let mut record_data = None;
                    #[cfg(closed)]
                    if let Some(upload_fn) = &self.upload_fn {
                        if !self.res.config.offline_mode
                            && !self.res.config.mods.intersects(Mods::UNRATED)
                            && !self.res.config.use_keyboard
                            && self.res.config.speed >= 1.0 - 1e-3
                        {
                            if let Some(player) = &self.player {
                                if let Some(chart) = &self.res.info.id {
                                    record_data = Some(encode_record(self, player.id, *chart));
                                }
                            }
                        }
                    }
                    let result = self.judge.result();
                    let record = if self.res.config.mods.intersects(Mods::UNRATED) || self.res.config.speed < 1.0 - 1e-3 {
                        None
                    } else {
                        Some(SimpleRecord {
                            score: result.score as _,
                            accuracy: result.accuracy as _,
                            full_combo: result.max_combo == result.num_of_notes,
                        })
                    };
                    self.next_scene = match self.mode {
                        GameMode::Normal | GameMode::NoRetry | GameMode::View => {
                            let historic_best = self.player.as_ref().map_or(0, |it| it.historic_best);
                            if let Some(new_rec) = &record {
                                if let Some(f) = &self.save_fn {
                                    f(new_rec.clone())?;
                                }
                                if let Some(best) = &mut self.best_record {
                                    best.update(new_rec);
                                } else {
                                    self.best_record = record.clone();
                                }
                                if let Some(best) = &self.best_record {
                                    if let Some(player) = &mut self.player {
                                        player.historic_best = player.historic_best.max(best.score as _);
                                    }
                                }
                            }
                            Some(NextScene::Overlay(Box::new(EndingScene::new(
                                self.res.background.clone(),
                                self.res.illustration.clone(),
                                self.res.player.clone(),
                                self.res.icons.clone(),
                                self.res.arc_icon.clone(),
                                self.res.icon_retry.clone(),
                                self.res.icon_proceed.clone(),
                                self.res.mod_icons.clone(),
                                self.res.info.clone(),
                                self.judge.result(),
                                &self.res.config,
                                self.res.res_pack.ending.clone(),
                                self.upload_fn.as_ref().map(Arc::clone),
                                self.player.as_ref().map(|it| it.rks),
                                historic_best,
                                record_data,
                                self.best_record.clone(),
                                if self.res.config.show_avg_fps { self.get_avg_fps() } else { None },
                            )?)))
                        }
                        GameMode::TweakOffset => Some(NextScene::PopWithResult(Box::new(None::<f32>))),
                        GameMode::Exercise => None,
                    };
                }
                self.res.alpha = (1. - (t / AFTER_TIME).min(1.).powi(2)) as f32;
                self.res.track_length
            }
        };
        let time = (time - offset as f64).max(0.);
        self.res.time = time;
        if !tm.paused() && self.pause_rewind.is_none() && self.mode != GameMode::View {
            self.gl.quad_gl.viewport(self.res.camera.viewport);
            self.judge.update(&mut self.res, &mut self.chart, &mut self.bad_notes);
            self.gl.quad_gl.viewport(None);
        }
        if let Some(update) = &mut self.update_fn {
            update(self.res.time, &mut self.res, &mut self.judge);
        }
        let counts = self.judge.counts();
        self.res.judge_line_color = if counts[2] + counts[3] == 0 && self.res.config.ap_fc_indicator {
            if counts[1] == 0 {
                self.res.res_pack.info.color_perfect()
            } else {
                self.res.res_pack.info.color_good()
            }
        } else {
            WHITE
        };
        if !self.dead
            && matches!(self.state, State::Playing)
            && (self.res.config.mods.contains(Mods::INSTANT_DEATH_AP) && counts[1] + counts[2] + counts[3] > 0
                || self.res.config.mods.contains(Mods::INSTANT_DEATH_FC) && counts[2] + counts[3] > 0)
        {
            if !self.music.paused() {
                self.music.pause()?;
            }
            tm.pause();
            self.dead = true;
            #[cfg(target_env = "ohos")]
            miniquad::native::set_interceptor_state(false);
            show_message(tl!("game-over")).error();
        }
        self.res.judge_line_color.a *= self.res.alpha;
        self.chart.update(&mut self.res);
        let res = &mut self.res;
        if res.config.interactive && is_key_pressed(KeyCode::Space) {
            if tm.paused() {
                if matches!(self.state, State::Playing) {
                    self.music.play()?;
                    tm.resume();
                }
            } else if matches!(self.state, State::Playing | State::BeforeMusic) {
                if !self.music.paused() {
                    self.music.pause()?;
                }
                tm.pause();
                suspend_sound();
                self.pause_need_blur = true;
            }
        }
        if Self::interactive(res, &self.state) {
            if is_key_pressed(KeyCode::Left) && res.config.use_keyboard {
                res.time -= 1.;
                let dst = (self.music.position() - 1.).max(0.);
                self.music.seek_to(dst)?;
                tm.seek_to(dst);
            }
            if is_key_pressed(KeyCode::Right) && res.config.use_keyboard {
                res.time += 5.;
                let dst = (self.music.position() + 5.).min(res.track_length);
                self.music.seek_to(dst)?;
                tm.seek_to(dst);
            }
            if is_key_pressed(KeyCode::Q) {
                self.should_exit = true;
            }
        }
        for e in &mut self.effects {
            e.update(&self.res);
        }
        if let Some((id, text)) = take_input() {
            let offset = self.offset().min(0.);
            match id.as_str() {
                "exercise_start" => {
                    if let Some(t) = parse_time(&text) {
                        if !(offset as f64..self.res.track_length.min(self.exercise_range.end - 3.).max(offset as f64)).contains(&t) {
                            show_message(tl!("ex-time-out-of-range")).error();
                        } else {
                            self.exercise_range.start = t;
                            show_message(tl!("ex-time-set")).ok();
                        }
                    } else {
                        show_message(tl!("ex-invalid-format")).error();
                    }
                }
                "exercise_end" => {
                    if let Some(t) = parse_time(&text) {
                        if !((self.exercise_range.start + 3.).max(offset as f64).min(self.res.track_length)..self.res.track_length).contains(&t) {
                            show_message(tl!("ex-time-out-of-range")).error();
                        } else {
                            self.exercise_range.end = t;
                            show_message(tl!("ex-time-set")).ok();
                        }
                    } else {
                        show_message(tl!("ex-invalid-format")).error();
                    }
                }
                _ => return_input(id, text),
            }
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if self.mode == GameMode::Exercise && tm.paused() {
            let touch = Touch {
                position: touch.position * self.touch_scale(),
                ..touch.clone()
            };
            if self.exercise_btns.0.touch(&touch) {
                request_input("exercise_start", InputBox::new().default_text(fmt_time(self.exercise_range.start as f32)));
                return Ok(true);
            }
            if self.exercise_btns.1.touch(&touch) {
                request_input("exercise_end", InputBox::new().default_text(fmt_time(self.exercise_range.end as f32)));
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        if self.res.config.show_avg_fps {
            let current_time = tm.real_time();
            if matches!(self.state, State::Playing) && !tm.paused() {
                let frame_delta = current_time - self.fps_last_frame_time;
                self.fps_total_time += frame_delta;
                self.fps_frame_count += 1;
            }
            self.fps_last_frame_time = current_time;
        }

        let res = &mut self.res;
        let asp = ui.viewport.2 as f32 / ui.viewport.3 as f32;
        if res.update_size(ui.viewport) || self.mode == GameMode::View {
            set_camera(&res.camera);
        }

        let msaa = res.config.sample_count > 1;

        let chart_onto = res
            .chart_target
            .as_ref()
            .map(|it| if msaa { it.input() } else { it.output() })
            .or(res.camera.render_target);
        push_camera_state();
        set_camera(&Camera2D {
            zoom: vec2(1., -asp),
            viewport: if res.chart_target.is_some() { None } else { Some(ui.viewport) },
            render_target: chart_onto,
            ..Default::default()
        });
        clear_background(BLACK);
        draw_background(*res.background);
        pop_camera_state();

        let chart_target_vp = if res.chart_target.is_some() {
            let vp = res.camera.viewport.unwrap();
            Some((vp.0 - ui.viewport.0, vp.1 - ui.viewport.1, vp.2, vp.3))
        } else {
            res.camera.viewport
        };
        self.gl.quad_gl.render_pass(chart_onto.map(|it| it.render_pass));
        self.gl.quad_gl.viewport(chart_target_vp);

        let h = 1. / res.aspect_ratio;
        draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., res.alpha * res.info.background_dim));

        self.chart.render(ui, res);

        self.gl.quad_gl.render_pass(
            res.chart_target
                .as_ref()
                .map(|it| it.output().render_pass)
                .or_else(|| res.camera.render_pass()),
        );

        self.bad_notes.retain(|dummy| dummy.render(res));
        let t = tm.real_time();
        let dt = (t - std::mem::replace(&mut self.last_update_time, t)) as f32;
        if res.config.particle {
            res.emitter.draw(dt);
        }

        let combo_debug = res.config.combo_text_debug();
        let combo = self.judge.combo();
        let alpha = res.alpha;
        let top = -1.0 / res.aspect_ratio;
        let eps = 2e-2 / res.aspect_ratio;
        let is_starting = matches!(self.state, State::Starting);
        let combo_top = top + eps * 2.0 - (1.0 - if is_starting { 1.0 } else { 0.0 }) * 0.4;
        let watermark_text = res.config.custom_watermark().to_string();

        self.ui(ui, tm)?;

        if combo_debug {
            let debug_info = format!(
                "Combo: {} | Pos: ({:.3}, {:.3}) | Size: 1.0 | Color: ({:.2}, {:.2}, {:.2}, {:.2})",
                combo,
                0.0,
                combo_top + 0.02,
                WHITE.r, WHITE.g, WHITE.b, alpha
            );

            ui.text(&debug_info)
                .pos(0.5, -0.40)
                .anchor(0.5, 0.5)
                .size(0.35)
                .color(semi_white(0.9 * alpha))
                .draw();

            let marker_size = 0.02;
            ui.fill_rect(
                Rect::new(-marker_size, combo_top - marker_size, marker_size * 2.0, marker_size * 2.0),
                Color::new(1.0, 0.0, 0.0, 0.8 * alpha),
            );
            ui.fill_rect(
                Rect::new(-0.05, combo_top - 0.001, 0.1, 0.002),
                Color::new(1.0, 0.0, 0.0, 0.5 * alpha),
            );
            ui.fill_rect(
                Rect::new(-0.001, combo_top - 0.05, 0.002, 0.1),
                Color::new(1.0, 0.0, 0.0, 0.5 * alpha),
            );
        }

        self.overlay_ui(ui, tm)?;

        if !watermark_text.is_empty() {
            let top = ui.top;
            ui.text(&watermark_text)
                .pos(0., top - 0.02)
                .anchor(0.5, 1.)
                .size(0.32)
                .color(Color::new(1., 1., 1., 0.3))
                .draw_using(&PGR_FONT);
        }

        if self.mode == GameMode::TweakOffset {
            push_camera_state();
            self.gl.quad_gl.viewport(None);
            set_camera(&Camera2D {
                zoom: vec2(1., -screen_aspect()),
                render_target: self.res.chart_target.as_ref().map(|it| it.output()).or(self.res.camera.render_target),
                ..Default::default()
            });
            self.tweak_offset(ui, Self::interactive(&self.res, &self.state));
            pop_camera_state();
        }

        if !self.res.no_effect && !self.effects.is_empty() {
            push_camera_state();
            set_camera(&Camera2D {
                zoom: vec2(1., asp),
                ..Default::default()
            });
            for e in &self.effects {
                e.render(&mut self.res);
            }
            pop_camera_state();
        }
        if msaa || !self.res.no_effect {
            if let Some(target) = &self.res.chart_target {
                self.gl.flush();
                push_camera_state();
                self.gl.quad_gl.viewport(None);
                set_camera(&Camera2D {
                    zoom: vec2(1., asp),
                    render_target: self.res.camera.render_target,
                    viewport: Some(ui.viewport),
                    ..Default::default()
                });
                draw_texture_ex(
                    target.output().texture,
                    -1.,
                    -ui.top,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(2., ui.top * 2.)),
                        ..Default::default()
                    },
                );
                pop_camera_state();
            }
        }
        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        if self.should_exit {
            super::set_ime_enabled(true);
            if tm.paused() {
                tm.resume();
            }
            tm.speed = 1.0;
            tm.adjust_time = false;
            match self.mode {
                GameMode::Normal => {
                    if let Some(rec) = &self.best_record {
                        NextScene::PopWithResult(Box::new(rec.clone()))
                    } else {
                        NextScene::Pop
                    }
                }
                GameMode::Exercise | GameMode::NoRetry | GameMode::View => NextScene::Pop,
                GameMode::TweakOffset => NextScene::PopWithResult(Box::new(None::<f32>)),
            }
        } else if let Some(next_scene) = self.next_scene.take() {
            if !matches!(next_scene, NextScene::None) && tm.paused() {
                tm.resume();
            }
            tm.speed = 1.0;
            tm.adjust_time = false;
            next_scene
        } else {
            NextScene::None
        }
    }
}
