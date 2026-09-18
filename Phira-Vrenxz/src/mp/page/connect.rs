//! 连接页（未连接时的根页面）：整屏显示服务器地址与两个入口 —— 「连接」与「返回主页」。
//!
//! 页面上**只有这两个按钮**：断开连接 / 重新连接 / 设置之类的入口一律不做，
//! 服务器地址与连接状态都是纯展示（地址在设置页里改，本页不开第三个入口）。
//!
//! 「返回主页」= 退出多人场景回到主界面，用的是会话里现成的返回路径
//! （`connect::Action::Back` → `MpSession::back` → `exit_scene` → `NextScene::Pop`），
//! 与大厅 / 房间页那颗「返回」是同一条路，没有另造机制；会话本身跨场景存活，
//! 所以返回主界面不会把连接与房间状态丢掉。
//!
//! 形状语言跟随大厅页（`lobby.rs`，刚按结算页 `ending.rs` 重做过的那一版）：
//! 面片一律是「平行四边形 + 竖向渐变 + 投影」（[`draw_parallelogram_ex`]，斜度取
//! [`PARALLELOGRAM_SLOPE`]），底部两枚按钮是斜角片 + 内侧一条亮色斜条
//! （左「返回主页」贴右端、右「连接」贴左端，与大厅页那对角钮同构）。
//!
//! ```text
//!  多人游戏   服务器：mp2.phira.cn:12345
//!
//!        ╱                                        ╱
//!       ╱      连接服务器后，即可与好友一起游玩      ╱
//!       ╲                                        ╲
//!
//!       [  返回主页 ▍ ][ ▍  连接  ]
//! ```

use macroquad::prelude::*;
use prpr::{
    ext::{draw_parallelogram_ex, PARALLELOGRAM_SLOPE},
    ui::{DRectButton, Ui},
};

use super::super::theme::{self, *};
use crate::mp::L10N_LOCAL;

/// 连接页可执行的动作。
pub enum Action {
    /// 发起连接
    Connect,
    /// 退出多人场景回到主界面（会话保留；与大厅 / 房间页的返回同一条路径）
    Back,
}

/// 连接页需要的只读状态。
pub struct View<'a> {
    /// 连接请求在途
    pub connecting: bool,
    /// 服务器地址（来自 `config.mp_address` 或深链接 `&server=`）
    pub address: &'a str,
}

impl View<'_> {
    /// 地址是否已配置。
    ///
    /// 空地址一定连不上（`TcpStream::connect("")` 必然失败），所以这种状态下「连接」
    /// 按钮直接置灰：与其让用户点了再吃一条「连接失败」，不如一开始就点不动。
    /// 「返回主页」不受影响 —— 连不上服务器时它恰恰是唯一的出路。
    #[inline]
    fn has_address(&self) -> bool {
        !self.address.trim().is_empty()
    }

    /// 「连接」此刻能不能点：地址没配、或已经有一个请求在途，都不行。
    #[inline]
    fn can_connect(&self) -> bool {
        !self.connecting && self.has_address()
    }
}

/// 面片内文字相对斜边的留白（斜边本身会吃掉一段水平空间）。
const INSET: f32 = 0.03 * SCALE;

#[derive(Default)]
pub struct ConnectPage {
    /// 左：返回主页（始终可点）
    back: DRectButton,
    /// 右：连接（地址为空 / 连接中时置灰）
    connect: DRectButton,
}

