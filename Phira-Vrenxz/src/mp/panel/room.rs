//! 房间内主体：状态卡 + 消息流 + 右侧玩家列 + 底部操作条。
//!
//! 底部操作条是“渲染与触摸同一来源”的核心：按钮集合由
//! [`action_items`] 从房间状态推导，渲染按它的顺序用 [`flow_rects`] 排布并
//! 逐个登记命中区，触摸侧对同一份 [`action_items`] 结果查按钮，
//! 因此不存在“看得到点不到”，也不需要在触摸里重写一遍状态判断。

use macroquad::prelude::*;
use phira_mp_common::{ClientRoomState, RoomState};
use prpr::{
    ext::{semi_black, semi_white, RectExt, SafeTexture},
    ui::{DRectButton, Scroll, Ui},
};

use super::{
    messages::MessageLog,
    widgets::{button, color_alpha, draw_player_row, flow_rects, sorted_user_ids, PANEL_WIDTH},
};
use crate::{
    client::UserManager,
    dir,
    mp::L10N_LOCAL,
};

/// 是否编译了聊天功能。
pub const CHAT_ENABLED: bool = cfg!(feature = "chat");

/// 底部操作条按钮的种类。
///
/// 取消类操作按语义拆开（`CancelLocalShare` / `CancelDownload` / `CancelReady`），
/// 旧实现把它们全都映射到同一个 `Cancel`，再在触摸侧按房间状态二次判断，
/// 是“看得到点不到”的隐患来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomAction {
    /// 房主：开始游戏
    Start,
    /// 房主：锁定/解锁房间
    LockRoom,
    /// 房主：切换普通/循环模式
    CycleRoom,
    /// 房主：设置/清除房间密码
    Password,
    /// 玩家：准备
    Ready,
    /// 玩家：取消准备
    CancelReady,
    /// 房主：取消本地谱面分享
    CancelLocalShare,
    /// 玩家：取消本地谱面下载/就绪
    CancelDownload,
    /// 预览当前谱面（autoplay）
    Preview,
    /// 打开观战浮层
    Spectate,
}

impl RoomAction {
    pub const ALL: [RoomAction; 10] = [
        RoomAction::Start,
        RoomAction::LockRoom,
        RoomAction::CycleRoom,
        RoomAction::Password,
        RoomAction::Ready,
        RoomAction::CancelReady,
        RoomAction::CancelLocalShare,
        RoomAction::CancelDownload,
        RoomAction::Preview,
        RoomAction::Spectate,
    ];

    fn index(self) -> usize {
        match self {
            RoomAction::Start => 0,
            RoomAction::LockRoom => 1,
            RoomAction::CycleRoom => 2,
            RoomAction::Password => 3,
            RoomAction::Ready => 4,
            RoomAction::CancelReady => 5,
            RoomAction::CancelLocalShare => 6,
            RoomAction::CancelDownload => 7,
            RoomAction::Preview => 8,
            RoomAction::Spectate => 9,
        }
    }
}

/// 操作条上的一项：动作 + 本地化后的标签。
pub struct ActItem {
    pub action: RoomAction,
    pub label: String,
}

/// 房间主体渲染需要的、来自面板的只读展示状态。
pub struct RoomView<'a> {
    /// 自己是否以观战者身份在房间里
    pub spectating: bool,
    /// 当前分享中的本地谱面 (uuid, 谱面名)
    pub local_chart: Option<(&'a str, &'a str)>,
    /// 自己是否已就绪（本地谱面同步流程）
    pub local_ready: bool,
    /// 房主是否已开始分享（本地谱面同步流程）
    pub host_started: bool,
    /// 服务端已指示下载、但玩家还没点“准备”
    pub pending_download: bool,
    /// 正在同步谱面（下载中）
    pub syncing: bool,
    /// 房间当前选中的在线谱面
    pub chart_id: Option<i32>,
}

/// 房主在当前房间状态下可否管理玩家。
pub fn manage_allowed(room: &ClientRoomState) -> bool {
    room.is_host && matches!(room.state, RoomState::SelectChart(_) | RoomState::LocalChart)
}

