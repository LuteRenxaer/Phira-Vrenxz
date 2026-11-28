
use macroquad::prelude::*;
use prpr::{
    ext::{draw_parallelogram_ex, semi_white, RectExt, SafeTexture, ScaleType, PARALLELOGRAM_SLOPE},
    ui::{DRectButton, Ui},
};

use crate::{client::UserManager, mp::L10N_LOCAL};

pub const SCALE: f32 = 0.7;

pub const MAX_ASPECT: f32 = 21. / 9.;

pub const MIN_ASPECT: f32 = 9. / 16.;

const MIN_BODY_H: f32 = 0.06;

pub const PAGE_PAD: f32 = 0.06 * SCALE;

pub const PAGE_PAD_WIDE: f32 = 0.1 * SCALE;

pub const HEADER_TOP: f32 = 0.045 * SCALE;

pub const HEADER_H: f32 = 0.15 * SCALE;

pub const HEADER_ICON_INSET: f32 = 0.02 * SCALE;

pub const BODY_GAP: f32 = 0.045 * SCALE;

pub const BAR_GAP: f32 = 0.0375 * SCALE;

pub const BAR_BTN_H: f32 = 0.15 * SCALE;

pub const BAR_ROW_GAP: f32 = 0.025 * SCALE;

pub const BAR_COL_GAP: f32 = 0.031 * SCALE;

pub const BAR_BTN_MIN_W: f32 = 0.25 * SCALE;

pub const BAR_BTN_MAX_W: f32 = 0.78 * SCALE;

pub const ROW_TALL: f32 = 0.1625 * SCALE;

pub const SECTION_GAP: f32 = 0.0375 * SCALE;

pub const CARD_PAD: f32 = 0.0375 * SCALE;

pub const CAPTION_H: f32 = 0.058 * SCALE;

pub const PANEL_INSET: f32 = 0.015 * SCALE;

pub const TITLE_BLOCK_H: f32 = 0.135 * SCALE;

pub const STATUS_RESERVE_H: f32 = 0.3 * SCALE;

pub const R_CARD: f32 = 0.01;
pub const R_ROW: f32 = 0.008;
pub const R_BTN: f32 = 0.01;

pub const STROKE_W: f32 = 0.0025 * SCALE;

pub const ICON_BTN: f32 = 0.15 * SCALE;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToolIcon {

    Play = 0,

    Ready = 1,

    Cancel = 2,

    Settings = 3,

    Cycle = 4,

    Lock = 5,

    Preview = 6,

    Create = 7,

    Join = 8,

    Refresh = 9,

    Disconnect = 10,

    Library = 11,

    LeaveRoom = 12,

    /// 房间密码（钥匙）
    Key = 13,
}

thread_local! {
    static TOOL_ICONS: std::cell::RefCell<[Option<SafeTexture>; 16]> =
        const { std::cell::RefCell::new([const { None }; 16]) };
}

pub fn set_tool_icons(icons: [Option<SafeTexture>; 16]) {
    TOOL_ICONS.with(|it| *it.borrow_mut() = icons);
}

pub fn tool_icon(kind: ToolIcon) -> Option<SafeTexture> {
    TOOL_ICONS.with(|it| it.borrow()[kind as usize].clone())
}

pub const FS_PAGE_TITLE: f32 = 0.7 * SCALE;

pub const FS_SECTION: f32 = 0.5 * SCALE;

pub const FS_BODY: f32 = 0.4 * SCALE;

pub const FS_SMALL: f32 = 0.35 * SCALE;

pub const FS_BUTTON: f32 = 0.5 * SCALE;

pub const FS_TAG: f32 = 0.32 * SCALE;

pub const FS_BIG: f32 = 0.7 * SCALE;

#[inline]
pub fn color_alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

#[inline]
pub fn text() -> Color {
    semi_white(0.95)
}

#[inline]
pub fn text_dim() -> Color {
    semi_white(0.78)
}

#[inline]
pub fn text_muted() -> Color {
    semi_white(0.52)
}

#[inline]
pub fn card() -> Color {
    semi_white(0.07)
}

#[inline]
pub fn card_soft() -> Color {
    semi_white(0.05)
}

#[inline]
pub fn row() -> Color {
    semi_white(0.05)
}

