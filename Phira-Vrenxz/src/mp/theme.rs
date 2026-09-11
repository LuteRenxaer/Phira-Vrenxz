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
    ext::{semi_white, RectExt, SafeTexture, ScaleType},
    ui::{DRectButton, Ui},
};

use crate::{client::UserManager, mp::L10N_LOCAL};

// ============================ 尺寸系统 ============================

/// **整体缩放**：多人界面所有尺寸（边长、间距、字号）都由它派生，
/// 想整体调大调小只改这一个数。
///
/// 直接按"多小"去改各个常量很容易改乱比例：这里保留一套基准值 × `SCALE` 的写法，
/// 改一处就能等比缩放，界面不会走形。
pub const SCALE: f32 = 0.7;

/// 竖屏页面左右边距。
pub const PAGE_PAD: f32 = 0.06 * SCALE;
/// 横屏页面左右边距（横屏更宽松）。
pub const PAGE_PAD_WIDE: f32 = 0.1 * SCALE;
/// 页头距屏幕顶端。
pub const HEADER_TOP: f32 = 0.045 * SCALE;
/// 页头高度（返回按钮与标题同一行）。
pub const HEADER_H: f32 = 0.15 * SCALE;
/// 返回按钮的相对内边距（图标与按钮边缘的距离系数）。
pub const HEADER_ICON_INSET: f32 = 0.02 * SCALE;
/// 页头与内容区的间距。
pub const BODY_GAP: f32 = 0.045 * SCALE;
/// 内容区与底部操作条的间距。
pub const BAR_GAP: f32 = 0.0375 * SCALE;
/// 操作条按钮高度。
pub const BAR_BTN_H: f32 = 0.15 * SCALE;
/// 操作条换行时的行间距。
pub const BAR_ROW_GAP: f32 = 0.025 * SCALE;
/// 操作条同一行内按钮的水平间距。
pub const BAR_COL_GAP: f32 = 0.031 * SCALE;
/// 操作条按钮的最小宽度（决定一行最多放几个按钮）。
pub const BAR_BTN_MIN_W: f32 = 0.25 * SCALE;
/// 操作条按钮的最大宽度（文字很长的按钮也不会宽得离谱）。
pub const BAR_BTN_MAX_W: f32 = 0.78 * SCALE;
/// 带第二行信息的列表行高。
pub const ROW_TALL: f32 = 0.1625 * SCALE;
/// 列表行之间的间距。
pub const ROW_GAP: f32 = 0.019 * SCALE;
/// 区块之间的间距。
pub const SECTION_GAP: f32 = 0.0375 * SCALE;
/// 区块内边距。
pub const CARD_PAD: f32 = 0.0375 * SCALE;
/// 面板小标题（文字 + 分隔线）占的高度。
pub const CAPTION_H: f32 = 0.058 * SCALE;
/// 面板内容相对容器边界的内收量。
pub const PANEL_INSET: f32 = 0.015 * SCALE;
/// 左栏「房名」标题块的高度。
pub const TITLE_BLOCK_H: f32 = 0.135 * SCALE;
/// 左栏给进度行 / 确认条预留的高度（谱面卡不会把它吃掉）。
pub const STATUS_RESERVE_H: f32 = 0.3 * SCALE;
/// 圆角。**统一取 0.01**：本体页面用的就是这种接近直角的面片（`rounded(0.01)`），
/// 大圆角会让多人界面显得像外挂的卡片式 App，而不是游戏本体的一部分。
pub const R_CARD: f32 = 0.01;
pub const R_ROW: f32 = 0.008;
pub const R_BTN: f32 = 0.01;
/// 描边宽度（UI 单位；973px 宽的窗口下约 1px）。
pub const STROKE_W: f32 = 0.0025 * SCALE;
/// 宽屏时列表的最大宽度（避免超宽屏上单行过长、正文难读）。
pub const MAX_LIST_W: f32 = 1.74;

/// 工具条按钮的方块边长（模仿爱笔思画底部工具条：图标在上、小字在下）。
///
/// 参考实测：爱笔思画那条工具带在 1904x990 的窗口里高约 54px，图标与文字各占一半；
/// 这里 0.15 × [`SCALE`] 单位在 973px 宽的窗口下约 51px，同一量级。
pub const ICON_BTN: f32 = 0.15 * SCALE;

// ============================ 工具条图标 ============================

/// 工具条上要用到的图标（启动时统一加载，见 `scene::main`）。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToolIcon {
    /// 开始游戏
    Play = 0,
    /// 准备
    Ready = 1,
    /// 取消类
    Cancel = 2,
    /// 设置密码等
    Settings = 3,
    /// 循环模式
    Cycle = 4,
    /// 锁定房间
    Lock = 5,
    /// 预览谱面
    Preview = 6,
    /// 观战
    Spectate = 7,
    /// 新建房间
    Create = 8,
    /// 加入房间
    Join = 9,
    /// 刷新
    Refresh = 10,
    /// 断开连接
    Disconnect = 11,
}

