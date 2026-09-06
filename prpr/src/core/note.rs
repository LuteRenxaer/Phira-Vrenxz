use super::{chart::ChartSettings, BpmList, CtrlObject, JudgeLine, Matrix, Object, Point, Resource};
pub use crate::{
    config::Mods,
    judge::{HitSound, JudgeStatus, LIMIT_BAD},
    parse::RPE_HEIGHT,
};
use macroquad::prelude::*;

const HOLD_PARTICLE_INTERVAL: f64 = 0.15;
pub(crate) const FADEOUT_TIME: f64 = 0.16;
const BAD_TIME: f64 = 0.5;

#[derive(Clone, Debug)]
pub enum NoteKind {
    Click,
    Hold { end_time: f64, end_height: f64 },
    Flick,
    Drag,
}

impl NoteKind {
    pub fn order(&self) -> i8 {
        match self {
            Self::Hold { .. } => 0,
            Self::Drag => 1,
            Self::Click => 2,
            Self::Flick => 3,
        }
    }
}

pub struct Note {
    pub object: Object,
    pub kind: NoteKind,
    pub hitsound: HitSound,
    pub time: f64,
    pub height: f64,
    pub speed: f64,
    pub color: Color,
    pub fx_color: Option<Color>,
    pub judge_area: f32,

    /// From the other side of the line
    pub above: bool,
    pub multiple_hint: bool,
    pub fake: bool,
    pub judge: JudgeStatus,
}

pub struct RenderConfig<'a> {
    pub settings: &'a ChartSettings,
    pub ctrl_obj: &'a mut CtrlObject,
    pub line_height: f64,
    pub appear_before: f64,
    pub draw_below: bool,
    pub incline_sin: f32,
}

#[allow(clippy::too_many_arguments)]
fn draw_tex(res: &Resource, texture: Texture2D, order: i8, x: f32, y: f32, color: Color, mut params: DrawTextureParams, clip: bool) {
    let Vec2 { x: w, y: h } = params.dest_size.unwrap();
    if h < 0. {
        return;
    }
    let mut p = [Point::new(x, y), Point::new(x + w, y), Point::new(x + w, y + h), Point::new(x, y + h)];
    if clip {
        if y + h <= 0. {
            return;
        }
        if y <= 0. {
            let r = -y / (y + h);
            p[0].y = 0.;
            p[1].y = 0.;
            let mut source = params.source.unwrap_or_else(|| Rect::new(0., 0., 1., 1.));
            source.y += source.h * r;
            params.source = Some(source);
        }
    }
    params.flip_y = true;
    draw_tex_pts(res, texture, order, p, color, params);
}
fn draw_tex_pts(res: &Resource, texture: Texture2D, order: i8, p: [Point; 4], color: Color, params: DrawTextureParams) {
    let mut p = p.map(|it| res.world_to_screen(it));
    if p[0].x.min(p[1].x.min(p[2].x.min(p[3].x))) > 1.
        || p[0].x.max(p[1].x.max(p[2].x.max(p[3].x))) < -1.
        || p[0].y.min(p[1].y.min(p[2].y.min(p[3].y))) > 1.
        || p[0].y.max(p[1].y.max(p[2].y.max(p[3].y))) < -1.
    {
        return;
    }
    let Rect { x: sx, y: sy, w: sw, h: sh } = params.source.unwrap_or(Rect { x: 0., y: 0., w: 1., h: 1. });

    if params.flip_x {
        p.swap(0, 1);
        p.swap(2, 3);
    }
    if params.flip_y {
        p.swap(0, 3);
        p.swap(1, 2);
    }

    #[rustfmt::skip]
    let vertices = [
        Vertex::new(p[0].x, p[0].y, 0., sx     , sy     , color),
        Vertex::new(p[1].x, p[1].y, 0., sx + sw, sy     , color),
        Vertex::new(p[2].x, p[2].y, 0., sx + sw, sy + sh, color),
        Vertex::new(p[3].x, p[3].y, 0., sx     , sy + sh, color),
    ];
    res.note_buffer
        .borrow_mut()
        .push((order, texture.raw_miniquad_texture_handle().gl_internal_id()), vertices);
}

