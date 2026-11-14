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
    ext::{draw_parallelogram, poll_future, LocalTask, RectExt, SafeTexture, ScaleType},
    scene::{NextScene, GameMode},
    ui::{DRectButton, Scroll, Ui},
};
use std::{borrow::Cow, collections::HashMap, path::PathBuf};


fn load_ui_icon(name: &str) -> Option<SafeTexture> {
    let path = std::path::Path::new("assets/challenge_web").join(name);
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(Texture2D::from_rgba8(w as u16, h as u16, &img).into())
}


fn load_rank_icon(name: &str) -> Option<SafeTexture> {
    let path = std::path::Path::new("assets/rank").join(name);
    let bytes = std::fs::read(path).ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(Texture2D::from_rgba8(w as u16, h as u16, &img).into())
}

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
    icon_sort: Option<SafeTexture>,
    icon_random: Option<SafeTexture>,
    icon_setting: Option<SafeTexture>,
    icon_play: Option<SafeTexture>,
    rank_icons: [Option<SafeTexture>; 6],
}

impl PhigrosLibraryPage {
    pub fn new() -> Self {
        let mut this = Self {
            charts: Vec::new(),
            selected: 2,
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
            icon_sort: load_ui_icon("sort.png"),
            icon_random: load_ui_icon("Random.png"),
            icon_setting: load_ui_icon("setting.png"),
            icon_play: load_ui_icon("play.png"),
            rank_icons: [
                load_rank_icon("phi.png"),
                load_rank_icon("V.png"),
                load_rank_icon("S.png"),
                load_rank_icon("A.png"),
                load_rank_icon("B.png"),
                load_rank_icon("F.png"),
            ],
        };
        this.reload();
        this
    }

