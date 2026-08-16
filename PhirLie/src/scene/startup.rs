//! 加载主页的加载页。仿造 prpr 的 LoadingScene,只保留背景、Tip 和右下角的加载图标。

use super::MainScene;
use crate::blue_archive_tips::BLUE_ARCHIVE_TIPS;
use prpr::{
    ext::{poll_future, semi_white, LocalTask},
    scene::{show_error, NextScene, Scene},
    time::TimeManager,
    ui::{FontArc, Ui},
};
use anyhow::Result;
use macroquad::prelude::*;
use ::rand::{seq::SliceRandom, thread_rng};

const FADE_IN_TIME: f32 = 0.5;
/// 主页加载完成后,加载页至少再显示这么久,避免一闪而过
const MIN_SHOW_TIME: f32 = 0.8;

pub struct StartupLoadingScene {
    load_task: LocalTask<Result<MainScene>>,
    ready_scene: Option<Box<dyn Scene>>,
    finish_time: f32,
    enter_time: f32,
    tip: &'static str,
    error: Option<String>,
}

impl StartupLoadingScene {
    pub fn new(fallback: FontArc) -> Self {
        let tip = BLUE_ARCHIVE_TIPS.choose(&mut thread_rng()).copied().unwrap_or("老师,欢迎回来!");
        Self {
            load_task: Some(Box::pin(async move { MainScene::new(fallback).await })),
            ready_scene: None,
            finish_time: f32::INFINITY,
            enter_time: f32::NAN,
            tip,
            error: None,
        }
    }
}

impl Scene for StartupLoadingScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.enter_time.is_nan() {
            self.enter_time = tm.now() as f32;
        }
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        if let Some(future) = self.load_task.as_mut() {
            if let Some(res) = poll_future(future.as_mut()) {
                self.load_task = None;
                match res {
                    Ok(scene) => {
                        self.ready_scene = Some(Box::new(scene));
                        self.finish_time = tm.now() as f32 + MIN_SHOW_TIME;
                    }
                    Err(err) => {
                        self.error = Some(format!("{err:#}"));
                        show_error(err.context("初始化失败"));
                    }
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        set_camera(&ui.camera());
        let t = tm.now() as f32;
        let top = ui.top;
        let full = ui.screen_rect();

        // 深色渐变背景
        ui.fill_rect(
            full,
            (
                Color::new(0.10, 0.12, 0.18, 1.),
                (full.x, full.y),
                Color::new(0.02, 0.03, 0.06, 1.),
                (full.x, full.bottom()),
            ),
        );

        let alpha = ((t - self.enter_time) / FADE_IN_TIME).clamp(0., 1.);
        ui.alpha(alpha, |ui| {
            if let Some(err) = &self.error {
                ui.text("启动失败")
                    .pos(0., 0.)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(0.5)
                    .color(RED)
                    .draw();
                ui.text(err)
                    .pos(0., 0.08)
                    .anchor(0.5, 0.)
                    .max_width(1.4)
                    .size(0.3)
                    .color(semi_white(0.7))
                    .draw();
            } else {
                // Tip(左下角)
                ui.text(&format!("Tip: {}", self.tip))
                    .pos(-0.95, top - 0.05)
                    .anchor(0., 1.)
                    .max_width(1.6)
                    .size(0.4)
                    .color(semi_white(0.6))
                    .draw();

                // 右下角 Loading... 扫描动画(仿 prpr LoadingScene)
                draw_loading_animation(ui, t, top);
            }
        });

        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        if self.ready_scene.is_some() && tm.now() as f32 > self.finish_time {
            let scene = self.ready_scene.take().unwrap();
            return NextScene::Replace(scene);
        }
        NextScene::None
    }
}

fn draw_loading_animation(ui: &mut Ui, now: f32, top: f32) {
    let load_text = "Loading...";
    let t = ui
        .text(load_text)
        .pos(0.93, top * 0.92)
        .anchor(1., 1.)
        .size(0.42)
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

    let progress_r = Rect::new(r.x + r.w * st, r.y, r.w * (en - st), r.h);
    ui.fill_rect(progress_r, WHITE);
    ui.scissor(progress_r, |ui| {
        ui.text(load_text)
            .pos(0.93, top * 0.92)
            .anchor(1., 1.)
            .size(0.42)
            .color(BLACK)
            .draw();
    });
}