fn draw_center(res: &Resource, tex: Texture2D, order: i8, scale: f32, color: Color) {
    let hf = vec2(scale, tex.height() * scale / tex.width());
    draw_tex(
        res,
        tex,
        order,
        -hf.x,
        -hf.y,
        color,
        DrawTextureParams {
            dest_size: Some(hf * 2.),
            ..Default::default()
        },
        false,
    );
}

impl Note {
    pub fn rotation(&self, line: &JudgeLine) -> f32 {
        line.object.rotation.now() + if self.above { 0. } else { 180. }
    }

    pub fn plain(&self) -> bool {
        !self.fake && !matches!(self.kind, NoteKind::Hold { .. }) && self.object.translation.1.keyframes.len() <= 1

    }

    pub fn update(&mut self, res: &mut Resource, parent_rot: f32, parent_tr: &Matrix, ctrl_obj: &mut CtrlObject, line_height: f64) {
        self.object.set_time(res.time);
        if let Some(color) = if let JudgeStatus::Hold(perfect, at, ..) = &mut self.judge {
            if res.time > *at {
                *at += HOLD_PARTICLE_INTERVAL / res.config.speed as f64;
                Some(self.fx_color.unwrap_or_else(|| {
                    if *perfect {
                        res.res_pack.info.fx_perfect()
                    } else {
                        res.res_pack.info.fx_good()
                    }
                }))
            } else {
                None
            }
        } else {
            None
        } {
            self.init_ctrl_obj(ctrl_obj, line_height);
            res.with_model(parent_tr * self.now_transform(res, ctrl_obj, 0., 0.), |res| {
                res.emit_at_origin(parent_rot + if self.above { 0. } else { 180. }, color)
            });
        }
    }

    pub fn dead(&self) -> bool {
        (!matches!(self.kind, NoteKind::Hold { .. }) || matches!(self.judge, JudgeStatus::Judged)) && self.object.dead()

    }

    fn init_ctrl_obj(&self, ctrl_obj: &mut CtrlObject, line_height: f64) {
        ctrl_obj.set_height((self.height - line_height + self.object.translation.1.now() as f64 / self.speed) * RPE_HEIGHT as f64 / 2.);
    }

    /// 花样 mod“横摆”：按 Note 时间戳生成确定性伪随机横向偏移（[-1, 1] × 幅度）。
    /// 每帧稳定（只依赖 Note 自身数据），便于按线并行渲染。
    fn random_x_jitter(&self) -> f32 {
        const AMPLITUDE: f32 = 0.45;
        let mut z = self.time.to_bits() ^ self.height.to_bits() ^ 0x9E37_79B9_7F4A_7C15;
        z ^= z >> 30;
        z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z ^= z >> 27;
        z = z.wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        // 取完整 32 位高位，均匀分布在 [0, 1]
        let u = (z >> 32) as u32;
        let f = u as f32 / u32::MAX as f32;
        (f * 2. - 1.) * AMPLITUDE
    }

    pub fn now_transform(&self, res: &Resource, ctrl_obj: &CtrlObject, base: f32, incline_sin: f32) -> Matrix {
        let incline_val = 1. - incline_sin * (base * res.aspect_ratio + self.object.translation.1.now()) * RPE_HEIGHT / 2. / 360.;
        let mut tr = self.object.now_translation(res);
        tr.x *= if matches!(self.kind, NoteKind::Hold { .. }) {
            1.
        } else {
            incline_val * ctrl_obj.pos.now_opt().unwrap_or(1.)
        };
        // 花样 mod：横摆（仅视觉，不改变判定数据）
        if res.config.mods.contains(Mods::RANDOM_X) {
            tr.x += self.random_x_jitter();
        }
        tr.y += base;
        let mut scale = self.object.scale.now_with_def(1.0, 1.0);
        scale.x *= ctrl_obj.size.now_opt().unwrap_or(1.0);
        if res.info.note_uniform_scale {
            scale.y *= ctrl_obj.size.now_opt().unwrap_or(1.0);
        } else {
            scale.y = 1.0;
        };
        self.object.now_rotation().append_nonuniform_scaling(&scale).append_translation(&tr)
    }

