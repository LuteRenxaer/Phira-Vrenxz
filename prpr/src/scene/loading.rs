prpr_l10n::tl_file!("loading");

use super::{draw_background, ending::RecordUpdateState, game::GameMode, GameScene, NextScene, Scene};
use crate::{
    config::Config,
    core::Resource,
    ext::{draw_parallelogram, poll_future, screen_aspect, semi_black, LocalTask, SafeTexture, BLACK_TEXTURE},
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

/// 进场编排时长（秒）：背景先推入 → 面板左偏淡入 → 封面单独淡入，三段错开。
const ENTER_BG_TIME: f32 = 0.30;
const ENTER_PANEL_DELAY: f32 = 0.08;
const ENTER_PANEL_TIME: f32 = 0.34;
const ENTER_COVER_DELAY: f32 = 0.16;
const ENTER_COVER_TIME: f32 = 0.30;
/// 面板进场时相对最终位置的横向偏移（负数 = 从左边偏一点点滑进来）。
const PANEL_IN_OFFSET: f32 = -0.06;
/// 白闪在整页滑入完成后淡掉的时间。
const FLASH_FADE_TIME: f32 = 0.45;

/// 原生 quad_gl 的绘制没有 ui.alpha 那一套，进场淡入只能自己乘进颜色里。
fn fade(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
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
/// 一局完整游玩的有效结算（非跳过、非 UNRATED 等）。
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
    /// 进入本场景那一刻的 `tm.now()`。
    ///
    /// 这个 TimeManager 是场景栈共享的、切场景不会重置（需要场景自己 `tm.reset()`），
    /// 所以进场动画必须用自己的进入时刻做基准，否则 `tm.now()` 一上手就是几十秒，
    /// `seg()` 直接算成 1 —— 动画一次都不会播。
    /// 用 `Option` 而不是 0：万一 `enter` 还没跑，基准未知也必须当作「动画刚开始」，
    /// 否则第一帧会按「早已到位」画满一屏，闪一下再滑进来。
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
        _preload: Option<(SafeTexture, SafeTexture, crate::core::Color)>,
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
            _preload,
            false,
            None,
        )
        .await
    }

    /// 与 [`LoadingScene::new`] 相同的加载场景，但以“谱面预览”模式启动谱面：
    /// `preview_mode` 下 GameScene 自然播完不结算，`interrupt` 置位（如多人房主开始游戏）立即退出。
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
        _preload: Option<(SafeTexture, SafeTexture, crate::core::Color)>,
        preview_mode: bool,
        interrupt: Option<Arc<std::sync::atomic::AtomicBool>>,
    ) -> Result<Self> {
        async fn load(fs: &mut Box<dyn FileSystem>, path: &str) -> Result<(Texture2D, Texture2D)> {
            let image = image::load_from_memory(&fs.load_file(path).await?).context("Failed to decode image")?;
            let (w, h) = (image.width(), image.height());
            let size = w as usize * h as usize;

            let original_rgba = image.to_rgba8();

            let blurred_rgb = image.to_rgb8();
            let mut pixel_data: Vec<[u8; 3]> = blurred_rgb
                .chunks_exact(3)
                .map(|chunk| [chunk[0], chunk[1], chunk[2]])
                .collect();
            fastblur::gaussian_blur(&mut pixel_data, w as _, h as _, 50.0);
            let blurred_rgb_u8: Vec<u8> = pixel_data
                .into_iter()
                .flat_map(|pixel| pixel.to_vec())
                .collect();

            let mut blurred_rgba = Vec::with_capacity(size * 4);
            for chunk in blurred_rgb_u8.chunks_exact(3) {
                blurred_rgba.extend_from_slice(chunk);
                blurred_rgba.push(255);
            }

            Ok((
                Texture2D::from_rgba8(w as _, h as _, &original_rgba),
                Texture2D::from_image(&Image {
                    width: w as _,
                    height: h as _,
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
        let (illustration, background): (SafeTexture, SafeTexture) = background
            .map(|(ill, back)| (ill.into(), back.into()))
            .unwrap_or_else(|| (BLACK_TEXTURE.clone(), BLACK_TEXTURE.clone()));
        let _get_size_fn = get_size_fn.unwrap_or_else(|| Rc::new(|| (screen_width() as u32, screen_height() as u32)));
        if info.tip.is_none() {
            info.tip = Some(crate::config::TIPS.choose(&mut thread_rng()).unwrap().to_owned());
        }
        let future = Box::pin(GameScene::new_preview(
            mode,
            info.clone(),
            config,
            fs,
            player,
            background.clone(),
            illustration.clone(),
            upload_fn,
            update_fn,
            save_fn,
            preview_mode,
            interrupt,
        ));
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
}

impl Scene for LoadingScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        self.enter_time = Some(tm.now() as f32);
        self.target = target;
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
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
        // 进场编排：三段错开、全部 ease-out ——
        //   背景：整块从屏幕右侧推入（最先动，先把后面盖住）
        //   面板：从左边偏一点点滑到位，同时淡入（卡片、文字、进度线都算面板）
        //   封面：只做透明度渐变，比面板再晚一点
        // LoadingScene 是 Overlay 进来的，底下就是选曲页，所以背景推入时能看见它。
        // 开了「减少动态效果」则三段直接到位。
        // 进场进度用「进入本场景之后过了多久」，不是 tm.now() 的绝对值；
        // 基准未知时当作 0（宁可从头播，也不能按「早已到位」画一帧满屏）
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

        // —— 底：随推入进度压一层黑 ——
        // 底下的选曲页是亮的（曲绘铺满整屏），背景直接扫过去会看着「白闪一下」；
        // 铺一层跟着进度加深的黑纱，滑入过程就干净了（背景到位后完全盖住，看不见它）。
        // 起步就有 0.25：第一帧背景还在屏幕外，底下的选曲页不能亮着跳一下。
        ui.fill_rect(Rect::new(-1., -top, 2., top * 2.), semi_black(0.25 + 0.65 * bg_p));

        // —— 背景层：整块从右边推入 ——
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
        // —— 面板 + 文字层：从左边偏一点点进来 + 透明度渐变 ——
        if panel_off != 0. {
            gl.push_model_matrix(Mat4::from_translation(vec3(panel_off, 0., 0.)));
        }
        let a = panel_p;

        let vo = -top / 10.;
        let r = draw_illustration(*self.illustration, 0.38, vo, 1., 1., fade(WHITE, cover_p));
        let h = r.h / 3.6;
        let main = Rect::new(-0.88, vo - h / 2. - top / 10., 0.78, h);
        draw_parallelogram(main, None, fade(semi_black(0.7), a), false);

        let p = (main.x + main.w * 0.09, main.y + main.h * 0.36);
        let mut text = ui.text(&self.info.name).pos(p.0, p.1).anchor(0., 0.5).size(0.7).color(fade(WHITE, a));
        if text.measure().w <= main.w * 0.6 {
            text.draw();
        } else {
            drop(text);
            ui.text(&self.info.name)
                .pos(p.0, p.1)
                .anchor(0., 0.5)
                .max_width(main.w * 0.6)
                .size(0.5)
                .color(fade(WHITE, a))
                .draw();
        }

        ui.text(&self.info.composer)
            .pos(main.x + main.w * 0.09, main.y + main.h * 0.73)
            .anchor(0., 0.5)
            .size(0.36)
            .color(fade(WHITE, a))
            .draw();

        let ext = 0.06;
        let sub = Rect::new(main.x + main.w * 0.71, main.y - main.h * ext, main.w * 0.26, main.h * (1. + ext * 2.));
        let mut ct = sub.center();
        ct.x += sub.w * 0.02;
        draw_parallelogram(sub, None, fade(WHITE, a), false);
        ui.text(&(self.info.difficulty as u32).to_string())
            .pos(ct.x, ct.y + sub.h * 0.05)
            .anchor(0.5, 1.)
            .size(0.88)
            .color(fade(BLACK, a))
            .draw();
        ui.text(self.info.level.split_whitespace().next().unwrap_or_default())
            .pos(ct.x, ct.y + sub.h * 0.09)
            .anchor(0.5, 0.)
            .size(0.34)
            .color(fade(BLACK, a))
            .draw();

        let t = ui.text("Chart")
            .pos(main.x + main.w / 6., main.y + main.h * 1.2)
            .anchor(0., 0.)
            .size(0.3)
            .color(fade(WHITE, a))
            .draw();
        ui.text(&self.charter)
            .pos(t.x, t.y + top / 20.)
            .anchor(0., 0.)
            .size(0.47)
            .color(fade(WHITE, a))
            .draw();
        let w = 0.027;
        let t = ui.text("Illustration")
            .pos(t.x - w, t.y + w / 0.13 / 13. * 5.)
            .anchor(0., 0.)
            .size(0.3)
            .color(fade(WHITE, a))
            .draw();
        ui.text(&self.info.illustrator)
            .pos(t.x, t.y + top / 20.)
            .anchor(0., 0.)
            .size(0.47)
            .color(fade(WHITE, a))
            .draw();

        if let Some(tip) = &self.info.tip {
            ui.text(tip)
                .pos(-0.91, top * 0.92)
                .anchor(0., 1.)
                .size(0.47)
                .color(fade(WHITE, a))
                .draw();
        }

        let load_text = "Loading...";
        let t = ui.text(load_text)
            .pos(0.87, top * 0.92)
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
                .pos(0.87, top * 0.92)
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

        // —— 白闪：图层最高 ——
        // 画在所有 model matrix 还原之后、所有 UI 之上：整屏白盖住一切。
        // 按下「开始」起一直白着，等这页滑入完成（面板就位）再线性淡掉并清掉全局状态。
        // 开「减少动态效果」时不画（这种闪屏正是那类设置要挡的东西）。
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

        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
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