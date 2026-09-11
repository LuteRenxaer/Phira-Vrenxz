//! 多人面板的公共绘制/几何工具：不持有任何状态。
//!
//! 这里的函数被大厅、房间、浮层共用。关键是两点“同一来源”：
//! - [`sorted_user_ids`] / [`player_ids`]：渲染与触摸用同一套排序；
//! - [`flow_rects`]：底部操作条的换行几何只算一次，渲染时由
//!   `DRectButton` 顺带登记命中区，触摸侧只按同一份
//!   [`super::room::action_items`] 结果查按钮，不会出现“看得到点不到”。

use macroquad::prelude::*;
use phira_mp_common::ClientRoomState;
use prpr::{
    core::Smooth,
    ext::{semi_black, semi_white, RectExt, SafeTexture},
    ui::{DRectButton, Ui},
};

use crate::{
    client::UserManager,
    mp::L10N_LOCAL,
};

/// 侧边面板宽度（面板坐标系下的 1 个单位 = 半个屏幕宽）。
pub const PANEL_WIDTH: f32 = 1.6;
/// 面板进出动画时长。
pub const ENTER_TRANSIT: f32 = 0.5;
/// 浮层（玩家列表 / 公共房间 / 结算 / 观战 / 房主菜单）的进出动画时长。
pub const OVERLAY_TRANSIT: f32 = 0.4;

pub fn screen_size() -> (u32, u32) {
    (screen_width() as u32, screen_height() as u32)
}

/// 浮层/动画是否还有可见进度（低于此值视为完全收起）。
#[inline]
pub fn visible(p: f32) -> bool {
    p > 1e-4
}

/// 该浮层此刻是否应当拦截（吞掉）触摸。
///
/// 语义（两个 `Smooth` 接口的区别必须分清）：
/// - `Smooth::transiting(t)`：判定区间是 `start_time..end_time` 且**包含起点**，所以动画
///   的第一帧、以及 `Smooth::default()`（`start_time = 0., end_time = 1.`）在 `t < 1.`
///   的开机首秒内都会返回 `true`；
/// - `Smooth::to()`：动画的**目标值**（收起 = `0.`，展开 = `1.`），与是否正在动画无关。
///
/// 因此只用 `transiting` 判断是不够的：默认初始态和「收起过程」都会被误判成“正在
/// 动画中”，把整个面板的按键全部吞掉。这里要求**朝打开方向**（`to() > 0.`）才拦截，
/// 既保留“已打开/正在打开的浮层在最上层拦截穿透”的语义，又不会在收起时或开机首秒
/// 吃掉触摸。所有浮层（manage / user_list / room_list / results / spectate）统一走这里，
/// 避免每处各写一套。
#[inline]
pub fn blocks_touch(p: &Smooth<f32>, t: f32) -> bool {
    p.transiting(t) && *p.to() > 0.
}

