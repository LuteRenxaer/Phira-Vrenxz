//! 房间页（进房后的根页面）。
//!
//! 版面按"游戏内房间"来排，而不是把一堆卡片竖着堆：
//!
//! ```text
//! ‹                                                  离开房间   ← 细页头：返回 + 无底色退出
//! 房间 #31205                                                      ← 左上角：房名
//! 已选谱面 #12  ·  千本桜  ·  已锁定  ·  3 名玩家                    ← 房名下面是当前谱面
//! ┌────────────────────────────┐   ┌───────────────────────┐
//! │ [同步/下载进度]            │   │ 玩家（3）             │   ← 右侧：用户列表
//! │                            │   │  ● 我    房主 已就绪 │
//! │ [房主要开始游戏了 准备/暂不]│   ├───────────────────────┤
//! └────────────────────────────┘   │ 房间消息（日志+聊天） │   ← 用户列表下面是聊天框
//! [开始游戏][观战][预览谱面]        │  ……                   │
//! [锁定][循环][密码]                │ [说些什么…]    [发送] │
//!   ↑ 功能按钮全部贴左下角          └───────────────────────┘
//! ```
//!
//! 三条必须守住的约定：
//! 1. **功能按钮只有一个来源**：[`action_items`] 从房间状态推导按钮集合，渲染按它排布并
//!    用 [`ActionButtons`] 登记命中区，触摸侧对同一份结果查按钮，因此不存在"看得到点不到"。
//! 2. **用户列表的渲染与触摸共用一份 id 顺序**（[`sorted_user_ids`]），行索引一一对应；
//!    列表里不显示服务端的回放录制器虚拟用户（它只是个 monitor，不是玩家）。
//! 3. **退出按钮没有底色**：只画文字（[`theme::text_button`]），命中区照旧登记。

use macroquad::prelude::*;
use phira_mp_common::{ClientRoomState, RoomState};
use prpr::{
    core::BOLD_FONT,
    ext::{RectExt, SafeTexture},
    ui::{DRectButton, Scroll, Ui},
};

use super::super::{
    messages::MessageLog,
    state::{sorted_user_ids, user_count},
    theme::{self, *},
};
use crate::{dir, mp::L10N_LOCAL, scene::Downloading};

/// 是否编译了聊天功能。
pub const CHAT_ENABLED: bool = cfg!(feature = "chat");

/// 顶部细页头的高度（只有返回按钮与「离开房间」，标题画在内容区左上角）。
const STRIP_H: f32 = 0.11;
/// 右侧用户列表的行高 / 行距（比整屏列表紧凑：同一列里还要放下聊天框）。
const USER_ROW_H: f32 = 0.13;
const USER_ROW_GAP: f32 = 0.012;

/// 功能按钮的种类。
///
/// 取消类操作按语义拆开（`CancelLocalShare` / `CancelDownload` / `CancelReady`），
/// 这样触摸侧不需要按房间状态二次判断，避免"看得到点不到"。
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

    /// 按钮配色：主操作用主色，取消类用危险色，其余用中性色。
    fn colors(self, accent: Color, spectating: bool) -> (Color, Color) {
        match self {
            RoomAction::Start | RoomAction::Ready => (primary(accent), WHITE),
            RoomAction::Spectate if spectating => (primary(accent), WHITE),
            RoomAction::CancelReady | RoomAction::CancelDownload | RoomAction::CancelLocalShare => {
                (danger(), WHITE)
            }
            _ => (secondary(), text()),
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
    /// 服务端已指示下载、但玩家还没点"准备"
    pub pending_download: bool,
    /// 正在同步谱面（下载中）
    pub syncing: bool,
    /// 房间当前选中的在线谱面
    pub chart_id: Option<i32>,
    /// 当前谱面的名字（在线谱面由服务端选谱消息带下来）
    pub chart_name: Option<String>,
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

/// 依据房间状态推导功能按钮（渲染与触摸共用同一集合）。
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
    /// 房主点某玩家 → 进入整屏管理页
    Manage(i32),
    Room(RoomAction),
    ChatInput,
    ChatSend,
    /// 内联确认条：true = 准备，false = 暂不
    Prompt(bool),
}

/// 房间页渲染所需的上下文。
pub struct Render<'a> {
    pub room: &'a ClientRoomState,
    /// 房间号（房名 = `房间 #<id>`）
    pub room_id: Option<&'a str>,
    pub view: &'a RoomView,
    pub messages: &'a mut MessageLog,
    pub chat_text: &'a str,
    /// 在线谱面下载中
    pub download: Option<&'a mut Downloading>,
    /// 本地谱面同步中
    pub syncing: bool,
    /// 会话级任务在跑
    pub busy: bool,
    /// 是否显示「准备 / 暂不」内联确认条
    pub prompt: bool,
    /// 用户头像的默认贴图
    pub icon: &'a SafeTexture,
    /// 自己的用户 id
    pub me: Option<i32>,
    /// 自己是否已就绪（协议只提供自己的就绪状态）
    pub me_ready: bool,
}

