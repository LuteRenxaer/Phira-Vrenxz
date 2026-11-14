
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::Ui,
};

use super::{session::scene_time, MP_SESSION};

#[derive(Default)]
pub struct MultiplayerScene;

impl MultiplayerScene {
    pub fn new() -> Self {
        Self
    }
}

impl Scene for MultiplayerScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {

        let t = scene_time(tm);
        MP_SESSION.with(|it| {
            if let Some(session) = it.borrow_mut().as_mut() {
                session.enter(t);

                session.play_bgm();
            }
        });
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        Ok(MP_SESSION.with(|it| it.borrow_mut().as_mut().is_some_and(|session| session.touch(tm, touch))))
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        MP_SESSION.with(|it| match it.borrow_mut().as_mut() {
            Some(session) => session.update(tm),
            None => Ok(()),
        })
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        MP_SESSION.with(|it| {
            if let Some(session) = it.borrow_mut().as_mut() {
                session.render(tm, ui);
            }
        });

        {
            use once_cell::sync::Lazy;
            use std::sync::atomic::{AtomicU32, Ordering};
            static SHOT: Lazy<Option<String>> = Lazy::new(|| std::env::var("PHIRA_SHOT").ok());
            static N: AtomicU32 = AtomicU32::new(0);
            if let Some(path) = SHOT.as_ref() {

                if N.fetch_add(1, Ordering::Relaxed) % 600 == 0 {
                    macroquad::texture::get_screen_data().export_png(path);
                }
            }
        }
        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        let t = scene_time(tm);
        let next = MP_SESSION
            .with(|it| it.borrow_mut().as_mut().and_then(|session| session.next_scene(t)))
            .unwrap_or_default();

        if !matches!(next, NextScene::None) {
            MP_SESSION.with(|it| {
                if let Some(session) = it.borrow_mut().as_mut() {
                    session.pause_bgm();
                }
            });
        }
        next
    }
}
