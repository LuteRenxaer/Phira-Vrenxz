//! 房间页（进房后的根页面）：状态卡 + 谱面下载/同步状态行 + 玩家入口 + 聊天流 +
//! 底部操作条。
//!
//! 底部操作条是“渲染与触摸同一来源”的核心：按钮集合由 [`action_items`] 从房间状态
//! 推导，渲染按它的顺序用 [`theme::flow_rects`] 排布并逐个登记命中区，触摸侧对同一份
//! [`action_items`] 结果查按钮，因此不存在“看得到点不到”，也不需要在触摸里重写一遍
//! 状态判断。
//!
//! 谱面下载进度、谱面同步进度、「房主要开始游戏啦」确认都不再是居中浮层：
//! 前者是房间页里的一行状态卡，后者是本页的内联确认条。

use macroquad::prelude::*;
use phira_mp_common::{ClientRoomState, RoomState};
use prpr::{
    ext::RectExt,
    ui::{DRectButton, Ui},
};

use super::super::{
    messages::MessageLog,
    theme::{self, *},
};
use crate::{dir, mp::L10N_LOCAL, scene::Downloading};

/// 是否编译了聊天功能。
pub const CHAT_ENABLED: bool = cfg!(feature = "chat");

/// 底部操作条按钮的种类。
///
/// 取消类操作按语义拆开（`CancelLocalShare` / `CancelDownload` / `CancelReady`），
/// 这样触摸侧不需要按房间状态二次判断，避免“看得到点不到”。
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
    /// 打开观战页
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

/// 会话传进来的、只读的房间展示状态（全部为拥有所有权的数据，避免借用纠缠）。
#[derive(Default, Clone)]
pub struct RoomView {
    /// 自己是否以观战者身份在房间里
    pub spectating: bool,
    /// 当前分享中的本地谱面 (uuid, 谱面名)
    pub local_chart: Option<(String, String)>,
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
    let local_uuid_ready = match (&room.state, &view.local_chart) {
        (RoomState::LocalChart, Some((uuid, _))) => {
            Path::new(&format!("{}/download/{uuid}/info.yml", dir::charts().unwrap_or_default())).exists()
        }
        _ => false,
    };
    match (&room.state, &view.local_chart) {
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
    // 观战入口：房内任何人都能打开观战页（观战者在此查看实时进度 / 退出观战）
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

/// 房间页可执行的动作。
pub enum Action {
    Back,
    Leave,
    OpenPlayers,
    Room(RoomAction),
    ChatInput,
    ChatSend,
    /// 内联确认条：true = 准备，false = 暂不
    Prompt(bool),
}

/// 房间页渲染所需的上下文。
pub struct Render<'a> {
    pub room: &'a ClientRoomState,
    /// 房间号（页头标题）
    pub room_id: Option<&'a str>,
    pub view: &'a RoomView,
    pub messages: &'a mut MessageLog,
    pub chat_text: &'a str,
    /// 在线谱面下载中（内联进度行）
    pub download: Option<&'a mut Downloading>,
    /// 本地谱面同步中
    pub syncing: bool,
    /// 会话级任务在跑
    pub busy: bool,
    /// 是否显示「准备 / 暂不」内联确认条
    pub prompt: bool,
}

#[derive(Default)]
pub struct RoomPage {
    back: DRectButton,
    leave: DRectButton,
    players: DRectButton,
    actions: ActionButtons,
    chat_btn: DRectButton,
    chat_send_btn: DRectButton,
    prompt_ready: DRectButton,
    prompt_later: DRectButton,
}

/// 信息块各部分的高度（会按可用高度整体缩放，保证横屏也不溢出）。
struct InfoMetrics {
    card: f32,
    status: f32,
    players: f32,
    prompt: f32,
    gap: f32,
}

impl InfoMetrics {
    fn new(scale: f32, ctx: &Render) -> Self {
        Self {
            card: 0.2 * scale,
            status: if ctx.download.is_some() {
                0.19 * scale
            } else if ctx.syncing {
                0.13 * scale
            } else if ctx.busy {
                0.02 * scale
            } else {
                0.
            },
            players: ROW_TALL * scale,
            prompt: if ctx.prompt { 0.22 * scale } else { 0. },
            gap: SECTION_GAP * scale,
        }
    }

