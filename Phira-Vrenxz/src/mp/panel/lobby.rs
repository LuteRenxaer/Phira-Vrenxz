//! 大厅（未进房）与未连接视图：居中卡片 + 三个大按钮 + 底部断开连接，
//! 以及未连接时的连接按钮。只负责“画 + 命中”，动作由面板执行。

use macroquad::prelude::*;
use prpr::{
    ext::{semi_black, semi_white, RectExt},
    ui::{DRectButton, Ui},
};

use super::widgets::{button, color_alpha, PANEL_WIDTH};
use crate::mp::L10N_LOCAL;

/// 大厅里可点的操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbyAction {
    CreateRoom,
    JoinRoom,
    OpenRoomList,
    Disconnect,
}

#[derive(Default)]
pub struct LobbyUi {
    connect_btn: DRectButton,
    create_room_btn: DRectButton,
    join_room_btn: DRectButton,
    room_list_btn: DRectButton,
    disconnect_btn: DRectButton,
}

impl LobbyUi {
    pub fn new() -> Self {
        Self::default()
    }

    /// 每帧先让所有按钮失效；只有本帧真正绘制到的按钮才会重建命中区，
    /// 避免隐藏按钮残留旧命中区被误触发。
    pub fn invalidate(&mut self) {
        self.connect_btn.invalidate();
        self.create_room_btn.invalidate();
        self.join_room_btn.invalidate();
        self.room_list_btn.invalidate();
        self.disconnect_btn.invalidate();
    }

    /// 未连接：居中连接按钮 + 关闭提示。
    pub fn render_connect(&mut self, ui: &mut Ui, t: f32) {
        let accent = ui.accent();
        let pw = PANEL_WIDTH;
        let pb = ui.top * 2.;
        let yc = (0.2 + (pb - 0.2) * 0.44).max(0.2);
        ui.text(mtl!("mp-connect-hint"))
            .pos(pw / 2., yc - 0.16)
            .anchor(0.5, 0.)
            .size(0.38)
            .color(semi_white(0.55))
            .max_width(pw - 0.2)
            .draw();
        let btn_r = Rect::new(pw / 2. - 0.22, yc - 0.09, 0.44, 0.14);
        button(ui, &mut self.connect_btn, t, btn_r, mtl!("connect"), 0.52, accent, WHITE);
        ui.text(mtl!("mp-close-hint"))
            .pos(pw / 2., pb - 0.05)
            .anchor(0.5, 1.)
            .size(0.3)
            .color(semi_white(0.3))
            .draw();
    }

    /// 已连接、未进房：居中卡片（创建 / 加入 / 公共房间）+ 底部断开连接。
    pub fn render(&mut self, ui: &mut Ui, t: f32) {
        let accent = ui.accent();
        let pw = PANEL_WIDTH;
        let pb = ui.top * 2.;
        let pad = 0.05;
        let avail = pw - pad * 2.;
        let btn_h = 0.19;
        let yc = (0.2 + (pb - 0.2) * 0.42).max(0.2);
        let gap = 0.045;
        let bw = (avail - gap * 2.) / 3.;
        let x0 = pad;
        let by = yc;

        // 卡片底
        let card = Rect::new(pad, yc - 0.03, avail, btn_h + 0.3);
        ui.fill_path(&card.rounded(0.02), semi_black(0.16));

        // 三个大按钮
        let create_r = Rect::new(x0, by, bw, btn_h);
        let join_r = Rect::new(x0 + (bw + gap), by, bw, btn_h);
        let list_r = Rect::new(x0 + (bw + gap) * 2., by, bw, btn_h);
        button(ui, &mut self.create_room_btn, t, create_r, mtl!("create-room"), 0.48, semi_black(0.32), semi_white(0.95));
        button(ui, &mut self.join_room_btn, t, join_r, mtl!("join-room"), 0.48, semi_black(0.32), semi_white(0.95));
        button(ui, &mut self.room_list_btn, t, list_r, mtl!("room-list"), 0.48, color_alpha(accent, 0.55), WHITE);

        // 状态小字
        ui.text(mtl!("mp-lobby-connected"))
            .pos(pw / 2., by + btn_h + 0.06)
            .anchor(0.5, 0.)
            .size(0.36)
            .color(accent)
            .draw();
        ui.text(mtl!("mp-lobby-not-room"))
            .pos(pw / 2., by + btn_h + 0.11)
            .anchor(0.5, 0.)
            .size(0.32)
            .color(semi_white(0.45))
            .draw();

        // 底部小字断开连接
        let w = ui.text(mtl!("disconnect")).size(0.34).measure().w + 0.1;
        let dr = Rect::new(pw / 2. - w / 2., pb - pad - 0.075, w, 0.06);
        button(ui, &mut self.disconnect_btn, t, dr, mtl!("disconnect"), 0.36, semi_black(0.3), semi_white(0.7));
    }

    /// 未连接视图的触摸。
    pub fn touch_connect(&mut self, touch: &Touch, t: f32) -> bool {
        self.connect_btn.touch(touch, t)
    }

    /// 大厅触摸；返回 Some 表示这次触摸已被某个按钮消费。
    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<LobbyAction> {
        if self.create_room_btn.touch(touch, t) {
            return Some(LobbyAction::CreateRoom);
        }
        if self.join_room_btn.touch(touch, t) {
            return Some(LobbyAction::JoinRoom);
        }
        if self.room_list_btn.touch(touch, t) {
            return Some(LobbyAction::OpenRoomList);
        }
        if self.disconnect_btn.touch(touch, t) {
            return Some(LobbyAction::Disconnect);
        }
        None
    }
}