    fn reload(&mut self) {
        self.charts = get_data().charts.clone();
        self.charts.sort_by(|a, b| b.info.name.cmp(&a.info.name));
        self.row_btns = (0..self.charts.len()).map(|_| DRectButton::new()).collect();
        if self.selected >= self.charts.len() {
            self.selected = self.charts.len().saturating_sub(1);
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
        Cow::Borrowed("")
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
        let top = ui.top;
        let x = |f: f32| -1. + 2. * f;
        let y = |f: f32| -top + 2. * top * f;
        // 对应 challenge_ui.html: list-bg
        ui.fill_rect(Rect::new(-1., -top, x(0.458) + 1., top * 2.), Color::new(0., 0., 0., 0.70));
        // 顶部 main-panel
        let panel = Rect::new(x(0.125), y(0.040), x(0.487) - x(0.125), y(0.130) - y(0.040));
        draw_parallelogram(panel, None, Color::new(0., 0., 0., 0.75), false);
        ui.text("Sort by").pos(x(0.125), y(0.040)).size(0.20).color(Color::new(0.42, 0.45, 0.50, 1.)).draw();
        if let Some(tex) = &self.icon_sort {
            let r = Rect::new(x(0.125), y(0.060), 0.035, 0.035);
            ui.fill_rect(r, (tex.clone().into_inner(), r, ScaleType::Fit, WHITE));
        }
        ui.text("Difficulty").pos(x(0.170), y(0.066)).size(0.26).draw();
        if let Some(tex) = &self.icon_random {
            let r = Rect::new(x(0.330), y(0.045), 0.040, 0.040);
            ui.fill_rect(r, (tex.clone().into_inner(), r, ScaleType::Fit, WHITE));
        }
        ui.text("Random").pos(x(0.345), y(0.068)).size(0.20).draw();
        if let Some(tex) = &self.icon_setting {
            let r = Rect::new(x(0.425), y(0.045), 0.040, 0.040);
            ui.fill_rect(r, (tex.clone().into_inner(), r, ScaleType::Fit, WHITE));
        }
        ui.text("Settings").pos(x(0.440), y(0.068)).size(0.20).draw();
        // 分类标签
        let tabs = [(0.120, 0.135), (0.180, 0.135), (0.240, 0.135), (0.300, 0.135), (0.360, 0.135), (0.120, 0.185), (0.210, 0.185)];
        for i in 0..7 {
            let (fx, fy) = tabs[i];
            let r = Rect::new(x(fx), y(fy), x(0.058) - x(0.), y(0.043) - y(0.));
            self.tab_btns[i].inner.set(ui, r);
            draw_parallelogram(r, None, if i == self.tab { WHITE } else { Color::new(0., 0., 0., 0.55) }, false);
            ui.text(TABS[i]).pos(r.center().x, r.center().y).anchor(0.5, 0.5).size(0.30).color(if i == self.tab { BLACK } else { Color::new(0.78, 0.78, 0.78, 1.) }).draw();
        }
        // 列表：80px 行高，选中行 38px
        let list = Rect::new(x(0.106), y(0.155), x(0.406) - x(0.106), y(0.97) - y(0.155));
        let row_h = (y(0.98) - y(0.18)) / 7.2;
        let rows = 7usize;
        let start = self.selected.saturating_sub(3).min(self.charts.len().saturating_sub(rows));
        let sel_y = list.y + (self.selected.saturating_sub(start)) as f32 * row_h;
        draw_parallelogram(Rect::new(x(0.100), sel_y + row_h * 0.08, x(0.406) - x(0.100), row_h * 0.84), None, Color::new(0., 0., 0., 0.85), false);
        if let Some(c) = self.charts.get(self.selected) {
            let dr = Rect::new(x(0.368), sel_y + row_h * 0.5 - (y(0.106) - y(0.)) / 2., x(0.440) - x(0.368), y(0.106) - y(0.));
            draw_parallelogram(dr, None, WHITE, false);
            ui.text(format!("{:.0}", c.info.difficulty)).pos(dr.center().x, dr.center().y - 0.010).anchor(0.5, 0.5).size(0.50).color(BLACK).draw();
            ui.text(c.info.level.split_whitespace().next().unwrap_or("IN")).pos(dr.center().x, dr.center().y + 0.030).anchor(0.5, 0.5).size(0.22).color(BLACK).draw();
        }
        let mut active_row = Rect::new(0., 0., 0., 0.);
        for (n, i) in (start..self.charts.len()).take(rows).enumerate() {
            let r = Rect::new(list.x, list.y + n as f32 * row_h, list.w, row_h);
            self.row_btns[i].inner.set(ui, r);
            let active = i == self.selected;
            if active { active_row = r; }
            let c = &self.charts[i];
            ui.text(&c.info.name)
                .pos(x(0.115), r.y + r.h * 0.18)
                .size(if active { 0.50 } else { 0.34 })
                .color(if active { WHITE } else { Color::new(0.78, 0.78, 0.78, 1.) })
                .max_width(x(0.36) - x(0.115))
                .draw();
            if active {
                ui.text(&c.info.composer).pos(x(0.115), r.y + r.h * 0.63).size(0.24).color(Color::new(0.6, 0.6, 0.6, 1.)).draw();
            }
            if !active {
                ui.text(format!("{:.0}", c.info.difficulty)).pos(x(0.400), r.center().y).anchor(1., 0.5).size(0.42).draw();
            }
        }
        // Mirror
        let mr = Rect::new(x(0.330), y(0.720), x(0.400) - x(0.330), y(0.780) - y(0.720));
        self.mirror_rect = mr;
        draw_parallelogram(mr, None, if self.mirror { WHITE } else { Color::new(0.06, 0.07, 0.09, 0.95) }, false);
        ui.text("Mirror").pos(mr.center().x, mr.center().y).anchor(0.5, 0.5).size(0.26).color(if self.mirror { BLACK } else { WHITE }).draw();
        // jacket-container: top213 right40 w860 h484
        let art = Rect::new(x(0.440), y(0.140), x(0.920) - x(0.440), y(0.700) - y(0.140));
        let selected_chart = self.charts.get(self.selected).cloned();
        let selected_cover = self.cover(self.selected);
        if let Some(chart) = selected_chart.as_ref() {
            if let Some(tex) = selected_cover {
                draw_parallelogram(art, Some((tex.clone().into_inner(), Rect::new(0., 0., 1., 1.))), WHITE, false);
            } else {
                draw_parallelogram(art, None, Color::new(0.06, 0.07, 0.09, 1.), false);
            }
            ui.text(format!("♫  {}", chart.info.name)).pos(art.x + 0.03, art.bottom() - 0.070).size(0.28).draw();
            ui.text(format!("music  ·  made by {}", chart.info.charter)).pos(art.x + 0.03, art.bottom() - 0.035).size(0.20).color(Color::new(1., 1., 1., 0.75)).draw();
        }
        // elem3: diff-list + score
        let labels = ["EZ", "HD", "IN", "AT"];
        let current = selected_chart.as_ref().map(|c| if c.info.level.starts_with("AT") { 3 } else if c.info.level.starts_with("IN") { 2 } else if c.info.level.starts_with("HD") { 1 } else { 0 }).unwrap_or(0);
        let nums = selected_chart.as_ref().map(|c| [3, 8, c.info.difficulty.round() as i32, 15]).unwrap_or([3, 8, 12, 15]);
        for i in 0..4 {
            let r = Rect::new(x(0.380 + i as f32 * 0.040), y(0.660), x(0.420) - x(0.380), y(0.740) - y(0.660));
            draw_parallelogram(r, None, if i == current { Color::new(0.80, 0.16, 0.16, 1.) } else { WHITE }, false);
            ui.text(labels[i]).pos(r.center().x, r.y + r.h * 0.20).anchor(0.5, 0.).size(0.18).color(if i == current { WHITE } else { BLACK }).draw();
            ui.text(format!("{}", nums[i])).pos(r.center().x, r.y + r.h * 0.50).anchor(0.5, 0.).size(0.30).color(if i == current { WHITE } else { BLACK }).draw();
        }
        if let Some(chart) = &selected_chart {
            let score = chart.record.as_ref().map_or(0, |r| r.score);
            ui.text(format!("{:07}", score)).pos(x(0.570), y(0.665)).size(0.46).draw();
            if let Some(rec) = &chart.record {
                ui.text(format!("{:.2}%", rec.accuracy)).pos(x(0.570), y(0.710)).size(0.22).color(Color::new(1., 1., 1., 0.7)).draw();
            }
            let ri = if score >= 1_000_000 { 0 } else if score >= 960_000 { 1 } else if score >= 920_000 { 2 } else if score >= 880_000 { 3 } else if score >= 820_000 { 4 } else { 5 };
            if let Some(tex) = &self.rank_icons[ri] {
                let rr = Rect::new(x(0.760), y(0.650), x(0.830) - x(0.760), y(0.750) - y(0.650));
                ui.fill_rect(rr, (tex.clone().into_inner(), rr, ScaleType::Fit, WHITE));
            }
        }
        // start-button: right0 bottom -30 w400 h200
        let pr = Rect::new(x(0.900), y(0.680), x(1.0) - x(0.900), y(0.840) - y(0.680));
        self.play_btn.inner.set(ui, pr);
        draw_parallelogram(pr, None, Color::new(1., 1., 1., 0.95), false);
        if let Some(tex) = &self.icon_play {
            let ir = Rect::new(pr.x + 0.03, pr.y + 0.02, pr.w - 0.06, pr.h - 0.04);
            ui.fill_rect(ir, (tex.clone().into_inner(), ir, ScaleType::Fit, WHITE));
        } else {
            ui.text("▶").pos(pr.center().x, pr.center().y).anchor(0.5, 0.5).size(0.60).color(BLACK).draw();
        }
        Ok(())
    }

    fn next_scene(&mut self, _s: &mut SharedState) -> NextScene {
        self.ready.take().unwrap_or(NextScene::None)
    }
}