/// 当前是否有可预览的谱面。
pub fn is_previewable(room: &ClientRoomState, view: &RoomView) -> bool {
    use std::path::Path;
    let local_uuid_ready = match (&room.state, view.local_chart) {
        (RoomState::LocalChart, Some((uuid, _))) => {
            Path::new(&format!("{}/download/{uuid}/info.yml", dir::charts().unwrap_or_default())).exists()
        }
        _ => false,
    };
    match (&room.state, view.local_chart) {
        (RoomState::SelectChart(Some(_)), _) => true,
        (RoomState::LocalChart, Some(_)) => local_uuid_ready,
        (RoomState::WaitingForReady, _) => view.chart_id.is_some(),
        _ => false,
    }
}

/// 依据房间状态推导底部操作条按钮（渲染与触摸共用同一集合）。
pub fn action_items(room: &ClientRoomState, view: &RoomView) -> Vec<ActItem> {
    let mut items = Vec::new();
    // 观战者只读：不显示开始/就绪等操作，仅保留观战入口
    if view.spectating {
        items.push(ActItem {
            action: RoomAction::Spectate,
            label: mtl!("spectate-title").into_owned(),
        });
        return items;
    }
    let is_host = room.is_host;
    match room.state {
        RoomState::SelectChart(_) => {
            if is_host {
                items.push(ActItem {
                    action: RoomAction::Start,
                    label: mtl!("request-start").into_owned(),
                });
                push_room_settings(&mut items, room);
            }
        }
        RoomState::LocalChart => {
            if is_host {
                if view.host_started {
                    items.push(ActItem {
                        action: RoomAction::CancelLocalShare,
                        label: mtl!("cancel-ready").into_owned(),
                    });
                } else {
                    items.push(ActItem {
                        action: RoomAction::Start,
                        label: mtl!("request-start").into_owned(),
                    });
                }
                push_room_settings(&mut items, room);
            } else if view.local_ready {
                items.push(ActItem {
                    action: RoomAction::CancelDownload,
                    label: mtl!("cancel-ready").into_owned(),
                });
            } else if view.pending_download && !view.syncing {
                items.push(ActItem {
                    action: RoomAction::Ready,
                    label: mtl!("ready").into_owned(),
                });
            }
        }
        RoomState::WaitingForReady => {
            if room.is_ready {
                items.push(ActItem {
                    action: RoomAction::CancelReady,
                    label: mtl!("cancel-ready").into_owned(),
                });
            } else {
                items.push(ActItem {
                    action: RoomAction::Ready,
                    label: mtl!("ready").into_owned(),
                });
            }
        }
        RoomState::Playing => {}
    }
    if is_previewable(room, view) {
        items.push(ActItem {
            action: RoomAction::Preview,
            label: mtl!("preview").into_owned(),
        });
    }
    // 观战入口：房内任何人都能打开观战浮层（观战者在此查看实时进度 / 退出观战）
    items.push(ActItem {
        action: RoomAction::Spectate,
        label: if view.spectating {
            mtl!("spectate-title").into_owned()
        } else {
            mtl!("spectate").into_owned()
        },
    });
    items
}

fn push_room_settings(items: &mut Vec<ActItem>, room: &ClientRoomState) {
    items.push(ActItem {
        action: RoomAction::LockRoom,
        label: mtl!("lock-room", "current" => room.locked.to_string()),
    });
    items.push(ActItem {
        action: RoomAction::CycleRoom,
        label: mtl!("cycle-room", "current" => room.cycle.to_string()),
    });
    items.push(ActItem {
        action: RoomAction::Password,
        label: mtl!("set-password").into_owned(),
    });
}

/// 按动作索引的按钮池：渲染写入命中区，触摸按同一动作查。
pub struct ActionButtons {
    btns: [DRectButton; RoomAction::ALL.len()],
}

impl Default for ActionButtons {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionButtons {
    pub fn new() -> Self {
        Self {
            btns: std::array::from_fn(|_| DRectButton::new()),
        }
    }

    pub fn get(&mut self, action: RoomAction) -> &mut DRectButton {
        &mut self.btns[action.index()]
    }