impl ConnectPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.connect.invalidate();
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, v: &View) {
        let accent = ui.accent();
        // 底部按标准操作条的高度预留：两枚按钮都落在操作条上，横竖屏都不会挤到内容
        let f = theme::frame(ui, BAR_BTN_H);
        // 面片直连 quad_gl 画，绕过了 Ui 的 alpha，得自己乘一份，
        // 否则整页淡入淡出时这几块面片会无视淡入、整片跳出来。
        let a = ui.alpha;

        // ==================== 页头：标题 + 服务器地址 ====================
        // 不用 theme::header：它会固定画一个返回图标，而本页的返回是操作条上那颗
        // 带文字的「返回主页」—— 同一个动作不留第二个入口。于是照大厅页页头的排法
        // 自己排一行（字号 / 颜色仍然全部取自 theme）。
        let hy = f.header.center().y;
        let title = mtl!("multiplayer");
        let tr = ui
            .text(title.as_ref())
            .pos(f.header.x, hy)
            .anchor(0., 0.5)
            .no_baseline()
            .size(FS_PAGE_TITLE)
            .color(text())
            .max_width(f.header.w * 0.5)
            .draw();
        // 地址为空时用「—」占位并提亮一档：缺了它就连不上（按钮也会一并置灰），
        // 得让人一眼看出问题出在哪一项 —— 地址依旧只在设置页里改，本页不开第三个入口。
        let (addr, addr_color) = if v.has_address() {
            (mtl!("mp-server", "addr" => v.address), theme::text_muted())
        } else {
            (mtl!("mp-server", "addr" => "—"), text())
        };
        let ax = tr.right() + INSET;
        if ax < f.header.right() - 0.06 {
            ui.text(addr.as_str())
                .pos(ax, hy)
                .anchor(0., 0.5)
                .no_baseline()
                .size(FS_TAG)
                .color(addr_color)
                .max_width(f.header.right() - ax)
                .draw();
        }

        // ==================== 主体：一块信息面片 ====================
        // 高度按**内容**给（一行说明 + 预留的状态行 + 进度条），不随屏幕拉伸 ——
        // 竖屏把面片拉高只会多出一大片空白，横竖屏的比例也就此固定下来，不会两个方向两副样子。
        // 宽度随屏幕走但不超过 1.2，免得平板横屏上拉成一条扁带子。
        let cw = f.body.w.min(1.2);
        let ch = 0.5 * SCALE;
        let cr = Rect::new(f.body.center().x - cw / 2., f.body.center().y - ch / 2., cw, ch);
        ui.apply(|_ui| panel(cr, card_soft(), card_soft(), a));
        let lean = cr.h * PARALLELOGRAM_SLOPE;
        let text_w = (cr.w - (lean + INSET) * 2.).max(0.05);

        // 说明文案：本页唯一的正文
        let hint = mtl!("mp-connect-hint");
        ui.text(hint.as_ref())
            .pos(cr.center().x, cr.y + ch * 0.40)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(FS_BODY)
            .color(text_dim())
            .max_width(text_w)
            .multiline()
            .draw();

        // 状态行 + 忙碌反馈：连接中才出现；空闲时这一行留空但仍然占位，不跳版。
        if v.connecting {
            let status = mtl!("mp-connecting");
            ui.text(status.as_ref())
                .pos(cr.center().x, cr.y + ch * 0.68)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(FS_SMALL)
                .color(color_alpha(accent, 0.95))
                .max_width(text_w)
                .draw();
            // 进度条压在面片下沿，不额外占版面
            let bw = text_w.min(0.5);
            theme::progress_bar(
                ui,
                Rect::new(cr.center().x - bw / 2., cr.y + ch * 0.86, bw, 0.008),
                None,
                t,
                accent,
            );
        }

        // ==================== 底部：返回主页（次要） + 连接（主） ====================
        // 排布交给 theme::button_bar：两枚按钮等宽铺满整条操作条（与大厅页底部那对
        // 「离开房间 / 创建房间」完全同一套算法），既没有空洞、也没有残缺的右边缘。
        // 文案沿用 multiplayer.ftl 里现成的键：mp-back（返回）/ connect（连接）——
        // locales 不在本次改动范围内，所以不为「返回主页」新造词条，语义仍是退出多人场景。
        let labels = [mtl!("mp-back").into_owned(), mtl!("connect").into_owned()];
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
        // 两枚按钮的亮条都贴在自己**内侧**那一边：左「返回主页」贴右端、右「连接」贴左端
        Self::action_btn(ui, t, Some(&mut self.back), rects[0], &labels[0], BtnStyle::secondary(), true);
        if v.can_connect() {
            Self::action_btn(ui, t, Some(&mut self.connect), rects[1], &labels[1], BtnStyle::primary(accent), false);
        } else {
            // 置灰且点不动：`DRectButton` 没有 disabled 状态，「不登记命中区」就是点不动。
            // 这里显式撤一次，免得上一帧留下的命中区（那时按钮还可点）继续吃点击。
            self.connect.invalidate();
            Self::action_btn(ui, t, None, rects[1], &labels[1], BtnStyle::disabled(), false);
        }
    }

    /// 斜角按钮：斜角片 + 内侧一条亮色斜条 + 居中文字（与大厅页底部两枚角钮同构）。
    ///
    /// `btn == None` 表示置灰态：不登记命中区、不响应按压，只用更暗的面片和弱化文字说明
    /// 「现在不能点」。置灰时的形状与可点状态**完全一致** —— 换个形状会让人以为按钮挪了位置。
    fn action_btn(ui: &mut Ui, t: f32, btn: Option<&mut DRectButton>, r: Rect, label: &str, style: BtnStyle, marker_right: bool) {
        let a = ui.alpha;
        let enabled = btn.is_some();
        let (rr, press) = match btn {
            Some(btn) => begin(btn, ui, t, r),
            None => (r, 0.),
        };
        let lean = rr.h * PARALLELOGRAM_SLOPE;
        // 面片与文字取按角色给定的颜色；按压只把面片往白里推一点（置灰态没有按压）
        let fill = if enabled { lerp_color(style.fill, WHITE, press * 0.10) } else { style.fill };
        // 亮条宽度按**高度**取（结算页那两枚角钮的亮条也是窄窄一条竖片），
        // 顺着按钮自己那条斜边贴上去；这么窄的条用 skew_panel 那种「上下各移一份 lean」
        // 的形状会自交，所以这里直接给四个顶点。
        let mw = (rr.h * 0.26).min(rr.w * 0.2);
        ui.apply(|_ui| {
            draw_parallelogram_ex(rr, None, fade(fill, a), fade(fill, a), true);
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
            theme::quad([top + dir, top, bottom + dir, bottom], [fade(style.marker, a); 4]);
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
            .color(style.fg)
            .max_width(text_r.w * 0.92)
            .draw();
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<Action> {
        // 「返回主页」始终可点，先判它（两枚按钮在版面上不重叠，顺序只影响可读性）
        if self.back.touch(touch, t) {
            return Some(Action::Back);
        }
        // 置灰的连接按钮渲染侧根本没登记命中区，这里自然也不会命中
        if self.connect.touch(touch, t) {
            return Some(Action::Connect);
        }
        None
    }
}

/// 一枚斜角按钮的配色（三种角色：主操作 / 次要操作 / 置灰）。
struct BtnStyle {
    fill: Color,
    fg: Color,
    marker: Color,
}

impl BtnStyle {
    /// 主操作（连接）：强调色实心 + 白字 + 白亮条。
    fn primary(accent: Color) -> Self {
        Self {
            fill: primary(accent),
            fg: WHITE,
            marker: WHITE,
        }
    }

    /// 次要操作（返回主页）：与大厅页底部那枚「离开房间」同色。
    fn secondary() -> Self {
        Self {
            fill: theme::secondary(),
            fg: text(),
            marker: text(),
        }
    }

    /// 置灰：底色取次要按钮的 0.7 倍 —— 介于「次要按钮」与「信息面片」之间，
    /// 既明确还是一枚按钮（不会看着像版面上的一个洞），又明显比旁边可点的按钮暗；
    /// 文字与亮条再各自压一档，于是「能点的返回主页 / 点不动的连接」一眼可分。
    fn disabled() -> Self {
        Self {
            fill: fade(theme::secondary(), 0.7),
            fg: text_muted(),
            marker: color_alpha(WHITE, 0.22),
        }
    }
}

/// 斜角面片语言：平行四边形 + 竖向渐变 + 投影（结算页画主面板用的是同一个函数）。
fn panel(r: Rect, top: Color, bottom: Color, a: f32) {
    draw_parallelogram_ex(r, None, fade(top, a), fade(bottom, a), true);
}

/// 面片透明度：[`draw_parallelogram_ex`] / [`theme::quad`] 绕过了 Ui 的 alpha，得自己乘。
fn fade(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}

/// 登记命中区，并算一次按压动画。
///
/// 面片是直连 quad_gl 画的、不吃 `DRectButton::build` 那套 Ui 变换，只能自己缩 ——
/// 否则按下时会「字在缩、底不动」。命中区一律用**未缩放**的矩形登记：视觉上缩一点没关系，
/// 但不能因此点不到。与大厅页 / 房间页里的同名小工具是同一套做法。
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
