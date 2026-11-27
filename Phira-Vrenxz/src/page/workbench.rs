//! 制谱工作台：在客户端内直接编辑 RPE 谱面（时间轴 + 判定线 + 音符）。
//!
//! 编辑能力（对齐 PhiraPro / RPE 制谱工作台的核心工作流）：
//! - 扫描 `data/charts/custom` 下的谱面并选择打开（只认 RPE，PEC 提示不支持）
//! - 以判定线为轨的时间轴，滚轮平移 / `+` `-` 缩放 / `Ctrl+滚轮` 缩放
//! - 点击空白落音符，点住音符拖动改时间与横向位置
//! - 1/4、1/8、1/16 拍吸附；蓝键 / 长条 / 黄键 / 绿键；线上 / 线下
//! - 长条自动带尾巴且随动、删除、撤销（20 步）、直接写回 RPE json（自动 `.bak`）
//!
//! 键盘：`←` `→` 移动拍游标，`Del`/`Backspace` 删选中音符，`Ctrl+Z` 撤销，
//! `Ctrl+S` 保存，`Home` 回到开头。

use std::{borrow::Cow, fs, path::PathBuf};

use super::{Page, SharedState};
use anyhow::{Context, Result};
use macroquad::prelude::*;
use prpr::{
    ext::{semi_white, RectExt},
    ui::Ui,
};
use serde_json::{json, Value};

const RPE_WIDTH: f64 = 1350.;
const DEN: f64 = 192.;
const UNDO_LIMIT: usize = 20;

const KIND_TAP: u32 = 1;
const KIND_HOLD: u32 = 2;
const KIND_FLICK: u32 = 3;
const KIND_DRAG: u32 = 4;

struct ChartEntry {
    path: PathBuf,
    name: String,
    level: String,
}

#[derive(Clone)]
struct NoteView {
    index: usize,
    beat: f64,
    end_beat: f64,
    x: f32,
    kind: u32,
    above: bool,
}

struct LineSummary {
    name: String,
    notes: usize,
}

#[derive(Clone, Copy)]
struct Drag {
    note: usize,
    start: Vec2,
    beat: f64,
    end_beat: f64,
    x: f32,
    /// 横向每像素对应的拍数（时间轴是横轴）
    beat_per_px: f32,
    /// 纵向每像素对应的 positionX
    x_per_px: f32,
}

struct Editor {
    path: PathBuf,
    value: Value,
    name: String,
    level: String,
    bpm: Vec<(f64, f64)>,
    lines: Vec<LineSummary>,
    line: usize,
    notes: Vec<NoteView>,
    view_start: f64,
    view_span: f64,
    snap: f64,
    cursor_beat: f64,
    new_kind: u32,
    new_above: bool,
    sel: Option<usize>,
    drag: Option<Drag>,
    undo: Vec<(usize, Option<Value>)>,
    dirty: bool,
}

fn triple_beats(v: &Value) -> f64 {
    let a = v.get(0).and_then(|x| x.as_f64()).unwrap_or(0.);
    let b = v.get(1).and_then(|x| x.as_f64()).unwrap_or(0.);
    let c = v.get(2).and_then(|x| x.as_f64()).unwrap_or(1.);
    if c.abs() < 1e-6 { a } else { a + b / c }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.abs() } else { gcd(b, a % b) }
}

fn beats_triple(beat: f64) -> Value {
    let beat = beat.max(0.);
    let mut int = beat.floor();
    let mut num = ((beat - int) * DEN).round() as i64;
    let mut den = DEN as i64;
    if num >= den {
        int += (num / den) as f64;
        num %= den;
    }
    if num == 0 {
        return json!([int as i64, 0, 1]);
    }
    let g = gcd(num, den).max(1);
    num /= g;
    den /= g;
    json!([int as i64, num, den])
}

