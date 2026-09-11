//! 公共房间列表页：整屏滚动列表，点行加入 / 行内「观战」旁观。

use macroquad::prelude::*;
use prpr::ui::{DRectButton, Scroll, Ui};

use super::super::{
    state::PublicRoom,
    theme::{self, *},
};
use crate::mp::L10N_LOCAL;

/// 公共房间列表页可执行的动作。
pub enum Action {
    Back,
    Refresh,
    /// 点房间行加入（对局中的房间由会话改判为观战）
    Join(String),
    /// 以观战者身份（monitor）旁观
    Spectate(String),
}

/// 公共房间列表页需要的只读状态。
pub struct View<'a> {
    /// `None` 表示还没拿到过列表（首次加载中）
    pub rooms: Option<&'a [PublicRoom]>,
    /// 拉取请求在途
    pub loading: bool,
    /// 自己所在房间（用于把该行标成「观战中」）
    pub joined: Option<&'a str>,
    /// 自己是否处于观战状态
    pub spectating: bool,
}

#[derive(Default)]
pub struct RoomListPage {
    back: DRectButton,
    refresh: DRectButton,
    scroll: Scroll,
    /// 行按钮（索引与 `ids` 对齐）
    rows: Vec<DRectButton>,
    /// 「观战」按钮（索引与 `ids` 对齐）
    watches: Vec<DRectButton>,
    /// 渲染时记录的房间 id：触摸只按这份数据索引，与渲染同一来源
    ids: Vec<String>,
}

impl RoomListPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.refresh.invalidate();
        for b in self.rows.iter_mut() {
            b.invalidate();
        }
        for b in self.watches.iter_mut() {
            b.invalidate();
        }
    }

    pub fn update(&mut self, t: f32) {
        self.scroll.update(t);
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, v: &View) {
        let accent = ui.accent();
        let f = theme::frame(ui, BAR_BTN_H);
        theme::header(
            ui,
            f.header,
            &mut self.back,
            &mut self.refresh,
            t,
            &mtl!("room-list-title"),
            None,
            Some((&mtl!("mp-refresh"), secondary(), text())),
        );

        let list_w = f.body.w.min(theme::MAX_LIST_W);
        let list_x = f.body.x + (f.body.w - list_w) / 2.;
        let rooms = v.rooms.unwrap_or(&[]);

        if rooms.is_empty() {
            let msg = if v.loading { mtl!("room-list-loading") } else { mtl!("room-list-empty") };
            ui.text(msg.as_ref())
                .pos(f.body.center().x, f.body.y + f.body.h * 0.35)
                .anchor(0.5, 0.)
                .size(FS_SECTION)
                .color(text_muted())
                .draw();
            if v.loading {
                theme::progress_bar(ui, Rect::new(list_x, f.body.y + f.body.h * 0.35 + 0.09, list_w, 0.012), None, t, accent);
            }
            self.ids.clear();
            self.rows.clear();
            self.watches.clear();
            theme::button(
                ui,
                &mut self.refresh,
                t,
                Rect::new(f.bar.center().x - 0.35, f.bar.y, 0.7, BAR_BTN_H),
                mtl!("mp-refresh"),
                FS_BUTTON,
                primary(accent),
                WHITE,
            );
            return;
        }

        let row_h = ROW_TALL;
        let step = row_h + ROW_GAP;
        let view_h = rooms.len() as f32 * step;
        self.ids.clear();
        self.rows.resize_with(rooms.len(), DRectButton::new);
        self.watches.resize_with(rooms.len(), DRectButton::new);

        let watch_w = 0.24f32.min(list_w * 0.28);
        ui.scope(|ui| {
            ui.dx(list_x);
            ui.dy(f.body.y);
            self.scroll.size((list_w, f.body.h));
            self.scroll.render(ui, |ui| {
                for (i, room) in rooms.iter().enumerate() {
                    let rr = Rect::new(0., i as f32 * step, list_w, row_h);
                    let watching_here = v.spectating && v.joined == Some(room.id.as_str());
                    let label = format!(
                        "#{}  ·  {}  ·  {}",
                        room.id,
                        room.state,
                        mtl!("mp-room-counts", "players" => room.player_count as u64, "spectators" => room.spectator_count as u64)
                    );
                    // 行主体（点行加入）：右侧给「观战」按钮留出空间
                    let main_r = Rect::new(rr.x, rr.y, rr.w - watch_w - 0.03, rr.h);
                    theme::row_button(ui, &mut self.rows[i], t, main_r, watching_here, accent, |ui, r| {
                        let mut right = r.right() - CARD_PAD;
                        if room.locked {
                            let s = mtl!("mp-room-locked");
                            right = theme::tag_right(ui, right, r.x + r.w * 0.45, r.center().y, &s, tag_bg(), text_dim());
                        }
                        theme::text_left(ui, r.x + CARD_PAD, r.center().y, FS_BODY, if room.locked { text_dim() } else { text() }, &label, right - r.x - CARD_PAD);
                    });
                    // 「观战」按钮
                    let wr = Rect::new(rr.right() - watch_w, rr.y + row_h * 0.18, watch_w, row_h * 0.64);
                    theme::button(
                        ui,
                        &mut self.watches[i],
                        t,
                        wr,
                        mtl!("spectate"),
                        FS_BUTTON,
                        if watching_here { primary(accent) } else { secondary() },
                        text(),
                    );
                    self.ids.push(room.id.clone());
                }
                (list_w, view_h)
            });
        });

        theme::text_left(
            ui,
            list_x,
            f.body.bottom() - 0.015,
            FS_SMALL,
            text_muted(),
            &mtl!("room-list-tap-hint"),
            list_w,
        );

        theme::button(
            ui,
            &mut self.refresh,
            t,
            Rect::new(f.bar.center().x - 0.35, f.bar.y, 0.7, BAR_BTN_H),
            mtl!("mp-refresh"),
            FS_BUTTON,
            primary(accent),
            WHITE,
        );
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<Action> {
        if self.back.touch(touch, t) {
            return Some(Action::Back);
        }
        if self.refresh.touch(touch, t) {
            return Some(Action::Refresh);
        }
        if self.scroll.touch(touch, t) {
            for b in self.rows.iter_mut() {
                b.inner.cancel();
            }
            return None;
        }
        // 先判定「观战」（行主体矩形已排除该区域，顺序只为更稳）
        for (i, b) in self.watches.iter_mut().enumerate() {
            if b.touch(touch, t) {
                return self.ids.get(i).cloned().map(Action::Spectate);
            }
        }
        for (i, b) in self.rows.iter_mut().enumerate() {
            if b.touch(touch, t) {
                return self.ids.get(i).cloned().map(Action::Join);
            }
        }
        None
    }
}
