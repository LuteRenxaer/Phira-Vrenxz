//! 居中浮层：玩家列表 / 公共房间列表 / 对局结算排名 / 房主对某玩家的操作菜单。
//!
//! 每个浮层都是一个「进出动画 + 滚动 + 按钮」的小单元，统一走
//! [`Overlays::touch`] 的分层规则（判定顺序 = 渲染顺序的逆序，动画期间一律
//! 吃掉触摸），避免浮层之间/浮层与面板之间互相穿透。

use macroquad::prelude::*;
use phira_mp_client::Client;
use phira_mp_common::{ClientRoomState, RoomResultEntry};
use prpr::{
    core::Smooth,
    ext::{semi_black, semi_white, RectExt, SafeTexture},
    ui::{DRectButton, Scroll, Ui},
};

use super::{
    state::PublicRoom,
    widgets::{
        blocks_touch, button, color_alpha, draw_player_row, draw_player_row_bg, overlay_panel, overlay_title, pill_text, sorted_user_ids, visible,
        OVERLAY_TRANSIT,
    },
};
use crate::mp::L10N_LOCAL;

/// 浮层里的可执行动作。
#[derive(Debug, Clone)]
pub enum OverlayAction {
    /// 以玩家身份加入房间（点公共房间列表行）
    JoinRoom(String),
    /// 以观战者身份进入房间（点公共房间列表的「观战」）
    SpectateRoom(String),
    /// 房主移交给某玩家
    TransferHost(i32),
    /// 房主踢出某玩家
    KickUser(i32),
}

/// [`Overlays::touch`] 的结果。
pub enum OverlayTouch {
    /// 被某个浮层消费，无动作
    Consumed,
    /// 触发了一个动作
    Action(OverlayAction),
    /// 没有浮层在响应，触摸交给下面的部件
    Pass,
}

/// 玩家列表浮层（展示用；房主管理请点房间右侧玩家列）。
#[derive(Default)]
struct UserList {
    p: Smooth<f32>,
    scroll: Scroll,
}

/// 公共房间列表浮层。
#[derive(Default)]
struct RoomList {
    p: Smooth<f32>,
    scroll: Scroll,
    /// 行命中区（渲染时记录，与渲染坐标一致）
    rows: Vec<(String, Rect)>,
    /// 每行「观战」按钮命中区
    spectate_rows: Vec<(String, Rect)>,
}

/// 对局结算排名浮层。
#[derive(Default)]
struct Results {
    p: Smooth<f32>,
    scroll: Scroll,
}

/// 房主对某玩家的操作菜单。
#[derive(Default)]
struct Manage {
    p: Smooth<f32>,
    target: Option<i32>,
    transfer_btn: DRectButton,
    kick_btn: DRectButton,
    cancel_btn: DRectButton,
}

#[derive(Default)]
pub struct Overlays {
    user_list: UserList,
    room_list: RoomList,
    results: Results,
    manage: Manage,
    /// 结算数据（收到 RoomResults 时写入）
    result_entries: Option<Vec<RoomResultEntry>>,
}

impl Overlays {
    pub fn new() -> Self {
        Self::default()
    }

    /// 每帧先失效所有浮层按钮。
    pub fn invalidate(&mut self) {
        self.manage.transfer_btn.invalidate();
        self.manage.kick_btn.invalidate();
        self.manage.cancel_btn.invalidate();
    }

    pub fn update(&mut self, t: f32) {
        if visible(self.user_list.p.now(t)) {
            self.user_list.scroll.update(t);
        }
        if visible(self.room_list.p.now(t)) {
            self.room_list.scroll.update(t);
        }
        if visible(self.results.p.now(t)) {
            self.results.scroll.update(t);
        }
    }

    // ---------- 开关 ----------

    pub fn open_user_list(&mut self, t: f32) {
        self.user_list.scroll.y_scroller.reset();
        self.user_list.p.goto(1., t, OVERLAY_TRANSIT);
    }

    pub fn open_room_list(&mut self, t: f32) {
        self.room_list.scroll.y_scroller.reset();
        self.room_list.p.goto(1., t, OVERLAY_TRANSIT);
    }

    pub fn close_room_list(&mut self, t: f32) {
        self.room_list.p.goto(0., t, OVERLAY_TRANSIT);
    }

