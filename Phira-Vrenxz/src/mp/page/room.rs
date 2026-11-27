
use macroquad::prelude::*;
use phira_mp_common::{ClientRoomState, RoomState};
use prpr::{
    ext::{semi_white, RectExt, SafeTexture, ScaleType, BLACK_TEXTURE},
    judge::icon_index,
    ui::{DRectButton, Scroll, Ui},
};

use super::super::{
    messages::MessageLog,
    state::{sorted_user_ids, user_count},
    theme::{self, *},
};
use crate::{dir, mp::L10N_LOCAL, page::Illustration, scene::Downloading};

pub const CHAT_ENABLED: bool = cfg!(feature = "chat");

const STRIP_H: f32 = 0.14 * SCALE;

const USER_ROW_H: f32 = 0.13 * SCALE;
const USER_ROW_GAP: f32 = 0.015 * SCALE;

const P_SLOPE: f32 = 0.14;

const DX0: f32 = 3.37958;
const DY0: f32 = 46.99935;
const DW: f32 = 472.83954;
const DH: f32 = 266.96377;

const BASE_DROP: f32 = 0.388;
const BASE_DROP_BOLD: f32 = 0.380;

const PANEL_LEFT: f32 = 3.38;
const PANEL_TOP: f32 = 47.38;

const PANEL_BOTTOM: f32 = 313.96;
const PANEL_RIGHT_TOP: f32 = 291.70;
const PANEL_RIGHT_BOTTOM: f32 = 261.32;

const STRIP_LEFT: f32 = 4.21;
const STRIP_RIGHT_TOP: f32 = 292.36;
const STRIP_BOTTOM: f32 = 78.34;

const BTN_X0: f32 = 27.62;
const BTN_TOP: f32 = 52.40;
const BTN_BOTTOM: f32 = 73.20;
const BTN_GAP: f32 = 2.90;
const BTN_SLOPE: f32 = 0.268;
const BTN_PAD: f32 = 8.0;
const BTN_MIN_W: f32 = 42.0;
const BTN_FS: f32 = 11.9;

const TAIL_SCALE: f32 = 0.80;

const BACK_X: f32 = 6.0;
const BACK_SCALE: f32 = 0.82;

const TITLE_X: f32 = 8.14;
const TITLE_BASE_Y: f32 = 93.86;
const TITLE_FS: f32 = 19.32;
const SUB_X: f32 = 9.15;
const SUB_BASE_Y: f32 = 105.81;
const SUB_FS: f32 = 8.66;

const ILLU_VEIL_BOTTOM: f32 = 132.0;

#[allow(dead_code)]
const CARD_TL: (f32, f32) = (4.35, 253.67);
#[allow(dead_code)]
const CARD_TR: (f32, f32) = (175.07, 253.36);
#[allow(dead_code)]
const CARD_BR: (f32, f32) = (165.66, 312.41);
#[allow(dead_code)]
const CARD_BL: (f32, f32) = (-13.85, 312.18);

const SCORE_ICON: (f32, f32, f32, f32) = (14.53, 267.37, 26.75, 32.26);

const SCORE_BADGE: (f32, f32, f32, f32) = (48.80, 258.10, 21.59, 15.19);

const SCORE_ID: (f32, f32, f32) = (74.09, 271.41, 16.06);

const SCORE_NUM: (f32, f32, f32) = (42.60, 293.20, 24.75);

const SCORE_ACC: (f32, f32, f32) = (43.01, 305.98, 9.15);

const PLAY_BOX: (f32, f32, f32, f32) = (185.51, 253.81, 81.91, 59.14);

const PLAY_QUAD: [(f32, f32); 4] = [(194.70, 253.81), (267.42, 253.81), (185.51, 312.95), (260.23, 312.95)];
const PLAY_TRI: (f32, f32, f32, f32) = (212.72, 268.56, 29.27, 31.24);
const PLAY_TRI_INNER: f32 = 0.85;
const PLAY_LABEL_BASE_Y: f32 = 266.00;
const PLAY_LABEL_FS: f32 = 9.00;

const R_TITLE: (f32, f32, f32) = (293.79, 68.01, 19.32);

const R_RIGHT: f32 = 474.38;
const R_COUNT_FS: f32 = 11.40;

const R_ROW_CY: f32 = 87.19;
const R_ROW_H: f32 = 30.00;
const R_ROW_GAP: f32 = 3.00;

const R_X: f32 = 277.52;

const R_SPLIT_Y: f32 = 177.47;

const R_INPUT_TOP: f32 = 275.02;
const R_INPUT_BOTTOM: f32 = 313.96;
const R_INPUT_RIGHT: f32 = 420.90;
const R_INPUT_FS: f32 = 10.50;

const R_INPUT_SLOPE: f32 = 0.1231;

const R_SEND_X: f32 = 423.65;
const R_SEND_RIGHT: f32 = 475.79;

const R_SEND_QUAD: [(f32, f32); 4] = [(428.44, 275.02), (475.34, 275.02), (423.65, 313.96), (475.79, 313.96)];
const R_SEND_FS: f32 = 14.30;

const BG_TOP: Color = Color::new(0.365, 0.365, 0.365, 1.);

const BG_BOTTOM: Color = Color::new(0.722, 0.722, 0.722, 1.);
const PANEL_TOP_C: Color = Color::new(0.533, 0.533, 0.533, 1.);
const PANEL_BOTTOM_C: Color = Color::new(1., 1., 1., 1.);

const STRIP_C: Color = Color::new(0., 0., 0., 0.72);
const BTN_FILL: Color = Color::new(0.702, 0.702, 0.702, 1.);
const SEND_FILL: Color = Color::new(0.404, 0.404, 0.404, 1.);
const ON_PANEL: Color = Color::new(0.06, 0.06, 0.08, 1.);
const ON_PANEL_DIM: Color = Color::new(0.25, 0.25, 0.28, 1.);
const ON_PANEL_ID: Color = Color::new(0.42, 0.42, 0.42, 1.);
const CHAT_VEIL: Color = Color::new(0., 0., 0., 0.28);

#[derive(Clone, Copy)]
struct Design {

    k: f32,

    ox: f32,
    oy: f32,
}

impl Design {
    fn new(top: f32) -> Self {
        let kx = 2. / DW;
        let ky = 2. * top / DH;
        let k = if kx < ky { kx } else { ky };

        Self {
            k,
            ox: -1. + (2. - DW * k) * 0.5,
            oy: -top + (2. * top - DH * k) * 0.5,
        }
    }

    fn x(&self, px: f32) -> f32 {
        self.ox + (px - DX0) * self.k
    }

    fn y(&self, py: f32) -> f32 {
        self.oy + (py - DY0) * self.k
    }

    fn w(&self, px: f32) -> f32 {
        px * self.k
    }

    fn h(&self, py: f32) -> f32 {
        py * self.k
    }

    fn fs(&self, px: f32) -> f32 {
        px * self.k / 0.08
    }

    fn slope(&self, s: f32) -> f32 {
        let _ = self;
        s
    }

