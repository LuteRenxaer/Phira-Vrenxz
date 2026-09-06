use super::{
    chart::ChartSettings,
    note::{note_render, FrameGeo, Geo, NoteRenderCfg, VertexSink},
    object::CtrlObject,
    Anim, AnimFloat, BpmList, Matrix, Note, NoteKind, Object, Point, RenderConfig, Resource, Vector,
};
use crate::{
    ext::{get_viewport, NotNanExt, SafeTexture},
    judge::JudgeStatus,
    ui::Ui,
};
use macroquad::prelude::*;
use miniquad::{RenderPass, Texture, TextureParams, TextureWrap};
use nalgebra::Rotation2;
use serde::Deserialize;
use std::cell::RefCell;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum UIElement {
    Pause = 1,
    ComboNumber = 2,
    Combo = 3,
    Score = 4,
    Bar = 5,
    Name = 6,
    Level = 7,
}

impl UIElement {
    pub fn from_u8(val: u8) -> Option<Self> {
        Some(match val {
            1 => Self::Pause,
            2 => Self::ComboNumber,
            3 => Self::Combo,
            4 => Self::Score,
            5 => Self::Bar,
            6 => Self::Name,
            7 => Self::Level,
            _ => return None,
        })
    }
}

pub struct GifFrames {
    /// time of each frame in milliseconds
    frames: Vec<(u128, SafeTexture)>,
    /// milliseconds
    total_time: u128,
}

impl GifFrames {
    pub fn new(frames: Vec<(u128, SafeTexture)>) -> Self {
        let total_time = frames.iter().map(|(time, _)| *time).sum();
        Self { frames, total_time }
    }

    pub fn get_time_frame(&self, time: u128) -> &SafeTexture {
        let mut time = time % self.total_time;
        for (t, frame) in &self.frames {
            if time < *t {
                return frame;
            }
            time -= t;
        }
        &self.frames.last().unwrap().1
    }

    pub fn get_prog_frame(&self, prog: f32) -> &SafeTexture {
        let time = (prog * self.total_time as f32) as u128;
        self.get_time_frame(time)
    }

    pub fn total_time(&self) -> u128 {
        self.total_time
    }
}

#[derive(Default)]
pub enum JudgeLineKind {
    #[default]
    Normal,
    Texture(SafeTexture, String),
    TextureGif(Anim<f32>, GifFrames, String),
    Text(Anim<String>),
    Paint(Anim<f32>, RefCell<(Option<RenderPass>, bool)>),
}

#[derive(Clone)]
pub struct JudgeLineCache {
    update_order: Vec<u32>,
    not_plain_count: usize,
    above_indices: Vec<usize>,
    below_indices: Vec<usize>,
}

impl JudgeLineCache {
    pub fn new(notes: &mut [Note]) -> Self {
        notes
            .sort_by_key(|it| (it.plain(), !it.above, it.speed.not_nan(), ((it.height + it.object.translation.1.now() as f64) * it.speed).not_nan()));
        let mut res = Self {
            update_order: Vec::new(),
            not_plain_count: 0,
            above_indices: Vec::new(),
            below_indices: Vec::new(),
        };
        res.reset(notes);
        res
    }

    pub(crate) fn reset(&mut self, notes: &mut [Note]) {
        self.update_order = (0..notes.len() as u32).collect();
        self.above_indices.clear();
        self.below_indices.clear();
        let mut index = notes.iter().position(|it| it.plain()).unwrap_or(notes.len());
        self.not_plain_count = index;
        while notes.get(index).is_some_and(|it| it.above) {
            self.above_indices.push(index);
            let speed = notes[index].speed;
            loop {
                index += 1;
                if !notes.get(index).is_some_and(|it| it.above && it.speed == speed) {
                    break;
                }
            }
        }
        while index != notes.len() {
            self.below_indices.push(index);
            let speed = notes[index].speed;
            loop {
                index += 1;
                if !notes.get(index).is_some_and(|it| it.speed == speed) {
                    break;
                }
            }
        }
    }

    /// 该线当前“活跃”（尚未消亡）的 Note 下标集合（`update_order`），
    /// 由每帧 [`JudgeLine::update`] 维护（retain 掉已 dead 的 Note）。
    ///
    /// [`Chart::load_metrics`] 据此统计负载指标，无需每帧全量遍历所有 Note。
    pub(crate) fn active_ids(&self) -> &[u32] {
        &self.update_order
    }