thread_local! {
    static TOOL_ICONS: std::cell::RefCell<[Option<SafeTexture>; 12]> =
        const { std::cell::RefCell::new([const { None }; 12]) };
}

/// 启动时装载工具条图标（缺哪个就哪个按钮只显示文字，不影响可用性）。
pub fn set_tool_icons(icons: [Option<SafeTexture>; 12]) {
    TOOL_ICONS.with(|it| *it.borrow_mut() = icons);
}

/// 取一个工具条图标。
pub fn tool_icon(kind: ToolIcon) -> Option<SafeTexture> {
    TOOL_ICONS.with(|it| it.borrow()[kind as usize].clone())
}

// ============================ 字号层级 ============================
//
// 基准值对齐游戏本体的习惯（标题 0.7 / 区块 0.5 / 正文 0.4 / 次要 0.35），
// 再乘上 [`SCALE`] 整体收一档 —— 多人页面的信息密度比本体页面高，
// 按本体的字号直接铺会显得"块头过大、东西没几个却占满屏"。

/// 页面标题（配合 `BOLD_FONT` 使用）。
pub const FS_PAGE_TITLE: f32 = 0.7 * SCALE;
/// 区块标题 / 卡片主标题。
pub const FS_SECTION: f32 = 0.5 * SCALE;
/// 正文（列表主文本）。
pub const FS_BODY: f32 = 0.4 * SCALE;
/// 次要说明文字。
pub const FS_SMALL: f32 = 0.35 * SCALE;
/// 按钮文本。
pub const FS_BUTTON: f32 = 0.5 * SCALE;
/// 徽标文本（比正文小一档，但仍要清晰可读）。
pub const FS_TAG: f32 = 0.32 * SCALE;
/// 大号强调文本（分数等）。
pub const FS_BIG: f32 = 0.7 * SCALE;

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
    semi_white(0.78)
}

/// 弱化提示文本色。
#[inline]
pub fn text_muted() -> Color {
    semi_white(0.52)
}

// —— 面片颜色：多人场景是**深色底**（`backgrounds/mp_bg.png` 实测亮度只有 20~30），
// 所以这里不能用游戏本体的 `semi_black`：黑压黑等于什么都没画 ——
// 实测过一版：整屏平均亮度 25.7，而背景本身是 25.1，也就是说界面几乎没给画面加任何东西，
// 看起来就是"一片黑里飘着几行白字 + 一个蓝按钮"。现在的原则是**叠加浅色玻璃**：
// 面片一律比背景亮，再用一道极细的描边把边界说清楚，深色底上才有层次。
//
// 叠加量做过对比：背景亮度约 25，`semi_white(0.07)` 叠加后面片亮度约 42，
// 与背景差 17，肉眼可辨但不会喧宾夺主；描边再亮一档（0.16）负责勾边。

/// 区块底色（浅色玻璃）。
#[inline]
pub fn card() -> Color {
    semi_white(0.07)
}

/// 更淡的区块底色（次级卡片 / 列表容器）。
#[inline]
pub fn card_soft() -> Color {
    semi_white(0.05)
}

/// 列表行底色。
#[inline]
pub fn row() -> Color {
    semi_white(0.05)
}

/// 次级按钮底色。
#[inline]
pub fn secondary() -> Color {
    semi_white(0.14)
}

/// 描边色（卡片 / 按钮 / 输入框的边界）。
#[inline]
pub fn stroke() -> Color {
    semi_white(0.16)
}

/// 分隔线色（面板内的行分隔）。
#[inline]
pub fn divider() -> Color {
    semi_white(0.1)
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
    semi_white(0.18)
}

/// 强调徽标底色（房主 / 已就绪 / 选中项）。
#[inline]
pub fn tag_accent(accent: Color) -> Color {
    color_alpha(accent, 0.32)
}