    pub fn open_manage(&mut self, id: i32, t: f32) {
        self.manage.target = Some(id);
        self.manage.p.goto(1., t, OVERLAY_TRANSIT);
    }

    pub fn close_manage(&mut self, t: f32) {
        self.manage.target = None;
        self.manage.p.goto(0., t, OVERLAY_TRANSIT);
    }

    pub fn show_results(&mut self, results: Vec<RoomResultEntry>, t: f32) {
        self.result_entries = Some(results);
        self.results.p.goto(1., t, OVERLAY_TRANSIT);
    }

    pub fn close_results(&mut self, t: f32) {
        self.results.p.goto(0., t, OVERLAY_TRANSIT);
    }

    /// 收起全部浮层（离开房间 / 断开连接 / 被踢出时调用）。
    pub fn close_all(&mut self, t: f32) {
        self.user_list.p.goto(0., t, OVERLAY_TRANSIT);
        self.room_list.p.goto(0., t, OVERLAY_TRANSIT);
        self.results.p.goto(0., t, OVERLAY_TRANSIT);
        self.manage.p.goto(0., t, OVERLAY_TRANSIT);
        self.manage.target = None;
    }

    // ---------- 触摸 ----------

    /// 按“最上层优先”的顺序处理触摸。
    ///
    /// 每个浮层「动画中吞触摸」都用 [`blocks_touch`]：只有朝打开方向（`to() > 0.`）
    /// 才拦截，收起过程与 `Smooth::default()` 的开机初始态不吞触摸。
    pub fn touch(&mut self, touch: &Touch, t: f32, room: Option<&ClientRoomState>) -> OverlayTouch {
        let ended = matches!(touch.phase, TouchPhase::Ended | TouchPhase::Cancelled);
        // 1. 房主操作菜单（最上层）
        if blocks_touch(&self.manage.p, t) {
            return OverlayTouch::Consumed;
        }
        if *self.manage.p.to() > 0.5 {
            if room.is_none() {
                self.close_manage(t);
                return OverlayTouch::Consumed;
            }
            if self.manage.target.is_some() {
                if self.manage.transfer_btn.touch(touch, t) {
                    let id = self.manage.target.take().unwrap();
                    self.manage.p.goto(0., t, OVERLAY_TRANSIT);
                    return OverlayTouch::Action(OverlayAction::TransferHost(id));
                }
                if self.manage.kick_btn.touch(touch, t) {
                    let id = self.manage.target.take().unwrap();
                    self.manage.p.goto(0., t, OVERLAY_TRANSIT);
                    return OverlayTouch::Action(OverlayAction::KickUser(id));
                }
                if self.manage.cancel_btn.touch(touch, t) {
                    self.close_manage(t);
                    return OverlayTouch::Consumed;
                }
                if ended {
                    self.close_manage(t);
                }
            } else {
                self.manage.p.goto(0., t, OVERLAY_TRANSIT);
            }
            return OverlayTouch::Consumed;
        }
        // 2. 玩家列表浮层
        if blocks_touch(&self.user_list.p, t) {
            return OverlayTouch::Consumed;
        }
        if *self.user_list.p.to() > 0.5 {
            if self.user_list.scroll.touch(touch, t) {
                return OverlayTouch::Consumed;
            }
            if ended {
                self.user_list.scroll.y_scroller.halt();
                self.user_list.p.goto(0., t, OVERLAY_TRANSIT);
            }
            return OverlayTouch::Consumed;
        }
        // 3. 公共房间列表浮层
        if blocks_touch(&self.room_list.p, t) {
            return OverlayTouch::Consumed;
        }
        if *self.room_list.p.to() > 0.5 {
            if ended {
                // 优先判定「观战」按钮：以 monitor 身份旁观
                let watch = self.room_list.spectate_rows.iter().find(|(_, r)| r.contains(touch.position)).cloned();
                self.close_room_list(t);
                if let Some((room_id, _)) = watch {
                    return OverlayTouch::Action(OverlayAction::SpectateRoom(room_id));
                }
                if let Some((room_id, _)) = self.room_list.rows.iter().find(|(_, r)| r.contains(touch.position)).cloned() {
                    return OverlayTouch::Action(OverlayAction::JoinRoom(room_id));
                }
            }
            return OverlayTouch::Consumed;
        }
        // 4. 结算浮层
        if blocks_touch(&self.results.p, t) {
            return OverlayTouch::Consumed;
        }
        if *self.results.p.to() > 0.5 {
            if self.results.scroll.touch(touch, t) {
                return OverlayTouch::Consumed;
            }
            if ended {
                self.close_results(t);
            }
            return OverlayTouch::Consumed;
        }
        // 5. 观战浮层由观战会话自行处理（见 `Spectate::touch`）
        OverlayTouch::Pass
    }