#[derive(Default)]
pub struct RoomPage {
    back: DRectButton,
    leave: DRectButton,
    /// 右侧用户列表的滚动区
    user_scroll: Scroll,
    user_rows: Vec<DRectButton>,
    /// 渲染时记录的用户 id：触摸只按这份数据索引，与渲染同一来源
    user_ids: Vec<i32>,
    actions: ActionButtons,
    chat_btn: DRectButton,
    chat_send_btn: DRectButton,
    prompt_ready: DRectButton,
    prompt_later: DRectButton,
}

/// 「当前谱面」一行：房间阶段文字（+ 已知的谱面名）。
fn current_chart_line(room: &ClientRoomState, view: &RoomView) -> String {
    let (state_text, name): (String, Option<String>) = match room.state {
        RoomState::SelectChart(None) => (mtl!("mp-state-choose").into_owned(), None),
        RoomState::SelectChart(Some(id)) => (mtl!("mp-state-chosen", "id" => id as u64), view.chart_name.clone()),
        RoomState::LocalChart => (
            mtl!("mp-state-local").into_owned(),
            view.local_chart.as_ref().map(|(_, n)| n.clone()),
        ),
        RoomState::WaitingForReady => (mtl!("mp-state-wait").into_owned(), view.chart_name.clone()),
        RoomState::Playing => (mtl!("mp-state-playing").into_owned(), view.chart_name.clone()),
    };
    match name.filter(|n| !n.is_empty()) {
        Some(n) => format!("{state_text}  ·  {n}"),
        None => state_text,
    }
}

/// 左栏信息块的"自然高度"（房名 + 谱面 + 标记行 + 状态行 + 确认条）。
fn info_height(ctx: &Render) -> f32 {
    // 行高 ≈ 0.1 × 字号（横屏下约等于字形高度的 1.4 倍）
    let mut h = 0.11 + 0.07 + 0.06;
    if ctx.download.is_some() {
        h += 0.17;
    } else if ctx.syncing {
        h += 0.13;
    } else if ctx.busy {
        h += 0.04;
    }
    if ctx.prompt {
        h += 0.26;
    }
    h
}