    pub fn render(&self, res: &mut Resource, config: &mut RenderConfig, bpm_list: &mut BpmList) {
        if matches!(self.judge, JudgeStatus::Judged) && !matches!(self.kind, NoteKind::Hold { .. }) {
            return;
        }
        if config.appear_before.is_finite() {

            let beat = bpm_list.beat(self.time);
            let time = bpm_list.time_beats(beat - config.appear_before);
            if time > res.time {
                return;
            }
        }
        let scale = (if res.config.double_hint && self.multiple_hint {
            res.res_pack.note_style_mh.click.width() / res.res_pack.note_style.click.width()
        } else {
            1.0
        }) * res.note_width;
        let ctrl_obj = &mut config.ctrl_obj;
        self.init_ctrl_obj(ctrl_obj, config.line_height);
        let mut color = Color {
            a: self.object.now_alpha(),
            ..self.color
        };
        color.a *= res.alpha * ctrl_obj.alpha.now_opt().unwrap_or(1.);
        // 花样 mod：幽灵（整条 Note 半透明）
        if res.config.mods.contains(Mods::GHOST) {
            color.a *= 0.35;
        }
        let spd = self.speed * ctrl_obj.y.now_opt().unwrap_or(1.) as f64;

        let line_height = config.line_height / res.aspect_ratio as f64 * spd;
        let height = self.height / res.aspect_ratio as f64 * spd;

        let base = height - line_height;
        let cover_base = if !config.settings.hold_partial_cover {
            height - line_height
        } else {
            match self.kind {
                NoteKind::Hold { end_time: _, end_height } => {
                    let end_height = end_height / res.aspect_ratio as f64 * spd;
                    end_height - line_height
                }
                _ => height - line_height,
            }
        };

        if !config.draw_below
            && (((res.time - FADEOUT_TIME >= self.time || self.fake && res.time >= self.time) && !matches!(self.kind, NoteKind::Hold { .. }))
                || (self.time > res.time && cover_base <= -0.001))
        {
            return;
        }
        let order = self.kind.order();
        // 屏幕上的 Note 数量较多（>= LOW_RES_NOTE_THRESHOLD）时，改用低分辨率贴图渲染
        let style = res.res_pack.style_for(res.config.double_hint && self.multiple_hint, res.low_res_notes);
        let mod_alpha = if res.config.has_mod(Mods::FADE_OUT) {
            ((self.time - res.time - LIMIT_BAD) / LIMIT_BAD).clamp(0., 1.)
        } else if res.config.has_mod(Mods::FADE_IN) {
            (1. - (self.time - res.time - LIMIT_BAD) / LIMIT_BAD).clamp(0., 1.)
        } else {
            1.
        };
        let draw = |res: &mut Resource, tex: Texture2D| {
            let mut color = color;
            if !config.draw_below {
                let alpha = (self.time - res.time).min(0.) / FADEOUT_TIME + 1.;
                color.a *= if self.fake && res.time >= self.time { 0. } else { alpha as f32 };
            }
            color.a *= mod_alpha as f32;
            res.with_model(self.now_transform(res, ctrl_obj, base as f32, config.incline_sin), |res| {
                draw_center(res, tex, order, scale, color);
            });
        };
        match self.kind {
            NoteKind::Click => {
                draw(res, *style.click);
            }
            NoteKind::Hold { end_time, end_height } => {
                res.with_model(self.now_transform(res, ctrl_obj, 0., 0.), |res| {
                    let style = res.res_pack.style_for(res.config.double_hint && self.multiple_hint, res.low_res_notes);
                    if matches!(self.judge, JudgeStatus::Judged) {

                        color.a *= 0.5;
                    }
                    if res.time >= end_time {
                        return;
                    }
                    let end_height = end_height / res.aspect_ratio as f64 * spd;
                    color.a *= mod_alpha as f32;

                    let h = if self.time <= res.time { line_height } else { height };
                    let bottom = (h - line_height) as f32;
                    let top = (end_height - line_height) as f32;
                    let tex = &style.hold;
                    let ratio = style.hold_ratio();


                    draw_tex(
                        res,
                        **(if res.res_pack.info.hold_repeat {
                            style.hold_body.as_ref().unwrap()
                        } else {
                            tex
                        }),
                        order,
                        -scale,
                        bottom,
                        color,
                        DrawTextureParams {
                            source: Some({
                                if res.res_pack.info.hold_repeat {
                                    let hold_body = style.hold_body.as_ref().unwrap();
                                    let width = hold_body.width();
                                    let height = hold_body.height();
                                    Rect::new(0., 0., 1., (top - bottom) / scale / 2. * width / height)
                                } else {
                                    style.hold_body_rect()
                                }
                            }),
                            dest_size: Some(vec2(scale * 2., top - bottom)),
                            ..Default::default()
                        },
                        false,
                    );

                    if res.time < self.time || res.res_pack.info.hold_keep_head {
                        let r = style.hold_head_rect();
                        let hf = vec2(scale, r.h / r.w * scale * ratio);
                        draw_tex(
                            res,
                            **tex,
                            order,
                            -scale,
                            bottom - if res.res_pack.info.hold_compact { hf.y } else { hf.y * 2. },
                            color,
                            DrawTextureParams {
                                source: Some(r),
                                dest_size: Some(hf * 2.),
                                ..Default::default()
                            },
                            false,
                        );
                    }

                    let r = style.hold_tail_rect();
                    let hf = vec2(scale, r.h / r.w * scale * ratio);
                    draw_tex(
                        res,
                        **tex,
                        order,
                        -scale,
                        top - if res.res_pack.info.hold_compact { hf.y } else { 0. },
                        color,
                        DrawTextureParams {
                            source: Some(r),
                            dest_size: Some(hf * 2.),
                            ..Default::default()
                        },
                        false,
                    );
                });
            }
            NoteKind::Flick => {
                draw(res, *style.flick);
            }
            NoteKind::Drag => {
                draw(res, *style.drag);
            }
        }
    }
}

