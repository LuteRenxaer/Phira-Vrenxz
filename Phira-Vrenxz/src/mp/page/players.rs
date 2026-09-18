//! 房主对单个玩家的管理页（整屏）。
//!
//! 玩家列表不再是独立页面：房间页右侧那一列就是用户列表，房主直接点行进入本页
//! （见 [`super::room`]）。所以这里只剩下「对某个玩家做什么」这一张卡片 + 两个按钮。

use macroquad::prelude::*;
use prpr::{
    ext::SafeTexture,
    ui::{DRectButton, Ui},
};

use super::super::theme::{self, *};
use crate::mp::L10N_LOCAL;

/// 房主管理页可执行的动作。
pub enum ManageAction {
    Back,
    Transfer,
    Kick,
}

/// 房主管理页需要的只读状态。
pub struct ManageView<'a> {
    pub id: i32,
    pub name: &'a str,
    pub icon: &'a SafeTexture,
}

#[derive(Default)]
pub struct ManagePage {
    back: DRectButton,
    transfer: DRectButton,
    kick: DRectButton,
}

impl ManagePage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.transfer.invalidate();
        self.kick.invalidate();
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, v: &ManageView) {
        let accent = ui.accent();
        let f = theme::frame(ui, BAR_BTN_H);
        let title = mtl!("mp-manage-title", "name" => v.name.to_owned());
        theme::header(ui, f.header, &mut self.back, &mut DRectButton::new(), t, &title, None, None);

        // 目标玩家卡片
        let card_w = f.body.w.min(1.2);
        let card_r = Rect::new(
            f.body.x + (f.body.w - card_w) / 2.,
            f.body.y,
            card_w,
            ROW_TALL * 1.6,
        );
        theme::card_rect(ui, card_r, card());
        theme::player_row_content(ui, card_r, t, v.icon, v.id, v.name, false, false, false, accent);

        // 底部操作条：设为房主 / 移出房间
        let bw = (f.bar.w.min(1.2) - BAR_COL_GAP) / 2.;
        let x = f.bar.center().x - (bw * 2. + BAR_COL_GAP) / 2.;
        theme::button(
            ui,
            &mut self.transfer,
            t,
            Rect::new(x, f.bar.y, bw, BAR_BTN_H),
            mtl!("mp-manage-transfer"),
            FS_BUTTON,
            primary(accent),
            WHITE,
        );
        theme::button(
            ui,
            &mut self.kick,
            t,
            Rect::new(x + bw + BAR_COL_GAP, f.bar.y, bw, BAR_BTN_H),
            mtl!("mp-manage-kick"),
            FS_BUTTON,
            danger(),
            WHITE,
        );
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<ManageAction> {
        if self.back.touch(touch, t) {
            return Some(ManageAction::Back);
        }
        if self.transfer.touch(touch, t) {
            return Some(ManageAction::Transfer);
        }
        if self.kick.touch(touch, t) {
            return Some(ManageAction::Kick);
        }
        None
    }
}