    fn base(&self, py: f32, fpx: f32) -> f32 {
        self.y(py) - BASE_DROP * 0.08 * self.fs(fpx)
    }

    fn base_bold(&self, py: f32, fpx: f32) -> f32 {
        self.y(py) - BASE_DROP_BOLD * 0.08 * self.fs(fpx)
    }

    fn px(&self, w: f32) -> f32 {
        w / self.k
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomAction {

    Start,

    LockRoom,

    CycleRoom,

    Password,

    Ready,

    CancelReady,

    CancelLocalShare,

    CancelDownload,

    Preview,

    Library,

    LeaveRoom,
}

impl RoomAction {
    pub const ALL: [RoomAction; 11] = [
        RoomAction::Start,
        RoomAction::LockRoom,
        RoomAction::CycleRoom,
        RoomAction::Password,
        RoomAction::Ready,
        RoomAction::CancelReady,
        RoomAction::CancelLocalShare,
        RoomAction::CancelDownload,
        RoomAction::Preview,
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
            RoomAction::Library => 9,
            RoomAction::LeaveRoom => 10,
        }
    }

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
            RoomAction::Library => I::Library,
            RoomAction::LeaveRoom => I::LeaveRoom,
        }
    }
}

pub struct ActItem {
    pub action: RoomAction,
    pub label: String,

    pub disabled: bool,
}

#[derive(Default, Clone)]
pub struct RoomView {

    pub local_chart: Option<(String, String)>,

    pub local_ready: bool,

    pub host_started: bool,

    pub pending_download: bool,

    pub syncing: bool,

    pub chart_id: Option<i32>,

    pub chart_name: Option<String>,
}

pub fn manage_allowed(room: &ClientRoomState) -> bool {
    room.is_host && matches!(room.state, RoomState::SelectChart(_) | RoomState::LocalChart)
}

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

fn short_label(action: RoomAction) -> &'static str {
    use RoomAction as A;
    match action {
        A::LockRoom => "锁",
        A::CycleRoom => "循",
        A::Password => "密",
        // 「离开房间」单独给一个字：以前和取消类一起用 ✕，一排 ✕ 分不清哪个是哪个
        A::LeaveRoom => "退",
        A::CancelReady | A::CancelDownload | A::CancelLocalShare => "✕",
        A::Preview => "览",
        A::Library => "库",
        A::Start | A::Ready => "▶",
    }
}

/// 依据房间状态推导功能按钮（渲染与触摸共用同一集合）。
///
/// disabled 的项**照常参与排布**（位置和房主看到的完全一致），只是渲染成暗色、
/// 并且不登记命中区：用户要求"不满足开始条件时不要把开始按钮藏起来，
/// 而是改成变暗 + 不可交互"，藏起来的话玩家只会以为这里本来就没有按钮。
pub fn action_items(room: &ClientRoomState, view: &RoomView) -> Vec<ActItem> {
    let mut items = Vec::new();
    let is_host = room.is_host;
    match room.state {
        RoomState::SelectChart(_) => {
            if is_host {
                items.push(ActItem {
                    action: RoomAction::Start,
                    label: mtl!("request-start").into_owned(),
                    disabled: false,
                });
                push_room_settings(&mut items, room);
            } else {
                // 非房主（观战功能已删，这里就是座位上的普通玩家）没有开始权限：
                // 以前直接把「开始」从集合里删掉、按钮位空着，现在用禁用态占住原位。
                items.push(ActItem {
                    action: RoomAction::Start,
                    label: mtl!("request-start").into_owned(),
                    disabled: true,
                });
            }
            // 「谱面库」：选谱阶段才有意义（在线谱 / 本地谱都是在这儿选）
            if is_host {
                items.push(ActItem {
                    action: RoomAction::Library,
                    label: mtl!("mp-library").into_owned(),
                    disabled: false,
                });
            }
        }
        RoomState::LocalChart => {
            if is_host {
                if view.host_started {
                    items.push(ActItem {
                        action: RoomAction::CancelLocalShare,
                        label: mtl!("cancel-ready").into_owned(),
                        disabled: false,
                    });
                } else {
                    items.push(ActItem {
                        action: RoomAction::Start,
                        label: mtl!("request-start").into_owned(),
                        disabled: false,
                    });
                }
                push_room_settings(&mut items, room);
            } else if view.local_ready {
                items.push(ActItem {
                    action: RoomAction::CancelDownload,
                    label: mtl!("cancel-ready").into_owned(),
                    disabled: false,
                });
            } else if view.pending_download && !view.syncing {
                items.push(ActItem {
                    action: RoomAction::Ready,
                    label: mtl!("ready").into_owned(),
                    disabled: false,
                });
            } else {
                // 还没就绪（谱面同步中 / 服务端还没让下载）：同理，禁用态的开始按钮
                items.push(ActItem {
                    action: RoomAction::Start,
                    label: mtl!("request-start").into_owned(),
                    disabled: true,
                });
            }
        }
        RoomState::WaitingForReady => {
            if room.is_ready {
                items.push(ActItem {
                    action: RoomAction::CancelReady,
                    label: mtl!("cancel-ready").into_owned(),
                    disabled: false,
                });
            } else {
                items.push(ActItem {
                    action: RoomAction::Ready,
                    label: mtl!("ready").into_owned(),
                    disabled: false,
                });
            }
        }
        RoomState::Playing => {}
    }
    if is_previewable(room, view) {
        items.push(ActItem {
            action: RoomAction::Preview,
            label: mtl!("preview").into_owned(),
            disabled: false,
        });
    }
    // 离开房间：放在最后（最右边），跟「开始游戏」那种主动作分开
    items.push(ActItem {
        action: RoomAction::LeaveRoom,
        label: mtl!("leave-room").into_owned(),
        disabled: false,
    });
    items
}

