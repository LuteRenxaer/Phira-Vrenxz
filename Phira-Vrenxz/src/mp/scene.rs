//! 多人游戏场景：与主菜单平级的独立整屏场景。
//!
//! 场景本身只是一层壳（[`MultiplayerScene`]），所有状态都在
//! [`crate::mp::MP_SESSION`] 的 [`super::MpSession`] 里，因此：
//! - 从主菜单压栈进入、返回时用 `NextScene::Pop` 回主菜单；
//! - 进入游玩 / 谱面预览 / 同步观战子场景（同样是压栈）再返回时，
//!   会话连接与房间状态完全保留。

use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::Ui,
};

use super::MP_SESSION;

/// 多人游戏整屏场景。
#[derive(Default)]
pub struct MultiplayerScene;

impl MultiplayerScene {
    pub fn new() -> Self {
        Self
    }
}

impl Scene for MultiplayerScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        MP_SESSION.with(|it| {
            if let Some(session) = it.borrow_mut().as_mut() {
                session.enter(tm.now() as f32);
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
        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        MP_SESSION
            .with(|it| it.borrow_mut().as_mut().and_then(|session| session.next_scene(tm.now() as f32)))
            .unwrap_or_default()
    }
}
