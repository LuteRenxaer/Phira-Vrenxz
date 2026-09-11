//! 多人场景的统一视觉系统。
//!
//! 这里集中定义**所有页面共用**的页面骨架尺寸、字号层级、颜色语义，以及按钮 /
//! 列表行 / 玩家行 / 进度条 / 标签等绘制工具。任何页面都只用这里的常量与函数，
//! 不允许在页面里各写一套数字，这样整屏分页的视觉才是同一套。
//!
//! 坐标系与游戏其余界面一致：x ∈ [-1, 1]（屏幕宽为 2），
//! y ∈ [-ui.top, ui.top]（可视高度 = `ui.top * 2.`）。
//!
//! 主色统一取 `ui.accent()`；底色全部是半透明的黑/白叠加，
//! 既不出现大面积纯黑/纯白，也不使用高饱和度的大色块。

use macroquad::prelude::*;
use prpr::{
    core::BOLD_FONT,
    ext::{semi_black, semi_white, RectExt, SafeTexture},
    ui::{DRectButton, Ui},
};

use crate::{client::UserManager, mp::L10N_LOCAL};

// ============================ 尺寸系统 ============================

/// 竖屏页面左右边距。
pub const PAGE_PAD: f32 = 0.05;
/// 横屏页面左右边距（横屏更宽松）。
pub const PAGE_PAD_WIDE: f32 = 0.08;
/// 页头距屏幕顶端。
pub const HEADER_TOP: f32 = 0.035;
/// 页头高度（返回按钮与标题同一行）。对齐本体标题行 / 控件高度（约 0.1~0.12）。
pub const HEADER_H: f32 = 0.12;
/// 返回按钮的相对内边距（图标与按钮边缘的距离系数）。
pub const HEADER_ICON_INSET: f32 = 0.02;
/// 页头与内容区的间距。
pub const BODY_GAP: f32 = 0.035;
/// 内容区与底部操作条的间距。
pub const BAR_GAP: f32 = 0.03;
/// 操作条按钮高度。本体控件高度约 0.1，这里略大一点保证移动端触控面积。
pub const BAR_BTN_H: f32 = 0.12;
/// 操作条换行时的行间距。
pub const BAR_ROW_GAP: f32 = 0.02;
/// 操作条同一行内按钮的水平间距。
pub const BAR_COL_GAP: f32 = 0.025;
/// 操作条按钮的最小宽度。
pub const BAR_BTN_MIN_W: f32 = 0.2;
/// 带第二行信息的列表行高。
pub const ROW_TALL: f32 = 0.13;
/// 列表行之间的间距。
pub const ROW_GAP: f32 = 0.015;
/// 区块之间的间距。
pub const SECTION_GAP: f32 = 0.03;
/// 区块内边距。
pub const CARD_PAD: f32 = 0.03;
/// 圆角。**统一取 0.01**：本体页面用的就是这种接近直角的面片（`rounded(0.01)`），
/// 大圆角会让多人界面显得像外挂的卡片式 App，而不是游戏本体的一部分。
pub const R_CARD: f32 = 0.01;
pub const R_ROW: f32 = 0.008;
pub const R_BTN: f32 = 0.01;
/// 宽屏时列表的最大宽度（避免超宽屏上单行过长、正文难读）。
pub const MAX_LIST_W: f32 = 1.74;

// ============================ 字号层级 ============================
//
// 取值对齐游戏本体的习惯（见 `page/library.rs`、`page/settings.rs`）：
// 标题 0.7 / 区块 0.5 / 正文 0.4 / 次要 0.35。
// 之前这套用的是 0.52/0.42/0.38/0.31 —— 整体偏小，而且相邻层级只差 0.04，
// 视觉上"糊成一片、没有层次"，这正是多人界面看着平、看着难看的主因。

/// 页面标题（配合 `BOLD_FONT` 使用）。
pub const FS_PAGE_TITLE: f32 = 0.7;
/// 区块标题 / 卡片主标题。
pub const FS_SECTION: f32 = 0.5;
/// 正文（列表主文本）。
pub const FS_BODY: f32 = 0.4;
/// 次要说明文字。
pub const FS_SMALL: f32 = 0.35;
/// 按钮文本。本体的按钮文字就是 0.5。
pub const FS_BUTTON: f32 = 0.5;
/// 徽标文本（比正文小一档，但仍要清晰可读）。
pub const FS_TAG: f32 = 0.32;
/// 大号强调文本（分数等）。
pub const FS_BIG: f32 = 0.7;

// ============================ 颜色 ============================

#[inline]
pub fn color_alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

/// 主文本色。
#[inline]
pub fn text() -> Color {
    semi_white(0.95)
}

/// 次要文本色。
#[inline]
pub fn text_dim() -> Color {
    semi_white(0.72)
}

/// 弱化提示文本色。
#[inline]
pub fn text_muted() -> Color {
    semi_white(0.45)
}

/// 区块底色。
#[inline]
pub fn card() -> Color {
    semi_black(0.3)
}

/// 更浅的区块底色（次级卡片 / 输入框）。
#[inline]
pub fn card_soft() -> Color {
    semi_black(0.2)
}