#[inline]
pub fn secondary() -> Color {
    semi_white(0.14)
}

#[inline]
pub fn stroke() -> Color {
    semi_white(0.16)
}

#[inline]
pub fn divider() -> Color {
    semi_white(0.1)
}

#[inline]
pub fn primary(accent: Color) -> Color {
    color_alpha(accent, 0.92)
}

#[inline]
pub fn danger() -> Color {
    Color::from_rgba(150, 54, 54, 235)
}

#[inline]
pub fn tag_bg() -> Color {
    semi_white(0.18)
}

#[inline]
pub fn tag_accent(accent: Color) -> Color {
    color_alpha(accent, 0.32)
}

#[inline]
pub fn panel_light() -> Color {
    Color::new(0.87, 0.87, 0.90, 0.97)
}

#[inline]
pub fn on_light() -> Color {
    Color::new(0.09, 0.09, 0.12, 1.)
}

#[inline]
pub fn on_light_dim() -> Color {
    Color::new(0.34, 0.34, 0.40, 1.)
}

pub fn quad(pts: [Vec2; 4], colors: [Color; 4]) {
    let gl = unsafe { get_internal_gl() }.quad_gl;
    let v = [
        Vertex::new(pts[0].x, pts[0].y, 0., 0., 0., colors[0]),
        Vertex::new(pts[1].x, pts[1].y, 0., 0., 0., colors[1]),
        Vertex::new(pts[2].x, pts[2].y, 0., 0., 0., colors[2]),
        Vertex::new(pts[3].x, pts[3].y, 0., 0., 0., colors[3]),
    ];
    gl.texture(None);
    gl.draw_mode(DrawMode::Triangles);
    gl.geometry(&v, &[0, 2, 3, 0, 1, 3]);
}

#[inline]
pub fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t.clamp(0., 1.);
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

pub fn slant_panel_grad(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, slant: f32, top: Color, bottom: Color) {
    let _ = ui;
    if w <= 0.01 || h <= 0.001 {
        return;
    }
    let lean = h * slant;
    quad(
        [
            Vec2::new(x, y),
            Vec2::new(x + w, y),
            Vec2::new(x, y + h),
            Vec2::new(x + w - lean, y + h),
        ],
        [top, top, bottom, bottom],
    );
}

pub fn slant_panel_grad_h(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, slant: f32, left: Color, right: Color) {
    let _ = ui;
    if w <= 0.01 || h <= 0.001 {
        return;
    }
    let lean = h * slant;
    quad(
        [
            Vec2::new(x, y),
            Vec2::new(x + w, y),
            Vec2::new(x, y + h),
            Vec2::new(x + w - lean, y + h),
        ],
        [left, right, left, right],
    );
}

pub fn slant_panel(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, slant: f32, fill: Color) {
    slant_panel_grad(ui, x, y, w, h, slant, fill, fill);
}

pub fn slant_edge(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, slant: f32, thickness: f32, color: Color) {
    let _ = ui;
    let lean = h * slant;
    quad(
        [
            Vec2::new(x, y),
            Vec2::new(x + thickness, y),
            Vec2::new(x + w - lean, y + h),
            Vec2::new(x + w - lean + thickness, y + h),
        ],
        [color, color, color, color],
    );
}

pub fn skew_panel(ui: &mut Ui, x: f32, y: f32, w: f32, h: f32, slope: f32, top: Color, bottom: Color) {
    let _ = ui;
    if w <= 0.01 || h <= 0.001 {
        return;
    }
    let lean = h * slope;
    quad(
        [
            Vec2::new(x + lean, y),
            Vec2::new(x + w, y),
            Vec2::new(x, y + h),
            Vec2::new(x + w - lean, y + h),
        ],
        [top, top, bottom, bottom],
    );
}

pub fn parallelogram(ui: &mut Ui, r: Rect, slope: f32, top: Color, bottom: Color) -> Rect {
    skew_panel(ui, r.x, r.y, r.w, r.h, slope, top, bottom);
    let lean = r.h * slope;
    Rect::new(r.x + lean, r.y, (r.w - lean * 2.).max(0.02), r.h)
}