    fn total(&self) -> f32 {
        let mut h = self.card + self.players;
        let mut gaps = 1; // 状态卡与玩家入口之间至少有一个间距
        if self.status > 0. {
            h += self.status;
            gaps += 1;
        }
        if self.prompt > 0. {
            h += self.prompt;
            gaps += 1;
        }
        h + self.gap * gaps as f32
    }
}

impl RoomPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.leave.invalidate();
        self.players.invalidate();
        self.actions.invalidate();
        self.chat_btn.invalidate();
        self.chat_send_btn.invalidate();
        self.prompt_ready.invalidate();
        self.prompt_later.invalidate();
    }

    /// 房间页没有需要逐帧推进的滚动区（消息流由会话统一更新）。
    pub fn update(&mut self, _t: f32) {}

    pub fn render(&mut self, ui: &mut Ui, t: f32, mut ctx: Render) {
        let accent = ui.accent();
        let room = ctx.room;

        // 先量出操作条高度（按钮集合来自 action_items，渲染与触摸同源）
        let items = action_items(room, ctx.view);
        let labels: Vec<String> = items.iter().map(|it| it.label.clone()).collect();
        let avail = 2. - theme::page_pad(ui) * 2.;
        let (bar_h, bar_rects) = theme::flow_rects(ui, &labels, avail, BAR_BTN_H, BAR_ROW_GAP, BAR_COL_GAP);
        let f = theme::frame(ui, bar_h);

        // —— 页头：返回 + 房间标题 + 离开房间 ——
        let title = match ctx.room_id {
            Some(id) => mtl!("mp-room-tag", "id" => id.to_owned()),
            None => mtl!("multiplayer").into_owned(),
        };
        let sub = mtl!("mp-n-players", "n" => room.users.len() as u64);
        theme::header(
            ui,
            f.header,
            &mut self.back,
            &mut self.leave,
            t,
            &title,
            Some(&sub),
            Some((&mtl!("leave-room"), danger(), WHITE)),
        );

        // —— 内容区：横屏双栏（左信息 / 右聊天），竖屏上下排 ——
        if f.wide {
            let gap = 0.035;
            let left_w = f.body.w * 0.54;
            let left = Rect::new(f.body.x, f.body.y, left_w, f.body.h);
            let right = Rect::new(f.body.x + left_w + gap, f.body.y, f.body.w - left_w - gap, f.body.h);
            // 信息块整体缩放以适配横屏较矮的内容区（0.5 下限保证极端窄屏也不溢出到操作条）
            let need = InfoMetrics::new(1., &ctx).total();
            let scale = (left.h / need.max(1e-3)).clamp(0.5, 1.);
            let m = InfoMetrics::new(scale, &ctx);
            self.render_info(ui, t, left, &mut ctx, accent, m);
            self.render_chat(ui, t, right, &mut ctx, accent);
        } else {
            // 竖屏：信息块高度按内容决定，但最多占内容区的 62%（其余留给聊天流）；
            // 若被压缩则整体缩放，避免信息块溢出到聊天区。
            let need = InfoMetrics::new(1., &ctx).total();
            let info_h = need.min(f.body.h * 0.62);
            let scale = (info_h / need.max(1e-3)).clamp(0.5, 1.);
            let m = InfoMetrics::new(scale, &ctx);
            let info = Rect::new(f.body.x, f.body.y, f.body.w, info_h);
            let chat = Rect::new(
                f.body.x,
                info.bottom() + SECTION_GAP,
                f.body.w,
                (f.body.bottom() - info.bottom() - SECTION_GAP).max(0.12),
            );
            self.render_info(ui, t, info, &mut ctx, accent, m);
            self.render_chat(ui, t, chat, &mut ctx, accent);
        }

        // —— 底部操作条（渲染与触摸同源）——
        for (i, item) in items.iter().enumerate() {
            let rel = bar_rects[i];
            let r = Rect::new(f.bar.x + rel.x, f.bar.y + rel.y, rel.w, rel.h);
            let (fill, fg) = if item.action == RoomAction::Spectate {
                if ctx.view.spectating {
                    (primary(accent), WHITE)
                } else {
                    (secondary(), text())
                }
            } else if matches!(item.action, RoomAction::Start | RoomAction::Ready) {
                (primary(accent), WHITE)
            } else {
                (secondary(), text())
            };
            theme::button(ui, self.actions.get(item.action), t, r, item.label.clone(), FS_BUTTON, fill, fg);
        }
    }

    /// 信息块：状态卡 / 下载或同步状态行 / 玩家入口 / 内联确认条。
    fn render_info(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color, m: InfoMetrics) {
        let room = ctx.room;
        let mut y = r.y;

        // —— 状态卡 ——
        let st = Rect::new(r.x, y, r.w, m.card);
        theme::card_accented(ui, st, card(), accent);
        let (title, sub): (String, Option<String>) = match room.state {
            RoomState::SelectChart(None) => (mtl!("mp-state-choose").into_owned(), None),
            RoomState::SelectChart(Some(id)) => (mtl!("mp-state-chosen", "id" => id as u64), None),
            RoomState::LocalChart => (
                mtl!("mp-state-local").into_owned(),
                ctx.view.local_chart.as_ref().map(|(_, name)| name.clone()),
            ),
            RoomState::WaitingForReady => (mtl!("mp-state-wait").into_owned(), None),
            RoomState::Playing => (mtl!("mp-state-playing").into_owned(), None),
        };
        theme::text_left(
            ui,
            st.x + CARD_PAD + 0.012,
            st.y + m.card * 0.32,
            FS_SECTION,
            text(),
            &title,
            st.w - CARD_PAD * 2.,
        );
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
        theme::text_left(
            ui,
            st.x + CARD_PAD + 0.012,
            st.y + m.card * 0.7,
            FS_SMALL,
            text_muted(),
            &parts.join("  ·  "),
            st.w - CARD_PAD * 2.,
        );
        y = st.bottom() + m.gap;

        // —— 谱面下载 / 同步状态行（原来的居中浮层改成页内状态行）——
        if let Some(dl) = ctx.download.as_deref_mut() {
            let dr = Rect::new(r.x, y, r.w, m.status);
            theme::card_rect(ui, dr, card_soft());
            dl.render_inline(ui, dr, t);
            y = dr.bottom() + m.gap;
        } else if ctx.syncing {
            let dr = Rect::new(r.x, y, r.w, m.status);
            theme::card_rect(ui, dr, card_soft());
            theme::text_left(
                ui,
                dr.x + CARD_PAD,
                dr.y + dr.h * 0.35,
                FS_SMALL,
                text_dim(),
                &mtl!("mp-syncing-chart"),
                dr.w - CARD_PAD * 2.,
            );
            theme::progress_bar(
                ui,
                Rect::new(dr.x + CARD_PAD, dr.bottom() - dr.h * 0.3, dr.w - CARD_PAD * 2., 0.012),
                None,
                t,
                accent,
            );
            y = dr.bottom() + m.gap;
        } else if ctx.busy {
            theme::progress_bar(ui, Rect::new(r.x, y, r.w, 0.012), None, t, accent);
            y += m.status + m.gap;
        }

        // —— 玩家入口（整屏玩家列表页）——
        let pr = Rect::new(r.x, y, r.w, m.players);
        let view = ctx.view;
        theme::row_button(ui, &mut self.players, t, pr, false, accent, |ui, rr| {
            let label = mtl!("mp-player-count", "n" => room.users.len() as u64);
            let right = theme::text_chevron(ui, rr.right() - CARD_PAD, rr.center().y);
            let right = if view.spectating {
                let s = mtl!("mp-watching");
                theme::tag_right(ui, right, rr.x + rr.w * 0.4, rr.center().y, &s, tag_bg(), text_dim())
            } else {
                right
            };
            theme::text_left(ui, rr.x + CARD_PAD, rr.center().y, FS_SECTION, text(), &label, right - rr.x - CARD_PAD);
        });
        y = pr.bottom() + m.gap;

        // —— 内联「准备 / 暂不」确认条（预览被房主开始时出现）——
        if ctx.prompt {
            let cr = Rect::new(r.x, y, r.w, m.prompt);
            theme::card_rect(ui, cr, tag_accent(accent));
            theme::text_left(
                ui,
                cr.x + CARD_PAD,
                cr.y + cr.h * 0.26,
                FS_SMALL,
                text(),
                &mtl!("preview-interrupted-content"),
                cr.w - CARD_PAD * 2.,
            );
            let bw = 0.26f32.min((cr.w - CARD_PAD * 2. - 0.03) / 2.);
            let by = cr.bottom() - 0.035 - 0.09;
            let rr = Rect::new(cr.right() - CARD_PAD - bw, by, bw, 0.09);
            let lr = Rect::new(rr.x - 0.03 - bw, by, bw, 0.09);
            theme::button(ui, &mut self.prompt_later, t, lr, mtl!("preview-not-now"), FS_SMALL, secondary(), text());
            theme::button(ui, &mut self.prompt_ready, t, rr, mtl!("preview-ready"), FS_SMALL, primary(accent), WHITE);
        }
    }

    /// 聊天流（`feature = "chat"` 时带输入行）。
    fn render_chat(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        let input_h = if CHAT_ENABLED { 0.115 } else { 0. };
        theme::section_label(ui, r.x, r.y, &mtl!("mp-chat-caption"));
        let list = Rect::new(
            r.x,
            r.y + 0.055,
            r.w,
            (r.h - 0.055 - if CHAT_ENABLED { input_h + 0.02 } else { 0. }).max(0.06),
        );
        theme::card_rect(ui, list, card_soft());
        ui.scope(|ui| {
            ui.dx(list.x + 0.015);
            ui.dy(list.y + 0.012);
            let inner = Rect::new(0., 0., list.w - 0.03, list.h - 0.024);
            ctx.messages.render(ui, inner);
        });
        if CHAT_ENABLED {
            let iy = r.bottom() - input_h;
            let br = Rect::new(r.x, iy, (r.w - 0.17).max(0.2), input_h);
            ui.fill_path(&br.rounded(R_BTN), card_soft());
            self.chat_btn
                .render_input(ui, br.feather(-0.008), t, ctx.chat_text, mtl!("chat-placeholder"), FS_BODY);
            let sb = Rect::new(br.right() + 0.015, iy, 0.155, input_h);
            theme::button(ui, &mut self.chat_send_btn, t, sb, mtl!("chat-send"), FS_BUTTON, primary(accent), WHITE);
        }
    }

    pub fn touch(&mut self, touch: &Touch, t: f32, room: &ClientRoomState, view: &RoomView, msgs: &mut MessageLog) -> Option<Action> {
        // 消息滚动区
        if msgs.touch(touch, t) {
            return None;
        }
        if self.back.touch(touch, t) {
            return Some(Action::Back);
        }
        if self.leave.touch(touch, t) {
            return Some(Action::Leave);
        }
        if self.players.touch(touch, t) {
            return Some(Action::OpenPlayers);
        }
        if CHAT_ENABLED {
            if self.chat_btn.touch(touch, t) {
                return Some(Action::ChatInput);
            }
            if self.chat_send_btn.touch(touch, t) {
                return Some(Action::ChatSend);
            }
        }
        if self.prompt_ready.touch(touch, t) {
            return Some(Action::Prompt(true));
        }
        if self.prompt_later.touch(touch, t) {
            return Some(Action::Prompt(false));
        }
        // 底部操作条：只按 action_items 给出的集合查按钮
        for item in action_items(room, view) {
            if self.actions.get(item.action).touch(touch, t) {
                return Some(Action::Room(item.action));
            }
        }
        None
    }
}