/// 列表行底色。
#[inline]
pub fn row() -> Color {
    semi_black(0.22)
}

/// 次级按钮底色。
#[inline]
pub fn secondary() -> Color {
    semi_black(0.4)
}

/// 主按钮底色（强调色）。
#[inline]
pub fn primary(accent: Color) -> Color {
    color_alpha(accent, 0.92)
}

/// 危险操作按钮底色。
#[inline]
pub fn danger() -> Color {
    Color::from_rgba(150, 54, 54, 235)
}

/// 徽标底色。
#[inline]
pub fn tag_bg() -> Color {
    semi_white(0.1)
}

/// 强调徽标底色（房主 / 已就绪 / 选中项）。
#[inline]
pub fn tag_accent(accent: Color) -> Color {
    color_alpha(accent, 0.26)
}

/// 选中行 / 自己所在行的底色。
///
/// 本体页面的选中态是"加深底色"（`semi_black(0.5)`，见 `page/library.rs` 的页签），
/// 而不是给整行铺一层主色——主色只留给主按钮与徽标，这样主色才有份量。
#[inline]
pub fn row_selected(_accent: Color) -> Color {
    semi_black(0.5)
}

// ============================ 页面骨架 ============================

/// 当前屏幕尺寸（用于检测尺寸变化后重排消息等）。
#[inline]
pub fn screen_size() -> (u32, u32) {
    (screen_width() as u32, screen_height() as u32)
}

/// 是否横屏（比 1:1 更宽）。横屏时页面可用双栏。
#[inline]
pub fn is_wide(ui: &Ui) -> bool {
    ui.top < 0.95
}

/// 当前方向的页面左右边距。
#[inline]
pub fn page_pad(ui: &Ui) -> f32 {
    if is_wide(ui) {
        PAGE_PAD_WIDE
    } else {
        PAGE_PAD
    }
}

/// 页面骨架：页头 / 内容区 / 底部操作条。
///
/// `bar_h` 为底部操作条所需高度（0 表示本页没有操作条）。
/// `wide` 为横屏（屏幕比 1:1 更宽），页面据此决定是否双栏。
pub struct Frame {
    pub header: Rect,
    pub body: Rect,
    /// 操作条区域（`bar.h == 0.` 表示本页没有）
    pub bar: Rect,
    pub wide: bool,
}

pub fn frame(ui: &Ui, bar_h: f32) -> Frame {
    let top = ui.top;
    let wide = top < 0.95;
    let pad = if wide { PAGE_PAD_WIDE } else { PAGE_PAD };
    let x = -1. + pad;
    let w = 2. - pad * 2.;
    let header = Rect::new(x, -top + HEADER_TOP, w, HEADER_H);
    let bar = if bar_h > 0. {
        Rect::new(x, top - pad - bar_h, w, bar_h)
    } else {
        Rect::new(x, top - pad, w, 0.)
    };
    let body_top = header.bottom() + BODY_GAP;
    let body_bottom = if bar_h > 0. { bar.y - BAR_GAP } else { top - pad };
    let body = Rect::new(x, body_top, w, (body_bottom - body_top).max(0.06));
    Frame { header, body, bar, wide }
}

/// 返回按钮（方块底 + 返回图标）。
///
/// 与 [`header`] 用的是同一套画法；房间页这类"标题画在内容区"的页面用它单独排一个
/// 细页头，避免整页再压一条标题行。
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

