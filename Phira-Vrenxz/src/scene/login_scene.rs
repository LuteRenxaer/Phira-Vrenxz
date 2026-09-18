
//! 启动页：黑屏 → 画面淡入 → 「点击继续」→ 交给首启向导或加载页。
//!
//! 语言选择原来挂在这一页的第二个阶段（LanguageSelect），现在整段搬进了首启向导
//! （`scene::SetupScene` 的第一步）：那一套要接着问登录、音量、其他设置，
//! 摆在同一块靠右的面板里才连得起来。

use super::{SetupScene, StartupLoadingScene};
use crate::blue_archive_tips::random_tip;
use crate::get_data;
prpr_l10n::tl_file!("login");
use prpr::{
    config::Config,
    ext::{create_audio_manger, semi_black, semi_white, SafeTexture, ScaleType, BLACK_TEXTURE},
    scene::{NextScene, Scene},
    task::Task,
    time::TimeManager,
    ui::{button_hit, FontArc, Ui, PREFER_REDUCED_MOTION},
};
use anyhow::Result;
use macroquad::prelude::*;
use sasa::{AudioClip, AudioManager, Music, MusicParams};
use std::sync::atomic::Ordering;
use tracing::info;
use ::rand::{seq::SliceRandom, thread_rng};

const BLACK_TIME: f32 = 1.5;
/// 黑屏结束后的白色闪光时长(渐入渐出)
const FLASH_TIME: f32 = 0.3;
/// 画面显示后到"点击继续"提示出现的时长
const SHOW_TIME: f32 = 0.8;
/// 画面/文字淡入时长
const FADE_IN_TIME: f32 = 0.35;
/// 切换前画面淡出时长
const FADE_OUT_TIME: f32 = 0.4;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Black,
    Show,
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
    tip: String,
    phase: Phase,
    fade_out_time: f32,
    pending_scene: Option<NextScene>,
}
//人类注释_这是一个普普通通的模糊效果
async fn load_blurred_bg(path: String) -> Result<BlurredBg> {
    let bytes = load_file(&path).await?;
    let img = image::load_from_memory(&bytes)?;
    let img = img.thumbnail(256, 256);
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let mut pixels: Vec<[u8; 3]> = rgb.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    fastblur::gaussian_blur(&mut pixels, w, h, 20.0);
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
                    load_blurred_bg("backgrounds/background.jpg".to_owned()).await
                }
            }
        });

        let custom_bgm = get_data().custom_startup_bgm_path.clone();
        let old_home = get_data().config.old_home;
        let music_task = Task::new(async move {
            let default_login = if old_home { "bgm/old/login.mp3" } else { "bgm/login.mp3" };
            match custom_bgm.as_deref() {
                Some(path) => match std::fs::read(path) {
                    Ok(data) => Ok(data),
                    Err(_) => load_file(default_login).await.map_err(Into::into),
                },
                None => load_file(default_login).await.map_err(Into::into),
            }
        });

        let tip = random_tip();

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
                    // 首次启动：先去向导把语言 / 登录 / 音量这些问完；走完向导（或者早就
                    // 走过、被迁移标记过的老玩家）直接进加载页。
                    let scene: Box<dyn prpr::scene::Scene> = if get_data().initial_setup_done {
                        Box::new(StartupLoadingScene::new(self.fallback.clone()))
                    } else {
                        Box::new(SetupScene::new(self.fallback.clone()))
                    };
                    self.start_fade_out(now, scene);
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
                            let bgm = audio.create_music(
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
        // 背景使用原始比例，不随 UI 比例缩放
        set_camera(&ui.bg_camera());
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

        // 遮罩
        ui.fill_rect(full, semi_black(0.3));

        // UI 使用带比例的 camera
        set_camera(&ui.camera());

        // 画面淡入
        let fade_in = if PREFER_REDUCED_MOTION.load(Ordering::Relaxed) {
            1.
        } else {
            (show_elapsed / FADE_IN_TIME).clamp(0., 1.)
        };

        ui.alpha(fade_in, |ui| {
            ui.text("Phira-Vrenxz")
                .pos(0., -0.10)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(1.4)
                .color(WHITE)
                .draw();

            ui.text(format!("v{}", env!("CARGO_PKG_VERSION")))
                .pos(0., top - 0.05)
                .anchor(0.5, 1.)
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
                    ui.text(tl!("startup-tap-to-continue"))
                        .pos(0., 0.20)
                        .anchor(0.5, 0.)
                        .size(0.5)
                        .color(semi_white(blink))
                        .draw();
                });
            }

            ui.text(tl!("startup-tip", "tip" => &self.tip))
                .pos(-0.95, top - 0.05)
                .anchor(0., 1.)
                .max_width(1.6)
                .size(0.38)
                .color(semi_white(0.75))
                .draw();
        });

        if self.phase == Phase::Show && show_elapsed >= 0. && show_elapsed < FLASH_TIME {
            let half = FLASH_TIME * 0.5;
            let flash = if show_elapsed < half {
                let t = show_elapsed / half;
                t * t
            } else {
                let t = (show_elapsed - half) / half;
                1. - t * t
            };
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