#[inline]
pub fn color_alpha(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

/// 危险操作（离开房间 / 踢人）的按钮底色。
#[inline]
pub fn danger() -> Color {
    Color::from_rgba(120, 40, 40, 235)
}

/// 以「自己优先、其余按 id 升序」排出的用户 id 列表（渲染与触摸共用同一排序）。
pub fn sorted_user_ids(room: &ClientRoomState, me: Option<i32>) -> Vec<i32> {
    let mut ids: Vec<i32> = room.users.keys().copied().collect();
    ids.sort_unstable();
    if let Some(m) = me {
        if let Some(pos) = ids.iter().position(|&x| x == m) {
            let me = ids.remove(pos);
            ids.insert(0, me);
        }
    }
    ids
}

/// 在 (x, y) 起始、宽度 avail 内按行自动换行排布按钮，返回总高度与每个按钮的矩形。
pub fn flow_rects(ui: &mut Ui, labels: &[String], x: f32, y: f32, avail: f32, row_h: f32, gap: f32, col_gap: f32) -> (f32, Vec<Rect>) {
    const TEXT_SIZE: f32 = 0.42;
    let mut rects = Vec::with_capacity(labels.len());
    let mut cx = x;
    let mut cy = y;
    let mut rows = 1usize;
    for label in labels {
        let w = ui.text(label.as_str()).size(TEXT_SIZE).measure().w + 0.11;
        let w = w.max(0.17);
        if cx + w > x + avail && cx > x {
            cx = x;
            cy += row_h + gap;
            rows += 1;
        }
        rects.push(Rect::new(cx, cy, w, row_h));
        cx += w + col_gap;
    }
    if rects.is_empty() {
        return (0., rects);
    }
    (rows as f32 * row_h + (rows - 1) as f32 * gap, rects)
}

/// 画一个小圆角胶囊文本（用于标题栏房间标签、观战按钮等）。
pub fn pill_text(ui: &mut Ui, r: Rect, text: &str, size: f32, bg: Color, fg: Color) {
    ui.fill_path(&r.rounded((r.h * 0.5).min(0.02)), bg);
    ui.text(text)
        .pos(r.center().x, r.center().y)
        .anchor(0.5, 0.5)
        .no_baseline()
        .size(size)
        .color(fg)
        .draw();
}

/// 画出玩家卡片行内容：头像 + 名字 + 右侧状态徽标（房主/我/观战/已就绪）。
/// 注意：本函数不画底色，底色由调用方（按钮路径）提供。
#[allow(clippy::too_many_arguments)]
pub fn draw_player_row(
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
    let avr = (r.h * 0.42).min(0.042);
    let cx = r.x + 0.045 + avr;
    // 头像
    ui.avatar(cx, cy, avr, t, UserManager::opt_avatar(id, icon));
    // 状态徽标从右往左排
    let mut tags_right = r.right() - 0.035;
    let mut tag = |ui: &mut Ui, text: &str, bg: Color, fg: Color, size: f32| {
        let w = ui.text(text).size(size).measure().w + 0.045;
        let x = tags_right - w;
        if x < r.x + 0.14 {
            return;
        }
        let pr = Rect::new(x, cy - 0.016, w, 0.032);
        ui.fill_path(&pr.rounded(0.016), bg);
        ui.text(text)
            .pos(pr.center().x, cy)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(size)
            .color(fg)
            .draw();
        tags_right = x - 0.02;
    };
    if watching {
        let watching_tag = mtl!("mp-watching");
        tag(ui, watching_tag.as_ref(), semi_white(0.1), semi_white(0.6), 0.3);
    }
    if is_me && me_ready {
        let ready_tag = mtl!("mp-ready-tag");
        tag(ui, ready_tag.as_ref(), color_alpha(accent, 0.28), WHITE, 0.3);
    }
    if is_me {
        let me_tag = if host { mtl!("mp-host") } else { mtl!("mp-you") };
        tag(
            ui,
            me_tag.as_ref(),
            if host { color_alpha(accent, 0.3) } else { semi_white(0.12) },
            if host { WHITE } else { semi_white(0.85) },
            0.3,
        );
    }
    // 名字（左对齐，扣除右侧徽标区）
    let name_x = r.x + 0.13;
    let name_max = (tags_right - name_x - 0.02).max(0.05);
    ui.text(name)
        .pos(name_x, cy)
        .anchor(0., 0.5)
        .no_baseline()
        .max_width(name_max)
        .size(0.4)
        .color(semi_white(0.92))
        .draw();
}

/// 玩家行底色（供不可点击的展示行使用）。
pub fn draw_player_row_bg(ui: &mut Ui, r: Rect) {
    ui.fill_path(&r.rounded(0.008), semi_black(0.22));
}

/// 画一个带阴影的可点击按钮（底色 + 居中文本），并登记命中区。
/// 渲染与触摸共用同一个 `DRectButton`，因此不存在“画一套、点另一套”。
pub fn button<'a>(ui: &mut Ui, btn: &mut DRectButton, t: f32, r: Rect, label: impl Into<std::borrow::Cow<'a, str>>, size: f32, fill: Color, fg: Color) {
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

/// 浮层外壳：全屏压暗 + 居中圆角面板，返回面板矩形。
/// 所有居中浮层（玩家列表 / 公共房间 / 结算 / 房主菜单 / 观战）都用它，
/// 保证遮罩与面板几何完全一致。
pub fn overlay_panel(ui: &mut Ui, p: f32, w: f32, h: f32, dim: f32) -> Rect {
    ui.fill_rect(ui.screen_rect(), semi_black(p * dim));
    let panel = Rect::new(-w / 2., -h / 2., w, h);
    ui.fill_path(&panel.rounded(0.018), semi_black(0.42));
    panel
}

/// 浮层标题（粗体）。
pub fn overlay_title(ui: &mut Ui, x: f32, y: f32, text: &str, size: f32) {
    ui.text(text)
        .pos(x, y)
        .size(size)
        .color(WHITE)
        .draw_using(&prpr::core::BOLD_FONT);
}
