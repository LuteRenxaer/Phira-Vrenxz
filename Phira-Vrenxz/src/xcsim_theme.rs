//! Phire-UI 风格的配色主题。
//!
//! 本文件原是 XC-SIM 风格调色板（`FIREFLY_` 粉紫暖色系，照搬自上游
//! `xcsim/src/theme_app.rs`）。按全局任务要求，视觉样式已整体替换为
//! Phire-UI 风格：中性深灰底 + 灰白强调色 + 白色文字 + 半透明白/黑遮罩，
//! 与 Phire 的 `phire/src/ui.rs` 体系（`accent()` = 0.85 灰白、
//! `background()` = 0.13 深灰，页面惯用 `semi_white` / `semi_black`）一致。
//!
//! 命名上保留 `FIREFLY_` 前缀与全部函数签名（panel / top_bar / accent /
//! draw_* 等），避免看代码时对不上旧版、也方便后续按原 API 接入使用；
//! 只有数值与观感换成 Phire-UI。

use macroquad::color::Color;

/// Phire-UI 主强调色（灰白，对应 Phire Ui::accent）
pub const FIREFLY_PINK: Color = Color::new(0.850, 0.850, 0.850, 1.0);
/// 强调色的浅色档（近白半透明）
pub const FIREFLY_PINK_SOFT: Color = Color::new(1.000, 1.000, 1.000, 0.85);
/// 强调线的深色档（比主强调略暗的灰，用于细线/描边）
pub const FIREFLY_PINK_DEEP: Color = Color::new(0.700, 0.700, 0.700, 1.0);

/// 标题文字亮白
pub const FIREFLY_CREAM: Color = Color::new(1.000, 1.000, 1.000, 1.0);
/// 正文亮白（Phire 页面的白色文字）
pub const FIREFLY_CREAM_SOFT: Color = Color::new(0.930, 0.930, 0.930, 1.0);

/// 次级强调（亮灰）
pub const FIREFLY_MINT: Color = Color::new(0.780, 0.780, 0.780, 1.0);
pub const FIREFLY_MINT_DEEP: Color = Color::new(0.550, 0.550, 0.550, 1.0);
/// 次级文字（中灰）
pub const FIREFLY_TEAL: Color = Color::new(0.450, 0.450, 0.450, 1.0);
pub const FIREFLY_TEAL_DEEP: Color = Color::new(0.320, 0.320, 0.320, 1.0);

/// 装饰亮灰
pub const FIREFLY_GOLD: Color = Color::new(0.900, 0.900, 0.900, 1.0);
pub const FIREFLY_PEACH: Color = Color::new(0.950, 0.950, 0.950, 1.0);

/// 深色底（对应 Phire Ui::background）
pub const FIREFLY_PLUM: Color = Color::new(0.130, 0.130, 0.130, 1.0);
pub const FIREFLY_PLUM_DEEP: Color = Color::new(0.090, 0.090, 0.090, 1.0);

/// 主面板底色（半透明深灰，压在背景图上）
#[inline]
pub fn panel() -> Color {
    Color::new(0.130, 0.130, 0.130, 0.93)
}

/// 次级面板/卡片底色，比 panel 更亮更透
#[inline]
pub fn panel_soft() -> Color {
    Color::new(0.200, 0.200, 0.200, 0.84)
}

/// 顶部栏底色（最暗、最实）
#[inline]
pub fn top_bar() -> Color {
    Color::new(0.090, 0.090, 0.090, 0.97)
}

/// 强调线（分隔线/焦点线，灰白细线）
#[inline]
pub fn accent_line() -> Color {
    FIREFLY_PINK_DEEP
}

/// 强调辉光（半透明灰白，用于焦点/选中态）
#[inline]
pub fn accent_glow() -> Color {
    Color::new(0.850, 0.850, 0.850, 0.28)
}

#[inline]
pub fn mint_accent() -> Color {
    FIREFLY_MINT
}

/// 标题文字色（纯白）
#[inline]
pub fn title_text() -> Color {
    FIREFLY_CREAM_SOFT
}

/// 副标题文字色（半透明白）
#[inline]
pub fn subtitle_text() -> Color {
    Color::new(1.000, 1.000, 1.000, 0.85)
}

#[inline]
pub fn pink_overlay(a: f32) -> Color {
    Color::new(0.850, 0.850, 0.850, a)
}

#[inline]
pub fn cream_text(a: f32) -> Color {
    Color::new(1.000, 1.000, 1.000, a)
}

#[inline]
pub fn mint_overlay(a: f32) -> Color {
    Color::new(0.780, 0.780, 0.780, a)
}

#[inline]
pub fn plum_overlay(a: f32) -> Color {
    Color::new(0.000, 0.000, 0.000, a)
}