    pub fn invalidate(&mut self) {
        for btn in &mut self.btns {
            btn.invalidate();
        }
    }
}

/// 房间内的可变 UI 部件。
pub struct RoomUi {
    leave_btn: DRectButton,
    user_list_btn: DRectButton,
    actions: ActionButtons,
    player_scroll: Scroll,
    /// 玩家列的行按钮，索引与 [`sorted_user_ids`] 顺序一致
    player_rows: Vec<DRectButton>,
    /// 已请求过头像的用户 id（避免每帧重复请求）
    avatar_req: Vec<i32>,
    chat_btn: DRectButton,
    chat_send_btn: DRectButton,
}

/// `RoomUi::touch` 的返回：本次触摸被什么消费了。
pub enum RoomTouch {
    /// 玩家列的行按钮（索引对应 [`sorted_user_ids`] 的第 i 项）
    PlayerRow(usize),
    /// 底部操作条的按钮
    Action(RoomAction),
    /// 需要弹出聊天输入框
    ChatInput,
    /// 发送聊天
    ChatSend,
    /// 离开房间
    Leave,
    /// 打开玩家列表浮层
    OpenUserList,
    /// 被滚动区/其他部件吃掉，无动作
    Consumed,
}

impl Default for RoomUi {
    fn default() -> Self {
        Self::new()
    }
}

impl RoomUi {
    pub fn new() -> Self {
        Self {
            leave_btn: DRectButton::new(),
            user_list_btn: DRectButton::new(),
            actions: ActionButtons::new(),
            player_scroll: Scroll::new(),
            player_rows: Vec::new(),
            avatar_req: Vec::new(),
            chat_btn: DRectButton::new().with_delta(-0.002),
            chat_send_btn: DRectButton::new(),
        }
    }

    pub fn invalidate(&mut self) {
        self.leave_btn.invalidate();
        self.user_list_btn.invalidate();
        self.actions.invalidate();
        self.chat_btn.invalidate();
        self.chat_send_btn.invalidate();
    }

    pub fn update(&mut self, t: f32, in_room: bool) {
        if self.player_scroll.matrix().is_some() && in_room {
            self.player_scroll.update(t);
        }
    }

    pub fn reset_player_cache(&mut self) {
        self.player_rows.clear();
    }

    /// 请求新出现的用户头像（避免每帧重复请求）。
    pub fn request_avatars(&mut self, ids: &[i32]) {
        for &id in ids {
            if !self.avatar_req.contains(&id) {
                self.avatar_req.push(id);
                UserManager::request(id);
            }
        }
    }

    /// 标题栏右上角的「离开房间」按钮（房内）。
    pub fn render_leave_button(&mut self, ui: &mut Ui, t: f32) {
        let er = Rect::new(PANEL_WIDTH - 0.05 - 0.17, 0.04, 0.17, 0.1);
        button(ui, &mut self.leave_btn, t, er, mtl!("leave-room"), 0.4, super::widgets::danger(), WHITE);
    }

