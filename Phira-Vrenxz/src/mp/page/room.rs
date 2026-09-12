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
//! 3. **只有一个退出入口**：左上角的返回图标（点它 = 离开房间、回主页），
//!    不再另摆一个「离开房间」的文字按钮 —— 两个按钮做同一件事看着就像重复。
//!
//! 版面（横屏）：
//! ```text
//! ‹                                              ← 细页头：只有返回图标
//! 房间 #31205                                    ← 左上角：房名
//! ┌ 谱面卡 ────────────────┐  ┌───────────────┐
//! │ 已选谱面 #12   3 名玩家│  │ 玩家（3）      │
//! │ 千本桜      [房主][锁定]│  │ ● 我  房主    │
//! └────────────────────────┘  ├───────────────┤
//!                             │ 房间消息       │
//! [开始游戏][锁定][循环][观战] │ ……  [说点什么] │
//!   ↑ 左下角：小尺寸、按文字宽度排，不铺满整行  └───────────────┘
//! ```

use macroquad::prelude::*;
use phira_mp_common::{ClientRoomState, RoomState};
use prpr::{
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

/// 顶部细页头的高度（只有一个返回图标，标题画在内容区左上角）。
const STRIP_H: f32 = 0.14 * SCALE;
/// 右侧用户列表的行高 / 行距（比整屏列表紧凑：同一列里还要放下聊天框）。
const USER_ROW_H: f32 = 0.13 * SCALE;
const USER_ROW_GAP: f32 = 0.015 * SCALE;
/// 左侧模糊背景上的超大房名字号。
const FS_HERO: f32 = 1.1;
/// 右侧栏分区标题（用户列表 / 聊天&日志）字号。
const FS_SUB: f32 = 0.55 * SCALE;

// ============ 横屏设计稿常量 ============
//
// 稿子画布 472×267（≈16:9），内容区 x 4..475 / y 47..314。
// 下面所有位置都按 **占页宽/页高的比例** 写：fx = (x-4)/471，fy = (y-47)/267，
// 游戏里 x = -1 + 2fx、y = -top + 2·top·fy，这样任何窗口比例下版面都和稿子一致。
/// 左面板占的宽度比例（稿子里左上角到 0.61）。
const L_PANEL_W: f32 = 0.61;
/// 面板右边缘的倾斜量 = 高度 × 该值（稿子实测 0.113）。
const L_SLANT: f32 = 0.113;
/// 顶部条高度比例（稿子 0.116）。
const STRIP_F: f32 = 0.116;
/// 顶栏按钮：文字外扩的内边距 / 最小宽度（按稿子的按钮宽度反推）。
const STRIP_BTN_PAD: f32 = 0.16;
const STRIP_BTN_MIN_W: f32 = 0.2;
/// 右栏内容左边界比例（稿子聊天/输入框都在 0.56 右侧）。
const R_COL_X: f32 = 0.60;
/// 用户列表与聊天之间的分隔线高度比例（稿子 0.49）。
const R_SPLIT: f32 = 0.49;
/// 输入框上沿比例（稿子 0.85）。
const R_INPUT_Y: f32 = 0.85;
/// 左下信息卡：占宽 / 上沿 / 下沿比例（稿子 0..0.40、0.772..0.996）。
const CARD_W: f32 = 0.40;
const CARD_TOP: f32 = 0.772;

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
    /// 房主：去谱面库选谱（退出多人场景但**保留房间与会话**，选完自动回来）
    Library,
    /// 离开房间（回主页，座位让出来）
    LeaveRoom,
}