pub struct BadNote {
    pub time: f64,
    pub kind: NoteKind,
    pub matrix: Matrix,
}

impl BadNote {
    pub fn render(&self, res: &mut Resource) -> bool {
        if res.time > self.time + BAD_TIME {
            return false;
        }
        res.with_model(self.matrix, |res| {
            let style = res.res_pack.style_for(false, res.low_res_notes);
            draw_center(
                res,
                match &self.kind {
                    NoteKind::Click => *style.click,
                    NoteKind::Drag => *style.drag,
                    NoteKind::Flick => *style.flick,
                    _ => unreachable!(),
                },
                self.kind.order(),
                res.note_width,
                Color::new(0.423529, 0.262745, 0.262745, ((self.time - res.time).max(-1.) / BAD_TIME + 1.) as f32),
            );
        });
        true
    }
}

// ============================ 顶点生成（按判定线并行） ============================
//
// Note 绘制链不直接写共享的 `Resource.note_buffer / model_stack`，而是写入
// “线程内独立暂存 + 本地模型栈”（[`Geo`]），由 [`VertexSink`] 决定去向：
// - 串行：直接写 `Resource.note_buffer`（等价旧实现）；
// - 并行：写每线独立的暂存，主线程按线序合批后一次 draw_all。
// 本段与上方旧绘制链并存：旧链（Note::render / BadNote::render）保留给未迁移调用方。

use super::resource::NoteStyle;
use crate::ext::SafeTexture;
use miniquad::gl::GLuint;

/// 顶点生成阶段的纹理信息：GL id + 宽高（宽高来自 CPU 元数据，无需 GL 调用）。
#[derive(Clone, Copy)]
pub(crate) struct GfxTex {
    pub id: GLuint,
    pub w: f32,
    pub h: f32,
}

impl GfxTex {
    fn from_tex(tex: &Texture2D) -> Self {
        Self {
            id: tex.raw_miniquad_texture_handle().gl_internal_id(),
            w: tex.width(),
            h: tex.height(),
        }
    }
}

/// 一个 Note 贴图样式（含 Atlas 派生量），等价于渲染所需的 [`NoteStyle`] 快照。
#[derive(Clone, Copy)]
pub(crate) struct StyleTex {
    pub click: GfxTex,
    pub hold: GfxTex,
    pub flick: GfxTex,
    pub drag: GfxTex,
    pub hold_body: Option<GfxTex>,
    /// to_uv(hold_atlas.1)：头部（顶部）所占 UV 高度
    pub head_uv: f32,
    /// to_uv(hold_atlas.0)：尾部（底部）所占 UV 高度
    pub tail_uv: f32,
}