    /// 房间主体：状态卡 + 消息流 + 玩家列 + 操作条。
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        ui: &mut Ui,
        t: f32,
        room: &ClientRoomState,
        me: Option<i32>,
        view: &RoomView,
        msgs: &mut MessageLog,
        chat_text: &str,
        icon: &SafeTexture,
    ) {
        let accent = ui.accent();
        let pw = PANEL_WIDTH;
        let pb = ui.top * 2.;
        let pad = 0.05;

        // 底部操作条内容（先测量布局，再据此预留空间）
        let items = action_items(room, view);
        let labels: Vec<String> = items.iter().map(|it| it.label.clone()).collect();
        let row_h = 0.105;
        let bar_gap = 0.028;
        let col_gap = 0.04;
        let bar_x = pad;
        let bar_w = pw - pad * 2.;
        let (bar_h, bar_rects) = flow_rects(ui, &labels, bar_x, 0., bar_w, row_h, bar_gap, 0.03);
        let bar_top = pb - pad - bar_h;
        let col_top = 0.17;
        let col_bottom = if bar_h > 0. { bar_top - 0.045 } else { pb - pad };

        // 两栏几何
        let avail_w = pw - pad * 2. - col_gap;
        let lw = avail_w * 0.62;
        let rw = avail_w - lw;
        let lx = pad;
        let rx = lx + lw + col_gap;
        let col_h = (col_bottom - col_top).max(0.05);

        // 左栏背景
        let lrect = Rect::new(lx, col_top, lw, col_h);
        ui.fill_path(&lrect.rounded(0.02), semi_black(0.18));

        // 状态卡
        let m = 0.03;
        let st_h = 0.2;
        let st = Rect::new(lx + m, col_top + m, lw - m * 2., st_h);
        ui.fill_path(&st.rounded(0.014), semi_black(0.24));
        // 状态卡左侧竖向高光条
        ui.fill_rect(Rect::new(st.x, st.y, 0.012, st.h), color_alpha(accent, 0.55));

        // 状态标题
        let (title, sub): (String, Option<String>) = match room.state {
            RoomState::SelectChart(None) => (mtl!("mp-state-choose").into_owned(), None),
            RoomState::SelectChart(Some(id)) => (mtl!("mp-state-chosen", "id" => id as u64), None),
            RoomState::LocalChart => (
                mtl!("mp-state-local").into_owned(),
                view.local_chart.map(|(_, name)| name.to_owned()),
            ),
            RoomState::WaitingForReady => (mtl!("mp-state-wait").into_owned(), None),
            RoomState::Playing => (mtl!("mp-state-playing").into_owned(), None),
        };
        ui.text(&title)
            .pos(st.x + 0.03, st.center().y - 0.045)
            .anchor(0., 0.5)
            .no_baseline()
            .max_width(st.w - 0.08)
            .size(0.46)
            .color(semi_white(0.95))
            .draw();
        // 第二行：谱面名 / 房间状态（锁定、循环、人数）
        let mut parts: Vec<String> = Vec::new();
        if let Some(name) = sub.filter(|s| !s.is_empty()) {
            parts.push(name);
        }
        if room.locked {
            parts.push(mtl!("mp-locked-tag").into_owned());
        }
        if room.cycle {
            parts.push(mtl!("mp-cycle-tag").into_owned());
        }
        parts.push(mtl!("mp-n-players", "n" => room.users.len() as u64));
        let sub_text = parts.join("  ·  ");
        ui.text(&sub_text)
            .pos(st.x + 0.03, st.center().y + 0.05)
            .anchor(0., 0.5)
            .no_baseline()
            .max_width(st.w - 0.08)
            .size(0.32)
            .color(semi_white(0.5))
            .draw();

        // 消息标题行 + 消息滚动区
        let caption_h = 0.09;
        let chat_h = if CHAT_ENABLED { 0.13 } else { 0. };
        let msg_top = st.bottom() + 0.005;
        let msg_area = Rect::new(lx + m, msg_top, lw - m * 2., (col_bottom - m - chat_h) - msg_top);
        if msg_area.h > 0.03 {
            ui.text(mtl!("mp-chat-caption"))
                .pos(msg_area.x, msg_area.y)
                .anchor(0., 0.)
                .size(0.32)
                .color(semi_white(0.45))
                .draw();
            let list = Rect::new(msg_area.x, msg_area.y + caption_h, msg_area.w, msg_area.h - caption_h);
            if list.h > 0.03 {
                msgs.render(ui, list);
            }
        }
        // 聊天输入行
        if CHAT_ENABLED {
            let y = col_bottom - m - 0.1;
            let br = Rect::new(lx + m, y, lw - m * 2. - 0.15, 0.1);
            ui.fill_path(&br.rounded(0.006), semi_black(0.15));
            self.chat_btn.render_input(ui, br.feather(-0.005), t, chat_text, mtl!("chat-placeholder"), 0.5);
            let sbr = Rect::new(br.right() + 0.01, y, 0.14, 0.1);
            button(ui, &mut self.chat_send_btn, t, sbr, mtl!("chat-send"), 0.4, color_alpha(accent, 0.8), WHITE);
        }

        // 右栏：玩家列表
        let rrect = Rect::new(rx, col_top, rw, col_h);
        ui.fill_path(&rrect.rounded(0.02), semi_black(0.18));
        let ids = sorted_user_ids(room, me);
        let user_count = ids.len();
        self.request_avatars(&ids);

        // 栏头：玩家按钮（点击打开全屏玩家浮层）
        let hdr_h = 0.09;
        let hdr = Rect::new(rx + m, col_top + m, rw - m * 2., hdr_h);
        button(
            ui,
            &mut self.user_list_btn,
            t,
            hdr,
            mtl!("mp-player-count", "n" => user_count as u64),
            0.42,
            semi_black(0.25),
            semi_white(0.92),
        );

        // 行滚动区
        let rows_top = col_top + m + hdr_h + 0.035;
        let rows_h = (col_bottom - m - rows_top).max(0.05);
        let rows_w = rw - m * 2.;
        let row_step = 0.13;
        let view_h = user_count as f32 * row_step;
        let my_state_ready = room.is_ready;
        let (local_ready, host_started) = (view.local_ready, view.host_started);
        let pool = &mut self.player_rows;
        pool.resize_with(ids.len(), DRectButton::new);
        ui.scope(|ui| {
            ui.dx(rx + m);
            ui.dy(rows_top);
            self.player_scroll.size((rows_w, rows_h));
            self.player_scroll.render(ui, |ui| {
                for (i, &id) in ids.iter().enumerate() {
                    let rr = Rect::new(0., i as f32 * row_step, rows_w, row_step - 0.015);
                    let Some(user) = room.users.get(&id) else { continue };
                    let is_me = user.id == me.unwrap_or(i32::MIN);
                    let row_bg = if is_me { color_alpha(accent, 0.12) } else { semi_black(0.17) };
                    pool[i].build(ui, t, rr, |ui, path| {
                        ui.fill_path(&path, row_bg);
                    });
                    // 就绪徽标仅对自己可观测（协议未提供他人就绪状态）
                    let me_ready = is_me
                        && match room.state {
                            RoomState::WaitingForReady => my_state_ready,
                            RoomState::LocalChart => {
                                if room.is_host {
                                    host_started
                                } else {
                                    local_ready
                                }
                            }
                            _ => false,
                        };
                    let crown = is_me && room.is_host;
                    draw_player_row(ui, rr, t, icon, user.id, &user.name, is_me, crown, user.monitor, me_ready, accent);
                }
                (rows_w, view_h)
            });
        });

        // 底部操作条
        for (i, item) in items.iter().enumerate() {
            let br0 = bar_rects[i];
            let r = Rect::new(br0.x, bar_top + br0.y, br0.w, row_h);
            let (fill, fg) = if item.action == RoomAction::Spectate {
                if view.spectating {
                    (accent, WHITE)
                } else {
                    (semi_black(0.34), semi_white(0.92))
                }
            } else if matches!(item.action, RoomAction::Start | RoomAction::Ready) {
                (accent, WHITE)
            } else {
                (semi_black(0.34), semi_white(0.92))
            };
            button(ui, self.actions.get(item.action), t, r, item.label.as_str(), 0.42, fill, fg);
        }
    }