pub fn triangle_right(ui: &mut Ui, r: Rect, fill: Color, inner: f32, inner_fill: Option<Color>) {
    let _ = ui;
    if r.w <= 0.01 || r.h <= 0.001 {
        return;
    }
    quad(
        [
            Vec2::new(r.x, r.y),
            Vec2::new(r.x, r.y),
            Vec2::new(r.x, r.bottom()),
            Vec2::new(r.right(), r.center().y),
        ],
        [fill, fill, fill, fill],
    );
    if let Some(c) = inner_fill {
        let iw = r.w * inner;
        let ih = r.h * inner;
        let ir = Rect::new(r.x + r.w - iw, r.y + (r.h - ih) * 0.5, iw, ih);
        quad(
            [
                Vec2::new(ir.x, ir.y),
                Vec2::new(ir.x, ir.y),
                Vec2::new(ir.x, ir.bottom()),
                Vec2::new(ir.right(), ir.center().y),
            ],
            [c, c, c, c],
        );
    }
}

#[inline]
pub fn row_selected(_accent: Color) -> Color {
    semi_white(0.13)
}

#[inline]
pub fn screen_size() -> (u32, u32) {
    (screen_width() as u32, screen_height() as u32)
}

#[inline]
pub fn layout_top(ui: &Ui) -> f32 {
    ui.top.min(1. / MIN_ASPECT)
}

#[inline]
pub fn is_wide(ui: &Ui) -> bool {
    ui.top < 0.95
}

#[inline]
pub fn page_pad(ui: &Ui) -> f32 {
    if is_wide(ui) {
        PAGE_PAD_WIDE
    } else {
        PAGE_PAD
    }
}

pub struct Frame {
    pub header: Rect,
    pub body: Rect,

    pub bar: Rect,
    pub wide: bool,

    pub bar_btn_h: f32,
}

pub fn frame(ui: &Ui, bar_h: f32) -> Frame {
    let top = layout_top(ui);
    let wide = top < 0.95;
    let pad = if wide { PAGE_PAD_WIDE } else { PAGE_PAD };
    let x = -1. + pad;
    let w = 2. - pad * 2.;

    let avail = (2. * top - HEADER_TOP - pad).max(0.1);

    let chrome = HEADER_H + BODY_GAP + if bar_h > 0. { BAR_GAP + bar_h } else { 0. };

    let k = if top < 1. / MAX_ASPECT || chrome + MIN_BODY_H > avail {
        (((avail - MIN_BODY_H) / chrome).clamp(0.35, 1.)).min(1.)
    } else {
        1.
    };
    let (header_h, body_gap, bar_gap, bar_h) = (HEADER_H * k, BODY_GAP * k, BAR_GAP * k, bar_h * k);

    let header = Rect::new(x, -top + HEADER_TOP, w, header_h);
    let bar = if bar_h > 0. {
        Rect::new(x, top - pad - bar_h, w, bar_h)
    } else {
        Rect::new(x, top - pad, w, 0.)
    };
    let body_top = header.bottom() + body_gap;
    let body_bottom = if bar_h > 0. { bar.y - bar_gap } else { top - pad };
    let body = Rect::new(x, body_top, w, (body_bottom - body_top).max(MIN_BODY_H));
    Frame {
        header,
        body,
        bar,
        wide,
        bar_btn_h: bar_h,
    }
}

pub fn back_button(ui: &mut Ui, btn: &mut DRectButton, t: f32, br: Rect) {
    btn.build(ui, t, br, |ui, path| {
        ui.fill_path(&path, secondary());
        match crate::scene::TEX_ICON_BACK.with(|it| it.borrow().clone()) {
            Some(icon) => {
                let ir = br.feather(-HEADER_ICON_INSET);
                ui.fill_rect(ir, (*icon, ir));
            }
            None => {
                ui.text("‹")
                    .pos(br.center().x, br.center().y)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(0.5)
                    .color(text())
                    .draw();
            }
        }
    });
}

