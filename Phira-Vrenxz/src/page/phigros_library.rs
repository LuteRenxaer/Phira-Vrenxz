//! 仿 Phigros 谱面选择页：左侧列表 + 右侧谱面详情。
//! 原 LibraryPage 保留，后续可在设置里切换两套 UI。
use super::{Page, SharedState};
use crate::{
    data::LocalChart,
    dir, get_data,
    scene::SongScene,
};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    ext::{poll_future, LocalTask, SafeTexture, ScaleType},
    scene::{NextScene, GameMode},
    ui::{DRectButton, Scroll, Ui},
};
use std::{borrow::Cow, collections::HashMap, path::PathBuf};

const TABS: [&str; 7] = ["本地", "上架", "特殊", "未上架", "热门", "XC-SIM", "内置"];

pub struct PhigrosLibraryPage {
    charts: Vec<LocalChart>,
    selected: usize,
    tab: usize,
    scroll: Scroll,
    row_btns: Vec<DRectButton>,
    tab_btns: [DRectButton; 7],
    play_btn: DRectButton,
    covers: HashMap<String, SafeTexture>,
    play_task: LocalTask<Result<NextScene>>,
    ready: Option<NextScene>,
    mirror: bool,
    mirror_rect: Rect,
}

impl PhigrosLibraryPage {
    pub fn new() -> Self {
        let mut this = Self {
            charts: Vec::new(),
            selected: 0,
            tab: 0,
            scroll: Scroll::new(),
            row_btns: Vec::new(),
            tab_btns: std::array::from_fn(|_| DRectButton::new()),
            play_btn: DRectButton::new(),
            covers: HashMap::new(),
            play_task: None,
            ready: None,
            mirror: false,
            mirror_rect: Rect::new(0., 0., 0., 0.),
        };
        this.reload();
        this
    }

    fn reload(&mut self) {
        self.charts = get_data().charts.clone();
        self.charts.sort_by(|a, b| b.info.name.cmp(&a.info.name));
        self.row_btns = (0..self.charts.len()).map(|_| DRectButton::new()).collect();
        if self.selected >= self.charts.len() {
            self.selected = 0;
        }
    }

    fn cover(&mut self, index: usize) -> Option<SafeTexture> {
        let chart = self.charts.get(index)?;
        let key = chart.local_path.clone();
        if let Some(tex) = self.covers.get(&key) {
            return Some(tex.clone());
        }
        let root = dir::charts().ok()?;
        let dir = PathBuf::from(root).join(chart.local_path.replace('\\', "/"));
        let Ok(rd) = std::fs::read_dir(&dir) else { return None };
        let mut candidates: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                    matches!(x.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "webp")
                })
            })
            .collect();
        candidates.sort_by_key(|p| {
            let n = p.file_name().and_then(|x| x.to_str()).unwrap_or_default().to_ascii_lowercase();
            if n.contains("illustration") || n.contains("background") { 0 } else { 1 }
        });
        let path = candidates.first()?;
        let bytes = std::fs::read(path).ok()?;
        let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        let tex: SafeTexture = Texture2D::from_rgba8(w as u16, h as u16, &img).into();
        self.covers.insert(key, tex.clone());
        Some(tex)
    }

    fn play(&mut self) {
        let Some(chart) = self.charts.get(self.selected).cloned() else { return };
        let mut mods = chart.mods;
        if self.mirror {
            mods.insert(prpr::config::Mods::FLIP_X);
        } else {
            mods.remove(prpr::config::Mods::FLIP_X);
        }
        let task = SongScene::global_launch_preview(
            chart.info.id,
            &chart.local_path,
            mods,
            chart.hp,
            GameMode::Normal,
            None,
            None,
            chart.record.clone(),
            true,
            false,
            false,
            None,
            false,
            None,
        );
        match task {
            Ok(task) => self.play_task = task,
            Err(err) => prpr::scene::show_error(err),
        }
    }
}