impl StyleTex {
    fn of(style: &NoteStyle) -> Self {
        let gfx = |tex: &SafeTexture| GfxTex::from_tex(tex);
        Self {
            click: gfx(&style.click),
            hold: gfx(&style.hold),
            flick: gfx(&style.flick),
            drag: gfx(&style.drag),
            hold_body: style.hold_body.as_ref().map(gfx),
            head_uv: style.hold_atlas.1 as f32 / style.hold.height(),
            tail_uv: style.hold_atlas.0 as f32 / style.hold.height(),
        }
    }

    pub fn hold_ratio(&self) -> f32 {
        self.hold.h / self.hold.w
    }

    pub fn hold_head_rect(&self) -> Rect {
        let sy = self.head_uv;
        Rect::new(0., 1. - sy, 1., sy)
    }

    pub fn hold_body_rect(&self) -> Rect {
        let sy = self.tail_uv;
        let ey = 1. - self.head_uv;
        Rect::new(0., sy, 1., ey - sy)
    }

    pub fn hold_tail_rect(&self) -> Rect {
        let ey = self.tail_uv;
        Rect::new(0., 0., 1., ey)
    }
}

/// 普通 / 多指提示两种样式（已按当前帧低分辨率标记选好贴图）。
#[derive(Clone, Copy)]
pub(crate) struct StyleSet {
    pub normal: StyleTex,
    pub mh: StyleTex,
}

/// 单帧 Note 顶点生成所需的全部只读标量/贴图快照（可安全跨线程共享）。
#[derive(Clone, Copy)]
pub(crate) struct FrameGeo<'b> {
    pub time: f64,
    pub alpha: f32,
    pub aspect: f32,
    pub note_width: f32,
    pub double_hint: bool,
    pub fade_out: bool,
    pub fade_in: bool,
    pub note_uniform_scale: bool,
    pub hold_repeat: bool,
    pub hold_keep_head: bool,
    pub hold_compact: bool,
    pub hold_partial_cover: bool,
    pub pe_alpha_extension: bool,
    /// 屏幕外剔除（档位接管后的生效值）
    pub cull: bool,
    /// 多指提示 Note 宽度比（始终取全分辨率 click 贴图，与原实现一致）
    pub mh_ratio: f32,
    /// 花样 mod：幽灵 / 横摆（快照预先判定，工作线程只读）
    pub ghost: bool,
    pub random_x: bool,
    pub styles: StyleSet,
    /// appear_before 扩展（负 alpha 编码）需要时用于 beat/time 换算
    pub bpm: Option<&'b BpmList>,
}

impl<'b> FrameGeo<'b> {
    pub(crate) fn snapshot(res: &Resource, settings: &ChartSettings, bpm: Option<&'b BpmList>) -> Self {
        let low = res.low_res_notes;
        let config = &res.config;
        let mods = config.mods;
        Self {
            time: res.time,
            alpha: res.alpha,
            aspect: res.aspect_ratio,
            note_width: res.note_width,
            double_hint: config.double_hint,
            fade_out: config.has_mod(Mods::FADE_OUT),
            fade_in: config.has_mod(Mods::FADE_IN),
            note_uniform_scale: res.info.note_uniform_scale,
            hold_repeat: res.res_pack.info.hold_repeat,
            hold_keep_head: res.res_pack.info.hold_keep_head,
            hold_compact: res.res_pack.info.hold_compact,
            hold_partial_cover: settings.hold_partial_cover,
            pe_alpha_extension: settings.pe_alpha_extension,
            cull: config.eff_cull(),
            mh_ratio: res.res_pack.note_style_mh.click.width() / res.res_pack.note_style.click.width(),
            ghost: mods.contains(Mods::GHOST),
            random_x: mods.contains(Mods::RANDOM_X),
            styles: StyleSet {
                normal: StyleTex::of(res.res_pack.style_for(false, low)),
                mh: StyleTex::of(res.res_pack.style_for(true, low)),
            },
            bpm,
        }
    }
}

/// Note 顶点输出单元：排序键 + 一个四顶点矩形（与旧 `NoteBuffer::push` 输入一致）。
#[derive(Clone, Copy, Default)]
pub(crate) struct NoteVertexItem {
    pub key: (i8, GLuint),
    pub verts: [Vertex; 4],
}

/// 顶点去向抽象。
pub(crate) trait VertexSink {
    fn push(&mut self, key: (i8, GLuint), verts: [Vertex; 4]);
}