pub fn header(
    ui: &mut Ui,
    r: Rect,
    back: &mut DRectButton,
    right: &mut DRectButton,
    t: f32,
    title: &str,
    sub: Option<&str>,
    right_btn: Option<(&str, Color, Color)>,
) {

    let bs = r.h;
    let br = Rect::new(r.x, r.y, bs, bs);
    back_button(ui, back, t, br);

    let mut right_edge = r.right();
    if let Some((label, fill, fg)) = right_btn {
        let w = (ui.text(label).size(FS_BUTTON).measure().w + 0.09).clamp(0.18, r.w * 0.45);
        let rr = Rect::new(r.right() - w, r.y + (r.h - 0.092) / 2., w, 0.092);
        button(ui, right, t, rr, label, FS_BUTTON, fill, fg);
        right_edge = rr.x - 0.02;
    }

    let tx = br.right() + 0.035;
    let tr = ui
        .text(title)
        .pos(tx, r.center().y)
        .anchor(0., 0.5)
        .no_baseline()
        .max_width((right_edge - tx).max(0.05))
        .size(FS_PAGE_TITLE)
        .color(text())
        .draw();
    if let Some(sub) = sub {
        let sx = tx + tr.w + 0.03;
        if sx < right_edge - 0.06 {
            ui.text(sub)
                .pos(sx, r.center().y)
                .anchor(0., 0.5)
                .no_baseline()
                .max_width(right_edge - sx)
                .size(FS_SMALL)
                .color(text_muted())
                .draw();
        }
    }
}

pub fn button<'a>(
    ui: &mut Ui,
    btn: &mut DRectButton,
    t: f32,
    r: Rect,
    label: impl Into<std::borrow::Cow<'a, str>>,
    size: f32,
    fill: Color,
    fg: Color,
) {
    let label = label.into();

    btn.build(ui, t, r, |ui, path| {
        ui.fill_path(&path, fill);
        ui.stroke_path(&path, STROKE_W, stroke());
        ui.text(label.as_ref())
            .pos(r.center().x, r.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(size)
            .color(fg)
            .max_width(r.w)
            .draw();
    });
}

pub fn button_static<'a>(ui: &mut Ui, r: Rect, label: impl Into<std::borrow::Cow<'a, str>>, size: f32, fill: Color, fg: Color) {
    let label = label.into();
    let path = r.rounded(R_BTN);
    ui.fill_path(&path, fill);
    ui.stroke_path(&path, STROKE_W, stroke());
    ui.text(label.as_ref())
        .pos(r.center().x, r.center().y)
        .anchor(0.5, 0.5)
        .no_baseline()
        .size(size)
        .color(fg)
        .max_width(r.w)
        .draw();
}

pub fn card_rect(ui: &mut Ui, r: Rect, fill: Color) {
    let path = r.rounded(R_CARD);
    ui.fill_path(&path, fill);
    ui.stroke_path(&path, STROKE_W, stroke());
}

pub fn card_flat(ui: &mut Ui, r: Rect, fill: Color) {
    ui.fill_path(&r.rounded(R_CARD), fill);
}

pub fn card_accented(ui: &mut Ui, r: Rect, fill: Color, accent: Color) {
    card_rect(ui, r, fill);
    ui.fill_rect(
        Rect::new(r.x + 0.001, r.y + R_CARD + 0.004, 0.012, r.h - R_CARD * 2. - 0.008),
        color_alpha(accent, 0.9),
    );
}

pub fn h_line(ui: &mut Ui, x: f32, y: f32, w: f32) {
    if w <= 0.02 {
        return;
    }
    ui.fill_rect(Rect::new(x, y, w, 0.0014), divider());
}

pub fn panel_caption(ui: &mut Ui, x: f32, y: f32, w: f32, label: &str) {
    section_label(ui, x, y, label);
    h_line(ui, x, y + CAPTION_H - 0.008 * SCALE, w);
}

pub fn section_label(ui: &mut Ui, x: f32, y: f32, s: &str) {
    ui.text(s)
        .pos(x, y)
        .anchor(0., 0.)
        .size(FS_SMALL)
        .color(text_muted())
        .draw();
}

pub fn text_left(ui: &mut Ui, x: f32, cy: f32, size: f32, color: Color, s: &str, max_w: f32) {
    if max_w <= 0.02 {
        return;
    }
    ui.text(s)
        .pos(x, cy)
        .anchor(0., 0.5)
        .no_baseline()
        .max_width(max_w)
        .size(size)
        .color(color)
        .draw();
}

