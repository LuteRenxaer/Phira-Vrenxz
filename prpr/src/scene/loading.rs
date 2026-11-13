prpr_l10n::tl_file!("loading");

use super::{challenge::ChallengeConfig, draw_background, ending::RecordUpdateState, game::GameMode, GameScene, NextScene, Scene};
use crate::{
    config::{Config, DynamicBackgroundMode},
    core::{DynamicBackground, Resource},
    ext::{
        draw_parallelogram, get_viewport, poll_future, screen_aspect, semi_black, LocalTask,
        SafeTexture, BLACK_TEXTURE,
    },
    fs::FileSystem,
    info::ChartInfo,
    judge::Judge,
    scene::SimpleRecord,
    task::Task,
    time::TimeManager,
    ui::{Ui, PREFER_REDUCED_MOTION},
};
use ::rand::{seq::SliceRandom, thread_rng};
use anyhow::{Context, Result};
use macroquad::prelude::*;
use regex::Regex;
use std::{
    rc::Rc,
    sync::{atomic::Ordering, Arc},
};

const BEFORE_TIME: f32 = 1.0;
const TRANSITION_TIME: f32 = 1.4;
const WAIT_TIME: f32 = 0.4;

const ENTER_BG_TIME: f32 = 0.30;
const ENTER_PANEL_DELAY: f32 = 0.08;
const ENTER_PANEL_TIME: f32 = 0.34;
const ENTER_COVER_DELAY: f32 = 0.16;
const ENTER_COVER_TIME: f32 = 0.30;

const PANEL_IN_OFFSET: f32 = -0.06;

const FLASH_FADE_TIME: f32 = 0.45;

fn fade(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}


fn random_local_chart_art() -> Option<SafeTexture> {
    let root = std::path::Path::new("data/charts");
    let mut files = Vec::new();
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                matches!(x.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp")
            }) {
                out.push(p);
            }
        }
    }
    walk(root, &mut files);
    if files.is_empty() { return None; }
    let idx = (macroquad::rand::gen_range(0, files.len() as i32)) as usize;
    let data = std::fs::read(&files[idx]).ok()?;
    let img = image::load_from_memory(&data).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(Texture2D::from_rgba8(w as u16, h as u16, &img).into())
}

fn draw_illustration(tex: Texture2D, x: f32, y: f32, w: f32, h: f32, color: Color) -> Rect {
    let scale = 0.076;
    let w = scale * 13. * w;
    let h = scale * 7. * h;
    let r = Rect::new(x - w / 2., y - h / 2., w, h);
    let tex_ratio = tex.width() / tex.height();
    let rect_ratio = w / h;
    let tex_rect = if tex_ratio > rect_ratio {
        let new_w = rect_ratio / tex_ratio;
        Rect::new((1. - new_w) / 2., 0., new_w, 1.)
    } else {
        let new_h = tex_ratio / rect_ratio;
        Rect::new(0., (1. - new_h) / 2., 1., new_h)
    };
    draw_parallelogram(r, Some((tex, tex_rect)), color, true);
    r
}

pub type UploadFn = Arc<dyn Fn(Vec<u8>) -> Task<Result<RecordUpdateState>>>;
pub type UpdateFn = Box<dyn FnMut(f64, &mut Resource, &mut Judge) + Send>;

#[derive(Clone, Copy, Debug)]
pub struct FinishedStats {
    pub score: u32,
    pub accuracy: f32,
    pub full_combo: bool,
    pub max_combo: u32,
    pub perfect: u32,
    pub good: u32,
    pub bad: u32,
    pub miss: u32,
}
pub type SaveFn = Box<dyn Fn(FinishedStats) -> Result<()> + Send>;

#[derive(Clone)]
pub struct BasicPlayer {
    pub avatar: Option<SafeTexture>,
    pub id: i32,
    pub rks: f32,
    pub historic_best: u32,
}

pub struct LoadingScene {
    info: ChartInfo,
    background: SafeTexture,
    illustration: SafeTexture,
    pub load_task: LocalTask<Result<GameScene>>,
    next_scene: Option<NextScene>,
    finish_time: f32,

    enter_time: Option<f32>,
    target: Option<RenderTarget>,
    charter: String,
}