impl Page for PhigrosLibraryPage {
    fn label(&self) -> Cow<'static, str> {
        Cow::Borrowed("谱面")
    }

    fn enter(&mut self, _s: &mut SharedState) -> Result<()> {
        self.reload();
        Ok(())
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        self.scroll.update(s.t);
        if let Some(task) = &mut self.play_task {
            if let Some(result) = poll_future(task.as_mut()) {
                self.play_task = None;
                match result {
                    Ok(scene) => self.ready = Some(scene),
                    Err(err) => prpr::scene::show_error(err),
                }
            }
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        for i in 0..7 {
            if self.tab_btns[i].touch(touch, t) {
                self.tab = i;
                return Ok(true);
            }
        }
        if self.play_btn.touch(touch, t) {
            self.play();
            return Ok(true);
        }
        if self.mirror_rect.contains(touch.position) {
            self.mirror = !self.mirror;
            return Ok(true);
        }
        for i in 0..self.charts.len() {
            if self.row_btns[i].touch(touch, t) {
                self.selected = i;
                return Ok(true);
            }
        }
        Ok(self.scroll.touch(touch, t))
    }

    fn render(&mut self, ui: &mut Ui, _s: &mut SharedState) -> Result<()> {
        let area = ui.content_rect();
        let top_y = area.y;
        // 顶部排序栏
        ui.text("排序方式").pos(-0.58, top_y).size(0.28).color(Color::new(1., 1., 1., 0.55)).draw();
        ui.text("默认").pos(-0.58, top_y + 0.035).size(0.40).draw();
        ui.text("随机").pos(0.02, top_y + 0.015).anchor(0.5, 0.).size(0.30).draw();
        ui.text("设置").pos(0.14, top_y + 0.015).anchor(0.5, 0.).size(0.30).draw();
        // 两行标签（5 + 2）
        let tab_y = area.y + 0.075;
        let tab_w = 0.175;
        for i in 0..7 {
            let (x, y) = if i < 5 {
                (-0.66 + i as f32 * tab_w, tab_y)
            } else {
                (-0.66 + (i - 5) as f32 * tab_w * 1.6, tab_y + 0.065)
            };
            let r = Rect::new(x, y, tab_w * if i < 5 { 1. } else { 1.55 } - 0.01, 0.06);
            self.tab_btns[i].inner.set(ui, r);
            ui.fill_rect(r, if i == self.tab { WHITE } else { Color::new(0.05, 0.06, 0.08, 0.92) });
            ui.text(TABS[i])
                .pos(r.center().x, r.center().y)
                .anchor(0.5, 0.5)
                .size(0.28)
                .color(if i == self.tab { BLACK } else { WHITE })
                .draw();
        }
        // 左侧列表
        let list = Rect::new(-0.66, tab_y + 0.14, 0.64, area.h - 0.24);
        let row_h = 0.095;
        let rows = ((list.h / row_h) as usize).max(1);
        let start = self.selected.saturating_sub(rows / 2).min(self.charts.len().saturating_sub(rows));
        for (n, i) in (start..self.charts.len()).take(rows).enumerate() {
            let r = Rect::new(list.x, list.y + n as f32 * row_h, list.w, row_h);
            self.row_btns[i].inner.set(ui, r);
            let active = i == self.selected;
            ui.fill_rect(r, if active { Color::new(0.10, 0.11, 0.14, 0.98) } else { Color::new(0.02, 0.025, 0.035, 0.35) });
            let c = &self.charts[i];
            ui.text(&c.info.name)
                .pos(r.x + 0.015, r.y + 0.022)
                .size(0.40)
                .color(WHITE)
                .max_width(r.w - 0.14)
                .draw();
            ui.text(&c.info.composer)
                .pos(r.x + 0.015, r.y + 0.060)
                .size(0.24)
                .color(Color::new(1., 1., 1., 0.55))
                .max_width(r.w - 0.14)
                .draw();
            let score = c.record.as_ref().map_or(0, |r| r.score);
            let rank = if score >= 1_000_000 { "Phi" } else if score >= 960_000 { "V" } else if score >= 920_000 { "S" } else if score >= 880_000 { "A" } else if score >= 820_000 { "B" } else if score >= 700_000 { "C" } else { "F" };
            ui.text(rank).pos(r.right() - 0.09, r.y + 0.020).anchor(1., 0.).size(0.34).color(Color::new(0.55, 0.75, 1., 1.)).draw();
            ui.text(format!("{:.0}", c.info.difficulty))
                .pos(r.right() - 0.01, r.center().y)
                .anchor(1., 0.5)
                .size(0.40)
                .draw();
        }
        // Mirror 按钮
        let mr = Rect::new(list.x, list.bottom() - 0.055, 0.20, 0.05);
        self.mirror_rect = mr;
        ui.fill_rect(mr, if self.mirror { WHITE } else { Color::new(0.08, 0.09, 0.11, 0.95) });
        ui.text("Mirror").pos(mr.center().x, mr.center().y).anchor(0.5, 0.5).size(0.26).color(if self.mirror { BLACK } else { WHITE }).draw();
        // 右侧详情
        let detail = Rect::new(0.03, tab_y + 0.14, 0.55, area.h - 0.24);
        let selected_chart = self.charts.get(self.selected).cloned();
        let selected_cover = self.cover(self.selected);
        if let Some(chart) = selected_chart {
            let art = Rect::new(detail.x, detail.y, detail.w, detail.h * 0.66);
            if let Some(tex) = selected_cover {
                ui.fill_rect(art, (*tex, art, ScaleType::CropCenter, WHITE));
            } else {
                ui.fill_rect(art, Color::new(0.06, 0.07, 0.09, 1.));
            }
            let info_h = 0.16;
            ui.fill_rect(Rect::new(art.x, art.bottom() - info_h, art.w, info_h), Color::new(0., 0., 0., 0.55));
            ui.text(&chart.info.name)
                .pos(art.x + 0.02, art.bottom() - 0.075)
                .size(0.44)
                .max_width(art.w - 0.04)
                .draw();
            ui.text(format!("♪ {}", chart.info.composer))
                .pos(art.x + 0.02, art.bottom() - 0.030)
                .size(0.24)
                .color(Color::new(1., 1., 1., 0.75))
                .draw();
            ui.text(format!("made by {}", chart.info.charter))
                .pos(art.x + 0.02, art.bottom() - 0.012)
                .size(0.20)
                .color(Color::new(1., 1., 1., 0.55))
                .draw();
            // 难度条
            let by = art.bottom() + 0.03;
            let labels = ["EZ", "HD", "IN", "AT"];
            let current = if chart.info.level.starts_with("AT") { 3 } else if chart.info.level.starts_with("IN") { 2 } else if chart.info.level.starts_with("HD") { 1 } else { 0 };
            for i in 0..4 {
                let r = Rect::new(detail.x + i as f32 * 0.105, by, 0.10, 0.085);
                ui.fill_rect(r, if i == current { Color::new(0.75, 0.10, 0.14, 1.) } else { Color::new(0.92, 0.92, 0.94, 1.) });
                ui.text(labels[i]).pos(r.center().x, r.y + 0.020).anchor(0.5, 0.).size(0.22).color(if i == current { WHITE } else { BLACK }).draw();
                ui.text(if i == current { format!("{:.0}", chart.info.difficulty) } else { "-".into() })
                    .pos(r.center().x, r.y + 0.050)
                    .anchor(0.5, 0.)
                    .size(0.34)
                    .color(if i == current { WHITE } else { BLACK })
                    .draw();
            }
            let score = chart.record.as_ref().map_or(0, |r| r.score);
            ui.text(format!("{:07}", score))
                .pos(detail.right() - 0.02, by + 0.02)
                .anchor(1., 0.)
                .size(0.50)
                .draw();
            if let Some(rec) = &chart.record {
                ui.text(format!("{:.2}%", rec.accuracy))
                    .pos(detail.right() - 0.02, by + 0.075)
                    .anchor(1., 0.)
                    .size(0.24)
                    .color(Color::new(1., 1., 1., 0.7))
                    .draw();
            }
        }
        // 功能区
        let func = Rect::new(0.62, tab_y + 0.14, 0.10, area.h - 0.24);
        ui.fill_rect(func, Color::new(0.04, 0.045, 0.055, 0.55));
        ui.text("功能区").pos(func.center().x, func.y + 0.05).anchor(0.5, 0.).size(0.26).draw();
        // 播放
        let pr = Rect::new(0.75, area.bottom() - 0.13, 0.18, 0.11);
        self.play_btn.inner.set(ui, pr);
        ui.fill_rect(pr, WHITE);
        ui.text("▶").pos(pr.center().x, pr.center().y).anchor(0.5, 0.5).size(0.60).color(BLACK).draw();
        Ok(())
    }

    fn next_scene(&mut self, _s: &mut SharedState) -> NextScene {
        self.ready.take().unwrap_or(NextScene::None)
    }
}