#[inline]
pub fn cream_white() -> Color {
    Color::new(1.000, 1.000, 1.000, 1.0)
}

#[inline]
pub fn plum_black() -> Color {
    Color::new(0.000, 0.000, 0.000, 1.0)
}

// ===== 以下是「画法」而不是「配色」：沿用 Phire-UI 的原语组合 =====
//
// Phire-UI 不引入自定义 shader，面板/阴影/渐变都用 Ui 自带原语
// （Ui::fill_path + RectExt::rounded 画圆角，fill_rect 画底），
// 观感为深灰面板 + 灰白细描边 + 黑色压暗渐变。

use macroquad::prelude::*;
use prpr::ext::RectExt;
use prpr::ui::Ui;

/// Phire-UI 风格页面顶栏：整宽暗灰条 + 底部一条灰白强调线。
///
/// 对应 Phire 顶部区域的 `top_bar()` + `accent_line()` 组合用法。
/// `top` 是 Ui 的坐标上边界（`ui.top` 是负值），`h` 是栏高。
pub fn draw_top_bar(ui: &mut Ui, x: f32, top: f32, w: f32, h: f32) {
    let bar = Rect::new(x, top, w, h);
    ui.fill_rect(bar, top_bar());
    // 强调线只占顶栏底部 0.006，跟 Phire 页头一致（细线而不是整条描边）
    let line = Rect::new(bar.x, bar.bottom() - 0.006, bar.w, 0.006);
    ui.fill_rect(line, accent_line());
}

/// Phire-UI 风格面板卡片：圆角 + 半透明深灰底 + 灰白细描边。
pub fn draw_panel(ui: &mut Ui, r: Rect, radius: f32, soft: bool) {
    let path = r.rounded(radius);
    ui.fill_path(&path, if soft { panel_soft() } else { panel() });
    // 描边：再画一圈略大的圆角矩形只填边——用 1.5px 的细线，颜色取强调线的低透明度版本
    let stroke = Rect::new(r.x - 0.0015, r.y - 0.0015, r.w + 0.003, r.h + 0.003);
    let mut c = accent_line();
    c.a = 0.55;
    ui.fill_path(&stroke.rounded(radius + 0.0015), c);
    ui.fill_path(&path, if soft { panel_soft() } else { panel() });
}

/// Phire-UI 风格卡片高亮：把灰白辉光铺在卡片底层，模拟 Phire 的选中光晕。
pub fn draw_accent_glow(ui: &mut Ui, r: Rect, radius: f32) {
    let glow = r.feather(0.012);
    ui.fill_path(&glow.rounded(radius + 0.012), accent_glow());
}

/// Phire-UI 风格的「软文字」：先按 8 个方向画一圈半透明黑影，再画正文。
///
/// 沿用 Phire/Phira 通用的软阴影文字手法（8 向偏移描边），
/// 让文字压在插画上时也读得清。
pub fn draw_soft_text(ui: &mut Ui, text: &str, x: f32, y: f32, anchor: (f32, f32), size: f32, color: Color) -> Rect {
    const OFFS: [(f32, f32); 8] = [
        (-0.004, 0.),
        (0.004, 0.),
        (0., -0.004),
        (0., 0.004),
        (-0.003, -0.003),
        (0.003, -0.003),
        (-0.003, 0.003),
        (0.003, 0.003),
    ];
    let shadow = Color::new(0., 0., 0., 0.18 * color.a);
    for (dx, dy) in OFFS {
        ui.text(text)
            .pos(x + dx, y + dy)
            .anchor(anchor.0, anchor.1)
            .no_baseline()
            .size(size)
            .color(shadow)
            .draw();
    }
    ui.text(text)
        .pos(x, y)
        .anchor(anchor.0, anchor.1)
        .no_baseline()
        .size(size)
        .color(color)
        .draw()
}

/// Phire-UI 风格的底部压暗渐变（Phire 页面底部常用黑色压暗）。
///
/// 用 14 段矩形模拟竖向渐变（黑底、透明度按 p^2 递增），观感与 Phire 一致。
pub fn draw_vgrad(ui: &mut Ui, r: Rect, max_alpha: f32, rgb: (f32, f32, f32)) {
    const N: i32 = 14;
    for i in 0..N {
        let p0 = i as f32 / N as f32;
        let p1 = (i + 1) as f32 / N as f32;
        let a = max_alpha * p1 * p1;
        ui.fill_rect(
            Rect::new(r.x, r.y + r.h * p0, r.w, r.h * (p1 - p0) + 0.0015),
            Color::new(rgb.0, rgb.1, rgb.2, a),
        );
    }
}
