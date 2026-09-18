//! 房间大厅（已连接、未进房）——多人模式的「选择房间」页。
//!
//! 版式骨架与形状语言**照结算页 ending.rs** 来（不再跟随谱面库那一版）：
//!
//! - 页头是一块平行四边形「主面板」：左边大字标题 + 紧跟其后的服务器地址，下面一行小字
//!   写连接状态，右边一簇斜角图标钮 —— 这正是 ending 主面板「左文字 + 右图标」的分工；
//! - 房间列表是**一块块独立的平行四边形面片**（ending 的统计块 s1 / s2 就是这种矮面板），
//!   而不是软底容器里套圆角列表行；
//! - 面板下面一条统计块沿用 ending 的排法：**小标签在下、大数字在上**（Max Combo / Accuracy）；
//! - 页面底部左右各一枚斜角角钮（左「离开房间」、右「创建房间」），内侧各贴一条亮色斜条 ——
//!   这正是 ending 两个角钮（重试 / 继续）的形体与分工。
//!
//! ```text
//!  ┌ 公共房间  服务器：127.0.0.1:31205                    [断开][刷新][加入] ┐
//!  │ 已连接到服务器                                                        │
//!  └───────────────────────────────────────────────────────────────────────┘
//!   ╱ #31205            3 名玩家          ╱ #31206            1 名玩家
//!  ╱  ID 31205  已锁定  准备中  [加入房间] ╱  ID 31206  游戏中      [加入房间]
//!   ...
//!  ┌ 公共房间  8                                          已锁定  2 ┐
//!  [   离开房间  ▍  ][  ▍  创建房间   ]
//! ```
//!
//! 三条约定：
//! 1. **形状只有一套**：全部走 ending 的「平行四边形 + 竖向渐变 + 投影」
//!    （[`prpr::ext::draw_parallelogram_ex`] 与 [`theme::skew_panel`]），页里不再自造第二种形状；
//! 2. **颜色仍归 [`theme`]**：ending 的黑面片压在多人场景的深色背景上等于没画，
//!    所以形状照搬、颜色继续用 theme 的浅色玻璃，字号也全部由 theme 的字号派生；
//! 3. **渲染与触摸同源**：每个可点元素都由本页持有的 `DRectButton` 登记命中区，
//!    触摸侧对同一份数据判定，不存在「看得到点不到」。

use macroquad::prelude::*;
use prpr::{
    ext::{draw_parallelogram_ex, SafeTexture, ScaleType, PARALLELOGRAM_SLOPE},
    ui::{DRectButton, Scroll, Ui},
};

use super::super::{
    state::PublicRoom,
    theme::{self, *},
};
use crate::mp::L10N_LOCAL;

/// 主页可执行的动作。
pub enum Action {
    Back,
    CreateRoom,
    JoinRoom,
    /// 重新拉取公共房间列表
    Refresh,
    Disconnect,
    /// 点房间行直接进房
    Join(String),
}

/// 主页需要的只读状态。
pub struct View<'a> {
    /// 服务器地址
    pub address: &'a str,
    /// `None` 表示还没拿到过列表（首次加载中）
    pub rooms: Option<&'a [PublicRoom]>,
    /// 拉取请求在途
    pub loading: bool,
    /// 是否有会话级任务在跑（建房 / 拉取房间列表…）
    pub busy: bool,
}

/// 斜角面片内部的留白：斜边本身会吃掉一段水平空间，另外再加这一档，
/// 文字才不会贴着斜边被切掉。
const INSET: f32 = 0.03 * SCALE;
/// 房间面板之间的间距。
const ROW_GAP: f32 = 0.025 * SCALE;
/// 房间面板的高度。取值让「横屏两列」时一块面板的长宽比约 6:1 ——
/// 和 ending 的统计块 s1（约 0.70 × 0.12）是同一个比例，一行才不像一条扁带子。
const ROW_H_WIDE: f32 = 0.22 * SCALE;
/// 竖屏只有一列、面板宽得多，行也就跟着加高，免得横里拉成一条细线。
const ROW_H_TALL: f32 = 0.335 * SCALE;
/// 底部统计块的高度（ending 的 s1 / s2 也都是「两行字」这么一个矮块）。
const STATS_H: f32 = 0.15 * SCALE;
/// 横屏时列表分几列（竖屏 1 列）。
///
/// 一块面板里要排「房号 / ID / 人数 / 状态 / 加入片」五段信息，
/// 横屏两列已经是能把每一段都排下的上限；再挤成三列文字就会互相压盖。
const WIDE_COLS: usize = 2;
/// 房号用的字号：比区块标题再大一档，把这一行里最要紧的信息压出来 ——
/// ending 里最大号的数字（分数）也是这么和标签拉开层级的。
const FS_ROOM_ID: f32 = FS_SECTION * 1.3;