impl LoadingScene {
    pub const TOTAL_TIME: f32 = BEFORE_TIME + TRANSITION_TIME + WAIT_TIME;

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        mode: GameMode,
        info: ChartInfo,
        config: Config,
        fs: Box<dyn FileSystem>,
        player: Option<BasicPlayer>,
        get_size_fn: Option<Rc<dyn Fn() -> (u32, u32)>>,
        upload_fn: Option<UploadFn>,
        update_fn: Option<UpdateFn>,
        save_fn: Option<SaveFn>,
        preload: Option<(SafeTexture, SafeTexture, crate::core::Color)>,
        challenge: Option<ChallengeConfig>,
    ) -> Result<Self> {
        Self::new_preview(
            mode,
            info,
            config,
            fs,
            player,
            get_size_fn,
            upload_fn,
            update_fn,
            save_fn,
            preload,
            false,
            None,
            challenge,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new_preview(
        mode: GameMode,
        mut info: ChartInfo,
        config: Config,
        mut fs: Box<dyn FileSystem>,
        player: Option<BasicPlayer>,
        get_size_fn: Option<Rc<dyn Fn() -> (u32, u32)>>,
        upload_fn: Option<UploadFn>,
        update_fn: Option<UpdateFn>,
        save_fn: Option<SaveFn>,
        preload: Option<(SafeTexture, SafeTexture, crate::core::Color)>,
        preview_mode: bool,
        interrupt: Option<Arc<std::sync::atomic::AtomicBool>>,
        challenge: Option<ChallengeConfig>,
    ) -> Result<Self> {
        if config.dynamic_background != DynamicBackgroundMode::Off {
            info.dynamic_background = config.dynamic_background.as_u8();
        }
        async fn load(fs: &mut Box<dyn FileSystem>, path: &str) -> Result<(Texture2D, Texture2D)> {
            let image = image::load_from_memory(&fs.load_file(path).await?).context("Failed to decode image")?;
            let (w, h) = (image.width(), image.height());

            let max_w = 1920.0;
            let scale = (max_w / w as f32).min(1.0);
            let (sw, sh) = ((w as f32 * scale) as u32, (h as f32 * scale) as u32);
            let scaled = image.resize_exact(sw, sh, image::imageops::FilterType::Triangle);
            let original_rgba = scaled.to_rgba8();

            let blur_scale = (1280.0 / sw as f32).min(1.0);
            let (bw, bh) = ((sw as f32 * blur_scale) as u32, (sh as f32 * blur_scale) as u32);
            let blurred = scaled.resize_exact(bw, bh, image::imageops::FilterType::Triangle);
            let blurred_rgb = blurred.to_rgb8();
            let mut pixel_data: Vec<[u8; 3]> = blurred_rgb
                .chunks_exact(3)
                .map(|chunk| [chunk[0], chunk[1], chunk[2]])
                .collect();
            fastblur::gaussian_blur(&mut pixel_data, bw as _, bh as _, 50.0);
            let blurred_rgb_u8: Vec<u8> = pixel_data
                .into_iter()
                .flat_map(|pixel| pixel.to_vec())
                .collect();

            let mut blurred_rgba = Vec::with_capacity(bw as usize * bh as usize * 4);
            for chunk in blurred_rgb_u8.chunks_exact(3) {
                blurred_rgba.extend_from_slice(chunk);
                blurred_rgba.push(255);
            }

            Ok((
                Texture2D::from_rgba8(sw as _, sh as _, &original_rgba),
                Texture2D::from_image(&Image {
                    width: bw as u16,
                    height: bh as u16,
                    bytes: blurred_rgba,
                }),
            ))
        }

        let background = match load(&mut fs, &info.illustration).await {
            Ok((ill, bg)) => Some((ill, bg)),
            Err(err) => {
                warn!("Failed to load background: {:?}", err);
                None
            }
        };
        let random = preload
            .map(|(ill, back, _)| (ill, back))
            .or_else(|| random_local_chart_art().map(|t| (t.clone(), t)));
        let (illustration, background): (SafeTexture, SafeTexture) = random
            .or_else(|| background.map(|(ill, back)| (ill.into(), back.into())))
            .unwrap_or_else(|| (BLACK_TEXTURE.clone(), BLACK_TEXTURE.clone()));
        let _get_size_fn = get_size_fn.unwrap_or_else(|| Rc::new(|| (screen_width() as u32, screen_height() as u32)));
        if info.tip.is_none() {
            info.tip = Some(crate::config::TIPS.choose(&mut thread_rng()).unwrap().to_owned());
        }
        let dynamic_bg = if info.dynamic_background > 0 {
            match fs.load_file(&info.illustration).await {
                Ok(bytes) => match image::load_from_memory(&bytes) {
                    Ok(image) => match DynamicBackground::new(
                        info.dynamic_background,
                        &image,
                        info.background_dim,
                        get_viewport(),
                    ) {
                        Ok(dbg) => Some(dbg),
                        Err(err) => {
                            warn!("failed to create dynamic background: {err:?}");
                            None
                        }
                    },
                    Err(err) => {
                        warn!("failed to decode dynamic background image: {err:?}");
                        None
                    }
                },
                Err(err) => {
                    warn!("failed to load dynamic background image: {err:?}");
                    None
                }
            }
        } else {
            None
        };
        let info_for_scene = info.clone();
        let background_for_scene = background.clone();
        let illustration_for_scene = illustration.clone();
        let future = Box::pin(async move {
            let mut scene = GameScene::new_preview(
                mode,
                info_for_scene,
                config,
                fs,
                player,
                background_for_scene,
                illustration_for_scene,
                upload_fn,
                update_fn,
                save_fn,
                preview_mode,
                interrupt,
                false,
                challenge,
            )
            .await?;
            scene.dynamic_bg = dynamic_bg;
            Ok(scene)
        });
        let charter = Regex::new(r"\[!:[0-9]+:([^:]*)\]").unwrap().replace_all(&info.charter, "$1").to_string();
        Ok(Self {
            info,
            background,
            illustration,
            load_task: Some(future),
            next_scene: None,
            finish_time: f32::INFINITY,
            enter_time: None,
            target: None,
            charter,
        })
    }

    /// HUD 编辑：加载页示例预览。不加载任何谱面、不进入游戏，纯示例数据供编辑器拖拽布局；
    /// load_task 置空后场景会一直停留在加载页（finish_time 无穷），点返回即回主菜单。
    pub fn hud_preview() -> Self {
        let mut info = ChartInfo::default();
        info.name = "示例谱面".to_owned();
        info.difficulty = 15.0;
        info.level = "IN 15".to_owned();
        info.charter = "Demo".to_owned();
        info.composer = "Demo".to_owned();
        info.illustrator = "Demo".to_owned();
        info.tip = Some(crate::config::TIPS.choose(&mut thread_rng()).unwrap().to_owned());
        let charter = Regex::new(r"\[!:[0-9]+:([^:]*)\]").unwrap().replace_all(&info.charter, "$1").to_string();
        Self {
            info,
            background: BLACK_TEXTURE.clone(),
            illustration: BLACK_TEXTURE.clone(),
            load_task: None,
            next_scene: None,
            finish_time: f32::INFINITY,
            enter_time: None,
            target: None,
            charter,
        }
    }
}

impl Scene for LoadingScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        self.enter_time = Some(tm.now() as f32);
        self.target = target;
        Ok(())
    }

    fn touch(&mut self, _tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if crate::hud::edit_active() {
            crate::hud::editor_touch(touch);
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        if crate::hud::edit_active() {
            if let Some(p) = crate::hud::take_goto() {
                if p != crate::hud::PageId::Loading {
                    crate::hud::request_goto(p);
                    self.next_scene = Some(NextScene::Pop);
                }
            }
        }
        if let Some(future) = self.load_task.as_mut() {
            loop {
                match poll_future(future.as_mut()) {
                    None => {
                        if self.target.is_none() {
                            break;
                        }
                        std::thread::yield_now();
                    }
                    Some(game_scene) => {

                        self.load_task = None;
                        self.next_scene =
                            Some(game_scene.map_or_else(|e| NextScene::PopWithResult(Box::new(e)), |it| NextScene::Replace(Box::new(it))));
                        self.finish_time = tm.now() as f32 + BEFORE_TIME;
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        let asp = screen_aspect();
        let top = 1. / asp;
        let now = tm.now() as f32;
        let intern = unsafe { get_internal_gl() };
        let gl = intern.quad_gl;
        set_camera(&Camera2D {
            zoom: vec2(1., -asp),
            render_target: self.target,
            ..Default::default()
        });
        crate::hud::set_cur_page(crate::hud::PageId::Loading);
        crate::hud::begin_frame();

        let enter = (now - self.enter_time.unwrap_or(now)).max(0.);
        let ease = |p: f32| 1. - (1. - p).powi(3);
        let seg = |delay: f32, dur: f32| {
            if PREFER_REDUCED_MOTION.load(Ordering::Relaxed) {
                1.
            } else {
                ease(((enter - delay) / dur).clamp(0., 1.))
            }
        };
        let bg_p = seg(0., ENTER_BG_TIME);
        let bg_off = 2. * (1. - bg_p);
        let panel_p = seg(ENTER_PANEL_DELAY, ENTER_PANEL_TIME);
        let panel_off = PANEL_IN_OFFSET * (1. - panel_p);
        let cover_p = seg(ENTER_COVER_DELAY, ENTER_COVER_TIME);

        ui.fill_rect(Rect::new(-1., -top, 2., top * 2.), semi_black(0.25 + 0.65 * bg_p));

        if bg_off != 0. {
            gl.push_model_matrix(Mat4::from_translation(vec3(bg_off, 0., 0.)));
        }
        draw_background(*self.background);
        if bg_off != 0. {
            gl.pop_model_matrix();
        }

        let dx = if now > self.finish_time {
            let p = ((now - self.finish_time) / TRANSITION_TIME).min(1.);
            p.powi(3) * 2.
        } else {
            0.
        };
        if dx != 0. {
            gl.push_model_matrix(Mat4::from_translation(vec3(dx, 0., 0.)));
        }

        if panel_off != 0. {
            gl.push_model_matrix(Mat4::from_translation(vec3(panel_off, 0., 0.)));
        }
        let a = panel_p;

        let load_text = "Loading...";
        let m = ui.text(load_text).size(0.44).measure();
        // HUD 自定义：加载进度文本（可移动；默认矩形基于原位置）。
        let load_slot = crate::hud::slot_or(ui, "loading", "loading", crate::hud::Cap(true, false, false), Rect::new(0.87 - m.w, top * 0.92 - 0.2, m.w, 0.2));
        let t = ui.text(load_text)
            .pos(load_slot.right(), load_slot.bottom())
            .anchor(1., 1.)
            .size(0.44)
            .color(fade(WHITE, a))
            .draw();
        let we = 0.2;
        let he = 0.5;
        let r = Rect::new(t.x - t.w * we, t.y - t.h * he, t.w * (1. + we * 2.), t.h * (1. + he * 2.));

        let p = 0.6;
        let s = 0.2;
        let t_val = ((now - 0.3).max(0.) % (p * 2. + s)) / p;
        let st = (t_val - 1.).clamp(0., 1.).powi(3);
        let en = 1. - (1. - t_val.min(1.)).powi(3);

        let mut progress_r = Rect::new(r.x + r.w * st, r.y, r.w * (en - st), r.h);
        ui.fill_rect(progress_r, fade(WHITE, a));
        progress_r.x += dx;
        ui.scissor(progress_r, |ui| {
            ui.text(load_text)
                .pos(load_slot.right(), load_slot.bottom())
                .anchor(1., 1.)
                .size(0.44)
                .color(fade(BLACK, a))
                .draw();
        });

        if panel_off != 0. {
            gl.pop_model_matrix();
        }
        if dx != 0. {
            gl.pop_model_matrix();
        }

        {
            let mut flash = *crate::scene::LAUNCH_FLASH.lock().unwrap();
            if flash.is_some() {
                let held = ENTER_PANEL_DELAY + ENTER_PANEL_TIME;
                let a = if PREFER_REDUCED_MOTION.load(Ordering::Relaxed) {
                    0.
                } else if enter < held {
                    1.
                } else {
                    let p = ((enter - held) / FLASH_FADE_TIME).clamp(0., 1.);
                    if p >= 1. {
                        flash = None;
                    }
                    1. - p
                };
                *crate::scene::LAUNCH_FLASH.lock().unwrap() = flash;
                if a > 0. {
                    ui.fill_rect(Rect::new(-1., -top, 2., top * 2.), Color::new(1., 1., 1., a));
                }
            }
        }

        if crate::hud::edit_active() {
            crate::hud::editor_render(ui);
        }

        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        if crate::hud::edit_active() {
            if let Some(scene) = self.next_scene.take() {
                return scene;
            }
        }
        if matches!(self.next_scene, Some(NextScene::PopWithResult(_))) {
            return self.next_scene.take().unwrap();
        }
        if tm.now() as f32 > self.finish_time + TRANSITION_TIME + WAIT_TIME {
            if let Some(scene) = self.next_scene.take() {
                return scene;
            }
        }
        NextScene::None
    }
}