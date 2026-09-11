//! 玩家列表页 + 房主管理页。
//!
//! 玩家列表原本是居中浮层，现在是整屏页面（可滚动，行高统一，点击行进入管理页）；
//! 房主管理原本是弹出菜单，现在是独立的整屏页面。

use macroquad::prelude::*;
use phira_mp_common::ClientRoomState;
use prpr::{
    ext::SafeTexture,
    ui::{DRectButton, Scroll, Ui},
};

use super::super::{
    state::sorted_user_ids,
    theme::{self, *},
};
use crate::mp::L10N_LOCAL;

/// 玩家列表页可执行的动作。
pub enum Action {
    Back,
    /// 房主点某玩家行 → 进入管理页
    Manage(i32),
}

/// 玩家列表页需要的只读状态。
pub struct View<'a> {
    pub room: &'a ClientRoomState,
    pub me: Option<i32>,
    pub icon: &'a SafeTexture,
    /// 自己是否已就绪（协议只提供自己的就绪状态）
    pub me_ready: bool,
}

#[derive(Default)]
pub struct PlayersPage {
    back: DRectButton,
    scroll: Scroll,
    /// 行按钮（索引与 `ids` 对齐）
    rows: Vec<DRectButton>,
    /// 渲染时记录的用户 id：触摸只按这份数据索引，与渲染同一来源
    ids: Vec<i32>,
}

impl PlayersPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        for b in self.rows.iter_mut() {
            b.invalidate();
        }
    }

    pub fn update(&mut self, t: f32) {
        self.scroll.update(t);
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, v: &View) {
        let accent = ui.accent();
        let f = theme::frame(ui, 0.);
        let ids = sorted_user_ids(v.room, v.me);
        let title = mtl!("mp-player-count", "n" => ids.len() as u64);
        theme::header(ui, f.header, &mut self.back, &mut DRectButton::new(), t, &title, None, None);

        let list_w = f.body.w.min(theme::MAX_LIST_W);
        let list_x = f.body.x + (f.body.w - list_w) / 2.;
        let manageable = super::room::manage_allowed(v.room);
        let row_h = ROW_TALL;
        let step = row_h + ROW_GAP;
        let view_h = ids.len() as f32 * step;
        self.ids.clear();
        self.rows.resize_with(ids.len(), DRectButton::new);

        ui.scope(|ui| {
            ui.dx(list_x);
            ui.dy(f.body.y);
            self.scroll.size((list_w, f.body.h));
            self.scroll.render(ui, |ui| {
                for (i, &id) in ids.iter().enumerate() {
                    // 行按钮索引与 `ids` 必须严格对齐：先记录 id，再决定是否跳过绘制
                    self.ids.push(id);
                    let rr = Rect::new(0., i as f32 * step, list_w, row_h);
                    let Some(user) = v.room.users.get(&id) else { continue };
                    let is_me = Some(id) == v.me;
                    let clickable = manageable && !is_me;
                    let inner = Rect::new(rr.x, rr.y, rr.w - if clickable { 0.05 } else { 0. }, rr.h);
                    theme::row_button(ui, &mut self.rows[i], t, rr, is_me, accent, |ui, _| {
                        theme::player_row_content(
                            ui,
                            inner,
                            t,
                            v.icon,
                            user.id,
                            &user.name,
                            is_me,
                            is_me && v.room.is_host,
                            user.monitor,
                            is_me && v.me_ready,
                            accent,
                        );
                        if clickable {
                            theme::text_chevron(ui, rr.right() - CARD_PAD * 0.5, rr.center().y);
                        }
                    });
                }
                (list_w, view_h)
            });
        });
        if manageable {
            theme::text_left(
                ui,
                list_x,
                f.body.bottom() - 0.015,
                FS_SMALL,
                text_muted(),
                &mtl!("mp-manage-hint"),
                list_w,
            );
        }
    }

    pub fn touch(&mut self, touch: &Touch, t: f32, manageable: bool) -> Option<Action> {
        if self.back.touch(touch, t) {
            return Some(Action::Back);
        }
        if self.scroll.touch(touch, t) {
            for b in self.rows.iter_mut() {
                b.inner.cancel();
            }
            return None;
        }
        if manageable {
            for (i, b) in self.rows.iter_mut().enumerate() {
                if b.touch(touch, t) {
                    return self.ids.get(i).copied().map(Action::Manage);
                }
            }
        }
        None
    }
}

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
        let card_r = Rect::new(f.body.x + (f.body.w - card_w) / 2., f.body.y, card_w, ROW_TALL * 1.4);
        theme::card_rect(ui, card_r, card());
        theme::player_row_content(ui, card_r, t, v.icon, v.id, v.name, false, false, false, false, accent);

        // 底部操作条：设为房主 / 移出房间
        let bw = (f.bar.w.min(1.2) - 0.04) / 2.;
        let x = f.bar.center().x - (bw * 2. + 0.04) / 2.;
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
            Rect::new(x + bw + 0.04, f.bar.y, bw, BAR_BTN_H),
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
