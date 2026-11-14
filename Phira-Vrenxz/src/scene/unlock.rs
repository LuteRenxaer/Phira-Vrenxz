use anyhow::{bail, Context, Result};
use macroquad::prelude::*;
use prpr::{
    config::Config,
    core::{demux_audio, Anim, Keyframe, Video},
    ext::{create_audio_manger, semi_black, semi_white, SafeTexture, ScaleType},
    fs::FileSystem,
    info::ChartInfo,
    scene::{BasicPlayer, GameMode, LoadingScene, NextScene, SaveFn, Scene, UpdateFn, UploadFn},
    time::TimeManager,
    ui::LoadingParams,
};
use sasa::{AudioManager, Music, MusicParams};
use tracing::warn;

use std::sync::atomic::{AtomicU32, Ordering};

#[allow(unused)]
fn trace_log(msg: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(r"F:\RustProject\phirLie\unlock_trace.log")
    {
        let _ = writeln!(
            f,
            "{} {}",
            chrono::Utc::now().format("%H:%M:%S%.3f"),
            msg
        );
    }
}

#[derive(Debug)]
enum State {
    Before,
    Playing,
    Blanking,
    Loading,
    Transforming,
}

struct Bgm {
    audio_manager: AudioManager,
    music: Music,
}

pub struct UnlockScene {
    loading_scene: Option<Box<LoadingScene>>,
    game_scene: Option<Box<dyn Scene>>,
    next_scene: Option<NextScene>,

    render_target: Option<RenderTarget>,
    video: Option<Video>,
    bgm: Option<Bgm>,

    background: SafeTexture,

    state: State,
}

impl UnlockScene {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        mode: GameMode,
        info: ChartInfo,
        config: Config,
        mut fs: Box<dyn FileSystem>,
        player: Option<BasicPlayer>,
        upload_fn: Option<UploadFn>,
        update_fn: Option<UpdateFn>,
        save_fn: Option<SaveFn>,
    ) -> Result<UnlockScene> {
        let bytes = fs
            .load_file(&info.unlock_video.clone().unwrap_or_else(|| "unlock.mp4".to_owned()))
            .await?;
        let video = Some(Video::new(bytes, 0., ScaleType::Inside, Anim::new(vec![Keyframe::new(0., 1., 0)]), Anim::default())?);
        let clip = demux_audio(video.as_ref().unwrap().video_file().path().to_str().unwrap())?;

        let bgm = match clip {
            Some(clip) => {
                let mut audio_manager = create_audio_manger(&config)?;
                let music = audio_manager.create_music(
                    clip,
                    MusicParams {
                        amplifier: config.volume_music,
                        ..Default::default()
                    },
                )?;
                Some(Bgm { audio_manager, music })
            }
            None => None,
        };

        use prpr::ext::BLACK_TEXTURE;
        let background = match async {
            let image = image::load_from_memory(&fs.load_file(&info.illustration).await?)
                .context("Failed to decode image")?;
            let (w, h) = (image.width(), image.height());
            let size = w as usize * h as usize;
            let blurred_rgb = image.to_rgb8();

            let mut blurred = Vec::with_capacity(size * 4);
            for input in blurred_rgb.chunks_exact(3) {
                blurred.extend_from_slice(input);
                blurred.push(255);
            }
            let bg = Texture2D::from_image(&Image { width: w as _, height: h as _, bytes: blurred });
            Ok::<_, anyhow::Error>(SafeTexture::from(bg))
        }.await {
            Ok(bg) => bg,
            Err(err) => {
                warn!("Failed to load unlock background: {:?}", err);
                BLACK_TEXTURE.clone()
            }
        };
        let loading_scene = Some(Box::new(LoadingScene::new(mode, info, config, fs, player, None, upload_fn, update_fn, save_fn, None, None).await?));

        trace_log(&format!(
            "UnlockScene::new done bgm={} video_dur={}",
            bgm.is_some(),
            video.as_ref().map(|v| v.duration).unwrap_or(-1.0)
        ));

        Ok(UnlockScene {
            loading_scene,
            next_scene: None,
            game_scene: None,

            render_target: None,
            video: video,
            bgm,

            background,

            state: State::Before,
        })
    }
}