/// 写入线程内暂存（并行路径，之后由主线程按线序合批）。
pub(crate) struct StagingSink<'a>(pub &'a mut Vec<NoteVertexItem>);

impl VertexSink for StagingSink<'_> {
    fn push(&mut self, key: (i8, GLuint), verts: [Vertex; 4]) {
        self.0.push(NoteVertexItem { key, verts });
    }
}

/// 线程内独立的“暂存 + 本地模型栈”上下文。
pub(crate) struct Geo<'a, 'b> {
    pub frame: FrameGeo<'b>,
    stack: Vec<Matrix>,
    sink: &'a mut dyn VertexSink,
}

impl<'a, 'b> Geo<'a, 'b> {
    pub(crate) fn new(frame: FrameGeo<'b>, root: Matrix, sink: &'a mut dyn VertexSink) -> Self {
        Self {
            frame,
            stack: vec![root],
            sink,
        }
    }

    #[inline]
    pub(crate) fn with_model(&mut self, model: Matrix, f: impl FnOnce(&mut Self)) {
        let model = self.stack.last().unwrap() * model;
        self.stack.push(model);
        f(self);
        self.stack.pop();
    }

    #[inline]
    pub(crate) fn transform_point(&self, pt: Point) -> Point {
        self.stack.last().unwrap().transform_point(&pt)
    }