    /// 渲染所需的缓存视图：非 plain 段长度、上方/下方分组起点。
    pub(crate) fn view(&self) -> (usize, &[usize], &[usize]) {
        (self.not_plain_count, &self.above_indices, &self.below_indices)
    }
}

/// 整条 Note 是否位于屏幕“上沿之外”（高度上限 = height_limit，判定公式与 plain 组一致）：
/// (Δ + 位移动画) × speed > 上限。Hold 要求首尾两端都越界才返回 true——
/// 只要任何一端还在屏内就必须正常渲染。
fn note_above_screen(note: &Note, line_height: f64, height_limit: f32) -> bool {
    if note.speed <= 0.0 {
        return false;
    }
    let tr = note.object.translation.1.now() as f64;
    let head = (note.height - line_height + tr) * note.speed;
    let reach = match note.kind {
        NoteKind::Hold { end_height, .. } => {
            let tail = (end_height as f64 - line_height + tr) * note.speed;
            head.min(tail)
        }
        _ => head,
    };
    reach > height_limit as f64
}

pub struct JudgeLine {
    pub object: Object,
    pub ctrl_obj: RefCell<CtrlObject>,
    pub kind: JudgeLineKind,
    /// Height Animation, decribes the `height` of the line at a specific time
    ///
    /// The `height` here can be considered as the absolute 'y' coordinate of the notes attached to this line, which is calculated by
    /// ∫ v(t) dt, where v(t) is the speed of the line at time t.
    pub height: AnimFloat,
    pub incline: AnimFloat,
    pub notes: Vec<Note>,
    pub color: Anim<Color>,
    pub parent: Option<usize>,
    pub rot_with_parent: bool,
    pub z_index: i32,
    /// Whether to show notes below the line, here below is defined in the time axis, which means the note should already be judged
    ///
    /// TODO: Not sure
    pub show_below: bool,
    pub attach_ui: Option<UIElement>,
    /// 纹理锚点（RPE "anchor": [x, y]）：贴图/文本相对判定线的对齐点，默认居中 (0.5, 0.5)
    pub texture_anchor: (f32, f32),

    pub cache: JudgeLineCache,
}

impl JudgeLine {
    pub fn update(&mut self, res: &mut Resource, tr: Matrix, parent_rot: f32) {

        self.height.set_time(res.time);
        let line_height = self.height.now();
        let mut ctrl_obj = self.ctrl_obj.borrow_mut();
        self.cache.update_order.retain(|id| {
            let note = &mut self.notes[*id as usize];
            note.update(res, parent_rot, &tr, &mut ctrl_obj, line_height as f64);
            !note.dead()
        });
        drop(ctrl_obj);
        match &mut self.kind {
            JudgeLineKind::Text(anim) => {
                anim.set_time(res.time);
            }
            JudgeLineKind::Paint(anim, ..) => {
                anim.set_time(res.time);
            }
            JudgeLineKind::TextureGif(anim, ..) => {
                anim.set_time(res.time);
            }
            _ => {}
        }
        self.color.set_time(res.time);
        self.cache.above_indices.retain_mut(|index| {
            while matches!(self.notes[*index].judge, JudgeStatus::Judged) {
                if self
                    .notes
                    .get(*index + 1)
                    .is_some_and(|it| it.above && it.speed == self.notes[*index].speed)
                {
                    *index += 1;
                } else {
                    return false;
                }
            }
            true
        });
        self.cache.below_indices.retain_mut(|index| {
            while matches!(self.notes[*index].judge, JudgeStatus::Judged) {
                if self.notes.get(*index + 1).is_some_and(|it| it.speed == self.notes[*index].speed) {
                    *index += 1;
                } else {
                    return false;
                }
            }
            true
        });
    }

    pub fn fetch_rot(&self, lines: &[JudgeLine]) -> f32 {
        let mut rot = self.object.rotation.now();
        if self.rot_with_parent {
            if let Some(parent) = self.parent {
                rot += lines[parent].fetch_rot(lines);
            }
        }
        rot
    }

    pub fn fetch_pos(&self, res: &Resource, lines: &[JudgeLine]) -> Vector {
        if let Some(parent) = self.parent {
            let parent = &lines[parent];
            let parent_translation = parent.fetch_pos(res, lines);
            return parent_translation + Rotation2::new(parent.fetch_rot(lines).to_radians()) * self.object.now_translation(res);
        }
        self.object.now_translation(res)
    }