impl Scene for UnlockScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        self.render_target = target;
        tm.reset();
        Ok(())
    }

    fn pause(&mut self, tm: &mut TimeManager) -> Result<()> {
        tm.pause();
        if let Some(bgm) = &mut self.bgm {
            bgm.music.pause()?;
        }
        Ok(())
    }

    fn resume(&mut self, tm: &mut TimeManager) -> Result<()> {
        tm.resume();
        if let Some(bgm) = &mut self.bgm {
            bgm.music.play()?;
        }
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        if let Some(bgm) = &mut self.bgm {
            bgm.audio_manager.recover_if_needed()?;
        }

        if self.game_scene.is_none() {
            self.loading_scene.as_mut().unwrap().update(tm)?;
            let loading_next_scene = self.loading_scene.as_mut().unwrap().next_scene(tm);
            match loading_next_scene {
                NextScene::PopWithResult(_) => self.next_scene = Some(loading_next_scene),
                NextScene::Replace(game_scene) => self.game_scene = Some(game_scene),
                _ => (),
            }
        }

        let t = tm.now();
        static FRAME: AtomicU32 = AtomicU32::new(0);
        let f = FRAME.fetch_add(1, Ordering::Relaxed);
        if f % 60 == 0 {
            let pos = self.bgm.as_ref().map(|b| b.music.position()).unwrap_or(-1.0);
            trace_log(&format!(
                "update f={} state={:?} t={:.3} pos={:.3} game_scene={}",
                f,
                self.state,
                t,
                pos,
                self.game_scene.is_some()
            ));
        }
        match self.state {
            State::Before => {
                if t > 0.5 {
                    trace_log(&format!("state Before->Playing t={:.3}", t));
                    self.state = State::Playing;
                    tm.reset();
                    if let Some(bgm) = &mut self.bgm {
                        bgm.music.seek_to(0.)?;
                        bgm.music.play()?;
                    }
                }
            }
            State::Playing => {
                // 以视频时长为主判断结束；音频仅作伴奏，若其元数据异常（长度异常大/解析失败）
                // 不作为卡死条件，避免"游玩完后一直停在高 FPS 空转"的假死现象
                if t > self.video.as_ref().unwrap().duration + 0.5 {
                    trace_log(&format!(
                        "state Playing->Blanking t={:.3} dur={:.3}",
                        t,
                        self.video.as_ref().unwrap().duration
                    ));
                    self.state = State::Blanking;
                    tm.reset();
                } else {
                    if let Some(bgm) = &mut self.bgm {
                        let pos = bgm.music.position();
                        tm.update(pos as _);
                        if f % 15 == 0 {
                            trace_log(&format!("playing f={} t={:.3} pos={:.3}", f, t, pos));
                        }
                    }
                    self.video.as_mut().unwrap().update(t)?;
                }
            }
            State::Blanking => {
                // 黑屏缓冲 1 秒，随后进入加载等待
                if t > 1. {
                    trace_log(&format!("state Blanking->Loading t={:.3}", t));
                    self.state = State::Loading;
                    tm.reset();
                }
            }
            State::Loading => {
                if t > 1. && self.game_scene.is_some() {
                    trace_log(&format!("state Loading->Transforming t={:.3}", t));
                    // 解锁音频不再需要，提前释放，避免与 GameScene 的音频设备冲突
                    self.bgm = None;
                    self.state = State::Transforming;
                    tm.reset();
                }
            }
            State::Transforming => {
                // 过渡动画从 t=0 重新计时，真正走完 Loading 淡出与背景渐入后再切换
                if t > 1. {
                    if self.game_scene.is_none() {
                        bail!("UnlockScene exited at State::Transforming without GameScene");
                    }
                    trace_log("Transforming: dropping loading_scene/bgm");
                    // 先释放所有资源，避免 drop 时阻塞主线程
                    self.loading_scene = None;
                    self.bgm = None;
                    trace_log("Transforming: dropping video");
                    self.video = None;
                    trace_log("Transforming: video dropped, setting next_scene");
                    self.next_scene = self.game_scene.take().map(NextScene::Replace);
                    trace_log("Transforming: next_scene set, exiting UnlockScene");
                }
            }
        }

        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut prpr::ui::Ui) -> Result<()> {
        let mut cam = ui.camera();
        let asp = -cam.zoom.y;
        let t = tm.now();
        cam.render_target = self.render_target;
        set_camera(&cam);
        clear_background(BLACK);

        match self.state {
            State::Playing => {
                if t > 0.05 {
                    self.video.as_ref().unwrap().render(t, asp, WHITE);
                }
            }
            State::Loading => {
                let pad = 0.07;
                let top = 1. / asp;
                ui.loading(
                    1. - pad,
                    top - pad,
                    t as f32,
                    WHITE,
                    LoadingParams {
                        width: 0.01,
                        radius: 0.04,
                        ..Default::default()
                    },
                );
            }
            State::Transforming => {
                let top = 1. / asp;
                if t < 0.5 {
                    let pad = 0.07;
                    let alpha = if t < 0.5 { 1. - t as f32 / 0.5 } else { 0. };
                    ui.loading(
                        1. - pad,
                        top - pad,
                        t as f32,
                        semi_white(alpha),
                        LoadingParams {
                            width: 0.01,
                            radius: 0.04,
                            ..Default::default()
                        },
                    );
                } else {
                    let alpha = if t < 0.5 { t as f32 / 0.5 * 0.3 } else { 0.3 };
                    let r = ui.screen_rect();
                    ui.fill_rect(r, (*self.background, r));
                    ui.fill_rect(r, semi_black(alpha));
                }
            }
            _ => (),
        }

        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        self.next_scene.take().unwrap_or_default()
    }
}