    #[inline]
    pub(crate) fn push_quad(&mut self, key: (i8, GLuint), verts: [Vertex; 4]) {
        self.sink.push(key, verts);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_tex_g(g: &mut Geo, texture: GfxTex, order: i8, x: f32, y: f32, color: Color, mut params: DrawTextureParams, clip: bool) {
    let Vec2 { x: w, y: h } = params.dest_size.unwrap();
    if h < 0. {
        return;
    }
    let mut p = [Point::new(x, y), Point::new(x + w, y), Point::new(x + w, y + h), Point::new(x, y + h)];
    if clip {
        if y + h <= 0. {
            return;
        }
        if y <= 0. {
            let r = -y / (y + h);
            p[0].y = 0.;
            p[1].y = 0.;
            let mut source = params.source.unwrap_or_else(|| Rect::new(0., 0., 1., 1.));
            source.y += source.h * r;
            params.source = Some(source);
        }
    }
    params.flip_y = true;
    draw_tex_pts_g(g, texture, order, p, color, params);
}

fn draw_tex_pts_g(g: &mut Geo, texture: GfxTex, order: i8, p: [Point; 4], color: Color, params: DrawTextureParams) {
    let mut p = p.map(|it| g.transform_point(it));
    if p[0].x.min(p[1].x.min(p[2].x.min(p[3].x))) > 1.
        || p[0].x.max(p[1].x.max(p[2].x.max(p[3].x))) < -1.
        || p[0].y.min(p[1].y.min(p[2].y.min(p[3].y))) > 1.
        || p[0].y.max(p[1].y.max(p[2].y.max(p[3].y))) < -1.
    {
        return;
    }
    let Rect { x: sx, y: sy, w: sw, h: sh } = params.source.unwrap_or(Rect { x: 0., y: 0., w: 1., h: 1. });

    if params.flip_x {
        p.swap(0, 1);
        p.swap(2, 3);
    }
    if params.flip_y {
        p.swap(0, 3);
        p.swap(1, 2);
    }

    #[rustfmt::skip]
    let vertices = [
        Vertex::new(p[0].x, p[0].y, 0., sx     , sy     , color),
        Vertex::new(p[1].x, p[1].y, 0., sx + sw, sy     , color),
        Vertex::new(p[2].x, p[2].y, 0., sx + sw, sy + sh, color),
        Vertex::new(p[3].x, p[3].y, 0., sx     , sy + sh, color),
    ];
    g.push_quad((order, texture.id), vertices);
}

fn draw_center_g(g: &mut Geo, tex: GfxTex, order: i8, scale: f32, color: Color) {
    let hf = vec2(scale, tex.h * scale / tex.w);
    draw_tex_g(
        g,
        tex,
        order,
        -hf.x,
        -hf.y,
        color,
        DrawTextureParams {
            dest_size: Some(hf * 2.),
            ..Default::default()
        },
        false,
    );
}

impl Note {
    /// Note 高度（beat 量纲）的采样参数，供并行渲染路径与旧路径共用。
    fn init_height(&self, line_height: f64) -> f64 {
        (self.height - line_height + self.object.translation.1.now() as f64 / self.speed) * RPE_HEIGHT as f64 / 2.
    }

    /// 以给定 aspect / uniform_scale 计算当前变换（供并行顶点生成使用，与 [`Note::now_transform`] 等价）。
    pub(crate) fn now_transform_scaled(
        &self,
        aspect_ratio: f32,
        note_uniform_scale: bool,
        random_x: bool,
        ctrl_obj: &CtrlObject,
        base: f32,
        incline_sin: f32,
    ) -> Matrix {
        let incline_val = 1. - incline_sin * (base * aspect_ratio + self.object.translation.1.now()) * RPE_HEIGHT / 2. / 360.;
        let mut tr = self.object.now_translation_scaled(aspect_ratio);
        tr.x *= if matches!(self.kind, NoteKind::Hold { .. }) {
            1.
        } else {
            incline_val * ctrl_obj.pos.now_opt().unwrap_or(1.)
        };
        if random_x {
            tr.x += self.random_x_jitter();
        }
        tr.y += base;
        let mut scale = self.object.scale.now_with_def(1.0, 1.0);
        scale.x *= ctrl_obj.size.now_opt().unwrap_or(1.0);
        if note_uniform_scale {
            scale.y *= ctrl_obj.size.now_opt().unwrap_or(1.0);
        } else {
            scale.y = 1.0;
        };
        self.object.now_rotation().append_nonuniform_scaling(&scale).append_translation(&tr)
    }
}

impl Object {
    /// 只读换算 translation（y 按 aspect 归一），供并行渲染在共享对象上安全调用。
    pub(crate) fn now_translation_scaled(&self, aspect_ratio: f32) -> crate::core::Vector {
        let mut tr = self.translation.now();
        tr.y /= aspect_ratio;
        tr
    }
}

/// 一条判定线上 Note 顶点生成的逐线配置。
pub(crate) struct NoteRenderCfg<'c> {
    pub ctrl: &'c mut CtrlObject,
    pub line_height: f64,
    pub appear_before: f64,
    pub draw_below: bool,
    pub incline_sin: f32,
    /// 最后一个执行了 ctrl 采样的 Note 高度，用于并行渲染后恢复真实 ctrl 的帧末状态
    pub last_h: Option<f64>,
}

/// 生成单个 Note 的顶点（与旧 `Note::render` 等价，读共享 Note / 写线程本地暂存）。
///
/// 仅在 `cfg.appear_before` 有限（pe_alpha_extension 负 alpha 编码）时需要 bpm。
pub(crate) fn note_render(note: &Note, g: &mut Geo, cfg: &mut NoteRenderCfg) {
    if matches!(note.judge, JudgeStatus::Judged) && !matches!(note.kind, NoteKind::Hold { .. }) {
        return;
    }
    if cfg.appear_before.is_finite() {
        let bpm = g.frame.bpm.expect("appear_before extension requires bpm list");
        let beat = bpm.beat_at(note.time);
        let time = bpm.time_beats_at(beat - cfg.appear_before);
        if time > g.frame.time {
            return;
        }
    }
    let frame = g.frame;
    let scale = (if frame.double_hint && note.multiple_hint { frame.mh_ratio } else { 1.0 }) * frame.note_width;
    let ctrl_obj = &mut cfg.ctrl;
    let h = note.init_height(cfg.line_height);
    note.init_ctrl_obj(ctrl_obj, cfg.line_height);
    cfg.last_h = Some(h);
    let mut color = Color {
        a: note.object.now_alpha(),
        ..note.color
    };
    color.a *= frame.alpha * ctrl_obj.alpha.now_opt().unwrap_or(1.);
    // 花样 mod：幽灵（整条 Note 半透明）
    if frame.ghost {
        color.a *= 0.35;
    }
    let spd = note.speed * ctrl_obj.y.now_opt().unwrap_or(1.) as f64;

    let line_height = cfg.line_height / frame.aspect as f64 * spd;
    let height = note.height / frame.aspect as f64 * spd;

    let base = height - line_height;
    let cover_base = if !frame.hold_partial_cover {
        height - line_height
    } else {
        match note.kind {
            NoteKind::Hold { end_time: _, end_height } => {
                let end_height = end_height / frame.aspect as f64 * spd;
                end_height - line_height
            }
            _ => height - line_height,
        }
    };

    if !cfg.draw_below
        && (((frame.time - FADEOUT_TIME >= note.time || note.fake && frame.time >= note.time) && !matches!(note.kind, NoteKind::Hold { .. }))
            || (note.time > frame.time && cover_base <= -0.001))
    {
        return;
    }
    let order = note.kind.order();
    let style = if frame.double_hint && note.multiple_hint { &frame.styles.mh } else { &frame.styles.normal };
    let mod_alpha = if frame.fade_out {
        ((note.time - frame.time - LIMIT_BAD) / LIMIT_BAD).clamp(0., 1.)
    } else if frame.fade_in {
        (1. - (note.time - frame.time - LIMIT_BAD) / LIMIT_BAD).clamp(0., 1.)
    } else {
        1.
    };
    let draw = |g: &mut Geo, tex: GfxTex| {
        let mut color = color;
        if !cfg.draw_below {
            let alpha = (note.time - frame.time).min(0.) / FADEOUT_TIME + 1.;
            color.a *= if note.fake && frame.time >= note.time { 0. } else { alpha as f32 };
        }
        color.a *= mod_alpha as f32;
        g.with_model(note.now_transform_scaled(frame.aspect, frame.note_uniform_scale, frame.random_x, ctrl_obj, base as f32, cfg.incline_sin), |g| {
            draw_center_g(g, tex, order, scale, color);
        });
    };
    match &note.kind {
        NoteKind::Click => {
            draw(g, style.click);
        }
        NoteKind::Hold { end_time, end_height } => {
            g.with_model(note.now_transform_scaled(frame.aspect, frame.note_uniform_scale, frame.random_x, ctrl_obj, 0., 0.), |g| {
                let style = if frame.double_hint && note.multiple_hint { &frame.styles.mh } else { &frame.styles.normal };
                if matches!(note.judge, JudgeStatus::Judged) {
                    color.a *= 0.5;
                }
                if frame.time >= *end_time {
                    return;
                }
                let end_height = end_height / frame.aspect as f64 * spd;
                color.a *= mod_alpha as f32;

                let h = if note.time <= frame.time { line_height } else { height };
                let bottom = (h - line_height) as f32;
                let top = (end_height - line_height) as f32;
                let ratio = style.hold_ratio();

                draw_tex_g(
                    g,
                    if frame.hold_repeat {
                        style.hold_body.expect("hold_repeat resource pack must provide hold_body")
                    } else {
                        style.hold
                    },
                    order,
                    -scale,
                    bottom,
                    color,
                    DrawTextureParams {
                        source: Some(if frame.hold_repeat {
                            let hold_body = style.hold_body.expect("hold_repeat resource pack must provide hold_body");
                            Rect::new(0., 0., 1., (top - bottom) / scale / 2. * hold_body.w / hold_body.h)
                        } else {
                            style.hold_body_rect()
                        }),
                        dest_size: Some(vec2(scale * 2., top - bottom)),
                        ..Default::default()
                    },
                    false,
                );

                if frame.time < note.time || frame.hold_keep_head {
                    let r = style.hold_head_rect();
                    let hf = vec2(scale, r.h / r.w * scale * ratio);
                    draw_tex_g(
                        g,
                        style.hold,
                        order,
                        -scale,
                        bottom - if frame.hold_compact { hf.y } else { hf.y * 2. },
                        color,
                        DrawTextureParams {
                            source: Some(r),
                            dest_size: Some(hf * 2.),
                            ..Default::default()
                        },
                        false,
                    );
                }

                let r = style.hold_tail_rect();
                let hf = vec2(scale, r.h / r.w * scale * ratio);
                draw_tex_g(
                    g,
                    style.hold,
                    order,
                    -scale,
                    top - if frame.hold_compact { hf.y } else { 0. },
                    color,
                    DrawTextureParams {
                        source: Some(r),
                        dest_size: Some(hf * 2.),
                        ..Default::default()
                    },
                    false,
                );
            });
        }
        NoteKind::Flick => {
            draw(g, style.flick);
        }
        NoteKind::Drag => {
            draw(g, style.drag);
        }
    }
}