pub fn text_left_bold(ui: &mut Ui, x: f32, cy: f32, size: f32, color: Color, s: &str, max_w: f32) {
    text_left(ui, x, cy, size, color, s, max_w);
}

pub fn pill_row(ui: &mut Ui, x: f32, cy: f32, limit: f32, items: &[(String, Color, Color)]) -> f32 {
    let mut cur = x;
    let h = 0.041 * SCALE;
    for (text, bg, fg) in items {
        let w = ui.text(text).size(FS_TAG).measure().w + 0.06 * SCALE;
        if cur + w > limit {
            break;
        }
        pill_text(ui, Rect::new(cur, cy - h / 2., w, h), text, FS_TAG, *bg, *fg);
        cur += w + 0.018 * SCALE;
    }
    cur
}

pub fn text_right(ui: &mut Ui, x: f32, cy: f32, size: f32, color: Color, s: &str, max_w: f32) {
    if max_w <= 0.02 {
        return;
    }
    ui.text(s)
        .pos(x, cy)
        .anchor(1., 0.5)
        .no_baseline()
        .max_width(max_w)
        .size(size)
        .color(color)
        .draw();
}

pub fn pill_text(ui: &mut Ui, r: Rect, text: &str, size: f32, bg: Color, fg: Color) {
    ui.fill_path(&r.rounded((r.h * 0.5).min(0.02)), bg);
    ui.text(text)
        .pos(r.center().x, r.center().y)
        .anchor(0.5, 0.5)
        .no_baseline()
        .size(size)
        .max_width(r.w)
        .color(fg)
        .draw();
}

pub fn tag_right(ui: &mut Ui, right: f32, left_limit: f32, cy: f32, text: &str, bg: Color, fg: Color) -> f32 {
    let w = ui.text(text).size(FS_TAG).measure().w + 0.055 * SCALE;
    let x = right - w;
    if x < left_limit {
        return right;
    }
    let r = Rect::new(x, cy - 0.021 * SCALE, w, 0.041 * SCALE);
    pill_text(ui, r, text, FS_TAG, bg, fg);
    x - 0.022 * SCALE
}

pub fn text_chevron(ui: &mut Ui, right: f32, cy: f32) -> f32 {
    text_right(ui, right, cy, FS_SECTION, text_muted(), "›", 0.05);
    right - 0.045
}

pub fn row_button(ui: &mut Ui, btn: &mut DRectButton, t: f32, r: Rect, selected: bool, accent: Color, content: impl FnOnce(&mut Ui, Rect)) {
    let fill = if selected { row_selected(accent) } else { row() };
    btn.build(ui, t, r, |ui, path| {
        ui.fill_path(&path, fill);
        ui.stroke_path(&path, STROKE_W, if selected { color_alpha(accent, 0.7) } else { divider() });
        content(ui, r);
    });
}

#[allow(clippy::too_many_arguments)]
pub fn player_row_content(
    ui: &mut Ui,
    r: Rect,
    t: f32,
    icon: &SafeTexture,
    id: i32,
    name: &str,
    is_me: bool,
    host: bool,
    me_ready: bool,
    accent: Color,
) {
    let cy = r.center().y;
    let avr = (r.h * 0.34).min(0.038);
    let cx = r.x + CARD_PAD + avr;
    ui.avatar(cx, cy, avr, t, UserManager::opt_avatar(id, icon));

    let mut tags_right = r.right() - CARD_PAD;
    if is_me && me_ready {
        let s = mtl!("mp-ready-tag");
        tags_right = tag_right(ui, tags_right, r.x + r.w * 0.4, cy, &s, tag_accent(accent), WHITE);
    }
    if is_me {
        let s = if host { mtl!("mp-host") } else { mtl!("mp-you") };
        let (bg, fg) = if host { (tag_accent(accent), WHITE) } else { (tag_bg(), text_dim()) };
        tags_right = tag_right(ui, tags_right, r.x + r.w * 0.4, cy, &s, bg, fg);
    }

    let name_x = cx + avr + 0.03;
    text_left(ui, name_x, cy, FS_BODY, text(), name, tags_right - name_x - 0.02);
}