/// 无底色文本按钮（如房间页右上角的「离开房间」）：只画文字，命中区照旧由
/// `DRectButton` 登记，因此渲染与触摸同源。
pub fn text_button<'a>(
    ui: &mut Ui,
    btn: &mut DRectButton,
    t: f32,
    r: Rect,
    label: impl Into<std::borrow::Cow<'a, str>>,
    size: f32,
    color: Color,
) {
    let label = label.into();
    btn.build(ui, t, r, |ui, _| {
        ui.text(label.as_ref())
            .pos(r.center().x, r.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(size)
            .color(color)
            .max_width(r.w)
            .draw();
    });
}

/// 页头：左侧返回按钮 + 标题（+ 可选副标题）+ 可选右侧按钮。
///
/// 返回按钮与右侧按钮都用调用方持有的 `DRectButton` 渲染，因此渲染与触摸天然同源
/// （触摸侧对同一个按钮调用 `touch`）。
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
    // —— 返回 ——
    let bs = r.h;
    let br = Rect::new(r.x, r.y, bs, bs);
    back_button(ui, back, t, br);

    // —— 右侧按钮 ——
    let mut right_edge = r.right();
    if let Some((label, fill, fg)) = right_btn {
        let w = (ui.text(label).size(FS_BUTTON).measure().w + 0.09).clamp(0.18, r.w * 0.45);
        let rr = Rect::new(r.right() - w, r.y + (r.h - 0.092) / 2., w, 0.092);
        button(ui, right, t, rr, label, FS_BUTTON, fill, fg);
        right_edge = rr.x - 0.02;
    }

    // —— 标题（+ 副标题）——
    let tx = br.right() + 0.035;
    let tr = ui
        .text(title)
        .pos(tx, r.center().y)
        .anchor(0., 0.5)
        .no_baseline()
        .max_width((right_edge - tx).max(0.05))
        .size(FS_PAGE_TITLE)
        .color(text())
        .draw_using(&BOLD_FONT);
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

// ============================ 基础绘制 ============================

/// 可点击按钮：底色 + 居中文本，命中区由 `DRectButton` 登记（渲染/触摸同源）。
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
    btn.render_shadow(ui, r, t, |ui, path| {
        ui.fill_path(&path, fill);
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

/// 不可点击的按钮样式块（用于展示型操作条、占位）。
pub fn button_static<'a>(ui: &mut Ui, r: Rect, label: impl Into<std::borrow::Cow<'a, str>>, size: f32, fill: Color, fg: Color) {
    let label = label.into();
    ui.fill_path(&r.rounded(R_BTN), fill);
    ui.text(label.as_ref())
        .pos(r.center().x, r.center().y)
        .anchor(0.5, 0.5)
        .no_baseline()
        .size(size)
        .color(fg)
        .max_width(r.w)
        .draw();
}

/// 区块底色（内容卡片）。
pub fn card_rect(ui: &mut Ui, r: Rect, fill: Color) {
    ui.fill_path(&r.rounded(R_CARD), fill);
}

/// 带强调色竖条的区块（房间状态卡等）。
pub fn card_accented(ui: &mut Ui, r: Rect, fill: Color, accent: Color) {
    card_rect(ui, r, fill);
    ui.fill_rect(Rect::new(r.x, r.y + R_CARD, 0.01, r.h - R_CARD * 2.), color_alpha(accent, 0.6));
}

/// 小号区块标签（如「房间消息」）。
pub fn section_label(ui: &mut Ui, x: f32, y: f32, s: &str) {
    ui.text(s)
        .pos(x, y)
        .anchor(0., 0.)
        .size(FS_SMALL)
        .color(text_muted())
        .draw();
}

/// 单行文本（左对齐、垂直居中、超出截断）。
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

/// 单行文本（右对齐）。
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

/// 圆角胶囊文本（返回它占用的宽度）。
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

/// 从右向左排一个徽标，返回新的右边界（放不下时返回原边界且不绘制）。
pub fn tag_right(ui: &mut Ui, right: f32, left_limit: f32, cy: f32, text: &str, bg: Color, fg: Color) -> f32 {
    let w = ui.text(text).size(FS_TAG).measure().w + 0.045;
    let x = right - w;
    if x < left_limit {
        return right;
    }
    let r = Rect::new(x, cy - 0.017, w, 0.034);
    pill_text(ui, r, text, FS_TAG, bg, fg);
    x - 0.018
}

/// 行尾的指示箭头（可点击行），返回新的右边界。
pub fn text_chevron(ui: &mut Ui, right: f32, cy: f32) -> f32 {
    text_right(ui, right, cy, FS_SECTION, text_muted(), "›", 0.05);
    right - 0.045
}

// ============================ 列表 ============================

/// 可点击列表行的外壳：负责底色与按压动画，内容由 `content` 绘制。
pub fn row_button(ui: &mut Ui, btn: &mut DRectButton, t: f32, r: Rect, selected: bool, accent: Color, content: impl FnOnce(&mut Ui, Rect)) {
    let fill = if selected { row_selected(accent) } else { row() };
    btn.build(ui, t, r, |ui, path| {
        ui.fill_path(&path, fill);
        content(ui, r);
    });
}

/// 不可点击的列表行底色。
pub fn row_static(ui: &mut Ui, r: Rect, selected: bool, accent: Color) {
    let fill = if selected { row_selected(accent) } else { row() };
    ui.fill_path(&r.rounded(R_ROW), fill);
}

/// 玩家行内容：头像 + 名字 + 右侧状态徽标。
/// 本函数只画内容，底色由调用方提供（保证可点击行与展示行视觉一致）。
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
    watching: bool,
    me_ready: bool,
    accent: Color,
) {
    let cy = r.center().y;
    let avr = (r.h * 0.34).min(0.038);
    let cx = r.x + CARD_PAD + avr;
    ui.avatar(cx, cy, avr, t, UserManager::opt_avatar(id, icon));

    let mut tags_right = r.right() - CARD_PAD;
    if watching {
        let s = mtl!("mp-watching");
        tags_right = tag_right(ui, tags_right, r.x + r.w * 0.4, cy, &s, tag_bg(), text_dim());
    }
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

/// 进度条：`p` 为 `None` 时画不确定进度（来回滑动的片段）。
pub fn progress_bar(ui: &mut Ui, r: Rect, p: Option<f32>, t: f32, accent: Color) {
    if r.w <= 0.01 || r.h <= 0.001 {
        return;
    }
    ui.fill_path(&r.rounded(r.h * 0.5), semi_black(0.35));
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

/// 操作条按钮排布：返回总高度与相对 (0, 0) 的按钮矩形。
///
/// 渲染与触摸共用同一份 `labels`（由 `action_items` 推导），因此不会出现
/// 「看得到点不到」。
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
