//! 本地回放列表与编辑器。
use super::{NextPage, Page, SharedState};
use anyhow::Result;
use inputbox::InputBox;
use macroquad::prelude::*;
use prpr::{
    ext::{poll_future, LocalTask},
    fs::load_info,
    scene::{request_input, show_error, show_message, take_input, GameMode, LoadingScene, NextScene},
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
    play_task: LocalTask<Result<NextScene>>,
    ready: Option<NextScene>,
}

impl ReplayManager {
    pub fn new() -> Self {
        let mut this = Self {
            items: Vec::new(),
            buttons: Vec::new(),
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
            show_error(anyhow::anyhow!("找不到回放对应谱面，请重新导入"));
            return;
        };
        prpr::replay::set_pending(item.replay.clone());
        let local_path = local_path.replace('\\', "/");
        self.play_task = Some(Box::pin(async move {
            let mut fs = crate::scene::fs_from_path(&local_path)?;
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

    fn update(&mut self, _s: &mut SharedState) -> Result<()> {
        if let Some(task) = &mut self.play_task {
            if let Some(result) = poll_future(task.as_mut()) {
                self.play_task = None;
                match result {
                    Ok(scene) => self.ready = Some(scene),
                    Err(err) => show_error(err),
                }
            }
        }
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
        ui.text("回放管理").pos(area.x, -top + 0.10).size(0.72).draw();
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
            ui.text(&item.replay.name).pos(r.x + 0.02, r.y + 0.025).size(0.42).draw();
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

pub struct ReplayEditPage {
    path: PathBuf,
    replay: prpr::replay::Replay,
    start: f32,
    end: f32,
    offset_ms: f32,
    speed: f32,
    btns: [DRectButton; 9],
    next: Option<NextPage>,
    preview_task: LocalTask<Result<NextScene>>,
    preview_ready: Option<NextScene>,
}

impl ReplayEditPage {
    pub fn new(path: PathBuf, replay: prpr::replay::Replay) -> Self {
        let end = replay
            .duration
            .max(replay.touches.iter().map(|t| t.t).fold(0., f32::max));
        Self {
            path,
            start: 0.,
            end,
            offset_ms: 0.,
            speed: replay.speed,
            replay,
            btns: std::array::from_fn(|_| DRectButton::new()),
            next: None,
            preview_task: None,
            preview_ready: None,
        }
    }

    fn row(ui: &mut Ui, y: f32, label: &str, value: &str, btn: &mut DRectButton, action: &str) {
        let r = Rect::new(-0.85, y, 1.7, 0.075);
        ui.fill_rect(r, Color::new(0.055, 0.060, 0.075, 0.78));
        ui.text(label).pos(r.x + 0.02, r.center().y).anchor(0., 0.5).size(0.38).draw();
        ui.text(value).pos(r.right() - 0.20, r.center().y).anchor(1., 0.5).size(0.32).draw();
        let b = Rect::new(r.right() - 0.22, r.y + 0.010, 0.20, 0.055);
        super::settings::draw_settings_action_button(ui, b, action, btn);
    }
}

impl Page for ReplayEditPage {
    fn label(&self) -> Cow<'static, str> {
        Cow::Borrowed("编辑回放")
    }

    fn update(&mut self, _s: &mut SharedState) -> Result<()> {
        if let Some((id, text)) = take_input() {
            match id.as_str() {
                "replay_start" => self.start = text.parse().unwrap_or(self.start),
                "replay_end" => self.end = text.parse().unwrap_or(self.end),
                "replay_offset" => self.offset_ms = text.parse().unwrap_or(self.offset_ms),
                "replay_speed" => self.speed = text.parse().unwrap_or(self.speed),
                _ => prpr::scene::return_input(id, text),
            }
        }
        if let Some(task) = &mut self.preview_task {
            if let Some(result) = poll_future(task.as_mut()) {
                self.preview_task = None;
                match result {
                    Ok(scene) => self.preview_ready = Some(scene),
                    Err(err) => show_error(err),
                }
            }
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        if self.btns[0].touch(touch, t) {
            request_input("replay_start", InputBox::new().default_text(&self.start.to_string()));
            return Ok(true);
        }
        if self.btns[1].touch(touch, t) {
            request_input("replay_end", InputBox::new().default_text(&self.end.to_string()));
            return Ok(true);
        }
        if self.btns[2].touch(touch, t) {
            request_input("replay_offset", InputBox::new().default_text(&self.offset_ms.to_string()));
            return Ok(true);
        }
        if self.btns[3].touch(touch, t) {
            request_input("replay_speed", InputBox::new().default_text(&self.speed.to_string()));
            return Ok(true);
        }
        if self.btns[4].touch(touch, t) {
            self.replay.trim(self.start, self.end);
            self.start = 0.;
            self.end = self.replay.duration;
            show_message("已裁剪").ok();
            return Ok(true);
        }
        if self.btns[5].touch(touch, t) {
            self.replay.shift(self.offset_ms / 1000.);
            self.offset_ms = 0.;
            show_message("已偏移").ok();
            return Ok(true);
        }
        if self.btns[8].touch(touch, t) {
            let Some(local_path) = self.replay.local_path.clone().or_else(|| {
                crate::get_data()
                    .charts
                    .iter()
                    .find(|c| c.info.name == self.replay.name)
                    .map(|c| c.local_path.clone())
            }) else {
                show_error(anyhow::anyhow!("找不到对应谱面"));
                return Ok(true);
            };
            let mut preview = self.replay.clone();
            preview.speed = self.speed;
            preview.shift(self.offset_ms / 1000.);
            preview.trim(self.start, self.end);
            prpr::replay::set_pending(preview);
            let local_path = local_path.replace('\\', "/");
            self.preview_task = Some(Box::pin(async move {
                let mut fs = crate::scene::fs_from_path(&local_path)?;
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
            return Ok(true);
        }
        if self.btns[6].touch(touch, t) {
            self.replay.speed = self.speed;
            if let Err(err) = self.replay.save_as(&self.path) {
                show_error(anyhow::Error::new(err));
            } else {
                show_message("回放已保存").ok();
                self.next = Some(NextPage::Pop);
            }
            return Ok(true);
        }
        if self.btns[7].touch(touch, t) {
            self.next = Some(NextPage::Pop);
            return Ok(true);
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, _s: &mut SharedState) -> Result<()> {
        let area = super::settings::draw_settings_content_panel(ui);
        super::settings::draw_settings_subpage_header(ui, "编辑回放");
        let top = ui.top;
        ui.text(&self.replay.name)
            .pos(area.x, -top + 0.13)
            .size(0.34)
            .color(Color::new(1., 1., 1., 0.6))
            .draw();
        let mut y = area.y;
        Self::row(ui, y, "起点（秒）", &format!("{:.3}", self.start), &mut self.btns[0], "设置");
        y += 0.085;
        Self::row(ui, y, "终点（秒）", &format!("{:.3}", self.end), &mut self.btns[1], "设置");
        y += 0.085;
        Self::row(ui, y, "时间偏移（ms）", &format!("{:.1}", self.offset_ms), &mut self.btns[2], "设置");
        y += 0.085;
        Self::row(ui, y, "流速", &format!("{:.2}", self.speed), &mut self.btns[3], "设置");
        y += 0.10;
        Self::row(ui, y, "裁剪到起点/终点", "", &mut self.btns[4], "执行");
        y += 0.085;
        Self::row(ui, y, "应用时间偏移", "", &mut self.btns[5], "执行");
        y += 0.10;
        Self::row(ui, y, "在游戏场景中预览", "", &mut self.btns[8], "预览");
        y += 0.085;
        Self::row(ui, y, "保存修改", "", &mut self.btns[6], "保存");
        y += 0.085;
        Self::row(ui, y, "取消", "", &mut self.btns[7], "返回");
        Ok(())
    }

    fn next_page(&mut self) -> NextPage {
        self.next.take().unwrap_or(NextPage::None)
    }

    fn next_scene(&mut self, _s: &mut SharedState) -> NextScene {
        self.preview_ready.take().unwrap_or(NextScene::None)
    }

    fn back_button_rect(&self, ui: &Ui) -> Option<Rect> {
        Some(super::settings::settings_back_rect(ui))
    }
}