    pub fn now_transform(&self, res: &Resource, lines: &[JudgeLine]) -> Matrix {
        Rotation2::new(self.fetch_rot(lines).to_radians())
            .to_homogeneous()
            .append_translation(&self.fetch_pos(res, lines))
    }

    /// 渲染判定线视觉与 Note（默认路径，与画面正常时期一致）。
    pub fn render(&self, ui: &mut Ui, res: &mut Resource, lines: &[JudgeLine], bpm_list: &mut BpmList, settings: &ChartSettings, id: usize) {
        let alpha = self.object.alpha.now_opt().unwrap_or(1.0) * res.alpha;
        let anchor = self.texture_anchor;
        let color = self.color.now_opt();
        let line_scaled = (self.object.scale.1.now() - 1.).abs() > 1e-4;
        res.with_model(self.now_transform(res, lines), |res| {
            if res.config.chart_debug {
                res.apply_model(|_| {
                    ui.text(id.to_string()).pos(0., -0.01).anchor(0.5, 1.).size(0.8).draw();
                });
            }
            res.with_model(self.object.now_scale(Vector::default()), |res| {
                res.apply_model(|res| match &self.kind {
                    JudgeLineKind::Normal => {
                        let mut color = color.unwrap_or(res.judge_line_color);
                        color.a *= alpha.max(0.0);
                        let len = res.info.line_length;
                        draw_line(-len, 0., len, 0., if line_scaled { 0.0076 } else { 0.01 }, color);
                    }
                    JudgeLineKind::Texture(texture, _) => {
                        let mut color = color.unwrap_or(WHITE);
                        color.a = alpha.max(0.0);
                        if color.a == 0.0 {
                            return;
                        }
                        let hf = vec2(texture.width(), texture.height());
                        // 锚点对齐：贴图以 (anchor.x, anchor.y) 为基准对齐判定线原点
                        draw_texture_ex(
                            **texture,
                            -hf.x * anchor.0,
                            -hf.y * anchor.1,
                            color,
                            DrawTextureParams {
                                dest_size: Some(hf),
                                flip_y: true,
                                ..Default::default()
                            },
                        );
                    }
                    JudgeLineKind::TextureGif(anim, frames, _) => {
                        let t = anim.now_opt().unwrap_or(0.0);
                        let frame = frames.get_prog_frame(t);
                        let mut color = color.unwrap_or(WHITE);
                        color.a = alpha.max(0.0);
                        let hf = vec2(frame.width(), frame.height());
                        draw_texture_ex(
                            **frame,
                            -hf.x * anchor.0,
                            -hf.y * anchor.1,
                            color,
                            DrawTextureParams {
                                dest_size: Some(hf),
                                flip_y: true,
                                ..Default::default()
                            },
                        );
                    }
                    JudgeLineKind::Text(anim) => {
                        let mut color = color.unwrap_or(WHITE);
                        color.a = alpha.max(0.0);
                        let now = anim.now();
                        res.apply_model_of(&Matrix::identity().append_nonuniform_scaling(&Vector::new(1., -1.)), |_| {
                            ui.text(&now).pos(0., 0.).anchor(anchor.0, anchor.1).size(1.).color(color).multiline().draw();
                        });
                    }
                    JudgeLineKind::Paint(anim, state) => {
                        let mut color = color.unwrap_or(WHITE);
                        color.a = alpha.max(0.0) * 2.55;
                        let mut gl = unsafe { get_internal_gl() };
                        let mut guard = state.borrow_mut();
                        let vp = get_viewport();
                        let pass = *guard.0.get_or_insert_with(|| {
                            let ctx = &mut gl.quad_context;
                            let tex = Texture::new_render_texture(
                                ctx,
                                TextureParams {
                                    width: vp.2 as _,
                                    height: vp.3 as _,
                                    format: miniquad::TextureFormat::RGBA8,
                                    filter: FilterMode::Linear,
                                    wrap: TextureWrap::Clamp,
                                },
                            );
                            RenderPass::new(ctx, tex, None)
                        });
                        gl.flush();
                        let old_pass = gl.quad_gl.get_active_render_pass();
                        gl.quad_gl.render_pass(Some(pass));
                        gl.quad_gl.viewport(None);
                        let size = anim.now();
                        if size <= 0. {
                            if guard.1 {
                                clear_background(Color::default());
                                guard.1 = false;
                            }
                        } else {
                            ui.fill_circle(0., 0., size / vp.2 as f32 * 2., color);
                            guard.1 = true;
                        }
                        gl.flush();
                        gl.quad_gl.render_pass(old_pass);
                        gl.quad_gl.viewport(Some(vp));
                    }
                })
            });
            if let JudgeLineKind::Paint(_, state) = &self.kind {
                let guard = state.borrow_mut();
                if guard.1 {
                    let ctx = unsafe { get_internal_gl() }.quad_context;
                    let tex = guard.0.as_ref().unwrap().texture(ctx);
                    let top = 1. / res.aspect_ratio;
                    draw_texture_ex(
                        Texture2D::from_miniquad_texture(tex),
                        -1.,
                        -top,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(vec2(2., top * 2.)),
                            ..Default::default()
                        },
                    );
                }
            }
            let mut config = RenderConfig {
                settings,
                ctrl_obj: &mut self.ctrl_obj.borrow_mut(),
                line_height: self.height.now() as f64,
                appear_before: f64::INFINITY,
                draw_below: self.show_below,
                incline_sin: self.incline.now_opt().map(|it| it.to_radians().sin()).unwrap_or_default(),
            };
            if alpha < 0.0 {
                if !settings.pe_alpha_extension {
                    return;
                }
                let w = (-alpha).floor() as u32;
                match w {
                    1 => {
                        return;
                    }
                    2 => {
                        config.draw_below = false;
                    }
                    w if (100..1000).contains(&w) => {
                        config.appear_before = (w as f64 - 100.) / 10.;
                    }
                    w if (1000..2000).contains(&w) => {}
                    _ => {}
                }
            }
            let (vw, vh) = (1.1, 1.);
            let p = [
                res.screen_to_world(Point::new(-vw, -vh)),
                res.screen_to_world(Point::new(-vw, vh)),
                res.screen_to_world(Point::new(vw, -vh)),
                res.screen_to_world(Point::new(vw, vh)),
            ];
            let height_above = p[0].y.max(p[1].y.max(p[2].y.max(p[3].y))) * res.aspect_ratio;
            let height_below = -p[0].y.min(p[1].y.min(p[2].y.min(p[3].y))) * res.aspect_ratio;
            let agg = res.config.eff_cull();
            // 非 plain（假音符 / Hold / 带位移动画）也做屏幕外提前剔除：
            // 与上方 plain 组相同的判定公式（(Δ+tr)·speed > height_above 即整条在屏上沿之外）。
            // Hold 要求首尾两端都越界才跳过——只要任何一端在屏内就正常渲染。
            for note in self.notes.iter().take(self.cache.not_plain_count).filter(|it| it.above) {
                if agg && note_above_screen(note, config.line_height, height_above) {
                    continue;
                }
                note.render(res, &mut config, bpm_list);
            }
            for index in &self.cache.above_indices {
                let speed = self.notes[*index].speed;
                let limit = height_above as f64 / speed;
                for note in self.notes[*index..].iter() {
                    if !note.above || speed != note.speed {
                        break;
                    }
                    if agg && note.height - config.line_height + note.object.translation.1.now() as f64 > limit {
                        break;
                    }
                    note.render(res, &mut config, bpm_list);
                }
            }
            res.with_model(Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)), |res| {
                for note in self.notes.iter().take(self.cache.not_plain_count).filter(|it| !it.above) {
                    if agg && note_above_screen(note, config.line_height, height_below) {
                        continue;
                    }
                    note.render(res, &mut config, bpm_list);
                }
                for index in &self.cache.below_indices {
                    let speed = self.notes[*index].speed;
                    let limit = height_below as f64 / speed;
                    for note in self.notes[*index..].iter() {
                        if speed != note.speed {
                            break;
                        }
                        if agg && note.height - config.line_height + note.object.translation.1.now() as f64 > limit {
                            break;
                        }
                        note.render(res, &mut config, bpm_list);
                    }
                }
            });
        });
    }

    /// 收集本线“Note 通道”所需的只读数据（每帧由主线程收集一次，交给工作线程做顶点生成）。
    pub(crate) fn note_view<'n>(&'n self, res: &Resource, lines: &[JudgeLine], frame: &FrameGeo) -> LineNoteView<'n> {
        let (not_plain_count, above_indices, below_indices) = self.cache.view();
        LineNoteView {
            notes: &self.notes,
            above_indices: above_indices.to_vec(),
            below_indices: below_indices.to_vec(),
            not_plain_count,
            line_alpha: self.object.alpha.now_opt().unwrap_or(1.0) * frame.alpha,
            show_below: self.show_below,
            line_height: self.height.now() as f64,
            incline_sin: self.incline.now_opt().map(|it| it.to_radians().sin()).unwrap_or_default(),
            m_line: self.now_transform(res, lines),
        }
    }
}

