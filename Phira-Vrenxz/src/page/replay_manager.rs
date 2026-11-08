//! 本地回放管理页与编辑器（自研格式，仿 Phira Pro 的交互，不复制其代码）。
use super::{NextPage, Page, SharedState};
use anyhow::Result;
use inputbox::InputBox;
use macroquad::prelude::*;
use prpr::{
    ext::{poll_future, LocalTask},
    fs::load_info,
    scene::{
        request_file, request_input, return_file, show_error, show_message, take_file, take_input,
        GameMode, LoadingScene, NextScene,
    },
    ui::{DRectButton, Ui},
};
use std::{borrow::Cow, path::PathBuf};

struct Item {
    path: PathBuf,
    replay: prpr::replay::Replay,
}

pub struct ReplayManager {
    items: Vec<Item>,
    play_btns: Vec<DRectButton>,
    del_btns: Vec<DRectButton>,
    filter_btns: [DRectButton; 8],
    search_btn: DRectButton,
    query: String,
    filter: usize,
    status: String,
    play_task: LocalTask<Result<NextScene>>,
    ready: Option<NextScene>,
}

impl ReplayManager {
    pub fn new() -> Self {
        let mut this = Self {
            items: Vec::new(),
            play_btns: Vec::new(),
            del_btns: Vec::new(),
            filter_btns: std::array::from_fn(|_| DRectButton::new()),
            search_btn: DRectButton::new(),
            query: String::new(),
            filter: 0,
            status: String::new(),
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
        self.play_btns = (0..self.items.len()).map(|_| DRectButton::new()).collect();
        self.del_btns = (0..self.items.len()).map(|_| DRectButton::new()).collect();
    }

    fn visible(&self) -> Vec<usize> {
        let q = self.query.to_lowercase();
        self.items
            .iter()
            .enumerate()
            .filter(|(_, i)| {
                let text = format!("{} {} {}", i.replay.name, i.replay.chart, i.replay.player.clone().unwrap_or_default());
                text.to_lowercase().contains(&q)
            })
            .filter(|(_, i)| match self.filter {
                1 => i.replay.local_path.is_some(),
                2 => i.replay.local_path.is_none(),
                _ => true,
            })
            .map(|(i, _)| i)
            .collect()
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

    fn button(ui: &mut Ui, r: Rect, label: &str, active: bool, btn: &mut DRectButton) {
        btn.inner.set(ui, r);
        ui.fill_rect(r, if active { WHITE } else { Color::new(0.07, 0.08, 0.10, 0.95) });
        ui.text(label)
            .pos(r.center().x, r.center().y)
            .anchor(0.5, 0.5)
            .size(0.30)
            .color(if active { BLACK } else { WHITE })
            .draw();
    }
}

impl Page for ReplayManager {
    fn label(&self) -> Cow<'static, str> {
        Cow::Borrowed("回放文件")
    }

    fn update(&mut self, _s: &mut SharedState) -> Result<()> {
        if let Some((id, text)) = take_input() {
            if id == "replay_search" {
                self.query = text;
            } else {
                prpr::scene::return_input(id, text);
            }
        }
        if let Some((id, file)) = take_file() {
            if id == "_import_replay" {
                let name = std::path::Path::new(&file)
                    .file_name()
                    .map(|v| v.to_string_lossy().to_string())
                    .unwrap_or_else(|| "imported.json".into());
                if let Err(err) = std::fs::copy(&file, std::path::Path::new("replays").join(name)) {
                    show_error(anyhow::Error::new(err));
                } else {
                    self.reload();
                    show_message("回放已导入").ok();
                }
            } else {
                return_file(id, file);
            }
        }
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
        let t = s.t;
        if self.search_btn.touch(touch, t) {
            request_input("replay_search", InputBox::new().default_text(&self.query));
            return Ok(true);
        }
        for i in 0..8 {
            if self.filter_btns[i].touch(touch, t) {
                match i {
                    0 => request_file("_import_replay"),
                    1 => self.reload(),
                    2 => {
                        self.items.sort_by_key(|i| std::cmp::Reverse(i.replay.recorded_at));
                        self.filter = 0;
                    }
                    3 => self.status = "多选功能暂未开放".into(),
                    4 => self.filter = 0,
                    5 => self.filter = 1,
                    6 => self.filter = 2,
                    _ => self.filter = 3,
                }
                return Ok(true);
            }
        }
        let visible = self.visible();
        for (row, idx) in visible.iter().enumerate().take(10) {
            if self.play_btns.get_mut(*idx).is_some_and(|b| b.touch(touch, t)) {
                self.play(*idx);
                return Ok(true);
            }
            if self.del_btns.get_mut(*idx).is_some_and(|b| b.touch(touch, t)) {
                let _ = std::fs::remove_file(&self.items[*idx].path);
                self.reload();
                show_message("回放已删除").ok();
                return Ok(true);
            }
            let _ = row;
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, _s: &mut SharedState) -> Result<()> {
        let top = ui.top;
        let panel = Rect::new(-0.85, -top + 0.07, 1.7, top * 2. - 0.14);
        ui.fill_rect(panel, Color::new(0.05, 0.07, 0.09, 0.97));
        ui.text("回放管理")
            .pos(panel.x + 0.02, panel.y + 0.025)
            .size(0.56)
            .color(WHITE)
            .draw();
        let sr = Rect::new(panel.right() - 0.48, panel.y + 0.02, 0.45, 0.06);
        ui.fill_rect(sr, Color::new(0.02, 0.025, 0.03, 1.));
        self.search_btn.inner.set(ui, sr);
        ui.text(if self.query.is_empty() { "搜索" } else { &self.query })
            .pos(sr.x + 0.02, sr.center().y)
            .anchor(0., 0.5)
            .size(0.30)
            .color(Color::new(1., 1., 1., 0.7))
            .draw();
        let labels = ["导入回放", "刷新列表", "最新录制", "多选回放", "全部回放", "本地录制", "导入谱面", "缺少谱面"];
        let bw = 0.40;
        let gap = 0.015;
        let mut by = panel.y + 0.10;
        for row in 0..2 {
            for col in 0..4 {
                let i = row * 4 + col;
                let r = Rect::new(panel.x + 0.02 + col as f32 * (bw + gap), by, bw, 0.065);
                let active = match i {
                    4 => self.filter == 0,
                    5 => self.filter == 1,
                    6 => self.filter == 2,
                    7 => self.filter == 3,
                    _ => false,
                };
                Self::button(ui, r, labels[i], active, &mut self.filter_btns[i]);
            }
            by += 0.075;
        }
        let visible = self.visible();
        self.status = format!("{} 个回放", visible.len());
        let mut y = by + 0.02;
        if visible.is_empty() {
            ui.text("暂无回放，请导入文件或调整搜索条件")
                .pos(panel.x + 0.03, y + 0.04)
                .size(0.32)
                .color(Color::new(1., 1., 1., 0.75))
                .draw();
        }
        for idx in visible.iter().take(10) {
            let item = &self.items[*idx];
            let r = Rect::new(panel.x + 0.02, y, panel.w - 0.04, 0.095);
            ui.fill_rect(r, Color::new(0.07, 0.08, 0.10, 0.92));
            ui.text(&item.replay.name)
                .pos(r.x + 0.02, r.y + 0.018)
                .size(0.38)
                .draw();
            ui.text(format!(
                "{}  {:.2}x  P/G/B/M {}/{}/{}/{}",
                item.replay.chart,
                item.replay.speed,
                item.replay.stats().0,
                item.replay.stats().1,
                item.replay.stats().2,
                item.replay.stats().3
            ))
            .pos(r.x + 0.02, r.y + 0.060)
            .size(0.25)
            .color(Color::new(1., 1., 1., 0.6))
            .draw();
            let p = Rect::new(r.right() - 0.30, r.y + 0.020, 0.12, 0.055);
            let d = Rect::new(r.right() - 0.16, r.y + 0.020, 0.12, 0.055);
            Self::button(ui, p, "播放", false, &mut self.play_btns[*idx]);
            Self::button(ui, d, "删除", false, &mut self.del_btns[*idx]);
            y += 0.105;
        }
        ui.text(&self.status)
            .pos(panel.center().x, panel.bottom() - 0.04)
            .anchor(0.5, 0.)
            .size(0.26)
            .color(Color::new(1., 1., 1., 0.5))
            .draw();
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
        ui.text(value).pos(r.right() - 0.26, r.center().y).anchor(1., 0.5).size(0.32).draw();
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