    /// 房间内触摸。`manage_allowed` 为真时玩家行变成“打开房主管理菜单”的按钮。
    pub fn touch(&mut self, touch: &Touch, t: f32, room: &ClientRoomState, view: &RoomView, msgs: &mut MessageLog) -> Option<RoomTouch> {
        // 消息滚动区
        if msgs.touch(touch, t) {
            return Some(RoomTouch::Consumed);
        }
        // 玩家列滚动
        if self.player_scroll.contains(touch) && self.player_scroll.touch(touch, t) {
            return Some(RoomTouch::Consumed);
        }
        // 聊天输入 / 发送
        if CHAT_ENABLED {
            if self.chat_btn.touch(touch, t) {
                return Some(RoomTouch::ChatInput);
            }
            if self.chat_send_btn.touch(touch, t) {
                return Some(RoomTouch::ChatSend);
            }
        }
        // 右上角：离开房间
        if self.leave_btn.touch(touch, t) {
            return Some(RoomTouch::Leave);
        }
        // 玩家列表浮层开关（栏头）
        if self.user_list_btn.touch(touch, t) {
            return Some(RoomTouch::OpenUserList);
        }
        // 房主点击玩家行 → 管理菜单（取代输入玩家 ID）
        if manage_allowed(room) {
            for (i, btn) in self.player_rows.iter_mut().enumerate() {
                if btn.touch(touch, t) {
                    return Some(RoomTouch::PlayerRow(i));
                }
            }
        }
        // 底部操作条：只按 action_items 给出的集合查按钮
        for item in action_items(room, view) {
            if self.actions.get(item.action).touch(touch, t) {
                return Some(RoomTouch::Action(item.action));
            }
        }
        None
    }
}