impl Editor {
    fn open(path: PathBuf) -> Result<Self> {
        let bytes = fs::read(&path).with_context(|| format!("读取谱面失败：{}", path.display()))?;
        let value: Value = serde_json::from_slice(&bytes)?;
        if value.get("judgeLineList").is_none() {
            anyhow::bail!("不是 RPE 谱面（缺少 judgeLineList）");
        }
        let meta = value.get("META").cloned().unwrap_or(Value::Null);
        let name = meta.get("name").and_then(|x| x.as_str()).unwrap_or("未命名").to_string();
        let level = meta.get("level").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let mut bpm = Vec::new();
        if let Some(list) = value.get("BPMList").and_then(|x| x.as_array()) {
            for item in list {
                let b = item.get("bpm").and_then(|x| x.as_f64()).unwrap_or(120.);
                let start = item.get("startTime").map(triple_beats).unwrap_or(0.);
                bpm.push((start, b));
            }
        }
        if bpm.is_empty() {
            bpm.push((0., 120.));
        }
        bpm.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut ed = Self {
            path,
            value,
            name,
            level,
            bpm,
            lines: Vec::new(),
            line: 0,
            notes: Vec::new(),
            view_start: 0.,
            view_span: 16.,
            snap: 0.25,
            cursor_beat: 0.,
            new_kind: KIND_TAP,
            new_above: true,
            sel: None,
            drag: None,
            undo: Vec::new(),
            dirty: false,
        };
        ed.refresh_lines();
        ed.rebuild_notes();
        if let Some(first) = ed.notes.first() {
            ed.cursor_beat = first.beat;
            ed.view_start = (first.beat - 3.).max(0.);
        }
        Ok(ed)
    }

    fn beat_to_sec(&self, beat: f64) -> f64 {
        let mut idx = 0;
        while idx + 1 < self.bpm.len() && self.bpm[idx + 1].0 <= beat {
            idx += 1;
        }
        let (start, b) = self.bpm[idx];
        let mut sec = 0.;
        for i in 1..=idx {
            let (s0, b0) = self.bpm[i - 1];
            let (s1, _) = self.bpm[i];
            sec += (s1 - s0) * 60. / b0;
        }
        sec + (beat - start) * 60. / b
    }