    // ---------- 渲染 ----------

    /// 玩家列表浮层（点击空白处关闭）。
    pub fn render_user_list(&mut self, ui: &mut Ui, t: f32, client: &Client, room: &ClientRoomState, icon: &SafeTexture) {
        let p = self.user_list.p.now(t);
        if !visible(p) {
            // 已完全收起：直接返回即可。
            // 注意：**不要**在这里再 `goto(0., t, ..)`——`goto` 会把 `start_time`
            // 重置成当前帧时间，而 `Smooth::transiting(t)` 的定义是
            // `(start_time..end_time).contains(&t)`（含起点），于是每一帧渲染都会
            // 重新把 start_time 推到「现在」，`transiting(now)` 便恒为真，
            // `Overlays::touch` 会把所有触摸都判成「动画中」而 Consumed。
            return;
        }
        let accent = ui.accent();
        let me = client.me().map(|it| it.id);
        let ids = sorted_user_ids(room, me);
        let n = ids.len();
        let max_rows = 12usize;
        let row_h = 0.16;
        let panel_w = 0.95;
        let panel_h = (0.32 + (n.min(max_rows) as f32) * (row_h + 0.02) + if n > max_rows { 0.06 } else { 0. })
            .min(ui.top * 2. - 0.1)
            .max(0.4);
        ui.abs_scope(|ui| {
            ui.alpha(p, |ui| {
                let panel = overlay_panel(ui, p, panel_w, panel_h, 0.5);
                ui.text(mtl!("mp-player-count", "n" => n as u64))
                    .pos(panel.x + 0.035, panel.y + 0.025)
                    .size(0.44)
                    .color(semi_white(0.92))
                    .draw();
                ui.text(mtl!("user-list-hint"))
                    .pos(panel.right() - 0.035, panel.y + 0.035)
                    .anchor(1., 0.)
                    .size(0.3)
                    .color(semi_white(0.45))
                    .draw();
                let top = panel.y + 0.13;
                let vh = panel_h - 0.17;
                ui.scope(|ui| {
                    ui.dx(panel.x + 0.03);
                    ui.dy(top);
                    self.user_list.scroll.size((panel_w - 0.06, vh));
                    self.user_list.scroll.render(ui, |ui| {
                        for (i, &id) in ids.iter().enumerate().take(max_rows) {
                            let rr = Rect::new(0., i as f32 * (row_h + 0.02), panel_w - 0.06, row_h);
                            let Some(user) = room.users.get(&id) else { continue };
                            let is_me = user.id == me.unwrap_or(i32::MIN);
                            let crown = is_me && room.is_host;
                            draw_player_row_bg(ui, rr);
                            draw_player_row(ui, rr, t, icon, user.id, &user.name, is_me, crown, user.monitor, false, accent);
                        }
                        (panel_w - 0.06, n.min(max_rows) as f32 * (row_h + 0.02))
                    });
                });
                if n > max_rows {
                    ui.text(mtl!("room-list-more"))
                        .pos(panel.x + 0.035, panel.bottom() - 0.06)
                        .size(0.3)
                        .color(semi_white(0.5))
                        .draw();
                }
            });
        });
    }