pub fn progress_bar(ui: &mut Ui, r: Rect, p: Option<f32>, t: f32, accent: Color) {
    if r.w <= 0.01 || r.h <= 0.001 {
        return;
    }
    ui.fill_path(&r.rounded(r.h * 0.5), semi_white(0.14));
    let (x, w) = match p {
        Some(p) => (r.x, r.w * p.clamp(0., 1.)),
        None => {
            let seg = r.w * 0.3;
            let slide = ((t * 0.6) % 1.4) / 1.4 * (r.w + seg) - seg;
            (r.x + slide.clamp(0., r.w - seg), seg)
        }
    };
    if w > 0.002 {
        ui.fill_path(&Rect::new(x, r.y, w, r.h).rounded(r.h * 0.5), color_alpha(accent, 0.85));
    }
}

pub fn flow_rects(ui: &mut Ui, labels: &[String], avail: f32, row_h: f32, row_gap: f32, col_gap: f32) -> (f32, Vec<Rect>) {
    let mut rects = Vec::with_capacity(labels.len());
    let mut cx = 0.;
    let mut cy = 0.;
    let mut rows = 1usize;
    for label in labels {
        let w = (ui.text(label.as_str()).size(FS_BUTTON).measure().w + 0.12).max(BAR_BTN_MIN_W);
        if cx + w > avail && cx > 0. {
            cx = 0.;
            cy += row_h + row_gap;
            rows += 1;
        }
        rects.push(Rect::new(cx, cy, w, row_h));
        cx += w + col_gap;
    }
    if rects.is_empty() {
        return (0., rects);
    }
    (rows as f32 * row_h + (rows - 1) as f32 * row_gap, rects)
}

pub fn tool_button(
    ui: &mut Ui,
    btn: &mut DRectButton,
    t: f32,
    r: Rect,
    icon: Option<&SafeTexture>,
    label: &str,
    active: bool,
    enabled: bool,
    accent: Color,
) {

    let fill = if !enabled {
        semi_white(0.05)
    } else if active {
        color_alpha(accent, 0.88)
    } else {
        secondary()
    };
    let fg = if !enabled {
        Color { a: 0.45, ..text() }
    } else if active {
        WHITE
    } else {
        text()
    };
    let label_fg = if !enabled {
        Color { a: 0.45, ..text_dim() }
    } else if active {
        WHITE
    } else {
        text_dim()
    };

    btn.build(ui, t, r, |ui, path| {
        ui.fill_path(&path, fill);
        ui.stroke_path(
            &path,
            STROKE_W,
            if active { color_alpha(accent, 0.95) } else { stroke() },
        );
        let label_h = r.h * 0.36;
        let icon_box = Rect::new(r.x, r.y + r.h * 0.06, r.w, r.h - label_h - r.h * 0.06);
        match icon {
            Some(tex) => {
                let s = icon_box.h.min(icon_box.w * 0.74);
                let ir = Rect::new(
                    icon_box.center().x - s / 2.,
                    icon_box.center().y - s / 2.,
                    s,
                    s,
                );
                ui.fill_rect(ir, (**tex, ir, ScaleType::Fit, fg));
            }
            None => {

                ui.text(label)
                    .pos(icon_box.center().x, icon_box.center().y)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(FS_SMALL)
                    .color(fg)
                    .max_width(r.w - 0.01)
                    .draw();
            }
        }
        if icon.is_some() {
            ui.text(label)
                .pos(r.center().x, r.bottom() - label_h * 0.55)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(FS_TAG)
                .color(label_fg)
                .max_width(r.w - 0.008)
                .draw();
        }
    });
    if !enabled {

        btn.invalidate();
    }
}

