//! 进入主页前的启动页(登录画面)。
//! 流程:纯黑屏 + 播放 login.mp3 持续 6.5 秒 → 闪光一下 → 显示画面(高斯模糊的
//! 随机 loginbg 背景 + PhirLie 标题 + 版本号 + Tip)→ 提示点击继续 → 若尚未选择
//! 过语言则弹出语言选择面板,否则直接进入加载主页的加载页。所有界面切换都带
//! 淡入/淡出动画。

use super::StartupLoadingScene;
use crate::blue_archive_tips::BLUE_ARCHIVE_TIPS;
use crate::{get_data, get_data_mut, save_data, sync_data};
use prpr::{
    config::Config,
    ext::{create_audio_manger, semi_black, semi_white, SafeTexture, ScaleType, BLACK_TEXTURE},
    scene::{NextScene, Scene},
    task::Task,
    time::TimeManager,
    ui::{button_hit, FontArc, RectButton, Ui, PREFER_REDUCED_MOTION},
};
use prpr_l10n::{LANG_IDENTS, LANG_NAMES};
use anyhow::Result;
use macroquad::prelude::*;
use sasa::{AudioClip, AudioManager, Music, MusicParams};
use std::sync::atomic::Ordering;
use tracing::info;
use ::rand::{seq::SliceRandom, thread_rng};

/// 纯黑屏时长(只播音乐)
const BLACK_TIME: f32 = 6.5;
/// 黑屏结束后的白色闪光时长(渐入渐出)
const FLASH_TIME: f32 = 0.18;
/// 画面显示后到"点击继续"提示出现的时长
const SHOW_TIME: f32 = 0.8;
/// 画面/文字淡入时长
const FADE_IN_TIME: f32 = 0.35;
/// 切换前画面淡出时长
const FADE_OUT_TIME: f32 = 0.3;
/// 语言选择面板弹出动画时长
const LANG_ANIM_TIME: f32 = 0.22;
/// 进入语言选择后忽略点击的冷却时长(避免上一阶段的点击误触)
const LANG_COOLDOWN: f32 = 0.3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Black,
    Show,
    LanguageSelect,
    FadeOut,
}

type BlurredBg = (u16, u16, Vec<u8>);

pub struct LoginScene {
    fallback: FontArc,
    bg_task: Option<Task<Result<BlurredBg>>>,
    music_task: Option<Task<Result<Vec<u8>>>>,
    background: SafeTexture,
    audio: Option<AudioManager>,
    bgm: Option<Music>,
    enter_time: f32,
    tip: &'static str,
    phase: Phase,
    lang_selected: Option<usize>,
    lang_btns: Vec<RectButton>,
    lang_enter_time: f32,
    lang_cooldown_until: f32,
    fade_out_time: f32,
    pending_scene: Option<NextScene>,
}

async fn load_blurred_bg(path: String) -> Result<BlurredBg> {
    let bytes = load_file(&path).await?;
    let img = image::load_from_memory(&bytes)?;
    let img = img.thumbnail(512, 512);
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let mut pixels: Vec<[u8; 3]> = rgb.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    fastblur::gaussian_blur(&mut pixels, w, h, 30.0);
    let flat: Vec<u8> = pixels.into_iter().flat_map(|p| p.to_vec()).collect();
    let mut rgba = Vec::with_capacity(w * h * 4);
    for chunk in flat.chunks_exact(3) {
        rgba.extend_from_slice(chunk);
        rgba.push(255);
    }
    Ok((w as u16, h as u16, rgba))
}

impl LoginScene {
    pub fn new(fallback: FontArc) -> Self {
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir("assets/loginbg") {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("bg") {
                    files.push(format!("loginbg/{name}"));
                }
            }
        }
        files.sort();
        let bg_path = files
            .choose(&mut thread_rng())
            .cloned()
            .unwrap_or_else(|| "loginbg/bg1.jpg".to_owned());

        let bg_task = Task::new(async move {
            match load_blurred_bg(bg_path.clone()).await {
                Ok(bg) => Ok(bg),
                Err(e) => {
                    info!("startup bg load failed for {bg_path}: {e:?}, falling back");
                    load_blurred_bg("background.jpg".to_owned()).await
                }
            }
        });

        let custom_bgm = get_data().custom_startup_bgm_path.clone();
        let music_task = Task::new(async move {
            match custom_bgm.as_deref() {
                Some(path) => match std::fs::read(path) {
                    Ok(data) => Ok(data),
                    Err(_) => load_file("bgm/login.mp3").await.map_err(Into::into),
                },
                None => load_file("bgm/login.mp3").await.map_err(Into::into),
            }
        });