    /// 公共房间浮层（点击行加入，右侧「观战」围观，点击空白处关闭）。
    pub fn render_room_list(&mut self, ui: &mut Ui, t: f32, rooms: Option<&[PublicRoom]>, loading: bool, joined_room: Option<String>, spectating: bool) {
        let p = self.room_list.p.now(t);
        if !visible(p) {
            // 见 `render_user_list` 的说明：收起时不要再 `goto(0.)`。
            return;
        }
        ui.abs_scope(|ui| {
            ui.alpha(p, |ui| {
                let panel_w = 0.95;
                let max_rows = 10usize;
                let row_h = 0.15;
                let loaded = rooms.is_some();
                let rooms = rooms.unwrap_or(&[]);
                let n = rooms.len().min(max_rows);
                let panel_h = (0.42 + n as f32 * (row_h + 0.02)).min(ui.top * 2. - 0.1);
                let panel = overlay_panel(ui, p, panel_w, panel_h, 0.5);
                let left = panel.x + 0.035;
                overlay_title(ui, left, panel.y + 0.03, &mtl!("room-list-title"), 0.48);
                self.room_list.rows.clear();
                self.room_list.spectate_rows.clear();
                if !loaded && loading {
                    ui.text(mtl!("room-list-loading"))
                        .pos(left, panel.y + 0.16)
                        .size(0.38)
                        .color(semi_white(0.6))
                        .draw();
                } else if loaded && rooms.is_empty() {
                    ui.text(mtl!("room-list-empty"))
                        .pos(left, panel.y + 0.16)
                        .size(0.38)
                        .color(semi_white(0.6))
                        .draw();
                }
                let mut y = panel.y + 0.13;
                for room in rooms.iter().take(max_rows) {
                    let rr = Rect::new(panel.x + 0.03, y, panel_w - 0.06, row_h);
                    ui.fill_path(&rr.rounded(0.01), semi_black(0.22));
                    let label = format!(
                        "#{}  ·  {}  ·  {}",
                        room.id,
                        room.state,
                        mtl!("mp-room-counts", "players" => room.player_count as u64, "spectators" => room.spectator_count as u64)
                    );
                    // 右侧预留「观战」按钮，文本区域相应收窄
                    let watch_w = 0.2;
                    let watch_r = Rect::new(rr.right() - watch_w - 0.02, rr.y + rr.h * 0.2, watch_w, rr.h * 0.6);
                    ui.text(&label)
                        .pos(rr.x + 0.03, rr.center().y)
                        .anchor(0., 0.5)
                        .size(0.38)
                        .max_width(rr.w - watch_w - (if room.locked { 0.26 } else { 0.1 }))
                        .color(if room.locked { semi_white(0.55) } else { semi_white(0.95) })
                        .draw();
                    if room.locked {
                        let locked_tag = mtl!("mp-room-locked");
                        pill_text(
                            ui,
                            Rect::new(watch_r.x - 0.19, rr.y + rr.h * 0.25, 0.17, rr.h * 0.5),
                            locked_tag.as_ref(),
                            0.3,
                            semi_white(0.08),
                            semi_white(0.6),
                        );
                    }
                    // 「观战」按钮：以 monitor 身份旁观（对局进行中也可围观）
                    let watching_here = spectating && joined_room.as_deref() == Some(room.id.as_str());
                    pill_text(
                        ui,
                        watch_r,
                        mtl!("spectate").as_ref(),
                        0.32,
                        if watching_here { color_alpha(accent_of(ui), 0.75) } else { semi_white(0.12) },
                        semi_white(0.95),
                    );
                    self.room_list.spectate_rows.push((room.id.clone(), watch_r));
                    // 点击整行剩余区域可加入（对局进行中的房间会被服务端拒绝，提示改用观战）
                    self.room_list
                        .rows
                        .push((room.id.clone(), Rect::new(rr.x, rr.y, rr.w - watch_w - 0.03, rr.h)));
                    y += row_h + 0.02;
                }
                if rooms.len() > max_rows {
                    ui.text(mtl!("room-list-more"))
                        .pos(left, y + 0.01)
                        .size(0.3)
                        .color(semi_white(0.5))
                        .draw();
                }
                ui.text(mtl!("room-list-tap-hint"))
                    .pos(panel.center().x, panel.bottom() - 0.035)
                    .anchor(0.5, 0.)
                    .size(0.3)
                    .color(semi_white(0.4))
                    .draw();
            });
        });
    }