fn push_room_settings(items: &mut Vec<ActItem>, room: &ClientRoomState) {
    items.push(ActItem {
        action: RoomAction::LockRoom,
        label: mtl!("lock-room", "current" => room.locked.to_string()),
        disabled: false,
    });
    items.push(ActItem {
        action: RoomAction::CycleRoom,
        label: mtl!("cycle-room", "current" => room.cycle.to_string()),
        disabled: false,
    });
    items.push(ActItem {
        action: RoomAction::Password,
        label: mtl!("set-password").into_owned(),
        disabled: false,
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
    /// 房间号（房名 = 房间 #<id>）
    pub room_id: Option<&'a str>,
    pub view: &'a RoomView,
    pub messages: &'a mut MessageLog,
    /// 服务端系统日志（与聊天分开显示在中间下方那块）
    pub log: &'a mut MessageLog,
    pub chat_text: &'a mut String,
    /// 在线谱面下载中
    pub download: Option<&'a mut Downloading>,

    pub syncing: bool,

    pub busy: bool,

    pub prompt: bool,

    pub icon: &'a SafeTexture,
    /// 自己的用户 id
    pub me: Option<i32>,
    /// 自己是否已就绪（协议只提供自己的就绪状态）
    pub me_ready: bool,
}

#[derive(Default)]
pub struct RoomPage {
    back: DRectButton,
    /// 聊天输入框是否处于编辑态（Windows 上由原生输入点接管键盘）
    pub chat_focus: bool,
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
    /// 左面板中部的曲绘（按当前谱面 id 懒加载；加载好之前不画）
    illu: Option<Illustration>,
    /// 上面那张图对应的谱面 id
    illu_key: Option<i32>,

    // ---- 以下都是「每帧重算太贵」的东西的缓存（见 RoomPage::refresh）----
    /// 功能按钮集合（action_items 里每个标签都要走一次 Fluent mtl!）
    item_cache: Vec<ActItem>,
    /// 上面那份按钮集合对应的房间/展示状态
    item_key: Option<ActionKey>,
    /// 本地谱面缓存的键：(房间阶段, 谱面 id)。
    ///
    /// 带上房间阶段是必须的：打完一局回到「等待就绪」时，本地最好成绩会变，
    /// 只按谱面 id 缓存的话成绩卡会一直显示上一局的旧成绩。
    chart_key: Option<(u8, Option<i32>)>,
    /// 本地最好成绩 (成绩, 准确率, 全连)
    chart_record: Option<(i32, f32, bool)>,
    /// 等级牌文字（谱面自己的等级串，如 "IN  Lv.13"）
    chart_badge: String,
    /// 本地谱面目录（曲绘从这里加载）
    chart_path: Option<String>,
    /// 顶栏按钮量出来的文字宽度：(标签, 字号, 世界宽度)
    width_cache: Vec<(String, u32, f32)>,
}

/// 功能按钮集合的缓存键：房间阶段 + 展示状态，任一变化才重算按钮集合。
///
/// 不缓存的话每帧都要重跑一遍 action_items：里面每个按钮标签都走一次 Fluent 的 mtl!
/// （查表 + 格式化 + 分配字符串），十几个按钮就是每帧十几次，低端机上这一项比整个绘制还贵。
#[derive(PartialEq, Clone, Copy)]
struct ActionKey {
    state: u8,
    chart_id: Option<i32>,
    has_local_chart: bool,
    is_host: bool,
    locked: bool,
    cycle: bool,
    is_ready: bool,
    local_ready: bool,
    host_started: bool,
    pending_download: bool,
    syncing: bool,
}

impl ActionKey {
    fn of(room: &ClientRoomState, view: &RoomView) -> Self {
        Self {
            state: match &room.state {
                RoomState::SelectChart(None) => 0,
                RoomState::SelectChart(Some(_)) => 1,
                RoomState::LocalChart => 2,
                RoomState::WaitingForReady => 3,
                RoomState::Playing => 4,
            },
            chart_id: view.chart_id,
            has_local_chart: view.local_chart.is_some(),
            is_host: room.is_host,
            locked: room.locked,
            cycle: room.cycle,
            is_ready: room.is_ready,
            local_ready: view.local_ready,
            host_started: view.host_started,
            pending_download: view.pending_download,
            syncing: view.syncing,
        }
    }
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

/// 房间当前谱面的本地信息：最好成绩 (成绩, 准确率, 全连) / 等级牌文字 / 本地目录。
///
/// **查找口径必须和"引擎写成绩"的口径一致**，否则打出来的成绩就查不到：
/// 引擎的 save_fn（scene/song.rs 尾部）是「先按 local_path 找本地谱面库条目，
/// 找不到才落到 local_records[local_path]」，选曲页也是这么查的（song.rs:538）。
/// 房间页以前只按 info.id 在 charts[] 里找，于是
/// **本地谱面分享的房间（本地路径是 download/{uuid}、成绩落在 local_records）
/// 永远查不到成绩**，评级位就一直是那张默认的 F —— 这正是"显示 F"的根因。
fn local_chart(view: &RoomView) -> Option<(Option<(i32, f32, bool)>, String, String)> {
    let data = crate::get_data();
    // 房间当前谱面在本地磁盘上的路径：在线谱 download/{id}，本地谱分享 download/{uuid}
    let path = match (view.chart_id, view.local_chart.as_ref()) {
        (Some(id), _) => format!("download/{id}"),
        (None, Some((uuid, _))) => format!("download/{uuid}"),
        (None, None) => return None,
    };
    // 本地谱面库条目：等级牌 / 曲绘从这里取（本地谱分享的 uuid 常常查不到，那就只有成绩）
    let entry = data
        .charts
        .iter()
        .find(|c| c.local_path == path)
        // 兜底：老数据里条目可能没写 local_path，只能按谱面 id 找
        .or_else(|| view.chart_id.and_then(|id| data.charts.iter().find(|c| c.info.id == Some(id))));
    // 最好成绩：与引擎同序 —— 先本地谱面库条目，再 local_records
    let record = entry
        .and_then(|c| c.record.as_ref())
        .map(|r| (r.score, r.accuracy, r.full_combo))
        .or_else(|| {
            data.local_records
                .get(&path)
                .and_then(|r| r.as_ref())
                .map(|r| (r.score, r.accuracy, r.full_combo))
        });
    Some((
        record,
        entry.map(|c| c.info.level.clone()).unwrap_or_default(),
        entry.map(|c| c.local_path.clone()).unwrap_or(path),
    ))
}

/// 左栏信息块的「自然高度」（竖屏用）。
fn info_height(ctx: &Render) -> f32 {
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
        for b in &mut self.user_rows {
            b.invalidate();
        }
    }

    pub fn update(&mut self, t: f32) {
        self.user_scroll.update(t);
        // 曲绘是异步加载的，加载完在这里收口（渲染线程不能建纹理）
        if let Some(illu) = &mut self.illu {
            illu.settle(t);
        }
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, mut ctx: Render) {
        let accent = ui.accent();
        self.refresh(ctx.room, ctx.view);
        // 把缓存的按钮集合「借出来」用一帧再放回去：render_* 里还要 &mut self 取按钮池，
        // 直接借用会撞借用检查器，而每帧 clone 一份又是实打实的分配。
        let items = std::mem::take(&mut self.item_cache);
        if !theme::is_wide(ui) {
            // 竖屏：保留原卡片堆叠布局
            self.render_portrait(ui, t, ctx, &items);
        } else {
            self.render_landscape(ui, t, &mut ctx, &items, accent);
        }
        self.item_cache = items;
    }

    /// 每帧开头刷新缓存：**只有房间状态 / 当前谱面真的变了才重算**。
    ///
    /// 这些东西每帧重算一次的代价都不小（Fluent 查表、本地谱面库线性扫描 + format!、
    /// 起一次图片加载任务），放在 60/120Hz 的渲染循环里就是「容易卡 UI」的来源。
    fn refresh(&mut self, room: &ClientRoomState, view: &RoomView) {
        let key = ActionKey::of(room, view);
        if self.item_key != Some(key) {
            self.item_key = Some(key);
            self.item_cache = action_items(room, view);
        }

        // 当前谱面（本地谱面库里查最好成绩 / 等级牌 / 目录）
        let chart_key = (key.state, view.chart_id);
        if self.chart_key != Some(chart_key) {
            self.chart_key = Some(chart_key);
            match local_chart(view) {
                Some((record, badge, path)) => {
                    self.chart_record = record;
                    self.chart_badge = badge;
                    self.chart_path = Some(path);
                }
                None => {
                    self.chart_record = None;
                    self.chart_badge.clear();
                    self.chart_path = None;
                }
            }
            // 换谱就换图：把上一张曲绘丢掉，下面重新开一次加载
            self.illu_key = None;
            self.illu = None;
        }

        // 曲绘：只有换谱面时才重新起任务（本地谱面库里查得到才有图）
        if self.illu_key != view.chart_id {
            self.illu_key = view.chart_id;
            if let Some(path) = self.chart_path.clone() {
                // full = true：曲绘要铺满整个左面板，缩略图（347×200）撑不满，放大就是糊的
                let illu = crate::page::local_illustration(path, BLACK_TEXTURE.clone(), true);
                // 房间页只要缩略图，立刻放行，别等「页面切换完成」的优先级门闩
                illu.notify();
                self.illu = Some(illu);
            }
        }
    }

    /// 量一次标签宽度就记住：顶栏按钮每帧各 measure 一次是纯浪费（文字布局不便宜）。
    fn label_w(&mut self, ui: &mut Ui, label: &str, size: f32) -> f32 {
        let bits = size.to_bits();
        if let Some((_, _, w)) = self.width_cache.iter().find(|(l, s, _)| l == label && *s == bits) {
            return *w;
        }
        let w = ui.text(label).size(size).measure().w;
        if self.width_cache.len() >= 32 {
            self.width_cache.clear();
        }
        self.width_cache.push((label.to_owned(), bits, w));
        w
    }

    // ==================== 横屏：严格照设计稿 ====================

    fn render_landscape(&mut self, ui: &mut Ui, t: f32, ctx: &mut Render, items: &[ActItem], accent: Color) {
        let top = theme::layout_top(ui);
        let a = ui.alpha;
        let m = 0.03;

        // —— 背景：没选谱面时纯色；选了谱面后铺它曲绘的高斯模糊图 ——
        let mut bg_done = false;
        if let Some(illu) = &self.illu {
            let ia = illu.alpha(t);
            if ia > 0.02 && (ctx.view.chart_id.is_some() || ctx.view.local_chart.is_some()) {
                let blur = illu.blurred.lock().unwrap().clone();
                let tex = blur.as_ref().unwrap_or(&illu.texture.0);
                cover_quad(
                    [
                        Vec2::new(-1., -top),
                        Vec2::new(1., -top),
                        Vec2::new(-1., top),
                        Vec2::new(1., top),
                    ],
                    tex,
                    ia * a,
                );
                // 压一层暗纱，保证面板上的字读得清
                ui.fill_rect(ui.screen_rect(), Color::new(0., 0., 0., 0.35 * ia * a));
                bg_done = true;
            }
        }
        if !bg_done {
            theme::slant_panel_grad(ui, -1., -top, 2., 2. * top, 0., fade(BG_TOP, a), fade(BG_BOTTOM, a));
        }

        // ---------- 顶部 tab 栏 ----------
        let bar = Rect::new(-1. + m, -top + 0.025, 2. - m * 2., 0.1);
        theme::card_flat(ui, bar, fade(STRIP_C, a));

        let back_s = (bar.h * 0.8).min(0.072);
        theme::back_button(
            ui,
            &mut self.back,
            t,
            Rect::new(bar.x + 0.012, bar.center().y - back_s * 0.5, back_s, back_s),
        );

        let title = match ctx.room_id {
            Some(id) => mtl!("mp-room-tag", "id" => id.to_owned()),
            None => mtl!("multiplayer").into_owned(),
        };
        let title_x = bar.x + back_s + 0.036;
        let chip_sz = FS_TAG;

        let main = items
            .iter()
            .find(|it| matches!(it.action, RoomAction::Start | RoomAction::Ready | RoomAction::CancelReady));
        let chips: Vec<&ActItem> = items
            .iter()
            .filter(|it| main.is_none_or(|m2| m2.action != it.action))
            .collect();

        let mut widths: Vec<f32> = chips
            .iter()
            .map(|it| {
                let tw = self.label_w(ui, short_label(it.action), chip_sz);
                (tw + 0.045).max(0.085)
            })
            .collect();
        let gap = 0.012;
        let gaps = gap * chips.len().saturating_sub(1) as f32;
        let chips_right = bar.right() - 0.012;
        let title_w = ui.text(&title).size(FS_SECTION).measure().w;
        let title_reserve = (title_w + 0.08).min(bar.w * 0.32);
        let avail = chips_right - (title_x + title_reserve) - gaps;
        let total: f32 = widths.iter().sum();
        if total > avail && avail > 0.02 && !widths.is_empty() {
            let k = avail / total;
            for w in &mut widths {
                *w = (*w * k).max(0.055);
            }
        }
        let chip_h = bar.h * 0.64;
        let mut cx = chips_right;
        for (i, item) in chips.iter().enumerate().rev() {
            let w = widths[i];
            cx -= w;
            if cx + w * 0.5 < title_x + title_reserve {
                break;
            }
            let r = Rect::new(cx, bar.center().y - chip_h * 0.5, w, chip_h);
            let label = short_label(item.action);
            if item.disabled {
                self.actions.get(item.action).invalidate();
                theme::button_static(
                    ui,
                    r,
                    label,
                    chip_sz,
                    Color::new(0.30, 0.30, 0.33, 1.),
                    Color::new(0.62, 0.62, 0.66, 1.),
                );
            } else {
                theme::button(ui, self.actions.get(item.action), t, r, label, chip_sz, secondary(), text());
            }
            cx -= gap;
        }
        theme::text_left_bold(ui, title_x, bar.center().y, FS_SECTION, semi_white(1.), &title, title_reserve);

        let content_top = bar.bottom() + 0.03;
        let content_bottom = top - m;

        // ---------- 左边：谱面信息 ----------
        let left = Rect::new(-1. + m, content_top, 0.62, (content_bottom - content_top).max(0.24));
        theme::slant_panel_grad(
            ui,
            left.x,
            left.y,
            left.w,
            left.h,
            0.,
            fade(PANEL_TOP_C, a),
            fade(PANEL_BOTTOM_C, a),
        );
        if let Some(illu) = &self.illu {
            let ia = illu.alpha(t);
            if ia > 0.02 {
                cover_quad(
                    [
                        Vec2::new(left.x, left.y),
                        Vec2::new(left.right(), left.y),
                        Vec2::new(left.x, left.bottom()),
                        Vec2::new(left.right(), left.bottom()),
                    ],
                    &illu.texture.1,
                    ia * a,
                );
                // 顶部一段白纱，压在曲绘上让谱名读得清
                let band = (left.h * 0.32).min(0.32);
                theme::slant_panel_grad(
                    ui,
                    left.x,
                    left.y,
                    left.w,
                    band,
                    0.,
                    fade(semi_white(0.82 * ia), a),
                    fade(Color::new(1., 1., 1., 0.), a),
                );
            }
        }
        let pad = 0.032;
        let (state_text, chart_name) = chart_parts(ctx.room, ctx.view);
        let name = match &chart_name {
            Some(n) if !n.is_empty() => n.clone(),
            _ => mtl!("multiplayer").into_owned(),
        };
        theme::text_left_bold(
            ui,
            left.x + pad,
            left.y + 0.055,
            FS_SECTION,
            ON_PANEL,
            &name,
            left.w - pad * 2.,
        );
        if !self.chart_badge.is_empty() {
            theme::text_right(
                ui,
                left.right() - pad,
                left.y + 0.052,
                FS_SMALL,
                ON_PANEL_ID,
                &self.chart_badge,
                left.w * 0.42,
            );
        }
        theme::text_left(
            ui,
            left.x + pad,
            left.y + 0.098,
            FS_SMALL,
            ON_PANEL_DIM,
            &state_text,
            left.w - pad * 2.,
        );

        if let Some(dl) = ctx.download.as_deref_mut() {
            let dr = Rect::new(left.x + pad, left.y + 0.125, left.w - pad * 2., 0.085);
            theme::card_rect(ui, dr, card());
            dl.render_inline(ui, dr, t);
        } else if ctx.syncing || ctx.busy {
            let pb = Rect::new(left.x + pad, left.y + 0.14, left.w * 0.66, 0.007);
            theme::progress_bar(ui, pb, None, t, accent);
        }

        // 本地最好成绩（评级图 + 分数 + 准确率），贴在面板底部
        let (score, accuracy, full_combo) = self.chart_record.unwrap_or((0, 0., false));
        theme::h_line(ui, left.x + pad, left.bottom() - 0.145, left.w - pad * 2.);
        if score > 0 {
            if let Some(icons) = crate::scene::TEX_RANK_ICONS.with(|it| it.borrow().clone()) {
                let r = Rect::new(left.x + pad, left.bottom() - 0.125, 0.08, 0.08);
                ui.fill_path(&r.rounded(R_ROW), Color::new(0., 0., 0., 0.18));
                let idx = icon_index(score as u32, full_combo);
                ui.fill_rect(r, (*icons[idx], r, ScaleType::Fit));
            }
        }
        theme::text_left_bold(
            ui,
            left.x + pad + 0.1,
            left.bottom() - 0.068,
            FS_BODY,
            ON_PANEL,
            &format!("{score:07}"),
            left.w - pad * 2. - 0.11,
        );
        theme::text_left(
            ui,
            left.x + pad + 0.1,
            left.bottom() - 0.098,
            FS_SMALL,
            ON_PANEL_DIM,
            &format!("{:.2}%", accuracy * 100.),
            left.w - pad * 2. - 0.11,
        );

        // ---------- 中间：聊天栏 + 日志 ----------
        let mid_x = -1. + m + 0.65;
        let mid_w = 0.6;
        let input_h = 0.115;
        let start_h = 0.135;
        let bottom_y = content_bottom;
        let input_y = bottom_y - input_h;
        let log_h = 0.19;
        let log_y = input_y - 0.03 - log_h;
        let chat_y = content_top;
        let chat_h = (log_y - 0.03 - chat_y).max(0.18);

        let chat = Rect::new(mid_x, chat_y, mid_w, chat_h);
        theme::panel_caption(ui, chat.x, chat.y, chat.w, &mtl!("mp-chat-title"));
        let chat_list = Rect::new(chat.x, chat.y + CAPTION_H, chat.w, (chat.h - CAPTION_H).max(0.05));
        let chat_inner = theme::parallelogram(ui, chat_list, 0., fade(CHAT_VEIL, a), fade(CHAT_VEIL, a));
        ui.scope(|ui| {
            ui.dx(chat_inner.x + 0.006);
            ui.dy(chat_inner.y + 0.006);
            ctx.messages.render(
                ui,
                Rect::new(0., 0., (chat_inner.w - 0.012).max(0.05), (chat_inner.h - 0.012).max(0.05)),
            );
        });

        let log = Rect::new(mid_x, log_y, mid_w, log_h);
        theme::panel_caption(ui, log.x, log.y, log.w, &mtl!("mp-log-title"));
        let log_list = Rect::new(log.x, log.y + CAPTION_H, log.w, (log.h - CAPTION_H).max(0.05));
        let log_inner = theme::parallelogram(
            ui,
            log_list,
            0.,
            fade(Color::new(0., 0., 0., 0.22), a),
            fade(Color::new(0., 0., 0., 0.22), a),
        );
        ui.scope(|ui| {
            ui.dx(log_inner.x + 0.006);
            ui.dy(log_inner.y + 0.006);
            ctx.log.render(
                ui,
                Rect::new(0., 0., (log_inner.w - 0.012).max(0.05), (log_inner.h - 0.012).max(0.05)),
            );
        });

        // 输入行（最底部）
        let send_w = 0.115;
        let input = Rect::new(mid_x, input_y, (mid_w - send_w - 0.02).max(0.1), input_h);
        let send = Rect::new(mid_x + mid_w - send_w, input_y, send_w, input_h);
        theme::card_rect(ui, input, card());
        self.chat_btn.render_input(
            ui,
            input.feather(-0.012),
            t,
            &*ctx.chat_text,
            mtl!("chat-placeholder"),
            FS_BODY,
        );
        #[cfg(target_os = "windows")]
        if self.chat_focus {
            crate::native_edit::edit("mp_chat", ctx.chat_text, true, false);
            let sel = crate::native_edit::selection("mp_chat");
            crate::native_edit::draw_caret_and_selection(
                ui,
                input.feather(-0.012),
                &*ctx.chat_text,
                false,
                sel,
                FS_BODY,
                t,
            );
        }
        theme::button(ui, &mut self.chat_send_btn, t, send, mtl!("chat-send"), FS_BUTTON, primary(accent), WHITE);

        // ---------- 右边：玩家列表 ----------
        let right_w = 0.6;
        let right_x = 1. - m - right_w;
        let users = Rect::new(
            right_x,
            content_top,
            right_w,
            (bottom_y - start_h - 0.03 - content_top).max(0.16),
        );
        theme::panel_caption(
            ui,
            users.x,
            users.y,
            users.w,
            &mtl!("mp-player-count", "n" => user_count(ctx.room) as u64),
        );
        let list = Rect::new(users.x, users.y + CAPTION_H, users.w, (users.h - CAPTION_H).max(0.06));
        let list_inner = theme::parallelogram(ui, list, 0., fade(card_soft(), a), fade(card_soft(), a));
        self.render_user_rows(ui, t, list_inner, ctx, accent);

        // ---------- 右下角：开始 / 准备 ----------
        if let Some(item) = main {
            let r = Rect::new(right_x, bottom_y - start_h, right_w, start_h);
            let label = item.label.clone();
            if item.disabled {
                self.actions.get(item.action).invalidate();
                theme::button_static(
                    ui,
                    r,
                    label,
                    FS_BUTTON,
                    Color::new(0.32, 0.32, 0.35, 0.9),
                    Color::new(0.6, 0.6, 0.64, 1.),
                );
            } else {
                theme::button(ui, self.actions.get(item.action), t, r, label, FS_BUTTON, primary(accent), WHITE);
            }
        }

        // ---------- 内联确认条：房主要开始了 ----------
        if ctx.prompt {
            let h = 0.13;
            let cr = Rect::new(mid_x, log_y + 0.03, mid_w, h);
            theme::card_rect(ui, cr, tag_accent(accent));
            theme::text_left(
                ui,
                cr.x + 0.022,
                cr.y + h * 0.34,
                FS_SMALL,
                text(),
                &mtl!("preview-interrupted-content"),
                cr.w - 0.044,
            );
            let bh = h * 0.32;
            let bw = 0.18f32.min((cr.w - 0.044) / 2.2);
            let by = cr.bottom() - bh - 0.012;
            let rr = Rect::new(cr.right() - 0.022 - bw, by, bw, bh);
            let ll = Rect::new(rr.x - 0.02 - bw, by, bw, bh);
            theme::button(ui, &mut self.prompt_later, t, ll, mtl!("preview-not-now"), FS_TAG, secondary(), text());
            theme::button(ui, &mut self.prompt_ready, t, rr, mtl!("preview-ready"), FS_TAG, primary(accent), WHITE);
        }
    }


    /// 左下角的最好成绩卡：等级图标 + 等级牌 + 谱面 ID + 最好成绩 + 准确率。
    ///
    /// 数值与画法都照 [crate::scene::song] 的选曲页：同一个 icon_index、
    /// 同一个 {:07} 成绩格式、同一个 {:.2}% 准确率。
    fn render_score_card(&mut self, ui: &mut Ui, ctx: &Render, d: &Design) {
        let a = ui.alpha;
        // —— 卡片本体 ——
        // 用户要求"把分数显示下面那条黑白渐变底部删掉"，所以**这里只留内容**：
        // 稿子那块 #888 → #fff 的横向渐变底、以及只为它勾边的三道黑描边一起删掉了。
        // （成绩 / 准确率 / #ID 仍然画在原来的位置，压在下方的浅色面板上。）

        // 用 refresh() 缓存好的那份（每帧去扫本地谱面库 + format! 太浪费）
        let record = self.chart_record;
        let badge = self.chart_badge.as_str();

        // —— 成绩三件套：**没有记录也照画** ——
        // 选曲页（scene/song.rs）那边 record 为 None 时是 score = 0 / accuracy = 0，
        // 这里以前没记录就留白 + 破折号，看着像坏了，改成和选曲页一致。
        let (score, accuracy, full_combo) = match record {
            Some((s, a, f)) => (s.max(0), a, f),
            None => (0, 0., false),
        };

        // —— 评级位（稿子 14.53,267.37 起 26.75×32.26）——
        // 画的必须是**玩家自己打出来的评级**：就是 assets/rank/ 里那套图，
        // 档位换算由 prpr::judge::icon_index 给出（F/C/B/A/S/V/FC/phi）。
        //
        // 判据是 score > 0：
        //   * 有成绩 → 画对应档位的图（成绩卡上的评级从此跟着自己的成绩走）；
        //   * 没成绩 / 这局 0 分 → **留空**。理由：icon_index(0, _) 恰好等于 0，
        //     也就是 rank/F.png，于是"没打过"和"打了 0 分"都会被画成大写 F，
        //     看着像随手乱画的字母（用户要的正是"F 只出现在真的有 F 档时"）。
        //     真正的 F 档（1..699999）仍然会画出 F。
        let (ix, iy, iw, ih) = SCORE_ICON;
        let icon_r = Rect::new(d.x(ix), d.y(iy), d.w(iw), d.h(ih));
        if score > 0 {
            match crate::scene::TEX_RANK_ICONS.with(|it| it.borrow().clone()) {
                Some(icons) => {
                    let idx = icon_index(score as u32, full_combo);
                    // 垫一层半透明深色底：assets/rank/*.png 里 F/C/B/A/S/V 都是**白色**字，
                    // 而成绩卡的渐变底按第 1 条删掉之后，这个位置压在浅色面板（甚至压在白底曲绘）上，
                    // 白字几乎看不见 —— 只包住图标本身垫一块暗底，评级才真的"显示得出来"
                    // （不是把整块卡片底又加回来）。
                    ui.fill_path(&icon_r.rounded(R_ROW), Color::new(0., 0., 0., 0.30));
                    ui.fill_rect(icon_r, (*icons[idx], icon_r, ScaleType::Fit));
                }
                // 图标没装载成功：留一块淡底，至少位置还在
                None => ui.fill_path(&icon_r.rounded(R_ROW), Color::new(0., 0., 0., 0.10)),
            }
        }

        // —— 等级牌（稿子 48.80,258.10 起 21.59×15.19）——
        let (bx, by, bw, bh) = SCORE_BADGE;
        let badge_r = Rect::new(d.x(bx), d.y(by), d.w(bw), d.h(bh));
        if !badge.is_empty() {
            let fill = fade(Color::new(0.62, 0.64, 0.70, 1.), a);
            theme::skew_panel(ui, badge_r.x, badge_r.y, badge_r.w, badge_r.h, d.slope(0.2), fill, fill);
            ui.text(badge)
                .pos(badge_r.center().x, badge_r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(d.fs(7.2))
                .color(WHITE)
                .max_width(badge_r.w * 0.94)
                .draw();
        }

        // —— 谱面 ID（稿子 #7891）——
        let id_text = match ctx.view.chart_id {
            Some(id) => format!("#{id}"),
            None => match &ctx.view.local_chart {
                Some((_, n)) if !n.is_empty() => n.clone(),
                _ => mtl!("mp-state-local").into_owned(),
            },
        };
        theme::text_left(
            ui,
            d.x(SCORE_ID.0),
            d.base(SCORE_ID.1, SCORE_ID.2),
            d.fs(SCORE_ID.2),
            ON_PANEL_ID,
            &id_text,
            d.x(CARD_BR.0) - d.x(SCORE_ID.0) - 0.01,
        );

        // —— 最好成绩：稿子里是大号黑数字（选曲页用的是同一个 {:07}）——
        let score_text = format!("{score:07}");
        theme::text_left_bold(
            ui,
            d.x(SCORE_NUM.0),
            d.base_bold(SCORE_NUM.1, SCORE_NUM.2),
            d.fs(SCORE_NUM.2),
            Color::new(0.04, 0.04, 0.06, 1.),
            &score_text,
            d.x(CARD_BR.0) - d.x(SCORE_NUM.0),
        );

        // —— 准确率（选曲页同样是 {:.2}%，同样永远显示）——
        theme::text_left(
            ui,
            d.x(SCORE_ACC.0),
            d.base(SCORE_ACC.1, SCORE_ACC.2),
            d.fs(SCORE_ACC.2),
            Color::new(0.1, 0.1, 0.12, 1.),
            &format!("{:.2}%", accuracy * 100.),
            d.x(CARD_BR.0) - d.x(SCORE_ACC.0),
        );
    }

    /// 横屏右侧用户列表。行索引与 [RoomPage::user_ids] 严格对齐。
    fn render_user_rows(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        let room = ctx.room;
        let ids = sorted_user_ids(room, ctx.me);
        let step = row_step(theme::layout_top(ui));
        let manageable = manage_allowed(room);
        let (icon, me, me_ready) = (ctx.icon, ctx.me, ctx.me_ready);
        self.user_ids.clear();
        self.user_rows.resize_with(ids.len(), DRectButton::new);
        ui.scope(|ui| {
            ui.dx(r.x);
            ui.dy(r.y);
            self.user_scroll.size((r.w, r.h));
            self.user_scroll.render(ui, |ui| {
                for (i, &id) in ids.iter().enumerate() {
                    // 行索引与 user_ids 必须严格对齐：先记录 id，再看能不能画
                    self.user_ids.push(id);
                    let Some(user) = room.users.get(&id) else { continue };
                    let is_me = Some(id) == me;
                    let rr = Rect::new(0., i as f32 * step, r.w, step);
                    // 稿子里的用户行**没有底色**（头像 + 名字直接压在背景上），
                    // 所以这里不铺 theme::row_button 那种圆角行底 —— 它每行要建一条
                    // lyon 路径再描一次边，房里人多时（十几个玩家）每帧就是几十次
                    // 路径构造 + 三角化，是「容易卡 UI」的主要来源之一。
                    let btn = &mut self.user_rows[i];
                    btn.inner.set(ui, rr);
                    if is_me {
                        ui.fill_rect(rr, row_selected(accent));
                    }
                    theme::player_row_content(
                        ui,
                        rr,
                        t,
                        icon,
                        user.id,
                        &user.name,
                        is_me,
                        is_me && room.is_host,
                        is_me && me_ready,
                        accent,
                    );
                    if manageable && !is_me {
                        theme::text_chevron(ui, rr.right() - CARD_PAD * 0.4, rr.center().y);
                    }
                }
                (r.w, ids.len() as f32 * step)
            });
        });
        // 人数变少时作废多出来的旧命中区，避免点到上一帧的行
        for i in ids.len()..self.user_rows.len() {
            self.user_rows[i].invalidate();
        }
    }

    /// 斜角平行四边形按钮（设计稿样式）：平行四边形底 + 居中文本，命中区照旧登记。
    ///
    /// slope 由调用方用 [Design::slope] 从稿子斜率折过来（等比映射下就是稿子斜率本身），
    /// 因此按钮的斜角和稿子一致，换窗口比例也不会走形。
    #[allow(clippy::too_many_arguments)]
    fn skew_btn(
        ui: &mut Ui,
        t: f32,
        btn: &mut DRectButton,
        r: Rect,
        label: &str,
        size: f32,
        fill: Color,
        fg: Color,
        slope: f32,
    ) {
        quad_btn(btn, ui, t, r, |ui, r| {
            theme::skew_panel(ui, r.x, r.y, r.w, r.h, slope, fill, fill);
            ui.text(label)
                .pos(r.center().x, r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(size)
                .color(fg)
                .max_width(r.w * 0.9)
                .draw();
        });
    }

    // ==================== 竖屏：卡片堆叠 ====================

    fn render_portrait(&mut self, ui: &mut Ui, t: f32, mut ctx: Render, items: &[ActItem]) {
        let accent = ui.accent();
        let top = theme::layout_top(ui);
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
            if r.w <= 0.01 {
                continue;
            }
            let icon = theme::tool_icon(item.action.icon());
            let active = matches!(item.action, RoomAction::Start | RoomAction::Ready);
            theme::tool_button(ui, self.actions.get(item.action), t, r, icon.as_ref(), &item.label, active, !item.disabled, accent);
        }
    }

    /// 竖屏左栏信息块：房名 → 谱面卡 → 进度行 → 确认条。
    fn render_info(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        let room = ctx.room;
        let mut y = r.y;

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

        let (state_text, chart_name) = chart_parts(room, ctx.view);
        let name = chart_name.filter(|n| !n.is_empty());
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
                    if !badges.is_empty() {
                        theme::pill_row(ui, inner_x, card_r.y + card_r.h * 0.85, card_r.right() - CARD_PAD, &badges);
                    }
                }
            }
            None => {
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

        if let Some(dl) = ctx.download.as_deref_mut() {
            let avail = r.bottom() - y - 0.006 * SCALE;
            if avail >= 0.1 * SCALE {
                let dr = Rect::new(r.x, y + PANEL_INSET, r.w, avail.min(0.2 * SCALE));
                theme::card_rect(ui, dr, card_soft());
                dl.render_inline(ui, dr, t);
                y = dr.bottom();
            } else {
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

    /// 竖屏用户列表：头像 + 名字 + 徽标，房主可点行进入管理页。
    fn render_users(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        let room = ctx.room;
        let ids = sorted_user_ids(room, ctx.me);
        let caption = mtl!("mp-player-count", "n" => ids.len() as u64);
        theme::panel_caption(ui, r.x, r.y, r.w, &caption);
        let list = Rect::new(r.x, r.y + CAPTION_H, r.w, (r.h - CAPTION_H).max(0.06));
        let a = ui.alpha;
        let list_inner = theme::parallelogram(ui, list, P_SLOPE, fade(card_soft(), a), fade(card_soft(), a));
        let inner = list_inner.feather(-PANEL_INSET);
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

    /// 竖屏聊天框：上半聊天、下半日志，跟输入行同框。
    fn render_chat(&mut self, ui: &mut Ui, t: f32, r: Rect, ctx: &mut Render, accent: Color) {
        let caption = mtl!("mp-chat-title");
        theme::panel_caption(ui, r.x, r.y, r.w, &caption);
        let input_h = if CHAT_ENABLED { (r.h * 0.26).clamp(0.11 * SCALE, 0.15 * SCALE) } else { 0. };
        let list = Rect::new(
            r.x,
            r.y + CAPTION_H,
            r.w,
            (r.h - CAPTION_H - if CHAT_ENABLED { input_h + 0.015 * SCALE } else { 0. }).max(0.05),
        );
        let a = ui.alpha;
        let list_inner = theme::parallelogram(ui, list, P_SLOPE, fade(card_soft(), a), fade(card_soft(), a));
        let inner_w = (list_inner.w - PANEL_INSET * 2.).max(0.05);
        let log_h = (list_inner.h * 0.34).clamp(0.1, 0.6);
        let chat_h = (list_inner.h - PANEL_INSET * 2. - log_h - 0.02).max(0.06);
        let x = list_inner.x + PANEL_INSET;
        let bottom = list_inner.y + PANEL_INSET;
        ui.scope(|ui| {
            ui.dx(x);
            ui.dy(bottom);
            ctx.log.render(ui, Rect::new(0., 0., inner_w, (log_h - PANEL_INSET).max(0.04)));
        });
        ui.scope(|ui| {
            ui.dx(x);
            ui.dy(bottom + log_h + 0.02);
            ctx.messages.render(ui, Rect::new(0., 0., inner_w, chat_h));
        });
        ui.fill_rect(
            Rect::new(x, bottom + log_h + 0.005, inner_w, 0.0025),
            Color::new(1., 1., 1., 0.18),
        );
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
                &*ctx.chat_text,
                mtl!("chat-placeholder"),
                (input_h * 3.2).clamp(0.24, FS_BODY),
            );
            #[cfg(target_os = "windows")]
            if self.chat_focus {
                crate::native_edit::edit("mp_chat", ctx.chat_text, true, false);
                let sel = crate::native_edit::selection("mp_chat");
                crate::native_edit::draw_caret_and_selection(
                    ui,
                    br.feather(-0.01 * SCALE),
                    &*ctx.chat_text,
                    false,
                    sel,
                    (input_h * 3.2).clamp(0.24, FS_BODY),
                    t,
                );
            }
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
    pub fn touch(
        &mut self,
        touch: &Touch,
        t: f32,
        room: &ClientRoomState,
        view: &RoomView,
        msgs: &mut MessageLog,
        log: &mut MessageLog,
    ) -> Option<Action> {
        // 聊天 / 日志滚动区优先
        if msgs.touch(touch, t) || log.touch(touch, t) {
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
        // 输入框 / 发送：**不按 feature 开关判**，画了就要能点（没画出来的按钮
        // 本来就没有命中区，多查一次不会有副作用）
        if self.chat_btn.touch(touch, t) {
            return Some(Action::ChatInput);
        }
        if self.chat_send_btn.touch(touch, t) {
            return Some(Action::ChatSend);
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
        // 功能按钮：查的就是渲染用的那一份集合（refresh 缓存），保证「看得到=点得到」
        self.refresh(room, view);
        let items = std::mem::take(&mut self.item_cache);
        let mut hit = None;
        for item in &items {
            if self.actions.get(item.action).touch(touch, t) {
                hit = Some(item.action);
                break;
            }
        }
        self.item_cache = items;
        if let Some(action) = hit {
            return Some(Action::Room(action));
        }
        None
    }
}

/// 面片透明度：theme::quad 绕过了 Ui 的 alpha，所以自己乘。
fn fade(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}

/// 把贴图按「等比铺满 + 居中裁切」铺进任意四边形（这里用来把曲绘正好铺进左面板的斜边梯形）。
///
/// [theme::quad] 只能纯色、[Ui::fill_rect] 只能铺矩形，而左面板右边是斜的，
/// 用矩形去铺会在下边多出一块，所以这里自己往 quad_gl 塞一个带 UV 的四边形。
fn cover_quad(pts: [Vec2; 4], tex: &SafeTexture, alpha: f32) {
    if alpha <= 0.004 {
        return;
    }
    let vp = prpr::ext::get_viewport();
    let vw = vp.2 as f32;
    // 世界 → 像素的缩放：横轴恒等于屏宽（2 个世界单位），纵轴同比例（见 Ui::top 的定义），
    // 所以 x、y 共用一个系数。用 vp.3 / (2·ui.top) 去算纵轴在非 16:9 下会算错。
    let scale = vw / 2.;
    let (minx, maxx) = pts.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let (miny, maxy) = pts.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), p| (a.min(p.y), b.max(p.y)));
    let px_w = (maxx - minx) * scale;
    let px_h = (maxy - miny) * scale;
    let (tw, th) = (tex.width() as f32, tex.height() as f32);
    let (mut u0, mut u1, mut v0, mut v1) = (0., 1., 0., 1.);
    if px_w > 0. && px_h > 0. && tw > 0. && th > 0. {
        // 目标比图更宽就裁上下，否则裁左右 —— 保证既不拉伸也不留边
        let keep = if px_w / px_h > tw / th {
            (tw / th) / (px_w / px_h)
        } else {
            (px_w / px_h) / (tw / th)
        };
        let pad = (1. - keep.clamp(0., 1.)) * 0.5;
        if px_w / px_h > tw / th {
            v0 = pad;
            v1 = 1. - pad;
        } else {
            u0 = pad;
            u1 = 1. - pad;
        }
    }
    let c = semi_white(alpha);
    let v = [
        Vertex::new(pts[0].x, pts[0].y, 0., u0, v0, c),
        Vertex::new(pts[1].x, pts[1].y, 0., u1, v0, c),
        Vertex::new(pts[2].x, pts[2].y, 0., u0, v1, c),
        Vertex::new(pts[3].x, pts[3].y, 0., u1, v1, c),
    ];
    let gl = unsafe { get_internal_gl() }.quad_gl;
    gl.texture(Some(**tex));
    gl.draw_mode(DrawMode::Triangles);
    gl.geometry(&v, &[0, 2, 3, 0, 1, 3]);
}

/// 稿子上的四边形（顺序：左上 / 右上 / 左下 / 右下）→ 世界坐标的实心面片。
///
/// 设计稿里不少形状是**梯形**（两条侧边斜率不同），用对称的斜边工具去近似会走形，
/// 所以这类形状直接照四角铺。
fn design_quad(d: &Design, pts: [(f32, f32); 4], colors: [Color; 4], center: Vec2, k: f32) {
    let map = |p: (f32, f32)| {
        let v = Vec2::new(d.x(p.0), d.y(p.1));
        Vec2::new(center.x + (v.x - center.x) * k, center.y + (v.y - center.y) * k)
    };
    theme::quad([map(pts[0]), map(pts[1]), map(pts[2]), map(pts[3])], colors);
}

/// 按压时 DRectButton 会把**它自己画的内容**按这个比例缩（见 `DRectButton::build`）；
/// 稿子面片是直接进 quad_gl 的、不吃 Ui 的变换，只能自己缩，
/// 否则按下时「字在缩、底不动」，看着像没反应。
fn press_scale(btn: &mut DRectButton, t: f32) -> f32 {
    if crate::get_data().prefer_reduced_motion {
        return 1.;
    }
    1. - (1. - btn.progress(t)) * 0.04
}

/// 自己画面的按钮：只登记命中区 + 复用 DRectButton 的按压动画。
///
/// 不用 `DRectButton::build`/`render_shadow` 是有原因的：那两个会先建一条 **lyon 圆角路径**
/// （还带阴影）再交给回调，而我们画的是 `theme::skew_panel` / `design_quad` 的四边形，
/// 那条路径一次都用不上 —— 顶栏 + 尾巴一共近十个按钮，每帧白建十次路径。
fn quad_btn(btn: &mut DRectButton, ui: &mut Ui, t: f32, r: Rect, draw: impl FnOnce(&mut Ui, Rect)) {
    btn.inner.set(ui, r);
    let k = press_scale(btn, t);
    let rr = if (k - 1.).abs() < 0.0001 {
        r
    } else {
        scaled(r, r.center(), k)
    };
    draw(ui, rr);
}

/// 以 `center` 为中心缩放一个矩形。
fn scaled(r: Rect, center: Vec2, k: f32) -> Rect {
    Rect::new(
        center.x + (r.x - center.x) * k,
        center.y + (r.y - center.y) * k,
        r.w * k,
        r.h * k,
    )
}

/// 横屏用户列表的行距（稿子 30px 行高 + 3px 间隔）。
fn row_step(top: f32) -> f32 {
    Design::new(top).h(R_ROW_H + R_ROW_GAP)
}