impl RoomAction {
    pub const ALL: [RoomAction; 12] = [
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
        RoomAction::Library,
        RoomAction::LeaveRoom,
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
            RoomAction::Library => 10,
            RoomAction::LeaveRoom => 11,
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

    /// 是不是"当前该做的事"（主色高亮的那一个）。
    fn is_primary(self, spectating: bool) -> bool {
        match self {
            RoomAction::Start | RoomAction::Ready => true,
            RoomAction::Spectate => spectating,
            _ => false,
        }
    }

    /// 工具带上的图标。
    fn icon(self) -> theme::ToolIcon {
        use theme::ToolIcon as I;
        match self {
            RoomAction::Start => I::Play,
            RoomAction::Ready => I::Ready,
            RoomAction::CancelReady | RoomAction::CancelDownload | RoomAction::CancelLocalShare => I::Cancel,
            RoomAction::Password => I::Settings,
            RoomAction::CycleRoom => I::Cycle,
            RoomAction::LockRoom => I::Lock,
            RoomAction::Preview => I::Preview,
            RoomAction::Spectate => I::Spectate,
            RoomAction::Library => I::Library,
            RoomAction::LeaveRoom => I::LeaveRoom,
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

/// 顶栏小方块用的单字标签。
///
/// 设计稿顶栏只画了三个按钮（打开谱面库 / 观战 / 预览），其余动作（锁定、循环、
/// 密码、离开房间、各种取消）作者没画 —— 这里收成同样的斜角小方块贴在顶栏尾部，
/// 既不动稿子的空区，也不丢功能。
fn short_label(action: RoomAction) -> String {
    use RoomAction as A;
    match action {
        A::LockRoom => "锁".to_owned(),
        A::CycleRoom => "循".to_owned(),
        A::Password => "密".to_owned(),
        A::LeaveRoom | A::CancelReady | A::CancelDownload | A::CancelLocalShare => "✕".to_owned(),
        A::Preview => "览".to_owned(),
        A::Spectate => "观".to_owned(),
        A::Library => "库".to_owned(),
        A::Start | A::Ready => "▶".to_owned(),
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
            // 「谱面库」：选谱阶段才有意义（在线谱 / 本地谱都是在这儿选）
            if is_host {
                items.push(ActItem {
                    action: RoomAction::Library,
                    label: mtl!("mp-library").into_owned(),
                });
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
    // 离开房间：放在最后（最右边），跟"开始游戏"那种主动作分开
    items.push(ActItem {
        action: RoomAction::LeaveRoom,
        label: mtl!("leave-room").into_owned(),
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
    /// 返回：离开当前房间（回到主页，而不是直接退出多人模式）
    Back,
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

/// 「当前谱面」的信息：房间阶段文字 + 谱面名。
fn chart_parts(room: &ClientRoomState, view: &RoomView) -> (String, Option<String>) {
    match room.state {
        RoomState::SelectChart(None) => (mtl!("mp-state-choose").into_owned(), None),
        RoomState::SelectChart(Some(id)) => (mtl!("mp-state-chosen", "id" => id as u64), view.chart_name.clone()),
        RoomState::LocalChart => (
            mtl!("mp-state-local").into_owned(),
            view.local_chart.as_ref().map(|(_, n)| n.clone()),
        ),
        RoomState::WaitingForReady => (mtl!("mp-state-wait").into_owned(), view.chart_name.clone()),
        RoomState::Playing => (mtl!("mp-state-playing").into_owned(), view.chart_name.clone()),
    }
}

/// 左栏信息块的"自然高度"（房名 + 谱面卡 + 状态行 + 确认条）。
fn info_height(ctx: &Render) -> f32 {
    // 行高 ≈ 0.1 × 字号（横屏下约等于字形高度的 1.4 倍），随 [`SCALE`] 一起缩放
    let mut h = TITLE_BLOCK_H + 0.275 * SCALE + 0.075 * SCALE;
    if ctx.download.is_some() {
        h += 0.21 * SCALE;
    } else if ctx.syncing {
        h += 0.16 * SCALE;
    } else if ctx.busy {
        h += 0.05 * SCALE;
    }
    if ctx.prompt {
        h += 0.32 * SCALE;
    }
    h
}

impl RoomPage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
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
        let top = ui.top;
        let pad = theme::page_pad(ui);
        let wide = theme::is_wide(ui);

        if !wide {
            // 竖屏：保留原卡片堆叠布局
            self.render_portrait(ui, t, ctx, &items);
            return;
        }

        // ============ 横屏：严格按设计稿（画布 472×267）的几何重写 ============
        //
        // 稿子坐标 → 比例：fx = (x-3.57)/472.65，fy = (y-47)/266.96；
        // 游戏里 x = -1 + 2fx、y = -top + 2·top·fy。
        // 字号也按稿子：size = 稿子 px / 267 * (2·top) / 0.08（稿子里的字号是 em，
        // 中文字形≈1em，所以这样换算出来的视觉大小和稿子一致）。
        let px = |f: f32| -1. + 2. * f;
        let py = |f: f32| -top + 2. * top * f;
        let hh = 2. * top;
        let fs = |font_px: f32| font_px / 267. * hh / 0.08;

        // —— 整屏底色：稿子是全屏灰渐变（#5d5d5d → #9d9d9d）——
        const BG_TOP: Color = Color::new(0.365, 0.365, 0.365, 1.);
        const BG_BOTTOM: Color = Color::new(0.616, 0.616, 0.616, 1.);
        theme::slant_panel_grad(ui, -1., -top, 2., hh, 0., BG_TOP, BG_BOTTOM);

        // —— 左面板：灰→白竖向渐变 + 右边缘倾斜（稿子 0.61 → 0.546）——
        const PANEL_TOP_C: Color = Color::new(0.533, 0.533, 0.533, 1.);
        const PANEL_BOTTOM_C: Color = Color::new(1., 1., 1., 1.);
        let panel_l = -1.;
        let panel_w = px(L_PANEL_W) - panel_l;
        let panel_top = -top;
        let panel_bottom = py(1.);
        theme::slant_panel_grad(ui, panel_l, panel_top, panel_w, panel_bottom - panel_top, L_SLANT, PANEL_TOP_C, PANEL_BOTTOM_C);

        // —— 左面板顶部黑条（稿子 0..0.118，只盖左面板）——
        let strip_bottom = py(STRIP_F);
        theme::slant_panel(ui, panel_l, panel_top, panel_w, strip_bottom - panel_top, L_SLANT, Color::new(0., 0., 0., 0.72));

        // —— 顶栏：返回图标 + 三个斜角按钮（打开谱面库 / 观战 / 预览，稿子顺序）——
        // 稿子按钮 y 0.020..0.101、高约页高 8%，宽度按文字；形状是"上边右移"的平行四边形。
        let btn_top = py(0.020);
        let btn_h = py(0.101) - btn_top;
        let back_s = btn_h;
        theme::back_button(ui, &mut self.back, t, Rect::new(px(0.012), btn_top, back_s, back_s));
        let mut bx = px(0.049);
        for item in &items {
            if !matches!(item.action, RoomAction::Library | RoomAction::Spectate | RoomAction::Preview) {
                continue;
            }
            let tw = ui.text(&item.label).size(fs(11.4)).measure().w;
            let w = (tw + 0.06).max(0.16);
            let r = Rect::new(bx, btn_top, w, btn_h);
            Self::skew_btn(ui, t, self.actions.get(item.action), r, &item.label, fs(11.4), Color::new(0.702, 0.702, 0.702, 1.), WHITE);
            bx += w + 0.012;
        }

        // —— 房名（稿子 fy 0.175，em 19.3px）+ 谱面:XXX（fy 0.22，em 8.65px）——
        let title = match ctx.room_id {
            Some(id) => mtl!("mp-room-tag", "id" => id.to_owned()),
            None => mtl!("multiplayer").into_owned(),
        };
        let (state_text, chart_name) = chart_parts(room, ctx.view);
        let text_x = px(0.011);
        let text_w = panel_w - (text_x - panel_l) - 0.14;
        theme::text_left_bold(ui, text_x, py(0.175), fs(19.3), Color::new(0.06, 0.06, 0.08, 1.), title.as_str(), text_w);
        let sub = match &chart_name {
            Some(n) if !n.is_empty() => format!("{}: {}", mtl!("mp-chart-label"), n),
            _ => state_text.clone(),
        };
        theme::text_left(ui, text_x, py(0.22), fs(8.65), Color::new(0.25, 0.25, 0.28, 1.), &sub, text_w);

        // —— 房间设置等其余动作：稿子左面板中间是空的，这里收成顶栏尾部的一排小方块 ——
        // （不占版面、也不破坏稿子的空区）
        let mut sx = bx + 0.01;
        let sq = btn_h * 0.86;
        for item in &items {
            match item.action {
                RoomAction::Library | RoomAction::Spectate | RoomAction::Preview
                | RoomAction::Start | RoomAction::Ready => {}
                _ => {
                    if sx + sq > panel_l + panel_w - 0.05 {
                        break;
                    }
                    let r = Rect::new(sx, btn_top + (btn_h - sq) * 0.5, sq, sq);
                    Self::skew_btn(ui, t, self.actions.get(item.action), r, &short_label(item.action), fs(11.4), Color::new(0.702, 0.702, 0.702, 1.), WHITE);
                    sx += sq + 0.008;
                }
            }
        }

        // —— 进度 / 同步状态：放在房名下方（稿子空区），不遮挡任何稿件元素 ——
        let mut cy2 = py(0.28);
        if let Some(dl) = ctx.download.as_deref_mut() {
            let dr = Rect::new(text_x, cy2, panel_w * 0.62, 0.13);
            theme::card_rect(ui, dr, card());
            dl.render_inline(ui, dr, t);
        } else if ctx.syncing {
            theme::text_left(ui, text_x, cy2 + 0.02, fs(8.65), Color::new(0.25, 0.25, 0.28, 1.), &mtl!("mp-syncing-chart"), text_w);
            theme::progress_bar(ui, Rect::new(text_x, cy2 + 0.06, panel_w * 0.5, 0.012), None, t, accent);
        } else if ctx.busy {
            theme::progress_bar(ui, Rect::new(text_x, cy2, panel_w * 0.5, 0.012), None, t, accent);
        }

        // —— 左下信息卡（稿子 x 0..0.40、y 0.772..0.996，白底 + 斜右边）——
        let card = Rect::new(panel_l, py(CARD_TOP), 2. * CARD_W, py(0.996) - py(CARD_TOP));
        theme::slant_panel_grad_h(ui, card.x, card.y, card.w, card.h, L_SLANT * 0.5, Color::new(0.56, 0.56, 0.56, 1.), Color::new(1., 1., 1., 1.));
        // 卡的左/上/下三边描一道黑边（稿子 stroke #000 0.5）
        let edge = Color::new(0., 0., 0., 0.75);
        ui.fill_rect(Rect::new(card.x, card.y, card.w, 0.0018), edge);
        ui.fill_rect(Rect::new(card.x, card.bottom() - 0.0018, card.w - card.h * L_SLANT * 0.5, 0.0018), edge);
        ui.fill_rect(Rect::new(card.x, card.y, 0.0018, card.h), edge);

        // 本地最好成绩（在线谱面按 id 在本地谱面库里查）
        let (rec, level) = match ctx.view.chart_id {
            Some(id) => {
                let data = crate::get_data();
                match data.charts.iter().find(|c| c.info.id == Some(id)) {
                    Some(c) => (c.record.as_ref().map(|r| (r.score, r.accuracy)), Some(c.info.level.clone())),
                    None => (None, None),
                }
            }
            None => (None, None),
        };
        // 封面位（稿子 fy 0.823..0.945、宽约页宽 5.7%）：没有封面图就用等级色块占位
        let cov = Rect::new(px(0.023), py(0.823), 2. * 0.057, py(0.945) - py(0.823));
        ui.fill_path(&cov.rounded(R_ROW), Color::new(0.62, 0.64, 0.70, 1.));
        if let Some(lv) = &level {
            ui.text(lv)
                .pos(cov.center().x, cov.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(fs(8.65))
                .color(WHITE)
                .max_width(cov.w - 0.01)
                .draw();
        }
        // 等级小牌（稿子 x 0.096..0.141、y 0.790..0.847）
        let badge = Rect::new(px(0.096), py(0.790), 2. * 0.045, py(0.847) - py(0.790));
        theme::slant_panel(ui, badge.x, badge.y, badge.w, badge.h, 0.2, Color::new(0.62, 0.64, 0.70, 1.));
        if let Some(lv) = &level {
            ui.text(lv)
                .pos(badge.center().x, badge.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(fs(7.5))
                .color(WHITE)
                .max_width(badge.w)
                .draw();
        }
        // 谱面 ID（稿子 "#7891"，灰） / 最好成绩（稿子大黑数字） / 准确率
        let id_text = match ctx.view.chart_id {
            Some(id) => format!("#{id}"),
            None => mtl!("mp-state-local").into_owned(),
        };
        theme::text_left(ui, px(0.149), py(0.838), fs(16.), Color::new(0.42, 0.42, 0.42, 1.), &id_text, card.right() - px(0.149) - 0.03);
        let big = match rec {
            Some((score, _)) => format!("{score:07}"),
            None => chart_name.clone().filter(|n| !n.is_empty()).unwrap_or_else(|| state_text.clone()),
        };
        theme::text_left_bold(ui, px(0.083), py(0.923), fs(24.8), Color::new(0.04, 0.04, 0.06, 1.), &big, card.right() - px(0.083) - 0.02);
        if let Some((_, acc)) = rec {
            theme::text_left(ui, px(0.083), py(0.970), fs(9.1), Color::new(0.1, 0.1, 0.12, 1.), &format!("{:.2}%", acc * 100.), card.right() - px(0.083) - 0.02);
        }

        // —— 主要动作：稿子在卡右侧画了个黑色 ▶（外黑内白），点它 = 开始 / 准备 ——
        let main_action = items.iter().find(|it| matches!(it.action, RoomAction::Start | RoomAction::Ready | RoomAction::CancelReady));
        if let Some(item) = main_action {
            let tri = Rect::new(px(0.440), py(0.830), 2. * 0.065, py(0.950) - py(0.830));
            let btn = self.actions.get(item.action);
            btn.build(ui, t, tri, |ui, _| {
                prpr::ext::draw_parallelogram(tri, None, Color::new(0., 0., 0., 0.), false);
                theme::triangle_right(ui, tri, Color::new(0.02, 0.02, 0.03, 1.), 0.62, Some(WHITE));
                if item.action != RoomAction::Start {
                    theme::text_left(ui, tri.right() + 0.02, tri.center().y, fs(14.3), Color::new(0.05, 0.05, 0.07, 1.), &item.label, 0.4);
                }
            });
        }

        // —— 右栏标题：用户列表（稿子 x 0.615、fy 0.079，黑字直接压在底图上）+ 人数 ——
        theme::text_left_bold(ui, px(0.615), py(0.079), fs(19.3), Color::new(0.05, 0.05, 0.07, 1.), &mtl!("user-list"), 0.5);
        let count = mtl!("mp-n-players", "n" => user_count(room) as u64);
        theme::text_right(ui, px(0.997), py(0.079), fs(11.4), Color::new(0.15, 0.15, 0.18, 1.), &count, 0.4);

        // —— 用户列表（稿子头像 fy 0.151、名字 fy 0.176）到 fy 0.489 的细线 ——
        let right_x = px(0.567);
        let right_w = px(0.997) - right_x;
        let split_y = py(0.489);
        let users = Rect::new(right_x, py(0.115), right_w, (split_y - py(0.115) - 0.01).max(0.12));
        self.render_users_list(ui, t, users, &mut ctx, accent);
        theme::h_line(ui, right_x, split_y, right_w);

        // —— 聊天区（fy 0.489..0.847，稿子里聊天字是白的 → 这里给一块深色底保证可读）——
        let thick_y = py(0.847);
        let chat_r = Rect::new(right_x, split_y + 0.012, right_w, (thick_y - split_y - 0.024).max(0.06));
        ui.fill_path(&chat_r.rounded(R_CARD), Color::new(0., 0., 0., 0.28));
        ui.scope(|ui| {
            ui.dx(chat_r.x + 0.012);
            ui.dy(chat_r.y + 0.01);
            ctx.messages.render(ui, Rect::new(0., 0., chat_r.w - 0.024, chat_r.h - 0.02));
        });
        // 稿子那条加粗的线（2.5px）在输入框上方
        ui.fill_rect(Rect::new(right_x, thick_y, right_w, 0.0094 * hh / 1.125 * 1.2), Color::new(0., 0., 0., 0.85));

        // —— 输入行：白底输入框（稿子 x 0.559..0.883）+ 深灰「发送」斜角按钮（0.891..1.0）——
        let input_y = py(0.85);
        let input_h = py(0.996) - input_y;
        let input_r = Rect::new(right_x, input_y, px(0.883) - right_x, input_h);
        ui.fill_path(&input_r.rounded(R_BTN), Color::new(1., 1., 1., 1.));
        if CHAT_ENABLED {
            self.chat_btn.render_input(ui, input_r.feather(-0.014), t, ctx.chat_text, mtl!("chat-placeholder"), fs(17.));
        }
        let send_r = Rect::new(px(0.891), input_y, px(1.) - px(0.891), input_h);
        Self::skew_btn(ui, t, &mut self.chat_send_btn, send_r, &mtl!("chat-send"), fs(14.3), Color::new(0.404, 0.404, 0.404, 1.), Color::new(0., 0., 0., 1.));

        // —— 内联确认条（房主要开始游戏了 / 准备·暂不）——
        if ctx.prompt {
            let h = 0.22 * SCALE;
            let cr = Rect::new(text_x, py(0.58), panel_w * 0.7, h);
            theme::card_rect(ui, cr, tag_accent(accent));
            theme::text_left(ui, cr.x + CARD_PAD, cr.y + 0.05 * SCALE, fs(11.4), text(), &mtl!("preview-interrupted-content"), cr.w - CARD_PAD * 2.);
            let bh = h * 0.42;
            let bw = 0.22 * SCALE;
            let by = cr.bottom() - bh - 0.015;
            let rr = Rect::new(cr.right() - CARD_PAD - bw, by, bw, bh);
            let ll = Rect::new(rr.x - 0.03 - bw, by, bw, bh);
            theme::button(ui, &mut self.prompt_later, t, ll, mtl!("preview-not-now"), fs(11.4), secondary(), text());
            theme::button(ui, &mut self.prompt_ready, t, rr, mtl!("preview-ready"), fs(11.4), primary(accent), WHITE);
        }
    }

    /// 竖屏：保留原卡片堆叠布局。
    fn render_portrait(&mut self, ui: &mut Ui, t: f32, mut ctx: Render, items: &[ActItem]) {
        let accent = ui.accent();
        let top = ui.top;
        let pad = theme::page_pad(ui);
        let page_x = -1. + pad;
        let page_w = 2. - pad * 2.;
        theme::back_button(ui, &mut self.back, t, Rect::new(page_x, -top + HEADER_TOP, STRIP_H, STRIP_H));
        let body_top = -top + HEADER_TOP + STRIP_H + BODY_GAP;
        let body_bottom = top - pad * 0.6;
        let body = Rect::new(page_x, body_top, page_w, (body_bottom - body_top).max(0.12));
        let labels: Vec<String> = items.iter().map(|it| it.label.clone()).collect();
        let (bar, bar_rects) = theme::tool_bar(ui, &labels, body.x, body.right(), body.bottom(), theme::BarAlign::Left);
        let content_h = (bar.y - BAR_GAP - body.y).max(0.1);
        let gap = SECTION_GAP;
        let mut info_h = info_height(&ctx).min(content_h * 0.45);
        let mut users_h = (content_h * 0.22).clamp(0.18, 1.2);
        let mut chat_h = content_h - info_h - users_h - gap * 2.;
        if chat_h < 0.24 {
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
        for (i, item) in items.iter().enumerate() {
            let Some(r) = bar_rects.get(i).copied() else { continue };
            if r.w <= 0.01 { continue; }
            let icon = theme::tool_icon(item.action.icon());
            let active = item.action.is_primary(ctx.view.spectating);
            theme::tool_button(ui, self.actions.get(item.action), t, r, icon.as_ref(), &item.label, active, accent);
        }
    }

    /// 设计稿里按钮的倾斜量（上边右移 = 高 × 该值，稿子实测约 0.27）。
    const SKEW_SLOPE: f32 = 0.27;

    /// 斜角平行四边形按钮（设计稿样式）：平四边形底 + 居中文本，命中区照旧登记。
    fn skew_btn(
        ui: &mut Ui,
        t: f32,
        btn: &mut DRectButton,
        r: Rect,
        label: &str,
        size: f32,
        fill: Color,
        fg: Color,
    ) {
        btn.render_shadow(ui, r, t, |ui, _path| {
            theme::skew_panel(ui, r.x, r.y, r.w, r.h, Self::SKEW_SLOPE, fill, fill);
            ui.text(label)
                .pos(r.center().x, r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(size)
                .color(fg)
                .max_width(r.w)
                .draw();
        });
    }


    /// 左栏信息块：房名（左上角）→ 谱面卡 → 进度行 → 确认条。
    fn render_info(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        let room = ctx.room;
        let mut y = r.y;

        // —— 房名：主色竖条 + 大号粗体（左上角的第一眼信息）——
        let title = match ctx.room_id {
            Some(id) => mtl!("mp-room-tag", "id" => id.to_owned()),
            None => mtl!("multiplayer").into_owned(),
        };
        ui.fill_rect(
            Rect::new(r.x, y + 0.015 * SCALE, 0.016 * SCALE, FS_PAGE_TITLE * 0.1),
            color_alpha(accent, 0.95),
        );
        theme::text_left_bold(
            ui,
            r.x + 0.04 * SCALE,
            y + FS_PAGE_TITLE * 0.05,
            FS_PAGE_TITLE,
            text(),
            title.as_str(),
            r.w - 0.04 * SCALE,
        );
        y += TITLE_BLOCK_H;

        // —— 谱面卡：阶段小字 + 大号谱面名 + 房间标记徽标 ——
        let (state_text, chart_name) = chart_parts(room, ctx.view);
        let name = chart_name.filter(|n| !n.is_empty());
        // 卡片吃掉左栏大部分高度：既让"当前谱面"成为这一栏的主体，
        // 也避免左栏中间留一大片空白（底部是功能按钮，只能往上填）。
        // 余下 `STATUS_RESERVE_H` 留给进度行 / 确认条。
        let card_h = (r.bottom() - y - STATUS_RESERVE_H).clamp(0.25 * SCALE, 0.56 * SCALE);
        let card_r = Rect::new(r.x, y, r.w, card_h);
        theme::card_accented(ui, card_r, card(), accent);
        let inner_x = card_r.x + CARD_PAD + PANEL_INSET;
        let inner_w = card_r.w - CARD_PAD * 2. - PANEL_INSET;
        let players = mtl!("mp-n-players", "n" => user_count(room) as u64);
        let tall = card_h > 0.25 * SCALE;
        match &name {
            Some(n) => {
                theme::text_left(ui, inner_x, card_r.y + card_r.h * 0.22, FS_TAG, text_muted(), &state_text, inner_w * 0.6);
                theme::text_right(ui, card_r.right() - CARD_PAD, card_r.y + card_r.h * 0.22, FS_TAG, text_dim(), &players, inner_w * 0.38);
                theme::text_left_bold(
                    ui,
                    inner_x,
                    card_r.y + card_r.h * (if tall { 0.55 } else { 0.5 }),
                    FS_SECTION,
                    text(),
                    n,
                    inner_w,
                );
                if tall {
                    let mut badges: Vec<(String, Color, Color)> = Vec::new();
                    if room.locked {
                        badges.push((mtl!("mp-locked-tag").into_owned(), tag_bg(), text_dim()));
                    }
                    if room.cycle {
                        badges.push((mtl!("mp-cycle-tag").into_owned(), tag_bg(), text_dim()));
                    }
                    if ctx.view.spectating {
                        badges.push((mtl!("mp-watching").into_owned(), tag_accent(accent), WHITE));
                    }
                    if !badges.is_empty() {
                        theme::pill_row(ui, inner_x, card_r.y + card_r.h * 0.85, card_r.right() - CARD_PAD, &badges);
                    }
                }
            }
            None => {
                // 还没有谱面：整张卡就是一句状态（大字 + 人数）
                theme::text_right(
                    ui,
                    card_r.right() - CARD_PAD,
                    card_r.center().y,
                    FS_TAG,
                    text_dim(),
                    &players,
                    inner_w * 0.4,
                );
                theme::text_left_bold(
                    ui,
                    inner_x,
                    card_r.center().y,
                    FS_SECTION,
                    text(),
                    &state_text,
                    inner_w - inner_w * 0.42,
                );
            }
        }
        y = card_r.bottom() + SECTION_GAP;

        // —— 谱面下载 / 同步 / 会话任务（按剩余高度自适应，横屏不会顶到按钮）——
        if let Some(dl) = ctx.download.as_deref_mut() {
            let avail = r.bottom() - y - 0.006 * SCALE;
            if avail >= 0.1 * SCALE {
                let dr = Rect::new(r.x, y + PANEL_INSET, r.w, avail.min(0.2 * SCALE));
                theme::card_rect(ui, dr, card_soft());
                dl.render_inline(ui, dr, t);
                y = dr.bottom();
            } else {
                // 高度实在不够：只留一条不确定进度条
                theme::progress_bar(ui, Rect::new(r.x, y + PANEL_INSET, r.w, 0.012 * SCALE), None, t, accent);
                y += 0.04 * SCALE;
            }
        } else if ctx.syncing {
            let avail = r.bottom() - y - 0.006 * SCALE;
            let dr = Rect::new(r.x, y + PANEL_INSET, r.w, avail.clamp(0.075 * SCALE, 0.135 * SCALE));
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
                Rect::new(dr.x + CARD_PAD, dr.bottom() - dr.h * 0.24, dr.w - CARD_PAD * 2., 0.012 * SCALE),
                None,
                t,
                accent,
            );
            y = dr.bottom();
        } else if ctx.busy {
            theme::progress_bar(ui, Rect::new(r.x, y + PANEL_INSET, r.w, 0.012 * SCALE), None, t, accent);
            y += 0.04 * SCALE;
        }

        // —— 「房主要开始游戏了」确认条（贴信息块底部，不遮任何东西）——
        if ctx.prompt {
            let h = (r.bottom() - y - 0.01 * SCALE).clamp(0.19 * SCALE, 0.32 * SCALE);
            let cr = Rect::new(r.x, r.bottom() - h, r.w, h);
            theme::card_rect(ui, cr, tag_accent(accent));
            theme::text_left(
                ui,
                cr.x + CARD_PAD,
                cr.y + 0.055 * SCALE,
                FS_SMALL,
                text(),
                &mtl!("preview-interrupted-content"),
                cr.w - CARD_PAD * 2.,
            );
            let bh = (h * 0.44).clamp(0.1 * SCALE, 0.135 * SCALE);
            let bw = ((cr.w - CARD_PAD * 2. - BAR_COL_GAP) / 2.).min(0.3 * SCALE);
            let by = cr.bottom() - bh - 0.012 * SCALE;
            let rr = Rect::new(cr.right() - CARD_PAD - bw, by, bw, bh);
            let ll = Rect::new(rr.x - BAR_COL_GAP - bw, by, bw, bh);
            let size = (bh * 3.4).clamp(0.22, FS_SMALL);
            theme::button(ui, &mut self.prompt_later, t, ll, mtl!("preview-not-now"), size, secondary(), text());
            theme::button(ui, &mut self.prompt_ready, t, rr, mtl!("preview-ready"), size, primary(accent), WHITE);
        }
    }



    /// 玩家列表行绘制（深紫面板上的圆角行）。
    fn render_users_list(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        let room = ctx.room;
        let ids = sorted_user_ids(room, ctx.me);
        let inner = r.feather(-PANEL_INSET);
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
        for i in ids.len()..self.user_rows.len() {
            self.user_rows[i].invalidate();
        }
    }

    /// 用户列表：头像 + 名字 + 徽标，房主可点行进入管理页。
    fn render_users(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        let room = ctx.room;
        let ids = sorted_user_ids(room, ctx.me);
        let caption = mtl!("mp-player-count", "n" => ids.len() as u64);
        theme::panel_caption(ui, r.x, r.y, r.w, &caption);
        let list = Rect::new(r.x, r.y + CAPTION_H, r.w, (r.h - CAPTION_H).max(0.06));
        theme::card_flat(ui, list, card_soft());
        let inner = list.feather(-PANEL_INSET);
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
        let caption = mtl!("mp-chat-caption");
        theme::panel_caption(ui, r.x, r.y, r.w, &caption);
        let input_h = if CHAT_ENABLED { (r.h * 0.26).clamp(0.11 * SCALE, 0.15 * SCALE) } else { 0. };
        let list = Rect::new(
            r.x,
            r.y + CAPTION_H,
            r.w,
            (r.h - CAPTION_H - if CHAT_ENABLED { input_h + 0.015 * SCALE } else { 0. }).max(0.05),
        );
        theme::card_flat(ui, list, card_soft());
        ui.scope(|ui| {
            ui.dx(list.x + PANEL_INSET);
            ui.dy(list.y + PANEL_INSET);
            ctx.messages.render(
                ui,
                Rect::new(0., 0., (list.w - PANEL_INSET * 2.).max(0.05), (list.h - PANEL_INSET * 2.).max(0.04)),
            );
        });
        if CHAT_ENABLED {
            let iy = r.bottom() - input_h;
            let send_w = (0.17f32 * SCALE).min(r.w * 0.28).max(0.1 * SCALE);
            let br = Rect::new(r.x, iy, (r.w - send_w - 0.015 * SCALE).max(0.14), input_h);
            let path = br.rounded(R_BTN);
            ui.fill_path(&path, card());
            ui.stroke_path(&path, STROKE_W, stroke());
            self.chat_btn.render_input(
                ui,
                br.feather(-0.01 * SCALE),
                t,
                ctx.chat_text,
                mtl!("chat-placeholder"),
                (input_h * 3.2).clamp(0.24, FS_BODY),
            );
            let sb = Rect::new(br.right() + 0.015 * SCALE, iy, send_w, input_h);
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