pub fn tool_bar(ui: &mut Ui, labels: &[String], x: f32, right: f32, bottom: f32, align: BarAlign) -> (Rect, Vec<Rect>) {
    let avail = (right - x).max(0.1);
    if labels.is_empty() {
        return (Rect::new(x, bottom, avail, 0.), Vec::new());
    }
    let gap = BAR_COL_GAP;
    let widths: Vec<f32> = labels
        .iter()
        .map(|l| {
            let w = ui.text(l.as_str()).size(FS_SMALL).measure().w + 0.03 * SCALE;
            w.max(ICON_BTN)
        })
        .collect();
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    let mut used = 0.;
    for (i, w) in widths.iter().enumerate() {
        let next = if cur.is_empty() { *w } else { used + gap + *w };
        if next > avail && !cur.is_empty() {
            rows.push(std::mem::take(&mut cur));
            used = *w;
        } else {
            used = next;
        }
        cur.push(i);
    }
    if !cur.is_empty() {
        rows.push(cur);
    }
    let h = ICON_BTN;
    let total_h = rows.len() as f32 * h + (rows.len() - 1) as f32 * BAR_ROW_GAP;
    let top = bottom - total_h;
    let mut rects = vec![Rect::new(x, top, 0., 0.); labels.len()];
    for (r, row) in rows.iter().enumerate() {
        let row_w: f32 = row.iter().map(|&i| widths[i]).sum::<f32>() + (row.len() - 1) as f32 * gap;
        let mut cx = match align {
            BarAlign::Right => right - row_w,
            _ => x,
        };
        let yy = top + r as f32 * (h + BAR_ROW_GAP);
        for &i in row {
            rects[i] = Rect::new(cx, yy, widths[i], h);
            cx += widths[i] + gap;
        }
    }
    (Rect::new(x, top, avail, total_h), rects)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BarAlign {

    Left,

    Right,

    Fill,
}

#[allow(clippy::too_many_arguments)]
pub fn button_bar(
    ui: &mut Ui,
    labels: &[String],
    x: f32,
    right: f32,
    bottom: f32,
    btn_h: f32,
    row_gap: f32,
    col_gap: f32,
    align: BarAlign,
) -> (Rect, Vec<Rect>) {
    let avail = (right - x).max(0.1);
    if labels.is_empty() {
        return (Rect::new(x, bottom, avail, 0.), Vec::new());
    }

    let rows: Vec<Vec<usize>> = match align {
        BarAlign::Fill => {
            let per_row = (((avail + col_gap) / (BAR_BTN_MIN_W + col_gap)).floor() as usize).clamp(1, labels.len());
            (0..labels.len()).collect::<Vec<_>>().chunks(per_row).map(|c| c.to_vec()).collect()
        }
        _ => {
            let widths: Vec<f32> = labels
                .iter()
                .map(|l| (ui.text(l.as_str()).size(FS_BUTTON).measure().w + 0.09).clamp(BAR_BTN_MIN_W, BAR_BTN_MAX_W))
                .collect();
            let mut rows: Vec<Vec<usize>> = Vec::new();
            let mut cur: Vec<usize> = Vec::new();
            let mut used = 0.;
            for (i, w) in widths.iter().enumerate() {
                let next = if cur.is_empty() { *w } else { used + col_gap + *w };
                if next > avail && !cur.is_empty() {
                    rows.push(std::mem::take(&mut cur));
                    used = *w;
                } else {
                    used = next;
                }
                cur.push(i);
            }
            if !cur.is_empty() {
                rows.push(cur);
            }
            rows
        }
    };
    let total_h = rows.len() as f32 * btn_h + (rows.len() - 1) as f32 * row_gap;
    let top = bottom - total_h;
    let mut rects = vec![Rect::new(x, top, 0., 0.); labels.len()];
    for (r, row) in rows.iter().enumerate() {
        let cnt = row.len() as f32;
        let yy = top + r as f32 * (btn_h + row_gap);
        let (mut cx, w) = match align {

            BarAlign::Fill => (x, (avail - (cnt - 1.) * col_gap) / cnt),
            BarAlign::Left | BarAlign::Right => (x, 0.),
        };
        if align != BarAlign::Fill {
            let ws: Vec<f32> = row
                .iter()
                .map(|&i| (ui.text(labels[i].as_str()).size(FS_BUTTON).measure().w + 0.09).clamp(BAR_BTN_MIN_W, BAR_BTN_MAX_W))
                .collect();
            let row_w: f32 = ws.iter().sum::<f32>() + (cnt - 1.) * col_gap;
            if align == BarAlign::Right {
                cx = right - row_w;
            }
            for (&i, &cw) in row.iter().zip(ws.iter()) {
                rects[i] = Rect::new(cx, yy, cw, btn_h);
                cx += cw + col_gap;
            }
            continue;
        }
        for &i in row {
            rects[i] = Rect::new(cx, yy, w, btn_h);
            cx += w + col_gap;
        }
    }
    (Rect::new(x, top, avail, total_h), rects)
}

// ==================== 操作条上的斜角按钮（连接页 / 服务器管理共用） ====================

/// 斜角按钮的配色（三种角色：主操作 / 次要操作 / 置灰）
pub struct BarStyle {
    pub fill: Color,
    pub fg: Color,
    pub marker: Color,
}

impl BarStyle {
    /// 主操作：强调色实心 + 白字 + 白亮条
    pub fn primary(accent: Color) -> Self {
        Self {
            fill: primary(accent),
            fg: WHITE,
            marker: WHITE,
        }
    }

    /// 次要操作：与大厅页底部那枚「离开房间」同色
    pub fn secondary() -> Self {
        Self {
            fill: secondary(),
            fg: text(),
            marker: text(),
        }
    }

    /// 置灰：底色取次要按钮的 0.7 倍，文字与亮条再压一档
    pub fn disabled() -> Self {
        Self {
            fill: color_alpha(secondary(), 0.7),
            fg: text_muted(),
            marker: color_alpha(WHITE, 0.22),
        }
    }
}

/// 斜角面片：平行四边形（`marker` 给定时在内侧斜边贴一条亮条）。
///
/// 面片直连 quad_gl，不吃 Ui 的 alpha，透明度由调用方乘进颜色里。
pub fn bar_plate(ui: &mut Ui, r: Rect, fill: Color, marker: Option<Color>, marker_right: bool) {
    let lean = r.h * PARALLELOGRAM_SLOPE;
    ui.apply(|_ui| {
        draw_parallelogram_ex(r, None, fill, fill, true);
        if let Some(mc) = marker {
            let mw = (r.h * 0.26).min(r.w * 0.2);
            let (top, bottom, dir) = if marker_right {
                (
                    Vec2::new(r.right(), r.y),
                    Vec2::new(r.right() - lean, r.bottom()),
                    Vec2::new(-mw, 0.),
                )
            } else {
                (
                    Vec2::new(r.x + lean, r.y),
                    Vec2::new(r.x, r.bottom()),
                    Vec2::new(mw, 0.),
                )
            };
            quad([top + dir, top, bottom + dir, bottom], [mc; 4]);
        }
    });
}

/// 斜角按钮：斜角片 + 内侧亮条 + 居中文字。`btn == None` 表示置灰（不登记命中区）。
pub fn bar_button(
    ui: &mut Ui,
    t: f32,
    btn: Option<&mut DRectButton>,
    r: Rect,
    label: &str,
    style: &BarStyle,
    marker_right: bool,
) {
    let enabled = btn.is_some();
    let (rr, press) = match btn {
        Some(btn) => bar_begin(btn, ui, t, r),
        None => (r, 0.),
    };
    let fill = if enabled {
        lerp_color(style.fill, WHITE, press * 0.10)
    } else {
        style.fill
    };
    bar_plate(ui, rr, fill, Some(style.marker), marker_right);
    let mw = (rr.h * 0.26).min(rr.w * 0.2);
    let text_r = if marker_right {
        Rect::new(rr.x, rr.y, rr.w - mw, rr.h)
    } else {
        Rect::new(rr.x + mw, rr.y, rr.w - mw, rr.h)
    };
    ui.text(label)
        .pos(text_r.center().x, text_r.center().y)
        .anchor(0.5, 0.5)
        .no_baseline()
        .size(FS_BUTTON)
        .color(style.fg)
        .max_width(text_r.w * 0.92)
        .draw();
}

/// 登记命中区并算一次按压动画；返回（按压时缩过的矩形, 按压力度）
fn bar_begin(btn: &mut DRectButton, ui: &mut Ui, t: f32, r: Rect) -> (Rect, f32) {
    btn.inner.set(ui, r);
    let press = (1. - btn.progress(t)).clamp(0., 1.);
    let c = r.center();
    let k = 1. - press * 0.04;
    (
        Rect::new(c.x + (r.x - c.x) * k, c.y + (r.y - c.y) * k, r.w * k, r.h * k),
        press,
    )
}
