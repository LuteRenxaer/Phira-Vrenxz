//! 连接页（未连接时的根页面）：整屏显示连接入口与服务器地址。

use macroquad::prelude::*;
use prpr::ui::{DRectButton, Ui};

use super::super::theme::{self, *};
use crate::mp::L10N_LOCAL;

/// 连接页可执行的动作。
pub enum Action {
    Connect,
    Back,
}

/// 连接页需要的只读状态。
pub struct View<'a> {
    /// 连接请求在途
    pub connecting: bool,
    /// 服务器地址（来自 `config.mp_address` 或深链接 `&server=`）
    pub address: &'a str,
}

#[derive(Default)]
pub struct ConnectPage {
    back: DRectButton,
    connect: DRectButton,
}

impl ConnectPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.connect.invalidate();
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, v: &View) {
        let accent = ui.accent();
        let f = theme::frame(ui, 0.);
        theme::header(ui, f.header, &mut self.back, &mut DRectButton::new(), t, &mtl!("multiplayer"), None, None);

        // 卡片：竖向居中偏上，宽度受限于内容区且不超过 1.2（避免平板横屏上过宽）
        let cw = f.body.w.min(1.2);
        let ch = 0.42;
        let cx = f.body.x + (f.body.w - cw) / 2.;
        let cy = f.body.y + (f.body.h - ch).max(0.) * 0.42;
        let card = Rect::new(cx, cy, cw, ch);
        theme::card_rect(ui, card, card_soft());

        ui.text(mtl!("mp-connect-hint"))
            .pos(card.center().x, card.y + 0.045)
            .anchor(0.5, 0.)
            .size(FS_SMALL)
            .color(text_dim())
            .max_width(card.w - 0.08)
            .multiline()
            .draw();
        ui.text(mtl!("mp-server", "addr" => v.address))
            .pos(card.center().x, card.y + 0.135)
            .anchor(0.5, 0.)
            .size(FS_SMALL)
            .color(text_muted())
            .max_width(card.w - 0.08)
            .draw();

        let bw = (cw - 0.12).min(0.62);
        let br = Rect::new(card.center().x - bw / 2., card.bottom() - 0.045 - 0.115, bw, 0.115);
        if v.connecting {
            theme::button_static(ui, br, mtl!("mp-connecting"), FS_BUTTON, secondary(), text_dim());
            theme::progress_bar(
                ui,
                Rect::new(br.x + 0.04, br.bottom() + 0.02, br.w - 0.08, 0.012),
                None,
                t,
                accent,
            );
        } else {
            theme::button(ui, &mut self.connect, t, br, mtl!("connect"), FS_BUTTON, primary(accent), WHITE);
        }
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<Action> {
        if self.back.touch(touch, t) {
            return Some(Action::Back);
        }
        if self.connect.touch(touch, t) {
            return Some(Action::Connect);
        }
        None
    }
}
