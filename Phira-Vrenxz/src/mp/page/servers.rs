
use macroquad::prelude::*;
use prpr::{
    config::MpServer,
    ui::{DRectButton, Ui},
};

use super::super::theme::{self, *};
use crate::mp::L10N_LOCAL;

/// 透明度相乘：面片直连 quad_gl 画，不吃 Ui 的 alpha，得自己乘一份。
fn fade(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}

pub enum Action {
    /// 选中第 n 个服务器作为当前服务器
    Select(usize),
    /// 删掉第 n 个
    Delete(usize),
    /// 添加一个（名称 → 地址两步输入由会话层负责）
    Add,
    Back,
}

pub struct View<'a> {
    pub servers: &'a [MpServer],
    /// 当前生效的地址（`config.mp_address`）
    pub current: &'a str,
}

#[derive(Default)]
pub struct ServersPage {
    /// 页头返回（图标）
    back: DRectButton,
    /// 底部返回（文字按钮）
    back_bar: DRectButton,
    add: DRectButton,
    /// 每行一个「选中」按钮
    rows: Vec<DRectButton>,
    /// 每行一个「删除」按钮
    del: Vec<DRectButton>,
    /// 添加流程里刚填的名称（等地址填完再一起入表）
    pub pending_name: Option<String>,
}

impl ServersPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.back_bar.invalidate();
        self.add.invalidate();
        for it in self.rows.iter_mut() {
            it.invalidate();
        }
        for it in self.del.iter_mut() {
            it.invalidate();
        }
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, v: &View) {
        let accent = ui.accent();
        let f = theme::frame(ui, BAR_BTN_H);
        let a = ui.alpha;

        // 页头：返回 + 标题
        let hs = (f.header.h * 0.9).min(0.08);
        theme::back_button(
            ui,
            &mut self.back,
            t,
            Rect::new(f.header.x, f.header.center().y - hs * 0.5, hs, hs),
        );
        ui.text(mtl!("mp-servers-title").as_ref())
            .pos(f.header.x + hs + 0.02, f.header.center().y)
            .anchor(0., 0.5)
            .no_baseline()
            .size(FS_PAGE_TITLE)
            .color(text())
            .max_width(f.header.w - hs - 0.04)
            .draw();

        // 列表：每行一条斜角片（与操作条上的按钮同一套形状），左边名称、右边地址，
        // 当前那行用强调色 + 左侧亮条标出来
        let n = v.servers.len();
        self.rows.resize_with(n, DRectButton::default);
        self.del.resize_with(n, DRectButton::default);
        let gap = 0.012;
        let row_h = if n == 0 {
            0.1
        } else {
            ((f.body.h - gap * (n - 1) as f32) / n as f32).min(0.13)
        };
        let cur = v.current.trim();
        if n == 0 {
            ui.text(mtl!("mp-servers-empty").as_ref())
                .pos(f.body.x, f.body.center().y)
                .anchor(0., 0.5)
                .no_baseline()
                .size(FS_BODY)
                .color(text_dim())
                .max_width(f.body.w)
                .draw();
        }
        let dw = (row_h * 0.9).min(0.08);
        for i in 0..n {
            let s = &v.servers[i];
            let y = f.body.y + i as f32 * (row_h + gap);
            let r = Rect::new(f.body.x, y, f.body.w, row_h);
            let active = s.addr.trim() == cur;
            // 行本身只当「选中」用，右侧让出一枚删除按钮的宽度
            let sel_r = Rect::new(r.x, r.y, r.w - dw - 0.012, r.h);
            self.rows[i].inner.set(ui, sel_r);
            let fill = if active { primary(accent) } else { theme::secondary() };
            let marker = if active { Some(fade(WHITE, a)) } else { None };
            theme::bar_plate(ui, sel_r, fade(fill, a), marker, false);
            let fg = if active { WHITE } else { text() };
            ui.text(s.name.as_str())
                .pos(sel_r.x + 0.05, sel_r.center().y)
                .anchor(0., 0.5)
                .no_baseline()
                .size(FS_BODY)
                .color(fg)
                .max_width(sel_r.w * 0.4)
                .draw();
            ui.text(s.addr.as_str())
                .pos(sel_r.right() - 0.02, sel_r.center().y)
                .anchor(1., 0.5)
                .no_baseline()
                .size(FS_TAG)
                .color(if active { WHITE } else { text_muted() })
                .max_width(sel_r.w * 0.4)
                .draw();
            let dr = Rect::new(r.right() - dw - 0.006, r.center().y - dw * 0.5, dw, dw);
            theme::bar_button(
                ui,
                t,
                Some(&mut self.del[i]),
                dr,
                "×",
                &theme::BarStyle::secondary(),
                true,
            );
        }

        // 底部：添加服务器 + 返回
        let labels = [
            mtl!("mp-servers-add").into_owned(),
            mtl!("mp-back").into_owned(),
        ];
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
        // 两枚按钮的亮条都贴在自己内侧那一边（左「添加」贴右端、右「返回」贴左端）
        theme::bar_button(
            ui,
            t,
            Some(&mut self.add),
            rects[0],
            labels[0].as_str(),
            &theme::BarStyle::primary(accent),
            true,
        );
        theme::bar_button(
            ui,
            t,
            Some(&mut self.back_bar),
            rects[1],
            labels[1].as_str(),
            &theme::BarStyle::secondary(),
            false,
        );
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<Action> {
        for i in 0..self.del.len() {
            if self.del[i].touch(touch, t) {
                return Some(Action::Delete(i));
            }
        }
        for i in 0..self.rows.len() {
            if self.rows[i].touch(touch, t) {
                return Some(Action::Select(i));
            }
        }
        if self.add.touch(touch, t) {
            return Some(Action::Add);
        }
        if self.back.touch(touch, t) || self.back_bar.touch(touch, t) {
            return Some(Action::Back);
        }
        None
    }
}