/// 选中行 / 自己所在行的底色。
///
/// 深色底上"选中"要比普通行**更亮**（普通行 `semi_white(0.05)`），
/// 而不是像本体那样更深（本体是亮背景下的反向做法）。
#[inline]
pub fn row_selected(_accent: Color) -> Color {
    semi_white(0.13)
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

/// 不可点击的按钮样式块（用于展示型操作条、占位）。
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

/// 区块底色（内容卡片）：浅色玻璃 + 一道极细描边。
pub fn card_rect(ui: &mut Ui, r: Rect, fill: Color) {
    let path = r.rounded(R_CARD);
    ui.fill_path(&path, fill);
    ui.stroke_path(&path, STROKE_W, stroke());
}

/// 只有底色、不描边的区块（列表容器这类需要"融进背景"的面片）。
pub fn card_flat(ui: &mut Ui, r: Rect, fill: Color) {
    ui.fill_path(&r.rounded(R_CARD), fill);
}

/// 带强调色竖条的区块（房间状态卡等）。
pub fn card_accented(ui: &mut Ui, r: Rect, fill: Color, accent: Color) {
    card_rect(ui, r, fill);
    ui.fill_rect(
        Rect::new(r.x + 0.001, r.y + R_CARD + 0.004, 0.012, r.h - R_CARD * 2. - 0.008),
        color_alpha(accent, 0.9),
    );
}

/// 一条极细的分隔线（面板内部用）。
pub fn h_line(ui: &mut Ui, x: f32, y: f32, w: f32) {
    if w <= 0.02 {
        return;
    }
    ui.fill_rect(Rect::new(x, y, w, 0.0014), divider());
}

/// 面板小标题：文字 + 底下一条分隔线，自身负责占位高度。
pub fn panel_caption(ui: &mut Ui, x: f32, y: f32, w: f32, label: &str) {
    section_label(ui, x, y, label);
    h_line(ui, x, y + CAPTION_H - 0.008 * SCALE, w);
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

/// 单行文本（粗体，用于房名 / 谱面名这类需要压住画面的标题）。
pub fn text_left_bold(ui: &mut Ui, x: f32, cy: f32, size: f32, color: Color, s: &str, max_w: f32) {
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
        .draw_using(&BOLD_FONT);
}

/// 一行徽标（从左往右排，返回结束时的 x）。放不下的徽标会被跳过。
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
    let w = ui.text(text).size(FS_TAG).measure().w + 0.055 * SCALE;
    let x = right - w;
    if x < left_limit {
        return right;
    }
    let r = Rect::new(x, cy - 0.021 * SCALE, w, 0.041 * SCALE);
    pill_text(ui, r, text, FS_TAG, bg, fg);
    x - 0.022 * SCALE
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
        ui.stroke_path(&path, STROKE_W, if selected { color_alpha(accent, 0.7) } else { divider() });
        content(ui, r);
    });
}

/// 不可点击的列表行底色。
pub fn row_static(ui: &mut Ui, r: Rect, selected: bool, accent: Color) {
    let fill = if selected { row_selected(accent) } else { row() };
    let path = r.rounded(R_ROW);
    ui.fill_path(&path, fill);
    ui.stroke_path(&path, STROKE_W, if selected { color_alpha(accent, 0.7) } else { divider() });
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

/// 工具条按钮：一个方块，**上面图标、下面一行小字**（模仿爱笔思画的底部工具条）。
///
/// 跟 [`button`] 的区别：`button` 是"一排宽条 + 居中文字"，占地方；
/// 这里的方块只有 [`ICON_BTN`] 见方（图标占上半、小字占下半），
/// 一条工具带因此又矮又短，视觉重心让给内容（聊天 / 列表）。
pub fn tool_button(
    ui: &mut Ui,
    btn: &mut DRectButton,
    t: f32,
    r: Rect,
    icon: Option<&SafeTexture>,
    label: &str,
    active: bool,
    accent: Color,
) {
    let fill = if active { color_alpha(accent, 0.88) } else { secondary() };
    let fg = if active { WHITE } else { text() };
    let label_fg = if active { WHITE } else { text_dim() };
    btn.render_shadow(ui, r, t, |ui, path| {
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
                // 没有图标：把文字画大一点占住图标区，方块尺寸保持一致
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
}

/// 工具带：把 [`tool_button`] 方块排成一行（放不下才换行），整体贴着 `bottom`。
///
/// 方块宽度 = max([`ICON_BTN`], 文字宽度 + 内边距)，因此不用把按钮拉宽来凑满一行；
/// `align` 决定整条带子贴左边界还是右边界（房间页贴左、主页贴右）。
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

/// 按钮带的水平对齐方式。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BarAlign {
    /// 宽度随文字走、从左边界开始排（右侧会留白，适合按钮少而短的场合）
    Left,
    /// 宽度随文字走、贴着右边界排（主页右下角那一簇按钮）
    Right,
    /// **等分铺满整行**：每个按钮一样宽、整行正好占满可用宽度（默认用这个）
    Fill,
}

/// 操作按钮带：把按钮排成一行（放不下才换行），整体贴着 `bottom`。
///
/// 之前用的是"等分列数的网格"（3 列 2 行），结果 5 个按钮排成 3+2，第二行右侧空一格，
/// 而且整条带子只占左栏宽度 —— 底部一大片横向空间空着，这就是看着别扭的根源。
/// 现在默认 [`BarAlign::Fill`]：[`BAR_BTN_MIN_W`] 决定一行最多放几个，每个按钮等宽，
/// 整行从左边界一直铺到右边界，既没有洞也没有残缺的右边缘。
///
/// 返回 `(整条带子的矩形, 每个按钮的矩形)`；`labels` 的顺序即按钮顺序。
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
    // 行划分：Fill 用最小宽度算出每行最多几个（等分）；Left/Right 用文字宽度贪心换行。
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
            // 等分：每个按钮一样宽，整行铺满
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