        let tip = BLUE_ARCHIVE_TIPS.choose(&mut thread_rng()).copied().unwrap_or("老师,欢迎回来!");

        Self {
            fallback,
            bg_task: Some(bg_task),
            music_task: Some(music_task),
            background: BLACK_TEXTURE.clone(),
            audio: None,
            bgm: None,
            enter_time: f32::NAN,
            tip,
            phase: Phase::Black,
            lang_selected: get_data()
                .language
                .as_ref()
                .and_then(|it| it.parse::<prpr_l10n::LanguageIdentifier>().ok())
                .and_then(|ident| LANG_IDENTS.iter().position(|it| *it == ident)),
            lang_btns: LANG_NAMES.iter().map(|_| RectButton::new()).collect(),
            lang_enter_time: f32::NAN,
            lang_cooldown_until: f32::NEG_INFINITY,
            fade_out_time: f32::NAN,
            pending_scene: None,
        }
    }

    fn start_fade_out(&mut self, now: f32, scene: Box<dyn Scene>) {
        self.phase = Phase::FadeOut;
        self.fade_out_time = now;
        self.pending_scene = Some(NextScene::Replace(scene));
    }
}

impl Scene for LoginScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.enter_time.is_nan() {
            self.enter_time = tm.now() as f32;
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        let now = tm.now() as f32;
        match self.phase {
            Phase::Black => Ok(true),
            Phase::Show => {
                // 点击继续提示出现后才响应
                if now - self.enter_time >= BLACK_TIME + SHOW_TIME && touch.phase == TouchPhase::Ended {
                    button_hit();
                    if get_data().has_chosen_language {
                        let scene = StartupLoadingScene::new(self.fallback.clone());
                        self.start_fade_out(now, Box::new(scene));
                    } else {
                        self.phase = Phase::LanguageSelect;
                        self.lang_enter_time = now;
                        self.lang_cooldown_until = now + LANG_COOLDOWN;
                    }
                }
                Ok(true)
            }
            Phase::LanguageSelect => {
                if now < self.lang_cooldown_until {
                    return Ok(true);
                }
                for (i, btn) in self.lang_btns.iter_mut().enumerate() {
                    if btn.touch(touch) {
                        button_hit();
                        let data = get_data_mut();
                        data.language = Some(LANG_IDENTS[i].to_string());
                        data.has_chosen_language = true;
                        sync_data();
                        let _ = save_data();
                        let scene = StartupLoadingScene::new(self.fallback.clone());
                        self.start_fade_out(now, Box::new(scene));
                        return Ok(true);
                    }
                }
                Ok(true)
            }
            Phase::FadeOut => Ok(true),
        }
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        if let Some(task) = &mut self.bg_task {
            if let Some(res) = task.take() {
                self.bg_task = None;
                match res {
                    Ok((w, h, rgba)) => self.background = Texture2D::from_rgba8(w, h, &rgba).into(),
                    Err(e) => info!("startup bg failed: {e:?}"),
                }
            }
        }
        if let Some(task) = &mut self.music_task {
            if let Some(res) = task.take() {
                self.music_task = None;
                match res {
                    Ok(data) => {
                        let config = Config::default();
                        match create_audio_manger(&config).and_then(|mut audio| {
                            let clip = AudioClip::new(data)?;
                            let mut bgm = audio.create_music(
                                clip,
                                MusicParams {
                                    amplifier: 1.0,
                                    loop_mix_time: 0.0,
                                    ..Default::default()
                                },
                            )?;
                            Ok((audio, bgm))
                        }) {
                            Ok((audio, mut bgm)) => {
                                let _ = bgm.play();
                                self.audio = Some(audio);
                                self.bgm = Some(bgm);
                            }
                            Err(e) => info!("startup bgm failed: {e:?}"),
                        }
                    }
                    Err(e) => info!("startup bgm load failed: {e:?}"),
                }
            }
        }

