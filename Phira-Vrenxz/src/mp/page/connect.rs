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
        let ch = (0.42f32 * SCALE).min(f.body.h * 0.9);
        let cx = f.body.x + (f.body.w - cw) / 2.;
        let cy = f.body.y + (f.body.h - ch).max(0.) * 0.42;
        let card_r = Rect::new(cx, cy, cw, ch);
        theme::card_rect(ui, card_r, card_soft());

        ui.text(mtl!("mp-connect-hint"))
            .pos(card_r.center().x, card_r.y + 0.06 * SCALE)
            .anchor(0.5, 0.)
            .size(FS_SMALL)
            .color(text_dim())
            .max_width(card_r.w - 0.1)
            .multiline()
            .draw();
        // 服务器地址：单独一行胶囊，地址长了也不会跟说明文字挤在一起
        let addr = mtl!("mp-server", "addr" => v.address);
        let aw = (ui.text(addr.as_str()).size(FS_SMALL).measure().w + 0.07).min(card_r.w - 0.08);
        theme::pill_text(
            ui,
            Rect::new(card_r.center().x - aw / 2., card_r.y + 0.165 * SCALE, aw, 0.068 * SCALE),
            addr.as_str(),
            FS_SMALL,
            card(),
            text_muted(),
        );

        let bw = (cw - 0.15 * SCALE).min(0.76 * SCALE);
        let br = Rect::new(card_r.center().x - bw / 2., card_r.bottom() - 0.045 * SCALE - BAR_BTN_H, bw, BAR_BTN_H);
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