/// 单条判定线做 Note 顶点生成所需的最小只读数据（Send）。
pub(crate) struct LineNoteView<'n> {
    pub notes: &'n [Note],
    pub above_indices: Vec<usize>,
    pub below_indices: Vec<usize>,
    pub not_plain_count: usize,
    /// 线 alpha（object.alpha.now_opt() * frame.alpha），负值编码 pe_alpha_extension 特判
    pub line_alpha: f32,
    pub show_below: bool,
    pub line_height: f64,
    pub incline_sin: f32,
    pub m_line: Matrix,
}

/// 并行生成一条判定线上全部 Note 的顶点（与旧 `JudgeLine::render` 中的 Note 段落等价）。
///
/// 返回该线最后采样 ctrl 的 Note 高度（None = 本轮没有 Note 采样 ctrl），由主线程据此
/// 把真实判定线 ctrl 恢复为“帧末状态”（保证下一帧判定行为与串行实现一致）。
pub(crate) fn run_line_note_pass(frame: &FrameGeo, view: &LineNoteView, ctrl: &mut CtrlObject, sink: &mut dyn VertexSink) -> Option<f64> {
    let mut cfg = NoteRenderCfg {
        ctrl,
        line_height: view.line_height,
        appear_before: f64::INFINITY,
        draw_below: view.show_below,
        incline_sin: view.incline_sin,
        last_h: None,
    };
    if view.line_alpha < 0.0 {
        if !frame.pe_alpha_extension {
            return None;
        }
        let w = (-view.line_alpha).floor() as u32;
        match w {
            1 => {
                return None;
            }
            2 => {
                cfg.draw_below = false;
            }
            w if (100..1000).contains(&w) => {
                cfg.appear_before = (w as f64 - 100.) / 10.;
            }
            w if (1000..2000).contains(&w) => {}
            _ => {}
        }
    }
    let height_above;
    let height_below;
    {
        let inv = view.m_line.try_inverse().unwrap();
        let (vw, vh) = (1.1, 1.);
        let p = [
            inv.transform_point(&Point::new(-vw, -vh)),
            inv.transform_point(&Point::new(-vw, vh)),
            inv.transform_point(&Point::new(vw, -vh)),
            inv.transform_point(&Point::new(vw, vh)),
        ];
        height_above = p[0].y.max(p[1].y.max(p[2].y.max(p[3].y))) * frame.aspect;
        height_below = -p[0].y.min(p[1].y.min(p[2].y.min(p[3].y))) * frame.aspect;
    }
    let agg = frame.cull;
    let notes = view.notes;
    let mut geo = Geo::new(*frame, view.m_line, sink);
    // 上方（判定线之前）的 Note
    for note in notes.iter().take(view.not_plain_count).filter(|it| it.above) {
        if agg && note_above_screen(note, cfg.line_height, height_above) {
            continue;
        }
        note_render(note, &mut geo, &mut cfg);
    }
    for index in &view.above_indices {
        let speed = notes[*index].speed;
        let limit = height_above as f64 / speed;
        for note in notes[*index..].iter() {
            if !note.above || speed != note.speed {
                break;
            }
            if agg && note.height - cfg.line_height + note.object.translation.1.now() as f64 > limit {
                break;
            }
            note_render(note, &mut geo, &mut cfg);
        }
    }
    // 下方（判定线之后）的 Note：翻转 Y 后渲染
    geo.with_model(Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)), |geo| {
        for note in notes.iter().take(view.not_plain_count).filter(|it| !it.above) {
            if agg && note_above_screen(note, cfg.line_height, height_below) {
                continue;
            }
            note_render(note, geo, &mut cfg);
        }
        for index in &view.below_indices {
            let speed = notes[*index].speed;
            let limit = height_below as f64 / speed;
            for note in notes[*index..].iter() {
                if speed != note.speed {
                    break;
                }
                if agg && note.height - cfg.line_height + note.object.translation.1.now() as f64 > limit {
                    break;
                }
                note_render(note, geo, &mut cfg);
            }
        }
    });
    cfg.last_h
}
