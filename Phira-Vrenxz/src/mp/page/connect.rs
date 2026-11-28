
use macroquad::prelude::*;
use prpr::{
    ext::{draw_parallelogram_ex, PARALLELOGRAM_SLOPE},
    ui::{DRectButton, Ui},
};

use super::super::theme::{self, *};
use crate::mp::L10N_LOCAL;

pub enum Action {

    Connect,

    Back,

    /// 打开服务器管理页（地址簿：增删、选中）
    ManageServers,
}

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
    /// 面片里的「管理服务器」入口
    manage: DRectButton,
}

impl ConnectPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.connect.invalidate();
        self.manage.invalidate();
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

        // 「管理服务器」入口：地址簿（增删、选中）全在那一页，本页只留一个入口。
        // 形状与下面操作条那两枚按钮同一套斜角片语言。
        let mw = text_w.min(0.52);
        let mh = (ch * 0.22).min(0.085);
        let mr = Rect::new(cr.center().x - mw / 2., cr.y + ch * 0.44, mw, mh);
        theme::bar_button(
            ui,
            t,
            Some(&mut self.manage),
            mr,
            mtl!("mp-servers-manage").as_ref(),
            &theme::BarStyle::secondary(),
            false,
        );

        // 状态行 + 忙碌反馈：连接中才出现；空闲时这一行留空但仍然占位，不跳版。
        if v.connecting {
            let status = mtl!("mp-connecting");
            ui.text(status.as_ref())
                .pos(cr.center().x, cr.y + ch * 0.75)
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
                Rect::new(cr.center().x - bw / 2., cr.y + ch * 0.88, bw, 0.008),
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
        theme::bar_button(ui, t, Some(&mut self.back), rects[0], &labels[0], &theme::BarStyle::secondary(), true);
        if v.can_connect() {
            theme::bar_button(ui, t, Some(&mut self.connect), rects[1], &labels[1], &theme::BarStyle::primary(accent), false);
        } else {
            // 置灰且点不动：`DRectButton` 没有 disabled 状态，「不登记命中区」就是点不动。
            // 这里显式撤一次，免得上一帧留下的命中区（那时按钮还可点）继续吃点击。
            self.connect.invalidate();
            theme::bar_button(ui, t, None, rects[1], &labels[1], &theme::BarStyle::disabled(), false);
        }
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<Action> {
        // 面片里的「管理服务器」
        if self.manage.touch(touch, t) {
            return Some(Action::ManageServers);
        }
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

/// 斜角面片语言：平行四边形 + 竖向渐变 + 投影（结算页画主面板用的是同一个函数）。
fn panel(r: Rect, top: Color, bottom: Color, a: f32) {
    draw_parallelogram_ex(r, None, fade(top, a), fade(bottom, a), true);
}

/// 面片透明度：[`draw_parallelogram_ex`] / [`theme::quad`] 绕过了 Ui 的 alpha，得自己乘。
fn fade(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}
