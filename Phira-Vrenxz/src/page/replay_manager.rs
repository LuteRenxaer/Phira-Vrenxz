//! 本地回放管理：列表、播放、删除、刷新。
use super::{Page, SharedState};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{poll_future, LocalTask},
    fs::load_info,
    scene::{show_error, show_message, GameMode, LoadingScene, NextScene},
    ui::{DRectButton, Ui},
};
use std::{borrow::Cow, path::PathBuf};

struct Item {
    path: PathBuf,
    replay: prpr::replay::Replay,
}

pub struct ReplayManager {
    items: Vec<Item>,
    buttons: Vec<DRectButton>,
    query: String,
    play_task: LocalTask<Result<NextScene>>,
    ready: Option<NextScene>,
}

impl ReplayManager {
    pub fn new() -> Self {
        let mut this = Self {
            items: Vec::new(),
            buttons: Vec::new(),
            query: String::new(),
            play_task: None,
            ready: None,
        };
        this.reload();
        this
    }

    fn reload(&mut self) {
        self.items.clear();
        if let Ok(files) = std::fs::read_dir("replays") {
            for file in files.flatten() {
                let path = file.path();
                if path.extension().is_some_and(|e| e == "json") {
                    if let Some(replay) = prpr::replay::load(&path) {
                        self.items.push(Item { path, replay });
                    }
                }
            }
        }
        self.items.sort_by_key(|i| std::cmp::Reverse(i.replay.recorded_at));
        self.buttons = (0..self.items.len() * 2).map(|_| DRectButton::new()).collect();
    }

    fn play(&mut self, index: usize) {
        let Some(item) = self.items.get(index) else { return };
        let local_path = item.replay.local_path.clone().or_else(|| {
            crate::get_data()
                .charts
                .iter()
                .find(|c| c.info.name == item.replay.name)
                .map(|c| c.local_path.clone())
        });
        let Some(local_path) = local_path else {
            show_error(anyhow::anyhow!("找不到回放对应谱面，请重新导入该谱面"));
            return;
        };
        prpr::replay::set_pending(item.replay.clone());
        let root = match crate::dir::charts() {
            Ok(v) => v,
            Err(e) => {
                show_error(e);
                return;
            }
        };
        let full = local_path.replace('\\', "/");
        self.play_task = Some(Box::pin(async move {
            let mut fs = crate::scene::fs_from_path(&full)?;
            let info = load_info(fs.as_mut()).await?;
            let config = crate::get_data().config.clone();
            let scene = LoadingScene::new(
                GameMode::Replay,
                info,
                config,
                fs,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
            Ok(NextScene::Overlay(Box::new(scene)))
        }));
    }
}

impl Page for ReplayManager {
    fn label(&self) -> Cow<'static, str> {
        Cow::Borrowed("回放管理")
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        if let Some(task) = &mut self.play_task {
            if let Some(result) = poll_future(task.as_mut()) {
                self.play_task = None;
                match result {
                    Ok(scene) => self.ready = Some(scene),
                    Err(err) => show_error(err),
                }
            }
        }
        let _ = s;
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        for i in 0..self.items.len() {
            if self.buttons.get_mut(i * 2).is_some_and(|b| b.touch(touch, s.t)) {
                self.play(i);
                return Ok(true);
            }
            if self.buttons.get_mut(i * 2 + 1).is_some_and(|b| b.touch(touch, s.t)) {
                let _ = std::fs::remove_file(&self.items[i].path);
                self.reload();
                show_message("回放已删除").ok();
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, _s: &mut SharedState) -> Result<()> {
        let area = super::settings::draw_settings_content_panel(ui);
        let top = ui.top;
        ui.text("回放管理")
            .pos(area.x, -top + 0.10)
            .size(0.72)
            .draw();
        ui.text(format!("共 {} 份回放", self.items.len()))
            .pos(area.right(), -top + 0.115)
            .anchor(1., 0.)
            .size(0.34)
            .color(Color::new(1., 1., 1., 0.6))
            .draw();
        let mut y = area.y;
        for (i, item) in self.items.iter().enumerate().take(12) {
            let r = Rect::new(area.x, y, area.w, 0.11);
            ui.fill_rect(r, Color::new(0.055, 0.060, 0.075, 0.78));
            ui.text(&item.replay.name)
                .pos(r.x + 0.02, r.y + 0.025)
                .size(0.42)
                .draw();
            ui.text(format!(
                "{:.2}x  P/G/B/M {}/{}/{}/{}",
                item.replay.speed,
                item.replay.stats().0,
                item.replay.stats().1,
                item.replay.stats().2,
                item.replay.stats().3
            ))
            .pos(r.x + 0.02, r.y + 0.07)
            .size(0.26)
            .color(Color::new(1., 1., 1., 0.6))
            .draw();
            let p = Rect::new(r.right() - 0.30, r.y + 0.025, 0.12, 0.06);
            let d = Rect::new(r.right() - 0.16, r.y + 0.025, 0.12, 0.06);
            self.buttons[i * 2].inner.set(ui, p);
            self.buttons[i * 2 + 1].inner.set(ui, d);
            ui.text("播放").pos(p.center().x, p.center().y).anchor(0.5, 0.5).size(0.30).draw();
            ui.text("删除").pos(d.center().x, d.center().y).anchor(0.5, 0.5).size(0.30).draw();
            y += 0.13;
        }
        Ok(())
    }

    fn next_scene(&mut self, _s: &mut SharedState) -> NextScene {
        self.ready.take().unwrap_or(NextScene::None)
    }
}