#[derive(Default)]
pub struct LobbyPage {
    back: DRectButton,
    create: DRectButton,
    join: DRectButton,
    refresh: DRectButton,
    disconnect: DRectButton,
    /// 页头那块主面板（整块可点 = 刷新），与右上角三枚图标钮分工不同
    plate: DRectButton,
    /// 列表为空 / 拉取中时那块状态面板（点一下重新拉取）
    status: DRectButton,
    scroll: Scroll,
    /// 房间行的命中区（索引与 `ids` 对齐）
    rows: Vec<DRectButton>,
    /// 渲染时记录的房间 id：触摸只按这份数据索引，与渲染同一来源
    ids: Vec<String>,
}

impl LobbyPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.create.invalidate();
        self.join.invalidate();
        self.refresh.invalidate();
        self.disconnect.invalidate();
        self.plate.invalidate();
        self.status.invalidate();
        for b in self.rows.iter_mut() {
            b.invalidate();
        }
    }

    pub fn update(&mut self, t: f32) {
        self.scroll.update(t);
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, v: &View) {
        let accent = ui.accent();
        // 页头 / 内容区 / 底部操作条的位置仍由 theme::frame 算，和多人其余页面共用同一套边距
        let f = theme::frame(ui, BAR_BTN_H);
        // 面片是直连 quad_gl 画的（见 `panel`），绕过了 Ui 的 alpha，
        // 所以这里取当前页的 alpha 自己乘一份，否则切页淡入淡出时这些面片会整片跳出来。
        let a = ui.alpha;

        // ==================== 页头：ending 的主面板 ====================
        let plate = f.header;
        let lean = plate.h * PARALLELOGRAM_SLOPE;
        let pad = lean + INSET;

        // 右上角三枚斜角图标钮：先量出它们占的宽度，标题再拿剩下的 ——
        // 功能按钮宁可变窄也不能被标题挤掉（少一个就等于少一条出路）。
        let btn_h = plate.h * 0.62;
        let btn_w = btn_h * 1.4;
        let btn_gap = INSET * 0.9;
        let row_w = btn_w * 3. + btn_gap * 2.;
        let text_left = plate.x + pad;
        let text_right = (plate.right() - pad - row_w - INSET).max(text_left + 0.04);
        let text_w = text_right - text_left;

        // 面板本体：按下时向「选中行」的亮度过渡（ending 的面板没有按压态，
        // 但这块整片是命中区，得让人看得见自己按到了什么）。
        let (_, plate_press) = begin(&mut self.plate, ui, t, plate);
        let plate_c = lerp_color(card(), row_selected(accent), plate_press);
        // 闭包参数用不上：panel 直接进 quad_gl，apply 只是为了套上当前页的变换
        ui.apply(|_ui| panel(plate, plate_c, plate_c, a));

        // 第一行：大字标题 + 紧跟其后的服务器地址（与 theme::header 的「标题 + 副标题」同一排法）
        let title = mtl!("room-list-title");
        let line1 = plate.y + plate.h * 0.34;
        let tr = ui
            .text(title.as_ref())
            .pos(text_left, line1)
            .anchor(0., 0.5)
            .no_baseline()
            .max_width(text_w)
            .size(FS_PAGE_TITLE)
            .color(text())
            .draw();
        let addr = mtl!("mp-server", "addr" => v.address);
        let ax = tr.right() + INSET;
        if ax < text_right - 0.06 {
            ui.text(addr.as_str())
                .pos(ax, line1)
                .anchor(0., 0.5)
                .no_baseline()
                .max_width(text_right - ax)
                .size(FS_TAG)
                .color(theme::text_muted())
                .draw();
        }
        // 第二行：连接状态。这行字随面板一起可点（刷新），所以连接提示本身就是个按钮。
        let connected = mtl!("mp-lobby-connected");
        theme::text_left(
            ui,
            text_left,
            plate.y + plate.h * 0.76,
            FS_TAG,
            theme::text_muted(),
            connected.as_ref(),
            text_w,
        );

        // 断开 / 刷新 / 加入：三枚图标钮
        let btn_y = plate.center().y - btn_h * 0.5;
        let bx = plate.right() - pad - row_w;
        {
            let icons = [ToolIcon::Disconnect, ToolIcon::Refresh, ToolIcon::Join];
            for (i, icon) in icons.into_iter().enumerate() {
                let r = Rect::new(bx + i as f32 * (btn_w + btn_gap), btn_y, btn_w, btn_h);
                let btn = match i {
                    0 => &mut self.disconnect,
                    1 => &mut self.refresh,
                    _ => &mut self.join,
                };
                Self::icon_quad(ui, t, btn, r, theme::tool_icon(icon));
            }
        }

        // 拉取 / 建房在途：进度条压在页头面板的下沿。放这儿既不占版面，
        // 也不会像「容器下面悬一条线」那样在版面里看出一条空缝。
        if v.busy {
            theme::progress_bar(
                ui,
                Rect::new(text_left, plate.bottom() - INSET * 0.7, text_w.max(0.06), 0.006),
                None,
                t,
                accent,
            );
        }

        // ==================== 房间列表：一块块斜角面板 ====================
        // 从下往上排：统计块贴住内容区下沿，列表吃掉剩下的高度，两者永远不会压到一起
        let stats_h = (f.body.h * 0.18).clamp(0.075, STATS_H).min(f.body.h * 0.4);
        let stats = Rect::new(f.body.x, f.body.bottom() - stats_h, f.body.w, stats_h);
        let list = Rect::new(
            f.body.x,
            f.body.y,
            f.body.w,
            (stats.y - BODY_GAP - f.body.y).max(0.05),
        );
        let rooms = v.rooms.unwrap_or(&[]);

        if rooms.is_empty() {
            // 空列表 / 首次加载：中间一块状态面板，整块可点 = 重新拉取。
            // 加载中与「没有房间」共用同一块面板、同一个位置，进页时版面不会跳。
            self.rows.clear();
            self.ids.clear();
            // 列表没在画的时候必须把滚动矩阵撤掉，否则上一帧留下的命中区还在吃触摸
            self.scroll.set_matrix(None);
            let msg = if v.loading {
                mtl!("room-list-loading")
            } else {
                mtl!("room-list-empty")
            };
            let w = (list.w * 0.62).min(1.0);
            let h = (ROW_H_TALL * 0.8).clamp(0.1, (list.h - 0.02).max(0.1));
            let r = Rect::new(list.center().x - w / 2., list.center().y - h / 2., w, h);
            let (rr, press) = begin(&mut self.status, ui, t, r);
            let c = lerp_color(card(), row_selected(accent), press);
            ui.apply(|_ui| panel(rr, c, c, a));
            ui.text(msg.as_ref())
                .pos(rr.center().x, rr.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(FS_BODY)
                .color(theme::text_dim())
                .max_width(rr.w - INSET * 2.)
                .draw();
        } else {
            // 上一次留下的状态面板命中区必须作废，否则它会盖在列表上抢点击
            self.status.invalidate();
            let cols = if f.wide { WIDE_COLS } else { 1 };
            let base_h = if f.wide { ROW_H_WIDE } else { ROW_H_TALL };
            // 行高再受视口限制：极端比例下也不能让一行顶出列表区
            let row_h = base_h.min((list.h - ROW_GAP).max(0.06));
            let step = row_h + ROW_GAP;
            let col_gap = ROW_GAP * 1.2;
            let col_w = (list.w - col_gap * (cols - 1) as f32) / cols as f32;
            let view_h = rooms.len().div_ceil(cols) as f32 * step;

            self.ids.clear();
            self.rows.resize_with(rooms.len(), DRectButton::new);

            ui.scope(|ui| {
                ui.dx(list.x);
                ui.dy(list.y);
                self.scroll.size((list.w, list.h));
                self.scroll.render(ui, |ui| {
                    for (i, room) in rooms.iter().enumerate() {
                        let r = Rect::new(
                            (i % cols) as f32 * (col_w + col_gap),
                            (i / cols) as f32 * step,
                            col_w,
                            row_h,
                        );
                        let (rr, press) = begin(&mut self.rows[i], ui, t, r);
                        Self::render_room_row(ui, rr, room, accent, press);
                        self.ids.push(room.id.clone());
                    }
                    (list.w, view_h)
                });
            });
        }

        // ==================== 底部统计块（ending 的 label / value 排法） ====================
        let slean = stats.h * PARALLELOGRAM_SLOPE;
        ui.apply(|_ui| panel(stats, card(), card(), a));
        let s_left = stats.x + slean + INSET;
        let s_right = stats.right() - slean - INSET;
        let total = rooms.len();
        let locked = rooms.iter().filter(|it| it.locked).count();
        let label_y = stats.y + stats.h * 0.70;
        let value_y = stats.y + stats.h * 0.30;
        let list_label = mtl!("room-list");
        let lock_label = mtl!("mp-locked-tag");
        theme::text_left(ui, s_left, label_y, FS_TAG, theme::text_muted(), list_label.as_ref(), 0.4);
        theme::text_right(ui, s_right, label_y, FS_TAG, theme::text_muted(), lock_label.as_ref(), 0.4);
        theme::text_left(ui, s_left, value_y, FS_SECTION, text(), &total.to_string(), 0.4);
        theme::text_right(ui, s_right, value_y, FS_SECTION, text(), &locked.to_string(), 0.4);

        // ==================== 底部角钮：左离开 / 右创建 ====================
        let labels = [mtl!("leave-room").into_owned(), mtl!("create-room").into_owned()];
        let (_, rects) = theme::button_bar(
            ui,
            &labels,
            f.bar.x,
            f.bar.right(),
            f.bar.bottom(),
            f.bar.h,
            BAR_ROW_GAP,
            BAR_COL_GAP,
            theme::BarAlign::Fill,
        );
        // 两枚角钮的亮色斜条都贴在自己**内侧**那一边（左钮贴右端、右钮贴左端），
        // 和 ending 的 RETRY / PROCEED 一对角钮完全同构。
        Self::corner_btn(
            ui,
            t,
            &mut self.back,
            rects[0],
            labels[0].as_str(),
            secondary(),
            text(),
            text_dim(),
            true,
        );
        Self::corner_btn(
            ui,
            t,
            &mut self.create,
            rects[1],
            labels[1].as_str(),
            primary(accent),
            WHITE,
            WHITE,
            false,
        );
    }

    /// 房间面板的内容：左上房号、左下房间 ID、右上人数、右下状态（锁房时多一枚斜角小标），
    /// 行尾一块亮色斜角片当「加入」提示。
    ///
    /// 提示片**不单独建命中区**：整行本身就能点进房，再叠一个按钮，两个命中区会抢同一次点击。
    fn render_room_row(ui: &mut Ui, r: Rect, room: &PublicRoom, accent: Color, press: f32) {
        let a = ui.alpha;
        let lean = r.h * PARALLELOGRAM_SLOPE;
        let pad = INSET;
        let left = r.x + lean + pad;
        let right = r.right() - lean - pad;
        let line1 = r.y + r.h * 0.32;
        let line2 = r.y + r.h * 0.72;

        // 行尾的「加入」片：ending 里压在主面板上的那块白色 sub 面片（白底 + 深色字）。
        // 高度单独夹一档，竖屏那种高行才不会把这块片拉成一个尖三角。
        let join = mtl!("join-room");
        let chip_h = (r.h * 0.46).min(0.08);
        let chip_w = (ui.text(join.as_ref()).size(FS_TAG).measure().w + pad * 1.5)
            .max(chip_h * 1.5)
            .min(r.w * 0.3);
        let chip = Rect::new(right - chip_w, r.center().y - chip_h * 0.5, chip_w, chip_h);
        let info_right = chip.x - pad;

        // 左列宽度：房号是这一行最大号的字，占宽以「不超过信息区一半」为限
        let id_big = format!("#{}", room.id);
        let id_small = format!("ID {}", room.id);
        let usable = (info_right - left).max(0.1);
        let big_w = ui
            .text(id_big.as_str())
            .size(FS_ROOM_ID)
            .measure()
            .w
            .max(ui.text(id_small.as_str()).size(FS_TAG).measure().w)
            .min(usable * 0.45);

        // 右下：房间状态（服务端下发的文案）。锁房时左边再挂一枚斜角小标；
        // 实在挤不下就把锁房文案并进状态那一行字 —— 锁房提示无论如何都要看得见。
        let state = room.state.as_str();
        let avail = (usable - big_w - pad).max(0.05);
        let mut lower = String::new();
        let mut lower_right = info_right;
        let mut lock_rect = None;
        if room.locked {
            let s = mtl!("mp-locked-tag");
            let h = (r.h * 0.32).min(0.055);
            let w = (ui.text(s.as_ref()).size(FS_TAG).measure().w + pad * 1.2).max(h * 1.5);
            let need = w + pad * 0.6 + ui.text(state).size(FS_TAG).measure().w;
            if need <= avail {
                lock_rect = Some((s, Rect::new(lower_right - w, line2 - h * 0.5, w, h)));
                lower_right -= w + pad * 0.6;
            } else {
                lower = format!("{} · {}", state, s.as_ref());
            }
        }
        if lower.is_empty() {
            lower.push_str(state);
        }

        // —— 面片：面板本体 + 锁房小标 + 加入片 ——
        // 三者都直连 quad_gl，所以放在同一个 ui.apply 里画：Scroll 的位移只改 ui.transform，
        // 不套这一层的话这几块面片不会跟着列表滚。
        let row_c = lerp_color(card_soft(), row_selected(accent), press);
        ui.apply(|ui| {
            panel(r, row_c, row_c, a);
            if let Some((_, lr)) = &lock_rect {
                theme::skew_panel(
                    ui,
                    lr.x,
                    lr.y,
                    lr.w,
                    lr.h,
                    PARALLELOGRAM_SLOPE,
                    fade(tag_accent(accent), a),
                    fade(tag_accent(accent), a),
                );
            }
            theme::skew_panel(
                ui,
                chip.x,
                chip.y,
                chip.w,
                chip.h,
                PARALLELOGRAM_SLOPE,
                fade(text(), a),
                fade(text(), a),
            );
        });

        // —— 文字（一律画在面片之上）——
        if let Some((s, lr)) = &lock_rect {
            ui.text(s.as_ref())
                .pos(lr.center().x, lr.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(FS_TAG)
                .color(WHITE)
                .max_width(lr.w * 0.84)
                .draw();
        }
        ui.text(join.as_ref())
            .pos(chip.center().x, chip.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(FS_TAG)
            .color(on_light())
            .max_width(chip.w * 0.84)
            .draw();
        // 左上：房号（本行最大号的一行字）
        theme::text_left(ui, left, line1, FS_ROOM_ID, text(), &id_big, big_w.max(0.05));
        // 左下：房间 ID
        theme::text_left(ui, left, line2, FS_TAG, theme::text_muted(), &id_small, big_w.max(0.05));
        // 右上：人数
        let players = mtl!("mp-n-players", "n" => room.player_count as u64);
        theme::text_right(ui, info_right, line1, FS_BODY, text(), players.as_ref(), avail);
        // 右下：状态（可能已并入锁房文案）
        theme::text_right(
            ui,
            lower_right,
            line2,
            FS_TAG,
            theme::text_muted(),
            &lower,
            (lower_right - left - big_w - pad).max(0.05),
        );
    }

    /// 页头右上角的一枚斜角图标钮（ending 角钮的形体：平行四边形 + 居中图标）。
    fn icon_quad(ui: &mut Ui, t: f32, btn: &mut DRectButton, r: Rect, icon: Option<SafeTexture>) {
        let (rr, press) = begin(btn, ui, t, r);
        let a = ui.alpha;
        let fill = lerp_color(secondary(), color_alpha(ui.accent(), 0.9), press);
        ui.apply(|ui| {
            theme::skew_panel(ui, rr.x, rr.y, rr.w, rr.h, PARALLELOGRAM_SLOPE, fade(fill, a), fade(fill, a));
        });
        // 图标没装载成功时只剩一块底色：不写字占位，免得和一个并不存在的图标抢注意力
        if let Some(tex) = icon {
            let s = (rr.h * 0.46).min(rr.w * 0.5);
            let ir = Rect::new(rr.center().x - s / 2., rr.center().y - s / 2., s, s);
            ui.fill_rect(ir, (*tex, ir, ScaleType::Fit, text()));
        }
    }

    /// 底部角钮：斜角片 + 内侧一条亮色斜条 + 居中文字（ending 的 RETRY / PROCEED 同构）。
    ///
    /// `marker_right` 为真表示亮条贴右端（左侧那枚「离开房间」），否则贴左端。
    #[allow(clippy::too_many_arguments)]
    fn corner_btn(
        ui: &mut Ui,
        t: f32,
        btn: &mut DRectButton,
        r: Rect,
        label: &str,
        fill: Color,
        fg: Color,
        marker: Color,
        marker_right: bool,
    ) {
        let (rr, press) = begin(btn, ui, t, r);
        let a = ui.alpha;
        let lean = rr.h * PARALLELOGRAM_SLOPE;
        let body = lerp_color(fill, WHITE, press * 0.10);
        // 亮条宽度按**高度**取（ending 那两枚角钮的亮条也是窄窄一条竖片），
        // 顺着按钮自己那条斜边贴上去 —— 这里直接给四个顶点，是因为这么窄的条
        // 用 skew_panel 那种「上下各移一份 lean」的形状会自交。
        let mw = (rr.h * 0.26).min(rr.w * 0.2);
        ui.apply(|_ui| {
            draw_parallelogram_ex(rr, None, fade(body, a), fade(body, a), true);
            let (top, bottom, dir) = if marker_right {
                (
                    Vec2::new(rr.right(), rr.y),
                    Vec2::new(rr.right() - lean, rr.bottom()),
                    Vec2::new(-mw, 0.),
                )
            } else {
                (
                    Vec2::new(rr.x + lean, rr.y),
                    Vec2::new(rr.x, rr.bottom()),
                    Vec2::new(mw, 0.),
                )
            };
            theme::quad([top + dir, top, bottom + dir, bottom], [fade(marker, a); 4]);
        });
        // 文字居中在「让出亮条」之后的那一段里，不会压在亮条上
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
            .color(fg)
            .max_width(text_r.w * 0.92)
            .draw();
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<Action> {
        if self.back.touch(touch, t) {
            return Some(Action::Back);
        }
        if self.create.touch(touch, t) {
            return Some(Action::CreateRoom);
        }
        // 三枚图标钮必须先于页头面板判定，否则整块面板的命中区会把它们的点击吞掉
        if self.join.touch(touch, t) {
            return Some(Action::JoinRoom);
        }
        if self.refresh.touch(touch, t) {
            return Some(Action::Refresh);
        }
        if self.disconnect.touch(touch, t) {
            return Some(Action::Disconnect);
        }
        // 页头面板与这三枚图标钮在空间上是重叠的：某一枚正按着的时候绝不能再让面板
        // 记下同一根手指，否则抬手时两块命中区都会认为「点到了我」。
        let on_icon = self.join.inner.touching() || self.refresh.inner.touching() || self.disconnect.inner.touching();
        // 状态面板（加载中 / 没有房间）与页头那行连接状态：点一下都重新拉取列表
        if !on_icon && (self.status.touch(touch, t) || self.plate.touch(touch, t)) {
            return Some(Action::Refresh);
        }
        if self.scroll.contains(touch) && self.scroll.touch(touch, t) {
            for b in self.rows.iter_mut() {
                b.inner.cancel();
            }
            return None;
        }
        for (i, b) in self.rows.iter_mut().enumerate() {
            if b.touch(touch, t) {
                return self.ids.get(i).cloned().map(Action::Join);
            }
        }
        None
    }
}

/// ending 的面片语言：平行四边形 + 竖向渐变 + 投影。
///
/// 直接用结算页画主面板 / 统计块的同一个函数（[`draw_parallelogram_ex`]），
/// 斜度也取同一个 [`PARALLELOGRAM_SLOPE`]，所以本页的形状和结算页是同一套角度。
fn panel(r: Rect, top: Color, bottom: Color, a: f32) {
    draw_parallelogram_ex(r, None, fade(top, a), fade(bottom, a), true);
}

/// 面片透明度：[`draw_parallelogram_ex`] / [`theme::quad`] 绕过了 Ui 的 alpha，得自己乘。
fn fade(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}

/// 登记命中区，并算一次按压动画。
///
/// 返回（按压后的绘制矩形，按压量 0..1）。面片是直连 quad_gl 画的，不吃
/// `DRectButton::build` 那套 Ui 变换，只能自己缩 —— 否则按下时「字在缩、底不动」。
/// 命中区一律用**未缩放**的矩形登记：视觉上缩一点没关系，但不能因此点不到。
///
/// 这几个小工具与房间页里的同名私有函数是同一套做法（那边没有对外暴露），之所以各自
/// 留一份，是因为它们的全部内容就是「set 一次命中区 + 读一次 progress」。
fn begin(btn: &mut DRectButton, ui: &mut Ui, t: f32, r: Rect) -> (Rect, f32) {
    btn.inner.set(ui, r);
    // progress 每帧只能读一次（它自带状态），所以缩放与亮度都从这一个值派生
    let press = (1. - btn.progress(t)).clamp(0., 1.);
    (scaled(r, r.center(), 1. - press * 0.04), press)
}

/// 以 `center` 为中心缩放一个矩形。
fn scaled(r: Rect, center: Vec2, k: f32) -> Rect {
    Rect::new(
        center.x + (r.x - center.x) * k,
        center.y + (r.y - center.y) * k,
        r.w * k,
        r.h * k,
    )
}