impl RoomPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.leave.invalidate();
        self.actions.invalidate();
        self.chat_btn.invalidate();
        self.chat_send_btn.invalidate();
        self.prompt_ready.invalidate();
        self.prompt_later.invalidate();
        for b in self.user_rows.iter_mut() {
            b.invalidate();
        }
    }

    pub fn update(&mut self, t: f32) {
        self.user_scroll.update(t);
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, mut ctx: Render) {
        let accent = ui.accent();
        let room = ctx.room;
        let items = action_items(room, ctx.view);

        // ————————————— 版面 —————————————
        let top = ui.top;
        let pad = theme::page_pad(ui);
        let page_x = -1. + pad;
        let page_w = 2. - pad * 2.;
        let wide = theme::is_wide(ui);

        // —— 细页头：返回 + 无底色的「离开房间」 ——
        let strip = Rect::new(page_x, -top + HEADER_TOP, page_w, STRIP_H);
        theme::back_button(ui, &mut self.back, t, Rect::new(strip.x, strip.y, STRIP_H, STRIP_H));
        let leave = mtl!("leave-room");
        let lw = (ui.text(leave.as_ref()).size(FS_BUTTON).measure().w + 0.08).clamp(0.2, page_w * 0.4);
        let lr = Rect::new(strip.right() - lw, strip.y, lw, STRIP_H);
        theme::text_button(ui, &mut self.leave, t, lr, leave, FS_BUTTON, danger());

        // —— 内容区 ——
        let body_top = strip.bottom() + BODY_GAP;
        let body_bottom = top - pad * 0.6;
        let body = Rect::new(page_x, body_top, page_w, (body_bottom - body_top).max(0.12));

        // —— 功能按钮：全部贴左下角 ——
        // 列数按左栏可用宽度定（3/2/1 列），行数随之定；按钮高度按剩余高度自适应，
        // 这样横屏（纵向空间只有竖屏的一半多点）也不会把内容挤没。
        let left_w = if wide { (body.w * 0.42).max(0.5) } else { body.w };
        let cols = if left_w >= 0.75 {
            3
        } else if left_w >= 0.55 {
            2
        } else {
            1
        };
        let n = items.len().max(1);
        let rows = n.div_ceil(cols);
        let btn_h = ((body.h * 0.42 - (rows - 1) as f32 * BAR_ROW_GAP) / rows as f32).clamp(0.085, BAR_BTN_H);
        let bar_h = rows as f32 * btn_h + (rows - 1) as f32 * BAR_ROW_GAP;
        let btn_w = ((left_w - (cols - 1) as f32 * BAR_COL_GAP) / cols as f32).max(0.12);
        let bar = Rect::new(body.x, body.bottom() - bar_h, left_w, bar_h);
        let content_h = (bar.y - BAR_GAP - body.y).max(0.1);

        if wide {
            // 横屏：左栏 = 房名/谱面/功能按钮，右栏 = 用户列表 + 聊天框
            let info = Rect::new(body.x, body.y, left_w, content_h);
            let rx = info.right() + SECTION_GAP;
            let right = Rect::new(rx, body.y, (body.right() - rx).max(0.5), content_h);
            self.render_info(ui, t, info, &mut ctx, accent);
            self.render_side(ui, t, right, &mut ctx, accent);
        } else {
            // 竖屏：房名/谱面 → 用户列表 → 聊天框，功能按钮仍在最下方左侧
            let gap = SECTION_GAP;
            let mut info_h = info_height(&ctx).min(content_h * 0.45);
            let mut users_h = (content_h * 0.22).clamp(0.18, 1.2);
            let mut chat_h = content_h - info_h - users_h - gap * 2.;
            if chat_h < 0.24 {
                // 竖向实在不够：优先保聊天框，其次压用户列表，最后压信息块
                let need = 0.24 - chat_h;
                let cut = need.min(users_h - 0.18);
                users_h -= cut;
                info_h = (info_h - (need - cut)).max(0.2);
                chat_h = (content_h - info_h - users_h - gap * 2.).max(0.12);
            }
            let info = Rect::new(body.x, body.y, body.w, info_h);
            let users = Rect::new(body.x, info.bottom() + gap, body.w, users_h);
            let chat = Rect::new(body.x, users.bottom() + gap, body.w, chat_h);
            self.render_info(ui, t, info, &mut ctx, accent);
            self.render_users(ui, t, users, &mut ctx, accent);
            self.render_chat(ui, t, chat, &mut ctx, accent);
        }

        // —— 功能按钮（渲染与触摸同源：命中区在下面这次绘制里登记）——
        for (i, item) in items.iter().enumerate() {
            let r = Rect::new(
                bar.x + (i % cols) as f32 * (btn_w + BAR_COL_GAP),
                bar.y + (i / cols) as f32 * (btn_h + BAR_ROW_GAP),
                btn_w,
                btn_h,
            );
            let (fill, fg) = item.action.colors(accent, ctx.view.spectating);
            let size = (btn_h * 3.4).clamp(0.26, FS_BUTTON);
            theme::button(ui, self.actions.get(item.action), t, r, item.label.clone(), size, fill, fg);
        }
    }

    /// 左栏信息块：房名（左上角）→ 当前谱面 → 房间标记 → 进度行 → 确认条。
    fn render_info(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        let room = ctx.room;
        let mut y = r.y;

        // —— 房名 ——
        let title = match ctx.room_id {
            Some(id) => mtl!("mp-room-tag", "id" => id.to_owned()),
            None => mtl!("multiplayer").into_owned(),
        };
        ui.text(title.as_str())
            .pos(r.x, y)
            .anchor(0., 0.)
            .size(FS_PAGE_TITLE)
            .color(text())
            .max_width(r.w)
            .draw_using(&BOLD_FONT);
        y += 0.11;

        // —— 当前谱面 ——
        let chart = current_chart_line(room, ctx.view);
        theme::text_left(ui, r.x, y + 0.035, FS_BODY, text_dim(), &chart, r.w);
        y += 0.07;

        // —— 房间标记：锁定 / 循环 / 人数 / 观战 ——
        let mut parts: Vec<String> = Vec::new();
        if room.locked {
            parts.push(mtl!("mp-locked-tag").into_owned());
        }
        if room.cycle {
            parts.push(mtl!("mp-cycle-tag").into_owned());
        }
        parts.push(mtl!("mp-n-players", "n" => user_count(room) as u64));
        if ctx.view.spectating {
            parts.push(mtl!("mp-watching").into_owned());
        }
        theme::text_left(ui, r.x, y + 0.03, FS_TAG, text_muted(), &parts.join("  ·  "), r.w);
        y += 0.06;

        // —— 谱面下载 / 同步 / 会话任务（按剩余高度自适应，横屏不会顶到按钮）——
        if let Some(dl) = ctx.download.as_deref_mut() {
            let avail = r.bottom() - y - 0.005;
            if avail >= 0.08 {
                let dr = Rect::new(r.x, y + 0.012, r.w, avail.min(0.16));
                theme::card_rect(ui, dr, card_soft());
                dl.render_inline(ui, dr, t);
                y = dr.bottom();
            } else {
                // 高度实在不够：只留一条不确定进度条
                theme::progress_bar(ui, Rect::new(r.x, y + 0.012, r.w, 0.01), None, t, accent);
                y += 0.03;
            }
        } else if ctx.syncing {
            let avail = r.bottom() - y - 0.005;
            let dr = Rect::new(r.x, y + 0.012, r.w, avail.clamp(0.06, 0.11));
            theme::card_rect(ui, dr, card_soft());
            theme::text_left(
                ui,
                dr.x + CARD_PAD,
                dr.y + dr.h * 0.34,
                FS_SMALL,
                text_dim(),
                &mtl!("mp-syncing-chart"),
                dr.w - CARD_PAD * 2.,
            );
            theme::progress_bar(
                ui,
                Rect::new(dr.x + CARD_PAD, dr.bottom() - dr.h * 0.24, dr.w - CARD_PAD * 2., 0.01),
                None,
                t,
                accent,
            );
            y = dr.bottom();
        } else if ctx.busy {
            theme::progress_bar(ui, Rect::new(r.x, y + 0.01, r.w, 0.01), None, t, accent);
            y += 0.03;
        }

        // —— 「房主要开始游戏了」确认条（贴信息块底部，不遮任何东西）——
        if ctx.prompt {
            let h = (r.bottom() - y - 0.008).clamp(0.15, 0.26);
            let cr = Rect::new(r.x, r.bottom() - h, r.w, h);
            theme::card_rect(ui, cr, tag_accent(accent));
            theme::text_left(
                ui,
                cr.x + CARD_PAD,
                cr.y + 0.045,
                FS_SMALL,
                text(),
                &mtl!("preview-interrupted-content"),
                cr.w - CARD_PAD * 2.,
            );
            let bh = (h * 0.44).clamp(0.08, 0.11);
            let bw = ((cr.w - CARD_PAD * 2. - BAR_COL_GAP) / 2.).min(0.24);
            let by = cr.bottom() - bh - 0.01;
            let rr = Rect::new(cr.right() - CARD_PAD - bw, by, bw, bh);
            let ll = Rect::new(rr.x - BAR_COL_GAP - bw, by, bw, bh);
            let size = (bh * 3.4).clamp(0.22, FS_SMALL);
            theme::button(ui, &mut self.prompt_later, t, ll, mtl!("preview-not-now"), size, secondary(), text());
            theme::button(ui, &mut self.prompt_ready, t, rr, mtl!("preview-ready"), size, primary(accent), WHITE);
        }
    }

    /// 右栏：用户列表（上）+ 聊天框（下）。
    fn render_side(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        let users_h = (r.h * 0.46).clamp(0.14, (r.h - 0.3).max(0.14));
        let users = Rect::new(r.x, r.y, r.w, users_h);
        let chat = Rect::new(
            r.x,
            users.bottom() + SECTION_GAP,
            r.w,
            (r.bottom() - users.bottom() - SECTION_GAP).max(0.1),
        );
        self.render_users(ui, t, users, ctx, accent);
        self.render_chat(ui, t, chat, ctx, accent);
    }

    /// 用户列表：头像 + 名字 + 徽标，房主可点行进入管理页。
    fn render_users(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        let room = ctx.room;
        let ids = sorted_user_ids(room, ctx.me);
        theme::section_label(ui, r.x, r.y, &mtl!("mp-player-count", "n" => ids.len() as u64));
        let list = Rect::new(r.x, r.y + 0.045, r.w, (r.h - 0.045).max(0.06));
        theme::card_rect(ui, list, card_soft());
        let inner = list.feather(-0.012);
        let step = USER_ROW_H + USER_ROW_GAP;
        let manageable = manage_allowed(room);
        let (icon, me, me_ready) = (ctx.icon, ctx.me, ctx.me_ready);
        self.user_ids.clear();
        self.user_rows.resize_with(ids.len(), DRectButton::new);
        ui.scope(|ui| {
            ui.dx(inner.x);
            ui.dy(inner.y);
            self.user_scroll.size((inner.w, inner.h));
            self.user_scroll.render(ui, |ui| {
                for (i, &id) in ids.iter().enumerate() {
                    // 行索引与 `user_ids` 必须严格对齐：先记录 id，再看能不能画
                    self.user_ids.push(id);
                    let Some(user) = room.users.get(&id) else { continue };
                    let is_me = Some(id) == me;
                    let rr = Rect::new(0., i as f32 * step, inner.w, USER_ROW_H);
                    theme::row_button(ui, &mut self.user_rows[i], t, rr, is_me, accent, |ui, rr| {
                        theme::player_row_content(
                            ui,
                            rr,
                            t,
                            icon,
                            user.id,
                            &user.name,
                            is_me,
                            is_me && room.is_host,
                            user.monitor,
                            is_me && me_ready,
                            accent,
                        );
                        if manageable && !is_me {
                            theme::text_chevron(ui, rr.right() - CARD_PAD * 0.4, rr.center().y);
                        }
                    });
                }
                (inner.w, ids.len() as f32 * step)
            });
        });
        // 人数变少时作废多出来的旧命中区，避免点到上一帧的行
        for i in ids.len()..self.user_rows.len() {
            self.user_rows[i].invalidate();
        }
    }

    /// 聊天框：日志与聊天是同一个消息流，跟输入行同框。
    fn render_chat(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        theme::section_label(ui, r.x, r.y, &mtl!("mp-chat-caption"));
        let input_h = if CHAT_ENABLED { (r.h * 0.26).clamp(0.09, 0.12) } else { 0. };
        let list = Rect::new(
            r.x,
            r.y + 0.045,
            r.w,
            (r.h - 0.045 - if CHAT_ENABLED { input_h + 0.012 } else { 0. }).max(0.05),
        );
        theme::card_rect(ui, list, card_soft());
        ui.scope(|ui| {
            ui.dx(list.x + 0.014);
            ui.dy(list.y + 0.012);
            ctx.messages.render(
                ui,
                Rect::new(0., 0., (list.w - 0.028).max(0.05), (list.h - 0.024).max(0.04)),
            );
        });
        if CHAT_ENABLED {
            let iy = r.bottom() - input_h;
            let send_w = (0.14f32).min(r.w * 0.28).max(0.1);
            let br = Rect::new(r.x, iy, (r.w - send_w - 0.012).max(0.14), input_h);
            ui.fill_path(&br.rounded(R_BTN), card_soft());
            self.chat_btn.render_input(
                ui,
                br.feather(-0.008),
                t,
                ctx.chat_text,
                mtl!("chat-placeholder"),
                (input_h * 3.2).clamp(0.24, FS_BODY),
            );
            let sb = Rect::new(br.right() + 0.012, iy, send_w, input_h);
            theme::button(
                ui,
                &mut self.chat_send_btn,
                t,
                sb,
                mtl!("chat-send"),
                (input_h * 3.4).clamp(0.24, FS_BUTTON),
                primary(accent),
                WHITE,
            );
        }
    }

    pub fn touch(&mut self, touch: &Touch, t: f32, room: &ClientRoomState, view: &RoomView, msgs: &mut MessageLog) -> Option<Action> {
        // 聊天 / 日志滚动区优先
        if msgs.touch(touch, t) {
            return None;
        }
        // 用户列表滚动区
        if self.user_scroll.contains(touch) && self.user_scroll.touch(touch, t) {
            for b in self.user_rows.iter_mut() {
                b.inner.cancel();
            }
            return None;
        }
        if self.back.touch(touch, t) {
            return Some(Action::Back);
        }
        if self.leave.touch(touch, t) {
            return Some(Action::Leave);
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
        // 玩家行：只有房主能点进管理页（与渲染时的箭头提示一致）
        if manage_allowed(room) {
            for (i, b) in self.user_rows.iter_mut().enumerate() {
                if b.touch(touch, t) {
                    if let Some(&id) = self.user_ids.get(i) {
                        return Some(Action::Manage(id));
                    }
                }
            }
        }
        // 功能按钮：只按 action_items 给出的集合查按钮
        for item in action_items(room, view) {
            if self.actions.get(item.action).touch(touch, t) {
                return Some(Action::Room(item.action));
            }
        }
        None
    }
}
