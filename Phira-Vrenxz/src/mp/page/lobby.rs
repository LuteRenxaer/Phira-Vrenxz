//! 主页（已连接、未进房）。
//!
//! 版面就是"房间大厅"：**中间是公共房间列表**（点任意一行直接进房，行内「观战」按钮以
//! 观战者身份旁观），**右下角是一小簇缩小后的功能按钮**（创建房间 / 加入房间 / 刷新 /
//! 断开连接）。页头只有返回 + 标题 + 当前服务器地址，不再用整条底部操作条去占版面。
//!
//! ```text
//! ‹  多人游戏   ·   服务器 127.0.0.1:31205
//! ┌─────────────────────────────────────────┐
//! │ #31205 · 等待开始 · 3 名玩家      [观战] │  ← 中央：房间列表
//! │ #31206 · 游戏进行中 · 1 名玩家    [观战] │
//! └─────────────────────────────────────────┘
//!                          [创建房间] [加入房间]  ← 右下角：缩小后的按钮
//!                          [刷新]     [断开连接]
//! ```

use macroquad::prelude::*;
use prpr::ui::{DRectButton, Scroll, Ui};

use super::super::{
    state::PublicRoom,
    theme::{self, *},
};
use crate::mp::L10N_LOCAL;

/// 主页可执行的动作。
pub enum Action {
    Back,
    CreateRoom,
    JoinRoom,
    /// 重新拉取公共房间列表
    Refresh,
    Disconnect,
    /// 点房间行直接进房（对局中的房间由会话改判为观战）
    Join(String),
    /// 行内「观战」：以观战者身份（monitor）旁观
    Spectate(String),
}

/// 主页需要的只读状态。
pub struct View<'a> {
    /// 服务器地址
    pub address: &'a str,
    /// `None` 表示还没拿到过列表（首次加载中）
    pub rooms: Option<&'a [PublicRoom]>,
    /// 拉取请求在途
    pub loading: bool,
    /// 自己所在房间（用于把该行标成「观战中」）
    pub joined: Option<&'a str>,
    /// 自己是否处于观战状态
    pub spectating: bool,
    /// 是否有会话级任务在跑（建房 / 拉取房间列表…）
    pub busy: bool,
}

/// 右下角小按钮的尺寸。
const SMALL_BTN_H: f32 = 0.095;
const SMALL_BTN_W_MAX: f32 = 0.42;

#[derive(Default)]
pub struct LobbyPage {
    back: DRectButton,
    create: DRectButton,
    join: DRectButton,
    refresh: DRectButton,
    disconnect: DRectButton,
    scroll: Scroll,
    /// 行按钮（索引与 `ids` 对齐）
    rows: Vec<DRectButton>,
    /// 「观战」按钮（索引与 `ids` 对齐）
    watches: Vec<DRectButton>,
    /// 渲染时记录的房间 id：触摸只按这份数据索引，与渲染同一来源
    ids: Vec<String>,
}