        let t = tm.now() as f32;
        let elapsed = (t - self.enter_time).max(0.);
        if self.phase == Phase::Black && elapsed >= BLACK_TIME {
            self.phase = Phase::Show;
        }
        if let Some(audio) = &mut self.audio {
            let _ = audio.recover_if_needed();
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        set_camera(&ui.camera());
        let t = tm.now() as f32;
        let top = ui.top;
        let full = ui.screen_rect();

        if self.phase == Phase::Black {
            ui.fill_rect(full, BLACK);
            return Ok(());
        }

        let elapsed = (t - self.enter_time).max(0.);
        let show_elapsed = elapsed - BLACK_TIME;

        // 高斯模糊背景
        ui.fill_rect(full, (*self.background, full, ScaleType::CropCenter));

        // 遮罩(语言选择更暗)
        let dim = if self.phase == Phase::LanguageSelect { 0.55 } else { 0.3 };
        ui.fill_rect(full, semi_black(dim));

        // 画面淡入
        let fade_in = if PREFER_REDUCED_MOTION.load(Ordering::Relaxed) {
            1.
        } else {
            (show_elapsed / FADE_IN_TIME).clamp(0., 1.)
        };

        if self.phase == Phase::LanguageSelect {
            self.render_language_select(ui, t, top);
        } else {
            ui.alpha(fade_in, |ui| {
                ui.text("PhirLie")
                    .pos(0., -0.10)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(1.4)
                    .color(WHITE)
                    .draw();

                ui.text(concat!("v", env!("CARGO_PKG_VERSION")))
                    .pos(0., -0.10 + 1.4 * 0.4 + 0.06)
                    .anchor(0.5, 0.)
                    .size(0.4)
                    .color(semi_white(0.6))
                    .draw();

                // 点击继续提示(淡入)
                let hint_p = if PREFER_REDUCED_MOTION.load(Ordering::Relaxed) {
                    1.
                } else {
                    ((show_elapsed - SHOW_TIME) / 0.3).clamp(0., 1.)
                };
                if hint_p > 0. {
                    let blink = ((t * 2.0).sin() * 0.5 + 0.5) * 0.5 + 0.5;
                    ui.alpha(hint_p, |ui| {
                        ui.text("点击屏幕以继续")
                            .pos(0., 0.20)
                            .anchor(0.5, 0.)
                            .size(0.5)
                            .color(semi_white(blink))
                            .draw();
                    });
                }

                ui.text(&format!("Tip: {}", self.tip))
                    .pos(-0.95, top - 0.05)
                    .anchor(0., 1.)
                    .max_width(1.6)
                    .size(0.38)
                    .color(semi_white(0.75))
                    .draw();
            });
        }

        // 黑屏结束后的白色闪光(渐入渐出)
        if self.phase == Phase::Show && show_elapsed >= 0. && show_elapsed < FLASH_TIME {
            let p = show_elapsed / FLASH_TIME;
            let flash = (1. - (p * 2. - 1.).abs()).clamp(0., 1.);
            if flash > 0. {
                ui.fill_rect(full, Color::new(1., 1., 1., flash));
            }
        }

        // 淡出(切换前)
        if self.phase == Phase::FadeOut {
            let p = ((t - self.fade_out_time) / FADE_OUT_TIME).clamp(0., 1.);
            ui.fill_rect(full, Color::new(0., 0., 0., p));
        }

        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        if self.phase == Phase::FadeOut && tm.now() as f32 > self.fade_out_time + FADE_OUT_TIME {
            return self.pending_scene.take().unwrap_or_default();
        }
        NextScene::None
    }
}

impl LoginScene {
    fn render_language_select(&mut self, ui: &mut Ui, t: f32, _top: f32) {
        let p = if PREFER_REDUCED_MOTION.load(Ordering::Relaxed) {
            1.
        } else {
            ((t - self.lang_enter_time) / LANG_ANIM_TIME).clamp(0., 1.)
        };
        let ease = 1. - (1. - p).powi(3);
        let scale = 0.94 + 0.06 * ease;

        let panel_w = 0.7;
        let row_h = 0.056;
        let panel_h = 0.16 + LANG_NAMES.len() as f32 * row_h + 0.06;
        let ww = panel_w * scale;
        let wh = panel_h * scale;
        let panel = Rect::new(-ww / 2., -wh / 2., ww, wh);

        ui.alpha(ease, |ui| {
            ui.fill_rect(panel, Color::new(0.06, 0.07, 0.11, 0.92));

            ui.text("选择语言 / Select Language")
                .pos(panel.x, panel.y + 0.04)
                .anchor(0., 0.)
                .size(0.6 * scale)
                .color(WHITE)
                .draw();

            let mut y = panel.y + 0.15;
            for (i, name) in LANG_NAMES.iter().enumerate() {
                let r = Rect::new(panel.x + 0.04, y, panel.w - 0.08, row_h);
                self.lang_btns[i].set(ui, r);
                let selected = self.lang_selected == Some(i);
                if selected {
                    ui.fill_rect(r, semi_white(0.2));
                }
                let color = if selected { Color::new(1., 0.85, 0.3, 1.) } else { semi_white(0.9) };
                ui.text(*name)
                    .pos(r.center().x, r.center().y)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(0.4 * scale)
                    .color(color)
                    .draw();
                y += row_h;
            }
        });
    }
}
