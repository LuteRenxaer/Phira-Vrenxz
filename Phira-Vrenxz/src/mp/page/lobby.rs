//! 大厅页（已连接、未进房）：创建房间 / 加入房间 / 公共房间 / 断开连接。

use macroquad::prelude::*;
use prpr::ui::{DRectButton, Ui};

use super::super::theme::{self, *};
use crate::mp::L10N_LOCAL;

/// 大厅页可执行的动作。
pub enum Action {
    CreateRoom,
    JoinRoom,
    RoomList,
    Disconnect,
    Back,
}

/// 大厅页需要的只读状态。
pub struct View<'a> {
    /// 服务器地址
    pub address: &'a str,
    /// 是否有会话级任务在跑（建房 / 拉取房间列表…）
    pub busy: bool,
}

/// 大厅里的一个入口行（整行可点，右侧有指示箭头）。
fn entry(ui: &mut Ui, btn: &mut DRectButton, t: f32, r: Rect, label: &str, accent: Color) {
    theme::row_button(ui, btn, t, r, false, accent, |ui, r| {
        theme::text_left(ui, r.x + CARD_PAD, r.center().y, FS_SECTION, text(), label, r.w - CARD_PAD * 2. - 0.06);
        theme::text_right(ui, r.right() - CARD_PAD, r.center().y, FS_SECTION, text_muted(), "›", 0.06);
    });
}

#[derive(Default)]
pub struct LobbyPage {
    back: DRectButton,
    create: DRectButton,
    join: DRectButton,
    rooms: DRectButton,
    disconnect: DRectButton,
}

impl LobbyPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.create.invalidate();
        self.join.invalidate();
        self.rooms.invalidate();
        self.disconnect.invalidate();
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, v: &View) {
        let accent = ui.accent();
        let f = theme::frame(ui, BAR_BTN_H);
        theme::header(
            ui,
            f.header,
            &mut self.back,
            &mut DRectButton::new(),
            t,
            &mtl!("multiplayer"),
            Some(&mtl!("mp-lobby-connected")),
            None,
        );

        // 单列内容，宽度受限以免超宽屏上行过长
        let col_w = f.body.w.min(1.25);
        let x = f.body.x + (f.body.w - col_w) / 2.;
        let step = ROW_TALL + ROW_GAP;
        entry(ui, &mut self.create, t, Rect::new(x, f.body.y, col_w, ROW_TALL), &mtl!("create-room"), accent);
        entry(ui, &mut self.join, t, Rect::new(x, f.body.y + step, col_w, ROW_TALL), &mtl!("join-room"), accent);
        entry(ui, &mut self.rooms, t, Rect::new(x, f.body.y + step * 2., col_w, ROW_TALL), &mtl!("room-list"), accent);

        // 底部状态：服务器地址 + 任务进行中的进度条
        let y = f.body.y + step * 3. + 0.02;
        if y < f.body.bottom() - 0.02 {
            theme::text_left(
                ui,
                x,
                y,
                FS_SMALL,
                text_muted(),
                &mtl!("mp-server", "addr" => v.address),
                col_w,
            );
        }
        if v.busy {
            theme::progress_bar(ui, Rect::new(x, f.body.bottom() - 0.012, col_w, 0.012), None, t, accent);
        }

        // 底部操作条：断开连接
        let bw = 0.7f32.min(f.bar.w);
        let br = Rect::new(f.bar.center().x - bw / 2., f.bar.y, bw, BAR_BTN_H);
        theme::button(ui, &mut self.disconnect, t, br, mtl!("disconnect"), FS_BUTTON, danger(), WHITE);
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<Action> {
        if self.back.touch(touch, t) {
            return Some(Action::Back);
        }
        if self.create.touch(touch, t) {
            return Some(Action::CreateRoom);
        }
        if self.join.touch(touch, t) {
            return Some(Action::JoinRoom);
        }
        if self.rooms.touch(touch, t) {
            return Some(Action::RoomList);
        }
        if self.disconnect.touch(touch, t) {
            return Some(Action::Disconnect);
        }
        None
    }
}