    fn refresh_lines(&mut self) {
        self.lines.clear();
        if let Some(arr) = self.value.get("judgeLineList").and_then(|x| x.as_array()) {
            for line in arr {
                self.lines.push(LineSummary {
                    name: line.get("Name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    notes: line.get("notes").and_then(|x| x.as_array()).map(|x| x.len()).unwrap_or(0),
                });
            }
        }
        if self.line >= self.lines.len() {
            self.line = 0;
        }
    }

    fn rebuild_notes(&mut self) {
        self.notes.clear();
        self.sel = None;
        self.drag = None;
        let line = match self
            .value
            .get("judgeLineList")
            .and_then(|x| x.as_array())
            .and_then(|x| x.get(self.line))
        {
            Some(it) => it,
            None => return,
        };
        let Some(arr) = line.get("notes").and_then(|x| x.as_array()) else {
            return;
        };
        for (index, n) in arr.iter().enumerate() {
            let beat = n.get("startTime").map(triple_beats).unwrap_or(0.);
            let end_beat = n.get("endTime").map(triple_beats).unwrap_or(beat).max(beat);
            let x = n.get("positionX").and_then(|x| x.as_f64()).unwrap_or(0.) as f32;
            let kind = n.get("type").and_then(|x| x.as_u64()).unwrap_or(1) as u32;
            let above = n.get("above").and_then(|x| x.as_u64()).unwrap_or(1) == 1;
            self.notes.push(NoteView { index, beat, end_beat, x, kind, above });
        }
    }

    fn notes_array(&self) -> Option<&Vec<Value>> {
        self.value
            .get("judgeLineList")?
            .as_array()?
            .get(self.line)?
            .get("notes")?
            .as_array()
    }

    fn push_undo(&mut self, index: usize) {
        let old = self.notes_array().and_then(|x| x.get(index)).cloned();
        self.undo.push((index, old));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
    }

    fn undo_step(&mut self) -> bool {
        let Some((index, old)) = self.undo.pop() else {
            return false;
        };
        let Some(notes) = self
            .value
            .get_mut("judgeLineList")
            .and_then(|x| x.as_array_mut())
            .and_then(|x| x.get_mut(self.line))
            .and_then(|x| x.get_mut("notes"))
            .and_then(|x| x.as_array_mut())
        else {
            return false;
        };
        match old {
            Some(v) => {
                if index < notes.len() {
                    notes[index] = v;
                }
            }
            None => {
                if index < notes.len() {
                    notes.remove(index);
                }
            }
        }
        self.dirty = true;
        self.refresh_lines();
        self.rebuild_notes();
        true
    }

    fn add_note(&mut self, beat: f64, x: f32) -> Result<()> {
        let beat = self.snap_beat(beat);
        let start = beats_triple(beat);
        let end = if self.new_kind == KIND_HOLD {
            beats_triple(beat + 1.)
        } else {
            start.clone()
        };
        let note = json!({
            "above": if self.new_above { 1 } else { 2 },
            "alpha": 255,
            "endTime": end,
            "isFake": 0,
            "judgeArea": 1,
            "positionX": (x as f64) * RPE_WIDTH / 2.,
            "size": 1,
            "speed": 1,
            "startTime": start,
            "tint": [255, 255, 255],
            "type": self.new_kind,
            "visibleTime": 999999,
            "yOffset": 0
        });
        let line = self
            .value
            .get_mut("judgeLineList")
            .and_then(|x| x.as_array_mut())
            .and_then(|x| x.get_mut(self.line))
            .context("判定线不存在")?;
        if line.get("notes").is_none() {
            line["notes"] = json!([]);
        }
        let notes = line.get_mut("notes").and_then(|x| x.as_array_mut()).context("notes 不是数组")?;
        let index = notes.len();
        notes.push(note);
        self.undo.push((index, None));
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.dirty = true;
        self.refresh_lines();
        self.rebuild_notes();
        self.sel = self.notes.iter().position(|it| it.index == index);
        Ok(())
    }

    fn delete_selected(&mut self) -> bool {
        if self.drag.is_some() {
            return false;
        }
        let Some(sel) = self.sel else {
            return false;
        };
        let Some(note) = self.notes.get(sel).cloned() else {
            return false;
        };
        self.push_undo(note.index);
        if let Some(notes) = self
            .value
            .get_mut("judgeLineList")
            .and_then(|x| x.as_array_mut())
            .and_then(|x| x.get_mut(self.line))
            .and_then(|x| x.get_mut("notes"))
            .and_then(|x| x.as_array_mut())
        {
            if note.index < notes.len() {
                notes.remove(note.index);
            }
        }
        self.dirty = true;
        self.refresh_lines();
        self.rebuild_notes();
        true
    }

    fn snap_beat(&self, beat: f64) -> f64 {
        ((beat / self.snap).round() * self.snap).max(0.)
    }

    /// 拖动中：把预览缓存里的位置写回 json（不重建缓存，避免每帧抖动）
    fn write_drag(&mut self, pos: Vec2) {
        let Some(drag) = self.drag else {
            return;
        };
        let Some(view) = self.notes.get(drag.note).cloned() else {
            return;
        };
        let dv = pos - drag.start;
        // 横轴 = 时间（拍），纵轴 = 音符横向位置（positionX）
        let new_beat = self.snap_beat((drag.beat + (dv.x * drag.beat_per_px) as f64).max(0.));
        let new_x = (drag.x - dv.y * drag.x_per_px).clamp(-1.15, 1.15);
        let delta = new_beat - drag.beat;
        let new_end = (drag.end_beat + delta).max(new_beat);
        if let Some(notes) = self
            .value
            .get_mut("judgeLineList")
            .and_then(|x| x.as_array_mut())
            .and_then(|x| x.get_mut(self.line))
            .and_then(|x| x.get_mut("notes"))
            .and_then(|x| x.as_array_mut())
        {
            if let Some(n) = notes.get_mut(view.index) {
                n["startTime"] = beats_triple(new_beat);
                n["endTime"] = beats_triple(if view.kind == KIND_HOLD { new_end } else { new_beat });
                n["positionX"] = json!((new_x as f64) * RPE_WIDTH / 2.);
            }
        }
        if let Some(view) = self.notes.get_mut(drag.note) {
            view.beat = new_beat;
            view.end_beat = if view.kind == KIND_HOLD { new_end } else { new_beat };
            view.x = new_x;
        }
    }

    fn save(&mut self) -> Result<()> {
        let text = serde_json::to_string(&self.value)?;
        let mut bak = self.path.clone().into_os_string();
        bak.push(".bak");
        let bak = PathBuf::from(bak);
        if !bak.exists() {
            fs::copy(&self.path, &bak).ok();
        }
        fs::write(&self.path, text).with_context(|| format!("写入失败：{}", self.path.display()))?;
        self.dirty = false;
        Ok(())
    }
}

pub struct WorkbenchPage {
    charts: Vec<ChartEntry>,
    list_off: f32,
    editing: Option<Editor>,
    status: String,
    status_at: f32,
}

struct Layout {
    list: Rect,
    grid: Rect,
    tool: Rect,
    header_y: f32,
}

fn ui_top() -> f32 {
    let w = screen_width().max(1.);
    let h = screen_height().max(1.);
    (h / w).clamp(0.2, 4.)
}

fn layout_of() -> Layout {
    let top = ui_top();
    let tool_h = 0.19;
    let header = 0.10;
    let list = Rect::new(-0.98, -top + tool_h + 0.02, 0.34, (top * 2.) - tool_h - header - 0.06);
    let grid = Rect::new(-0.60, -top + tool_h + 0.02, 1.58, (top * 2.) - tool_h - header - 0.06);
    let tool = Rect::new(-0.98, -top + 0.02, 1.96, tool_h - 0.04);
    Layout { list, grid, tool, header_y: top - 0.05 }
}

fn beat_to_x(g: Rect, start: f64, span: f64, beat: f64) -> f32 {
    g.x + ((beat - start) / span) as f32 * g.w
}

fn x_to_beat(g: Rect, start: f64, span: f64, x: f32) -> f64 {
    start + ((x - g.x) / g.w) as f64 * span
}

fn posx_to_y(g: Rect, x: f32) -> f32 {
    g.center().y - x / 1.15 * (g.h * 0.5)
}

fn y_to_posx(g: Rect, y: f32) -> f32 {
    ((g.center().y - y) / (g.h * 0.5)) * 1.15
}

fn extract_field(head: &str, key: &str) -> Option<String> {
    let pos = head.find(key)?;
    let rest = &head[pos + key.len()..];
    let start = rest.find('"')? + 1;
    let rest = &rest[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

impl WorkbenchPage {
    pub fn new() -> Result<Self> {
        Ok(Self {
            charts: Self::scan(),
            list_off: 0.,
            editing: None,
            status: String::new(),
            status_at: -10.,
        })
    }

    fn scan() -> Vec<ChartEntry> {
        let mut out = Vec::new();
        let root = match crate::dir::custom_charts() {
            Ok(it) => it,
            Err(_) => return out,
        };
        let Ok(dirs) = fs::read_dir(&root) else {
            return out;
        };
        for entry in dirs.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let Ok(files) = fs::read_dir(&dir) else {
                continue;
            };
            for f in files.flatten() {
                let path = f.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                let Ok(bytes) = fs::read(&path) else {
                    continue;
                };
                let head_len = bytes.len().min(4096);
                let head = String::from_utf8_lossy(&bytes[..head_len]);
                if !head.contains("judgeLineList") && !head.contains("judgeLineGroup") {
                    continue;
                }
                let name = extract_field(&head, "\"name\"").unwrap_or_else(|| {
                    path.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default()
                });
                let level = extract_field(&head, "\"level\"").unwrap_or_default();
                out.push(ChartEntry { path, name, level });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    fn say(&mut self, s: &SharedState, msg: impl Into<String>) {
        self.status = msg.into();
        self.status_at = s.t;
    }

    fn kind_label(kind: u32) -> &'static str {
        match kind {
            KIND_HOLD => "长条",
            KIND_FLICK => "黄键",
            KIND_DRAG => "绿键",
            _ => "蓝键",
        }
    }

    fn kind_color(kind: u32) -> Color {
        match kind {
            KIND_HOLD => Color::new(0.36, 0.69, 1., 1.),
            KIND_FLICK => Color::new(1., 0.86, 0.34, 1.),
            KIND_DRAG => Color::new(0.47, 0.9, 0.4, 1.),
            _ => Color::new(0.56, 0.78, 1., 1.),
        }
    }

    fn button(ui: &mut Ui, r: Rect, label: &str, active: bool, size: f32) {
        let col = if active {
            Color::new(0.92, 0.42, 0.42, 0.92)
        } else {
            Color::new(1., 1., 1., 0.12)
        };
        ui.fill_path(&r.rounded(0.008), col);
        ui.text(label)
            .pos(r.center().x, r.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(size)
            .max_width(r.w - 0.008)
            .color(semi_white(1.))
            .draw();
    }

    fn open_chart(&mut self, s: &SharedState, idx: usize) {
        let Some(c) = self.charts.get(idx) else { return };
        let path = c.path.clone();
        match Editor::open(path) {
            Ok(ed) => {
                self.say(s, format!("已打开：{}", ed.name));
                self.editing = Some(ed);
            }
            Err(e) => self.say(s, format!("打开失败：{e}")),
        }
    }
}

impl WorkbenchPage {
    fn page_can_play_bgm(&self) -> bool {
        false
    }

    fn page_label(&self) -> Cow<'static, str> {
        Cow::Borrowed(crate::tr!("制谱工作台"))
    }

    fn page_back(&mut self) -> bool {
        if self.editing.is_some() {
            self.editing = None;
            true
        } else {
            false
        }
    }

    fn page_update(&mut self, s: &mut SharedState) -> Result<()> {
        let mut editing = self.editing.take();
        let r = self.update_editor(s, &mut editing);
        self.editing = editing;
        r
    }

    fn page_touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let lg = layout_of();
        let mut editing = self.editing.take();
        let r = self.touch_editor(touch, s, lg, &mut editing);
        self.editing = editing;
        r
    }
}

impl WorkbenchPage {
    fn update_editor(&mut self, s: &mut SharedState, editing: &mut Option<Editor>) -> Result<()> {
        let wheel = mouse_wheel().1;
        if let Some(ed) = editing {
            if wheel.abs() > 0.01 {
                if is_key_down(KeyCode::LeftControl) {
                    ed.view_span = (ed.view_span * (1. - wheel as f64 * 0.1)).clamp(1.0, 96.);
                } else {
                    ed.view_start = (ed.view_start - wheel as f64 * ed.view_span * 0.08).max(0.);
                }
            }
            if is_key_pressed(KeyCode::Equal) {
                ed.view_span = (ed.view_span * 0.85).clamp(1.0, 96.);
            }
            if is_key_pressed(KeyCode::Minus) {
                ed.view_span = (ed.view_span * 1.18).clamp(1.0, 96.);
            }
            if is_key_pressed(KeyCode::Delete) || is_key_pressed(KeyCode::Backspace) {
                if ed.delete_selected() {
                    self.say(s, "已删除音符");
                }
            }
            if is_key_pressed(KeyCode::Left) {
                ed.cursor_beat = (ed.cursor_beat - ed.snap).max(0.);
            }
            if is_key_pressed(KeyCode::Right) {
                ed.cursor_beat += ed.snap;
            }
            if is_key_pressed(KeyCode::Home) {
                ed.view_start = 0.;
                ed.cursor_beat = 0.;
            }
            if is_key_pressed(KeyCode::Z) && is_key_down(KeyCode::LeftControl) {
                if ed.undo_step() {
                    self.say(s, "已撤销");
                }
            }
            if is_key_pressed(KeyCode::S) && is_key_down(KeyCode::LeftControl) {
                match ed.save() {
                    Ok(_) => self.say(s, "已保存"),
                    Err(e) => self.say(s, format!("保存失败：{e}")),
                }
            }
        } else if wheel.abs() > 0.01 {
            self.list_off = (self.list_off - wheel * 0.08 * 2.).max(0.);
        }
        Ok(())
    }
}

impl WorkbenchPage {
    fn touch_editor(
        &mut self,
        touch: &Touch,
        s: &mut SharedState,
        lg: Layout,
        editing: &mut Option<Editor>,
    ) -> Result<bool> {
        match touch.phase {
            TouchPhase::Started => {
                if editing.is_none() {
                    let top = ui_top();
                    let panel = Rect::new(-0.9, -top + 0.1, 1.8, (top * 2.) - 0.36);
                    let row_h = 0.085;
                    let visible = (panel.h / row_h).floor() as usize;
                    if panel.contains(touch.position) {
                        for i in 0..self.charts.len().min(visible) {
                            let idx = i + (self.list_off / row_h) as usize;
                            if idx >= self.charts.len() {
                                break;
                            }
                            let r = Rect::new(
                                panel.x + 0.02,
                                panel.y + panel.h - 0.03 - (i as f32 + 1.) * row_h + 0.01,
                                panel.w - 0.04,
                                row_h - 0.012,
                            );
                            if r.contains(touch.position) {
                                self.open_chart(s, idx);
                                return Ok(true);
                            }
                        }
                    }
                    return Ok(false);
                }
                let Some(ed) = editing.as_mut() else { return Ok(false) };
                let p = touch.position;
                // 判定线列表
                if lg.list.contains(p) {
                    let row_h = (lg.list.h / ed.lines.len().max(1) as f32).min(0.07);
                    let mut hit = None;
                    for (i, _) in ed.lines.iter().enumerate() {
                        let y = lg.list.y + lg.list.h - 0.02 - (i as f32 + 1.) * row_h;
                        if y < lg.list.y {
                            break;
                        }
                        if Rect::new(lg.list.x + 0.015, y, lg.list.w - 0.03, row_h - 0.006).contains(p) {
                            hit = Some(i);
                            break;
                        }
                    }
                    if let Some(i) = hit {
                        ed.line = i;
                        ed.rebuild_notes();
                    }
                    return Ok(true);
                }
                // 工具条
                if lg.tool.contains(p) {
                    let bh = lg.tool.h - 0.03;
                    let y = lg.tool.y + 0.015;
                    let mut x = lg.tool.x + 0.02;
                    let bw = 0.13;
                    for kind in [KIND_TAP, KIND_HOLD, KIND_FLICK, KIND_DRAG] {
                        if Rect::new(x, y, bw, bh).contains(p) {
                            ed.new_kind = kind;
                            return Ok(true);
                        }
                        x += bw + 0.015;
                    }
                    let above_r = Rect::new(x, y, 0.15, bh);
                    if above_r.contains(p) {
                        ed.new_above = !ed.new_above;
                        return Ok(true);
                    }
                    x += 0.165;
                    let snap_r = Rect::new(x, y, 0.12, bh);
                    if snap_r.contains(p) {
                        ed.snap = if ed.snap >= 0.24 { 0.125 } else if ed.snap >= 0.124 { 0.0625 } else { 0.25 };
                        return Ok(true);
                    }
                    x += 0.135;
                    let undo_r = Rect::new(x, y, 0.12, bh);
                    if undo_r.contains(p) {
                        if ed.undo_step() {
                            self.say(s, "已撤销");
                        }
                        return Ok(true);
                    }
                    x += 0.135;
                    let save_r = Rect::new(x, y, 0.12, bh);
                    if save_r.contains(p) {
                        match ed.save() {
                            Ok(_) => self.say(s, "已保存"),
                            Err(e) => self.say(s, format!("保存失败：{e}")),
                        }
                        return Ok(true);
                    }
                    x += 0.135;
                    let back_r = Rect::new(x, y, 0.12, bh);
                    if back_r.contains(p) {
                        *editing = None;
                        return Ok(true);
                    }
                    return Ok(true);
                }
                // 网格
                if lg.grid.contains(p) {
                    let g = lg.grid;
                    let mut hit: Option<(usize, f32)> = None;
                    for (ci, n) in ed.notes.iter().enumerate() {
                        let nx = beat_to_x(g, ed.view_start, ed.view_span, n.beat);
                        let ny = posx_to_y(g, n.x);
                        let d = (Vec2::new(nx, ny) - p).length();
                        if d < 0.035 && hit.map(|(_, dd)| d < dd).unwrap_or(true) {
                            hit = Some((ci, d));
                        }
                    }
                    if let Some((ci, _)) = hit {
                        ed.sel = Some(ci);
                        let n = ed.notes[ci].clone();
                        let beat_per_px = (ed.view_span / lg.grid.w as f64) as f32;
                        let x_per_px = 1.15 / (lg.grid.h * 0.5);
                        ed.drag = Some(Drag {
                            note: ci,
                            start: p,
                            beat: n.beat,
                            end_beat: n.end_beat,
                            x: n.x,
                            beat_per_px,
                            x_per_px,
                        });
                    } else {
                        let beat = x_to_beat(g, ed.view_start, ed.view_span, p.x);
                        let px = y_to_posx(g, p.y);
                        match ed.add_note(beat, px) {
                            Ok(_) => self.say(s, "已添加音符"),
                            Err(e) => self.say(s, format!("{e}")),
                        }
                    }
                    return Ok(true);
                }
            }
            TouchPhase::Moved => {
                if let Some(ed) = editing.as_mut() {
                    if ed.drag.is_some() {
                        ed.write_drag(touch.position);
                        return Ok(true);
                    }
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if let Some(ed) = editing.as_mut() {
                    if ed.drag.is_some() {
                        ed.drag = None;
                        ed.dirty = true;
                        ed.rebuild_notes();
                        return Ok(true);
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

}

impl WorkbenchPage {
    fn page_render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        ui.fill_rect(ui.screen_rect(), Color::new(0.06, 0.07, 0.09, 1.));
        if self.editing.is_none() {
            self.render_browse(ui, s);
        } else {
            self.render_editor(ui);
        }
        let _ = t;
        Ok(())
    }
}

impl WorkbenchPage {
    fn render_browse(&mut self, ui: &mut Ui, _s: &mut SharedState) {
        let top = ui_top();
        ui.text(crate::tr!("制谱工作台"))
            .pos(-0.9, top - 0.09)
            .size(0.05)
            .color(semi_white(1.))
            .draw();
        ui.text("选择要编辑的 RPE 谱面（data/charts/custom）")
            .pos(-0.9, top - 0.15)
            .size(0.024)
            .color(semi_white(0.6))
            .draw();
        let panel = Rect::new(-0.9, -top + 0.1, 1.8, (top * 2.) - 0.36);
        ui.fill_path(&panel.rounded(0.02), Color::new(1., 1., 1., 0.05));
        if self.charts.is_empty() {
            ui.text(crate::tr!("无本地谱面"))
                .pos(0., 0.)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.03)
                .color(semi_white(0.7))
                .draw();
            return;
        }
        let row_h = 0.085;
        let visible = (panel.h / row_h).floor() as usize;
        let start_idx = (self.list_off / row_h) as usize;
        for i in 0..self.charts.len().min(visible) {
            let idx = i + start_idx;
            let Some(c) = self.charts.get(idx) else { break };
            let r = Rect::new(
                panel.x + 0.02,
                panel.y + panel.h - 0.03 - (i as f32 + 1.) * row_h + 0.01,
                panel.w - 0.04,
                row_h - 0.012,
            );
            ui.fill_path(&r.rounded(0.01), Color::new(1., 1., 1., 0.07));
            ui.text(&c.name)
                .pos(r.x + 0.03, r.y + r.h * 0.58)
                .size(0.028)
                .max_width(r.w - 0.24)
                .color(semi_white(0.95))
                .draw();
            ui.text(&c.level)
                .pos(r.right() - 0.03, r.y + r.h * 0.58)
                .anchor(1., 0.)
                .size(0.022)
                .color(semi_white(0.55))
                .draw();
        }
        if !self.status.is_empty() {
            ui.text(&self.status)
                .pos(0., -top + 0.06)
                .anchor(0.5, 0.)
                .no_baseline()
                .size(0.022)
                .color(Color::new(1., 0.75, 0.5, 0.9))
                .draw();
        }
    }

    fn render_editor(&mut self, ui: &mut Ui) {
        let lg = layout_of();
        let Some(ed) = &mut self.editing else { return };
        // 顶栏
        ui.text(format!("{}  {}", ed.name, ed.level))
            .pos(-0.98, lg.header_y)
            .size(0.034)
            .max_width(1.1)
            .color(semi_white(1.))
            .draw();
        ui.text(format!(
            "第 {} 线 · {} 音符 · 拍 {:.2} / {:.2}s",
            ed.line + 1,
            ed.lines.get(ed.line).map(|x| x.notes).unwrap_or(0),
            ed.cursor_beat,
            ed.beat_to_sec(ed.cursor_beat)
        ))
        .pos(0.9, lg.header_y)
        .anchor(1., 0.)
        .size(0.022)
        .color(semi_white(0.6))
        .draw();

        // 判定线列表
        ui.fill_path(&lg.list.rounded(0.014), Color::new(1., 1., 1., 0.05));
        let row_h = (lg.list.h / ed.lines.len().max(1) as f32).min(0.07);
        for (i, line) in ed.lines.iter().enumerate() {
            let y = lg.list.y + lg.list.h - 0.02 - (i as f32 + 1.) * row_h;
            if y < lg.list.y {
                break;
            }
            let r = Rect::new(lg.list.x + 0.015, y, lg.list.w - 0.03, row_h - 0.006);
            let sel = i == ed.line;
            ui.fill_path(
                &r.rounded(0.008),
                if sel { Color::new(0.92, 0.42, 0.42, 0.85) } else { Color::new(1., 1., 1., 0.07) },
            );
            let name = if line.name.is_empty() { format!("#{}", i + 1) } else { line.name.clone() };
            ui.text(&name)
                .pos(r.x + 0.02, r.y + r.h * 0.6)
                .size(0.021)
                .max_width(r.w - 0.1)
                .color(semi_white(0.95))
                .draw();
            ui.text(format!("{}", line.notes))
                .pos(r.right() - 0.015, r.y + r.h * 0.6)
                .anchor(1., 0.)
                .size(0.018)
                .color(semi_white(0.55))
                .draw();
        }

        // 网格
        let g = lg.grid;
        ui.fill_path(&g.rounded(0.014), Color::new(0., 0., 0., 0.65));
        let start_beat = ed.view_start.floor();
        let mut b = start_beat;
        while b <= ed.view_start + ed.view_span {
            let x = beat_to_x(g, ed.view_start, ed.view_span, b);
            if x >= g.x && x <= g.right() {
                let major = (b - b.floor()).abs() < 1e-6;
                ui.fill_rect(Rect::new(x, g.y, 0.002, g.h), Color::new(1., 1., 1., if major { 0.22 } else { 0.08 }));
            }
            b += 1.;
        }
        // 判定线（水平基准）
        ui.fill_rect(Rect::new(g.x, g.center().y - 0.002, g.w, 0.004), Color::new(1., 1., 1., 0.35));

        for (ci, n) in ed.notes.iter().enumerate() {
            let x0 = beat_to_x(g, ed.view_start, ed.view_span, n.beat);
            let x1 = beat_to_x(g, ed.view_start, ed.view_span, n.end_beat);
            if x1 < g.x - 0.02 || x0 > g.right() + 0.02 {
                continue;
            }
            let y = posx_to_y(g, n.x);
            if y < g.y || y > g.y + g.h {
                continue;
            }
            let sel = ed.sel == Some(ci);
            let col = Self::kind_color(n.kind);
            if n.end_beat > n.beat {
                let r = Rect::new(x0, y - 0.008, (x1 - x0).max(0.012), 0.016);
                ui.fill_path(&r.rounded(0.004), Color::new(col.r, col.g, col.b, 0.55));
            }
            let size = if sel { 0.026 } else { 0.02 };
            ui.fill_path(
                &Rect::new(x0 - size / 2., y - size / 2., size, size).rounded(0.006),
                if n.above { col } else { Color::new(col.r, col.g, col.b, 0.5) },
            );
            if sel {
                ui.text(format!("#{} {}", n.index + 1, Self::kind_label(n.kind)))
                    .pos(x0, y + 0.02)
                    .anchor(0.5, 0.)
                    .no_baseline()
                    .size(0.018)
                    .color(semi_white(0.9))
                    .draw();
            }
        }
        // 拍游标
        let cx = beat_to_x(g, ed.view_start, ed.view_span, ed.cursor_beat);
        if cx >= g.x && cx <= g.right() {
            ui.fill_rect(Rect::new(cx - 0.0015, g.y, 0.003, g.h), Color::new(0.95, 0.55, 0.35, 0.85));
        }

        // 底栏
        ui.fill_path(&lg.tool.rounded(0.012), Color::new(1., 1., 1., 0.06));
        let bh = lg.tool.h - 0.03;
        let y = lg.tool.y + 0.015;
        let mut x = lg.tool.x + 0.02;
        let bw = 0.13;
        for kind in [KIND_TAP, KIND_HOLD, KIND_FLICK, KIND_DRAG] {
            Self::button(ui, Rect::new(x, y, bw, bh), Self::kind_label(kind), ed.new_kind == kind, 0.022);
            x += bw + 0.015;
        }
        Self::button(ui, Rect::new(x, y, 0.15, bh), if ed.new_above { "线上" } else { "线下" }, false, 0.022);
        x += 0.165;
        Self::button(ui, Rect::new(x, y, 0.12, bh), &format!("1/{}", (1. / ed.snap).round() as i32), false, 0.022);
        x += 0.135;
        Self::button(ui, Rect::new(x, y, 0.12, bh), "撤销", false, 0.022);
        x += 0.135;
        Self::button(ui, Rect::new(x, y, 0.12, bh), "保存", false, 0.022);
        x += 0.135;
        Self::button(ui, Rect::new(x, y, 0.12, bh), "返回", false, 0.022);
        if ed.dirty {
            ui.text("未保存")
                .pos(lg.tool.right() - 0.02, y + bh / 2.)
                .anchor(1., 0.5)
                .no_baseline()
                .size(0.02)
                .color(Color::new(1., 0.7, 0.4, 1.))
                .draw();
        }

        // 状态
        if !self.status.is_empty() {
            ui.text(&self.status)
                .pos(0., lg.tool.y + lg.tool.h + 0.035)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.02)
                .color(semi_white(0.75))
                .draw();
        }
    }
}

impl Page for WorkbenchPage {
    fn can_play_bgm(&self) -> bool {
        self.page_can_play_bgm()
    }

    fn label(&self) -> Cow<'static, str> {
        self.page_label()
    }

    fn on_back_pressed(&mut self, _s: &mut SharedState) -> bool {
        self.page_back()
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        self.page_update(s)
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        self.page_touch(touch, s)
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        self.page_render(ui, s)
    }
}