impl LobbyPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.create.invalidate();
        self.join.invalidate();
        self.refresh.invalidate();
        self.disconnect.invalidate();
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
        let f = theme::frame(ui, 0.);
        let sub = mtl!("mp-server", "addr" => v.address);
        theme::header(
            ui,
            f.header,
            &mut self.back,
            &mut DRectButton::new(),
            t,
            &mtl!("multiplayer"),
            Some(&sub),
            None,
        );

        // —— 右下角按钮簇（2×2，整体贴右下角）——
        let cluster_w = (SMALL_BTN_W_MAX * 2. + BAR_COL_GAP).min(f.body.w * 0.5);
        let bw = (cluster_w - BAR_COL_GAP) / 2.;
        let bh = SMALL_BTN_H;
        let cluster = Rect::new(f.body.right() - cluster_w, f.body.bottom() - bh * 2. - BAR_ROW_GAP, cluster_w, bh * 2. + BAR_ROW_GAP);
        theme::button(ui, &mut self.create, t, Rect::new(cluster.x, cluster.y, bw, bh), mtl!("create-room"), FS_SMALL, primary(accent), WHITE);
        theme::button(
            ui,
            &mut self.join,
            t,
            Rect::new(cluster.x + bw + BAR_COL_GAP, cluster.y, bw, bh),
            mtl!("join-room"),
            FS_SMALL,
            secondary(),
            text(),
        );
        theme::button(
            ui,
            &mut self.refresh,
            t,
            Rect::new(cluster.x, cluster.y + bh + BAR_ROW_GAP, bw, bh),
            mtl!("mp-refresh"),
            FS_SMALL,
            secondary(),
            text(),
        );
        theme::button(
            ui,
            &mut self.disconnect,
            t,
            Rect::new(cluster.x + bw + BAR_COL_GAP, cluster.y + bh + BAR_ROW_GAP, bw, bh),
            mtl!("disconnect"),
            FS_SMALL,
            danger(),
            WHITE,
        );

        // —— 中央：公共房间列表（列表底部留出按钮簇的高度）——
        let list_w = f.body.w.min(theme::MAX_LIST_W);
        let list_x = f.body.x + (f.body.w - list_w) / 2.;
        let list_h = (cluster.y - BAR_GAP - f.body.y).max(0.12);
        let list = Rect::new(list_x, f.body.y, list_w, list_h);
        let rooms = v.rooms.unwrap_or(&[]);

        if rooms.is_empty() {
            let msg = if v.loading { mtl!("room-list-loading") } else { mtl!("room-list-empty") };
            ui.text(msg.as_ref())
                .pos(list.center().x, list.y + list.h * 0.4)
                .anchor(0.5, 0.)
                .size(FS_SECTION)
                .color(text_muted())
                .draw();
            if v.loading {
                theme::progress_bar(
                    ui,
                    Rect::new(list.x, list.y + list.h * 0.4 + 0.08, list.w, 0.012),
                    None,
                    t,
                    accent,
                );
            }
            self.ids.clear();
            self.rows.clear();
            self.watches.clear();
            if v.busy {
                theme::progress_bar(ui, Rect::new(list.x, list.bottom() - 0.012, list.w, 0.012), None, t, accent);
            }
            return;
        }

        let row_h = ROW_TALL;
        let step = row_h + ROW_GAP;
        let view_h = rooms.len() as f32 * step;
        let watch_w = 0.22f32.min(list_w * 0.26);
        self.ids.clear();
        self.rows.resize_with(rooms.len(), DRectButton::new);
        self.watches.resize_with(rooms.len(), DRectButton::new);

        ui.scope(|ui| {
            ui.dx(list.x);
            ui.dy(list.y);
            self.scroll.size((list.w, list.h));
            self.scroll.render(ui, |ui| {
                for (i, room) in rooms.iter().enumerate() {
                    let rr = Rect::new(0., i as f32 * step, list.w, row_h);
                    let watching_here = v.spectating && v.joined == Some(room.id.as_str());
                    let label = format!(
                        "#{}  ·  {}  ·  {}",
                        room.id,
                        room.state,
                        mtl!("mp-room-counts", "players" => room.player_count as u64, "spectators" => room.spectator_count as u64)
                    );
                    // 行主体（点行直接进房）：右侧给「观战」按钮留出空间
                    let main_r = Rect::new(rr.x, rr.y, rr.w - watch_w - 0.03, rr.h);
                    theme::row_button(ui, &mut self.rows[i], t, main_r, watching_here, accent, |ui, r| {
                        let mut right = r.right() - CARD_PAD;
                        if room.locked {
                            let s = mtl!("mp-room-locked");
                            right = theme::tag_right(ui, right, r.x + r.w * 0.45, r.center().y, &s, tag_bg(), text_dim());
                        }
                        theme::text_left(
                            ui,
                            r.x + CARD_PAD,
                            r.center().y,
                            FS_BODY,
                            if room.locked { text_dim() } else { text() },
                            &label,
                            right - r.x - CARD_PAD,
                        );
                    });
                    // 行内「观战」
                    let wr = Rect::new(rr.right() - watch_w, rr.y + row_h * 0.22, watch_w, row_h * 0.56);
                    theme::button(
                        ui,
                        &mut self.watches[i],
                        t,
                        wr,
                        mtl!("spectate"),
                        FS_SMALL,
                        if watching_here { primary(accent) } else { secondary() },
                        text(),
                    );
                    self.ids.push(room.id.clone());
                }
                (list.w, view_h)
            });
        });

        if v.busy {
            theme::progress_bar(ui, Rect::new(list.x, list.bottom() - 0.012, list.w, 0.012), None, t, accent);
        }
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
        if self.refresh.touch(touch, t) {
            return Some(Action::Refresh);
        }
        if self.disconnect.touch(touch, t) {
            return Some(Action::Disconnect);
        }
        if self.scroll.contains(touch) && self.scroll.touch(touch, t) {
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