    /// 对局结算排名浮层。
    pub fn render_results(&mut self, ui: &mut Ui, t: f32) {
        let p = self.results.p.now(t);
        if !visible(p) {
            // 见 `render_user_list` 的说明：收起时不要再 `goto(0.)`。
            return;
        }
        let results = self.result_entries.clone().unwrap_or_default();
        let n = results.len();
        ui.abs_scope(|ui| {
            ui.alpha(p, |ui| {
                let panel_w = 0.96;
                let row_h = 0.15;
                let panel_h = (0.3 + n as f32 * (row_h + 0.02)).min(ui.top * 2. - 0.1);
                let panel = overlay_panel(ui, p, panel_w, panel_h, 0.55);
                overlay_title(ui, panel.x + 0.04, panel.y + 0.035, &mtl!("results-title"), 0.52);
                let vh = panel_h - 0.16;
                ui.scope(|ui| {
                    ui.dx(panel.x + 0.04);
                    ui.dy(panel.y + 0.13);
                    self.results.scroll.size((panel_w - 0.08, vh));
                    self.results.scroll.render(ui, |ui| {
                        for (i, r) in results.iter().enumerate() {
                            let rr = Rect::new(0., i as f32 * (row_h + 0.02), panel_w - 0.08, row_h);
                            ui.fill_path(&rr.rounded(0.01), semi_black(0.22));
                            let medal = if r.aborted {
                                "✕"
                            } else if i == 0 {
                                "🥇"
                            } else if i == 1 {
                                "🥈"
                            } else if i == 2 {
                                "🥉"
                            } else {
                                ""
                            };
                            let line = if r.aborted {
                                format!("{medal}  {}  —  {}", r.user_name, mtl!("results-aborted"))
                            } else {
                                format!(
                                    "{medal}  {}  ·  {:07}  ·  {:.2}%  {} {}",
                                    r.user_name,
                                    r.score,
                                    r.accuracy * 100.,
                                    if r.full_combo { "FC" } else { "" },
                                    if r.max_combo > 0 {
                                        format!(" · {}combo", r.max_combo)
                                    } else {
                                        String::new()
                                    }
                                )
                            };
                            ui.text(line)
                                .pos(rr.x + 0.03, rr.center().y)
                                .anchor(0., 0.5)
                                .max_width(rr.w - 0.06)
                                .size(0.4)
                                .color(if r.aborted { semi_white(0.5) } else { WHITE })
                                .draw();
                        }
                        (panel_w - 0.08, n as f32 * (row_h + 0.02))
                    });
                });
            });
        });
    }

    /// 房主对某玩家的操作菜单（设为房主 / 移出房间 / 取消）。
    pub fn render_manage(&mut self, ui: &mut Ui, t: f32, client: &Client) {
        let p = self.manage.p.now(t);
        if !visible(p) {
            // 见 `render_user_list` 的说明：收起时不要再 `goto(0.)`。
            return;
        }
        // 目标消失 / 自己不再是房主 → 自动关闭
        let target_name = self.manage.target.and_then(|id| {
            client.blocking_state().and_then(|r| {
                if !r.is_host || !r.users.contains_key(&id) {
                    None
                } else {
                    r.users.get(&id).map(|u| u.name.clone())
                }
            })
        });
        if self.manage.target.is_some() && target_name.is_none() {
            self.manage.p.goto(0., t, OVERLAY_TRANSIT);
            return;
        }
        let accent = ui.accent();
        ui.abs_scope(|ui| {
            ui.alpha(p, |ui| {
                let pw = 0.72;
                let btn_h = 0.13;
                let gap = 0.035;
                let ph = 0.2 + btn_h * 2. + gap + 0.09;
                let panel = overlay_panel(ui, p, pw, ph, 0.5);
                ui.text(mtl!("mp-manage-title", "name" => target_name.as_deref().unwrap_or("")))
                    .pos(panel.x + 0.04, panel.y + 0.035)
                    .size(0.44)
                    .max_width(pw - 0.08)
                    .color(semi_white(0.92))
                    .draw();
                let x = panel.x + 0.05;
                let w = pw - 0.1;
                let mut y = panel.y + 0.16;
                button(ui, &mut self.manage.transfer_btn, t, Rect::new(x, y, w, btn_h), mtl!("mp-manage-transfer"), 0.46, accent, WHITE);
                y += btn_h + gap;
                button(
                    ui,
                    &mut self.manage.kick_btn,
                    t,
                    Rect::new(x, y, w, btn_h),
                    mtl!("mp-manage-kick"),
                    0.46,
                    Color::from_rgba(200, 70, 70, 235),
                    WHITE,
                );
                y += btn_h + gap;
                button(ui, &mut self.manage.cancel_btn, t, Rect::new(x, y, w, 0.09), mtl!("mp-manage-cancel"), 0.4, semi_black(0.3), semi_white(0.85));
            });
        });
    }
}

/// 已进入 `ui.alpha(..)` 作用域时的强调色（避免与外层 `ui.accent()` 借用冲突）。
#[inline]
fn accent_of(ui: &Ui) -> Color {
    ui.accent()
}
