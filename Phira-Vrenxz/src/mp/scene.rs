//! 多人游戏场景：与主菜单平级的独立整屏场景。
//!
//! 场景本身只是一层壳（[`MultiplayerScene`]），所有状态都在
//! [`crate::mp::MP_SESSION`] 的 [`super::MpSession`] 里，因此：
//! - 从主菜单压栈进入、返回时用 `NextScene::Pop` 回主菜单；
//! - 进入游玩 / 谱面预览子场景（同样是压栈）再返回时，
//!   会话连接与房间状态完全保留。

use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::Ui,
};

use super::{session::scene_time, MP_SESSION};

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
        // 会话内部一律用单调时钟（见 `scene_time` 的说明）：引擎弹出压栈场景时会把
        // `now()` 回拨到压栈那一刻，用 `now()` 会让重新进入时的整页淡入算成 0 透明。
        let t = scene_time(tm);
        MP_SESSION.with(|it| {
            if let Some(session) = it.borrow_mut().as_mut() {
                session.enter(t);
                // 首次进入、以及从子场景（游玩 / 预览）返回时都重新播放 BGM
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
        // 本地核对版面用：PHIRA_SHOT=<路径> 时，第 600 帧把**真正的帧缓冲**存成 PNG。
        // 截图走游戏自己（get_screen_data），不依赖系统截屏 —— GL 窗口用 CopyFromScreen
        // 抓出来经常是黑的，核对版面对不上。环境变量只读一次（每帧读会白白分配字符串）。
        {
            use once_cell::sync::Lazy;
            use std::sync::atomic::{AtomicU32, Ordering};
            static SHOT: Lazy<Option<String>> = Lazy::new(|| std::env::var("PHIRA_SHOT").ok());
            static N: AtomicU32 = AtomicU32::new(0);
            if let Some(path) = SHOT.as_ref() {
                // 每 600 帧覆盖一次：这样窗口被缩放之后还能拿到「缩放后」的那一帧
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
        // 压栈子场景或离开多人场景时，先把多人的 BGM 停掉
        // （回来时 `enter` 会重新播放），免得跟对局音乐叠在一起。
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
