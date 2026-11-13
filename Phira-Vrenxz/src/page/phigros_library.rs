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
        let Some(chart) = self.charts.get(self.selected) else { return };
        let task = SongScene::global_launch_preview(
            chart.info.id,
            &chart.local_path,
            chart.mods,
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
        for i in 0..self.charts.len() {
            if self.row_btns[i].touch(touch, t) {
                self.selected = i;
                return Ok(true);
            }
        }
        Ok(self.scroll.touch(touch, t))
    }

    fn render(&mut self, ui: &mut Ui, _s: &mut SharedState) -> Result<()> {
        let top = ui.top;
        let full = Rect::new(-0.96, -top + 0.03, 1.92, top * 2. - 0.06);
        // 顶部标签
        let tab_y = -top + 0.12;
        let tw = 1.85 / 7.;
        for i in 0..7 {
            let r = Rect::new(-0.93 + i as f32 * tw, tab_y, tw - 0.01, 0.075);
            self.tab_btns[i].inner.set(ui, r);
            ui.fill_rect(r, if i == self.tab { WHITE } else { Color::new(0.06, 0.07, 0.09, 0.9) });
            ui.text(TABS[i])
                .pos(r.center().x, r.center().y)
                .anchor(0.5, 0.5)
                .size(0.34)
                .color(if i == self.tab { BLACK } else { WHITE })
                .draw();
        }
        // 左侧列表
        let list = Rect::new(-0.93, tab_y + 0.10, 0.92, top * 2. - 0.28);
        ui.fill_rect(list, Color::new(0.035, 0.04, 0.05, 0.72));
        let row_h = 0.085;
        let rows = ((list.h / row_h) as usize).max(1);
        let start = (self.selected.saturating_sub(rows / 2)).min(self.charts.len().saturating_sub(rows));
        for (n, i) in (start..self.charts.len()).take(rows).enumerate() {
            let r = Rect::new(list.x, list.y + n as f32 * row_h, list.w, row_h);
            self.row_btns[i].inner.set(ui, r);
            let active = i == self.selected;
            ui.fill_rect(r, if active { Color::new(0.16, 0.18, 0.22, 0.95) } else { Color::new(0.02, 0.025, 0.035, 0.45) });
            let c = &self.charts[i];
            ui.text(&c.info.name)
                .pos(r.x + 0.03, r.y + 0.025)
                .size(0.46)
                .color(WHITE)
                .max_width(r.w - 0.2)
                .draw();
            ui.text(&c.info.composer)
                .pos(r.x + 0.03, r.y + 0.06)
                .size(0.26)
                .color(Color::new(1., 1., 1., 0.55))
                .max_width(r.w - 0.2)
                .draw();
            ui.text(format!("{:.0}", c.info.difficulty))
                .pos(r.right() - 0.03, r.center().y)
                .anchor(1., 0.5)
                .size(0.44)
                .draw();
        }
        // 右侧详情
        let detail = Rect::new(0.01, tab_y + 0.10, 0.92, top * 2. - 0.28);
        ui.fill_rect(detail, Color::new(0.03, 0.035, 0.045, 0.72));
        let selected_chart = self.charts.get(self.selected).cloned();
        let selected_cover = self.cover(self.selected);
        if let Some(chart) = selected_chart {
            if let Some(tex) = selected_cover {
                let art = Rect::new(detail.x + 0.02, detail.y + 0.02, detail.w - 0.04, detail.h * 0.60);
                ui.fill_rect(art, (*tex, art, ScaleType::CropCenter, WHITE));
            }
            let cy = detail.y + detail.h * 0.64;
            ui.text(&chart.info.name)
                .pos(detail.x + 0.04, cy)
                .size(0.58)
                .max_width(detail.w - 0.08)
                .draw();
            ui.text(format!("{}  ·  {}", chart.info.composer, chart.info.charter))
                .pos(detail.x + 0.04, cy + 0.055)
                .size(0.30)
                .color(Color::new(1., 1., 1., 0.7))
                .max_width(detail.w - 0.08)
                .draw();
            let d = chart.info.difficulty;
            let diff_r = Rect::new(detail.x + 0.04, cy + 0.11, 0.22, 0.09);
            ui.fill_rect(diff_r, Color::new(0.8, 0.12, 0.16, 1.));
            ui.text(format!("{:.0}", d))
                .pos(diff_r.center().x, diff_r.center().y)
                .anchor(0.5, 0.5)
                .size(0.52)
                .draw();
            ui.text(&chart.info.level)
                .pos(diff_r.right() + 0.03, diff_r.center().y)
                .anchor(0., 0.5)
                .size(0.30)
                .color(Color::new(1., 1., 1., 0.7))
                .draw();
            if let Some(rec) = &chart.record {
                ui.text(format!("{:07}", rec.score))
                    .pos(detail.right() - 0.04, cy + 0.10)
                    .anchor(1., 0.)
                    .size(0.58)
                    .draw();
                ui.text(format!("{:.2}%", rec.accuracy))
                    .pos(detail.right() - 0.04, cy + 0.16)
                    .anchor(1., 0.)
                    .size(0.28)
                    .color(Color::new(1., 1., 1., 0.7))
                    .draw();
            }
        }
        let pr = Rect::new(0.55, top - 0.15, 0.35, 0.10);
        self.play_btn.inner.set(ui, pr);
        ui.fill_rect(pr, WHITE);
        ui.text("▶")
            .pos(pr.center().x, pr.center().y)
            .anchor(0.5, 0.5)
            .size(0.52)
            .color(BLACK)
            .draw();
        Ok(())
    }

    fn next_scene(&mut self, _s: &mut SharedState) -> NextScene {
        self.ready.take().unwrap_or(NextScene::None)
    }
}
