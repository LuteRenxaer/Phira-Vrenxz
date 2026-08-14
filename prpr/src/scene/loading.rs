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
    ui::Ui,
};
use ::rand::{seq::SliceRandom, thread_rng};
use anyhow::{Context, Result};
use macroquad::prelude::*;
use regex::Regex;
use std::{rc::Rc, sync::Arc};

const BEFORE_TIME: f32 = 1.0;
const TRANSITION_TIME: f32 = 1.4;
const WAIT_TIME: f32 = 0.4;

fn draw_illustration(tex: Texture2D, x: f32, y: f32, w: f32, h: f32, color: Color) -> Rect {
    let scale = 0.076;
    let w = scale * 13. * w;
    let h = scale * 7. * h;
    let r = Rect::new(x - w / 2., y - h / 2., w, h);
    draw_parallelogram(r, Some((tex, Rect::new(0., 0., 1., 1.))), color, true);
    r
}

pub type UploadFn = Arc<dyn Fn(Vec<u8>) -> Task<Result<RecordUpdateState>>>;
pub type UpdateFn = Box<dyn FnMut(f64, &mut Resource, &mut Judge) + Send>;
pub type SaveFn = Box<dyn Fn(SimpleRecord) -> Result<()> + Send>;

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
    target: Option<RenderTarget>,
    charter: String,
}

impl LoadingScene {
    pub const TOTAL_TIME: f32 = BEFORE_TIME + TRANSITION_TIME + WAIT_TIME;

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
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
        let future = Box::pin(GameScene::new(
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
        ));
        let charter = Regex::new(r"\[!:[0-9]+:([^:]*)\]").unwrap().replace_all(&info.charter, "$1").to_string();
        Ok(Self {
            info,
            background,
            illustration,
            load_task: Some(future),
            next_scene: None,
            finish_time: f32::INFINITY,
            target: None,
            charter,
        })
    }
}

impl Scene for LoadingScene {
    fn enter(&mut self, _tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
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
        draw_background(*self.background);

        let dx = if now > self.finish_time {
            let p = ((now - self.finish_time) / TRANSITION_TIME).min(1.);
            p.powi(3) * 2.
        } else {
            0.
        };
        if dx != 0. {
            gl.push_model_matrix(Mat4::from_translation(vec3(dx, 0., 0.)));
        }

        let vo = -top / 10.;
        let r = draw_illustration(*self.illustration, 0.38, vo, 1., 1., WHITE);
        let h = r.h / 3.6;
        let main = Rect::new(-0.88, vo - h / 2. - top / 10., 0.78, h);
        draw_parallelogram(main, None, semi_black(0.7), false);

        let p = (main.x + main.w * 0.09, main.y + main.h * 0.36);
        let mut text = ui.text(&self.info.name).pos(p.0, p.1).anchor(0., 0.5).size(0.7).color(WHITE);
        if text.measure().w <= main.w * 0.6 {
            text.draw();
        } else {
            drop(text);
            ui.text(&self.info.name)
                .pos(p.0, p.1)
                .anchor(0., 0.5)
                .max_width(main.w * 0.6)
                .size(0.5)
                .color(WHITE)
                .draw();
        }

        ui.text(&self.info.composer)
            .pos(main.x + main.w * 0.09, main.y + main.h * 0.73)
            .anchor(0., 0.5)
            .size(0.36)
            .color(WHITE)
            .draw();

        let ext = 0.06;
        let sub = Rect::new(main.x + main.w * 0.71, main.y - main.h * ext, main.w * 0.26, main.h * (1. + ext * 2.));
        let mut ct = sub.center();
        ct.x += sub.w * 0.02;
        draw_parallelogram(sub, None, WHITE, false);
        ui.text(&(self.info.difficulty as u32).to_string())
            .pos(ct.x, ct.y + sub.h * 0.05)
            .anchor(0.5, 1.)
            .size(0.88)
            .color(BLACK)
            .draw();
        ui.text(self.info.level.split_whitespace().next().unwrap_or_default())
            .pos(ct.x, ct.y + sub.h * 0.09)
            .anchor(0.5, 0.)
            .size(0.34)
            .color(BLACK)
            .draw();

        let t = ui.text("Chart")
            .pos(main.x + main.w / 6., main.y + main.h * 1.2)
            .anchor(0., 0.)
            .size(0.3)
            .color(WHITE)
            .draw();
        ui.text(&self.charter)
            .pos(t.x, t.y + top / 20.)
            .anchor(0., 0.)
            .size(0.47)
            .color(WHITE)
            .draw();
        let w = 0.027;
        let t = ui.text("Illustration")
            .pos(t.x - w, t.y + w / 0.13 / 13. * 5.)
            .anchor(0., 0.)
            .size(0.3)
            .color(WHITE)
            .draw();
        ui.text(&self.info.illustrator)
            .pos(t.x, t.y + top / 20.)
            .anchor(0., 0.)
            .size(0.47)
            .color(WHITE)
            .draw();

        if let Some(tip) = &self.info.tip {
            ui.text(tip)
                .pos(-0.91, top * 0.92)
                .anchor(0., 1.)
                .size(0.47)
                .color(WHITE)
                .draw();
        }

        let load_text = "Loading...";
        let t = ui.text(load_text)
            .pos(0.87, top * 0.92)
            .anchor(1., 1.)
            .size(0.44)
            .color(WHITE)
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
        ui.fill_rect(progress_r, WHITE);
        progress_r.x += dx;
        ui.scissor(progress_r, |ui| {
            ui.text(load_text)
                .pos(0.87, top * 0.92)
                .anchor(1., 1.)
                .size(0.44)
                .color(BLACK)
                .draw();
        });

        if dx != 0. {
            gl.pop_model_matrix();
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