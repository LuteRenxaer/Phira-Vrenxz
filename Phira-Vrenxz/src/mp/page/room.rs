
//! 房间页（进房后的根页面）。
//!
//! 版面**严格照设计稿（SVG）来**，而不是照着自己发挥：
//!
//! ```text
//! ┌────────── 左面板 x 3.38..291.70（右边缘向左倾斜 30.38）──────────┐ ┌── 右栏 x 277.52.. ──┐
//! │ [打开谱面库][预览] [锁][循][密][退]        ← 顶部黑条 y 47.4..78.3 │ │  用户列表  y 68.0    │
//! │ 房间 #31205                                 ← 基线 y 93.9          │ │  ● XXX    y 87.2     │
//! │ 谱面:xxx                                    ← 基线 y 105.8         │ │ ───────── y 177.5    │
//! │                                                                    │ │  XXX：这是聊天       │
//! │              这里是曲绘                                             │ │                      │
//! │                                                                    │ │  这是输入框   [发送] │
//! │ ┌ 成绩卡 ───────┐ ┌ 开始 ▶ ┐                                       │ │                      │
//! └─┴───────────────┴─┴────────┴───────────────────────────────────────┘ └──────────────────────┘
//! ```
//!
//! **所有位置都是「稿子坐标」**（稿子 viewBox 472.84×266.96，内容组 translate(-3.38,-47)，
//! 因此元素坐标落在 x 3.38..476.22 / y 47.00..313.96）。换算成世界坐标由 [Design] 统一负责，
//! 所以窗口比例怎么变版面都和稿子一致 —— 页面里不再出现「凭感觉写的 0.61 / 0.49」这类数字。
//!
//! 三条必须守住的约定：
//! 1. **功能按钮只有一个来源**：[action_items] 从房间状态推导按钮集合，渲染按它排布并
//!    用 [ActionButtons] 登记命中区，触摸侧对同一份结果查按钮，因此不存在「看得到点不到」。
//! 2. **用户列表的渲染与触摸共用一份 id 顺序**（[sorted_user_ids]），行索引一一对应；
//!    列表里不显示服务端的回放录制器虚拟用户（它只是个 monitor，不是玩家）。
//! 3. **左下角是成绩**：等级图标 + 等级牌 + 谱面 ID + 最好成绩 + 准确率，
//!    画法与 [crate::scene::song] 的选曲页完全一致（同一个 icon_index、同一个 {:07}）。

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

/// 是否编译了聊天功能。
pub const CHAT_ENABLED: bool = cfg!(feature = "chat");

/// 顶部细页头的高度（竖屏用）。
const STRIP_H: f32 = 0.14 * SCALE;
/// 竖屏用户列表的行高 / 行距。
const USER_ROW_H: f32 = 0.13 * SCALE;
const USER_ROW_GAP: f32 = 0.015 * SCALE;
/// 竖屏列表 / 聊天框的斜边斜率（世界坐标：斜移量 = 高度 × slope）。
///
/// 竖屏卡片比横屏那些斜角控件高得多，照搬稿子的 0.268 会斜得离谱，
/// 这里取一个温和的值，形状仍是正经的平行四边形。
const P_SLOPE: f32 = 0.14;

// ============================================================
//                 设计稿坐标（单位：稿子 px）
// ============================================================
//
// 稿子 viewBox = 472.83954 × 266.96377，内容组 translate(-3.37958, -46.99935)，
// 于是元素坐标的范围是 x 3.37958..476.21912、y 46.99935..313.96312。
// 下面每一个数字都是拿 SVG 的 path / text 算出来的（不是目测），改版面时对着稿子改这里。

/// 稿子内容区原点与尺寸（世界坐标从 3.38 / 47.00 映射到 -1 / -top）。
const DX0: f32 = 3.37958;
const DY0: f32 = 46.99935;
const DW: f32 = 472.83954;
const DH: f32 = 266.96377;

/// 稿子里文字用的 y 是**基线**，而 theme::text_* 是垂直居中
/// （anchor(0, .5)，行高 = ascent + descent）。字体实测（assets/fonts/font.ttf）
/// ascent 1.069em / descent −0.293em ⇒ 基线在行心下方 0.388em；
/// 粗体 bold.ttf 是 0.88 / −0.12 ⇒ 0.380em。
const BASE_DROP: f32 = 0.388;
const BASE_DROP_BOLD: f32 = 0.380;

// —— 左面板（稿子 path：M3.37958,311.4112 l0.19815,-263.88251 l288.1165,1.23326
//    l-30.37586,264.39833 z）——
const PANEL_LEFT: f32 = 3.38;
const PANEL_TOP: f32 = 47.38;
const PANEL_BOTTOM: f32 = 313.16;
const PANEL_RIGHT_TOP: f32 = 291.70;
const PANEL_RIGHT_BOTTOM: f32 = 261.32;

// —— 顶部黑条（稿子 path：M4.20617,78.33671 v-30.95899 l288.15034,0.30719
//    l-3.8594,31.11622 z）——
const STRIP_LEFT: f32 = 4.21;
const STRIP_RIGHT_TOP: f32 = 292.36;
const STRIP_BOTTOM: f32 = 78.34;

// —— 顶栏平行四边形主按钮（打开谱面库 / 预览）——
// 稿子里这两个按钮的实测外框是 (27.62,52.28)-(102.13,72.97)、(147.78,52.56)-(195.84,73.91)：
// 高约 20.7、间距约 2.9、上边右移约 5.7（斜率 0.27）。按钮从 BTN_X0 起顺序排布，
// 因此中间少一个按钮时后面的按钮会自动往左收，不会留下空洞。
const BTN_X0: f32 = 27.62;
const BTN_TOP: f32 = 52.40;
const BTN_BOTTOM: f32 = 73.20;
const BTN_GAP: f32 = 2.90;
const BTN_SLOPE: f32 = 0.268;
const BTN_PAD: f32 = 8.0;
const BTN_MIN_W: f32 = 42.0;
const BTN_FS: f32 = 11.9;
/// 顶栏尾部那些"单字小方块"比主按钮矮一档（次级操作视觉上就该小一圈），
/// 顺带把省下来的横向空间留给主按钮，主按钮才保得住稿子的宽度。
const TAIL_SCALE: f32 = 0.80;

// —— 返回按钮：稿子顶栏没画，但房间页必须能退出 ——
// 摆在黑条最左边（第一个按钮 27.62 之前的空位），比按钮矮一档以免贴边。
const BACK_X: f32 = 6.0;
const BACK_SCALE: f32 = 0.82;

// —— 房名 / 谱面（稿子 text：房名 translate(8.14429,93.85793) scale(0.48307)、
//    谱面 translate(9.14765,105.8108) scale(0.21639)，font-size 40）——
const TITLE_X: f32 = 8.14;
const TITLE_BASE_Y: f32 = 93.86;
const TITLE_FS: f32 = 19.32;
const SUB_X: f32 = 9.15;
const SUB_BASE_Y: f32 = 105.81;
const SUB_FS: f32 = 8.66;

/// 曲绘上那层白纱的下沿（稿子坐标）：只盖住房名 / 谱面 / 状态这几行，往下不再压白。
const ILLU_VEIL_BOTTOM: f32 = 132.0;

// —— 左下成绩卡（稿子 path：M4.34946,253.66919 … l-179.52034,-0.2325 z，
//    四角 (4.35,253.67) (175.07,253.36) (165.66,312.41) (-13.85,312.18)；
//    左下角落在画布外，所以可见的左边缘就是屏边）——
const CARD_TL: (f32, f32) = (4.35, 253.67);
const CARD_TR: (f32, f32) = (175.07, 253.36);
const CARD_BR: (f32, f32) = (165.66, 312.41);
const CARD_BL: (f32, f32) = (-13.85, 312.18);
/// 卡内：等级图标（稿子 image 136×164 @ scale .19668 → 26.75×32.26 @ 14.53,267.37）
const SCORE_ICON: (f32, f32, f32, f32) = (14.53, 267.37, 26.75, 32.26);
/// 卡内：等级牌（稿子 image 216×152 @ scale .09997 → 21.59×15.19 @ 48.80,258.10）
const SCORE_BADGE: (f32, f32, f32, f32) = (48.80, 258.10, 21.59, 15.19);
/// 卡内：谱面 ID（稿子 #7891 @ (74.09,271.41) scale .40142）
const SCORE_ID: (f32, f32, f32) = (74.09, 271.41, 16.06);
/// 卡内：最好成绩（稿子 07891666 @ (42.60,293.20) scale .61887）
const SCORE_NUM: (f32, f32, f32) = (42.60, 293.20, 24.75);
/// 卡内：准确率（稿子 86.67% @ (43.01,305.98) scale .22867）
const SCORE_ACC: (f32, f32, f32) = (43.01, 305.98, 9.15);

// —— 主要动作：稿子那个「外黑内白」的 ▶ 落在一块白色平行四边形里
//    （白底 path：M185.50728,312.94577 l9.18998,-59.1355 h72.72069 l-7.19216,59.1355 z；
//      ▶ 两个三角分别在 212.72..241.99 × 268.56..299.80 与内缩 0.85 的白色三角）——
const PLAY_BOX: (f32, f32, f32, f32) = (185.51, 253.81, 81.91, 59.14);
/// 白底的四角（稿子是个梯形：左边缘斜率 0.155、右边缘 0.122，不是平行四边形）
const PLAY_QUAD: [(f32, f32); 4] = [(194.70, 253.81), (267.42, 253.81), (185.51, 312.95), (260.23, 312.95)];
const PLAY_TRI: (f32, f32, f32, f32) = (212.72, 268.56, 29.27, 31.24);
const PLAY_TRI_INNER: f32 = 0.85;
const PLAY_LABEL_BASE_Y: f32 = 266.00;
const PLAY_LABEL_FS: f32 = 9.00;

// —— 右栏 ——
/// 右栏标题「用户列表」（稿子 translate(293.78919,68.0129) scale(0.48307)）
const R_TITLE: (f32, f32, f32) = (293.79, 68.01, 19.32);
/// 人数（右对齐到右栏右边缘；稿子没画，但房内人数是必要信息）
const R_RIGHT: f32 = 474.38;
const R_COUNT_FS: f32 = 11.40;
/// 用户列表第一行圆心（稿子 circle center 302.18,87.19 r 9.99）+ 行高 / 行距
const R_ROW_CY: f32 = 87.19;
const R_ROW_H: f32 = 30.00;
const R_ROW_GAP: f32 = 3.00;
/// 列表 / 聊天区左边界（稿子细线 M277.52194,177.61922）
const R_X: f32 = 277.52;
/// 用户列表与聊天之间的细线（稿子 y 177.47）
const R_SPLIT_Y: f32 = 177.47;
/// 聊天区下沿 = 加粗线
const R_CHAT_BOTTOM: f32 = 273.33;
/// 输入框上方的加粗线（稿子 path M474.96912,273.92431 l-207.72575,-1.19207，stroke-width 2.5）
const R_THICK_X: f32 = 267.24;
const R_THICK_W: f32 = 2.50;
/// 输入框：右栏底部，右到发送按钮左边缘
const R_INPUT_TOP: f32 = 275.02;
const R_INPUT_BOTTOM: f32 = 313.96;
const R_INPUT_RIGHT: f32 = 420.90;
const R_INPUT_FS: f32 = 10.50;
/// 输入框的斜度：和「发送」左边缘同一个角度，两个控件才像一对。
const R_INPUT_SLOPE: f32 = 0.1231;
/// 发送按钮（稿子 path：M423.64757,313.96045 l4.79477,-38.94175 h46.89659 l0.45435,38.94175 z）
const R_SEND_X: f32 = 423.65;
const R_SEND_RIGHT: f32 = 475.79;
/// 发送按钮的四角（稿子同样是梯形：左边缘斜率 0.123、右边缘几乎垂直）
const R_SEND_QUAD: [(f32, f32); 4] = [(428.44, 275.02), (475.34, 275.02), (423.65, 313.96), (475.79, 313.96)];
const R_SEND_FS: f32 = 14.30;

/// 设计稿取色。
const BG_TOP: Color = Color::new(0.365, 0.365, 0.365, 1.); // #5d5d5d
const BG_BOTTOM: Color = Color::new(0.616, 0.616, 0.616, 1.); // #9d9d9d
const PANEL_TOP_C: Color = Color::new(0.533, 0.533, 0.533, 1.); // #888
const PANEL_BOTTOM_C: Color = Color::new(1., 1., 1., 1.); // #fff
const CARD_LEFT_C: Color = Color::new(0.533, 0.533, 0.533, 1.); // #888
const CARD_RIGHT_C: Color = Color::new(1., 1., 1., 1.); // #fff
const STRIP_C: Color = Color::new(0., 0., 0., 0.72);
const BTN_FILL: Color = Color::new(0.702, 0.702, 0.702, 1.); // #b3b3b3
const SEND_FILL: Color = Color::new(0.404, 0.404, 0.404, 1.); // #676767
const ON_PANEL: Color = Color::new(0.06, 0.06, 0.08, 1.);
const ON_PANEL_DIM: Color = Color::new(0.25, 0.25, 0.28, 1.);
const ON_PANEL_ID: Color = Color::new(0.42, 0.42, 0.42, 1.);
const CHAT_VEIL: Color = Color::new(0., 0., 0., 0.28);

/// 设计稿坐标 → 世界坐标。
///
/// 世界坐标横屏下 x ∈ [-1, 1]（屏宽 2）、y ∈ [-top, top]（屏高 2·top）；
/// 稿子的 x / y 各自独立映射，所以版面在任意窗口比例下都和稿子同构。
#[derive(Clone, Copy)]
struct Design {
    top: f32,
}

impl Design {
    fn new(top: f32) -> Self {
        Self { top }
    }

    /// 稿子 x → 世界 x。
    fn x(&self, px: f32) -> f32 {
        -1. + 2. * (px - DX0) / DW
    }

    /// 稿子 y → 世界 y。
    fn y(&self, py: f32) -> f32 {
        2. * self.top * (py - DY0) / DH - self.top
    }

    /// 稿子横向长度 → 世界横向长度。
    fn w(&self, px: f32) -> f32 {
        2. * px / DW
    }

    /// 稿子纵向长度 → 世界纵向长度。
    fn h(&self, py: f32) -> f32 {
        2. * self.top * py / DH
    }

    /// 稿子字号（px）→ Ui::text(..).size(..) 的参数。
    ///
    /// DrawText::get_scale 里字号的世界高度 = 0.08 × size，而稿子的字号在世界上
    /// 应当占「字号 / 稿高 × 屏高」，于是 size = 字号 × 2·top / (稿高 × 0.08)。
    fn fs(&self, px: f32) -> f32 {
        px * 25. * self.top / DH
    }

    /// 稿子上的斜率（dx/dy）→ [theme::slant_panel_grad] / [theme::skew_panel] 的 slant。
    ///
    /// 那两个工具的定义是「斜移量 = 世界高度 × slant」；世界里 x、y 的缩放比不同
    /// （2/稿宽 与 2·top/稿高），所以这里把比例折算进去，斜边才和稿子同角度。
    fn slope(&self, s: f32) -> f32 {
        s * DH / (DW * self.top)
    }

    /// 稿子的**基线** y → theme::text_left 的 cy（常规字重）。
    fn base(&self, py: f32, fpx: f32) -> f32 {
        self.y(py) - BASE_DROP * 0.08 * self.fs(fpx)
    }

    /// 同上，粗体。
    fn base_bold(&self, py: f32, fpx: f32) -> f32 {
        self.y(py) - BASE_DROP_BOLD * 0.08 * self.fs(fpx)
    }

    /// 世界长度 → 稿子 px（横向）。
    fn px(&self, w: f32) -> f32 {
        w * DW / 2.
    }
}

/// 功能按钮的种类。
///
/// 取消类操作按语义拆开（CancelLocalShare / CancelDownload / CancelReady），
/// 这样触摸侧不需要按房间状态二次判断，避免「看得到点不到」。
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
    /// 房主：去谱面库选谱（退出多人场景但**保留房间与会话**，选完自动回来）
    Library,
    /// 离开房间（回主页，座位让出来）
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

    /// 竖屏工具条上的图标。
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

/// 操作条上的一项：动作 + 本地化后的标签。
pub struct ActItem {
    pub action: RoomAction,
    pub label: String,
}

/// 会话传进来的、只读的房间展示状态（全部为拥有所有权的数据，避免借用纠缠）。
#[derive(Default, Clone)]
pub struct RoomView {
    /// 当前分享中的本地谱面 (uuid, 谱面名)
    pub local_chart: Option<(String, String)>,
    /// 自己是否已就绪（本地谱面同步流程）
    pub local_ready: bool,
    /// 房主是否已开始分享（本地谱面同步流程）
    pub host_started: bool,
    /// 服务端已指示下载、但玩家还没点「准备」
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
/// 设计稿顶栏只画了（打开谱面库 / 预览）两个主按钮，其余动作（锁定、循环、
/// 密码、离开房间、各种取消）作者没画 —— 这里收成同样的斜角小方块贴在顶栏尾部，
/// 既不动稿子的空区，也不丢功能。
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
pub fn action_items(room: &ClientRoomState, view: &RoomView) -> Vec<ActItem> {
    let mut items = Vec::new();
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
    // 离开房间：放在最后（最右边），跟「开始游戏」那种主动作分开
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
    /// 房间号（房名 = 房间 #<id>）
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
    /// 等级牌文字
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

/// 本地谱面库里那张谱：最好成绩 (成绩, 准确率, 全连) / 等级牌文字 / 本地目录。
fn local_chart(id: Option<i32>) -> Option<(Option<(i32, f32, bool)>, String, String)> {
    let id = id?;
    let data = crate::get_data();
    let c = data.charts.iter().find(|c| c.info.id == Some(id))?;
    Some((
        c.record.as_ref().map(|r| (r.score, r.accuracy, r.full_combo)),
        format!("{}{:.0}", c.info.level, c.info.difficulty),
        c.local_path.clone(),
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
            match local_chart(view.chart_id) {
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
        let d = Design::new(ui.top);
        let top = ui.top;

        // 稿子面片是直接进 quad_gl 的（见 theme::quad），绕过了 Ui 的透明度，
        // 所以这里自己乘一份，否则切页淡入淡出时这几块会整片跳出来。
        let a = ui.alpha;

        // —— 整屏底色：稿子的灰渐变 #5d5d5d → #9d9d9d ——
        theme::slant_panel_grad(ui, -1., -top, 2., 2. * top, 0., fade(BG_TOP, a), fade(BG_BOTTOM, a));

        // —— 左面板：竖向 #888→#fff，右边缘向左倾斜（稿子 291.70 → 261.32）——
        let panel_x = d.x(PANEL_LEFT);
        let panel_y = d.y(PANEL_TOP);
        let panel_w = d.x(PANEL_RIGHT_TOP) - panel_x;
        let panel_h = d.y(PANEL_BOTTOM) - panel_y;
        let panel_slant = d.w(PANEL_RIGHT_TOP - PANEL_RIGHT_BOTTOM) / panel_h;
        theme::slant_panel_grad(
            ui,
            panel_x,
            panel_y,
            panel_w,
            panel_h,
            panel_slant,
            fade(PANEL_TOP_C, a),
            fade(PANEL_BOTTOM_C, a),
        );
        let panel_right_bottom = d.x(PANEL_RIGHT_BOTTOM);

        // —— 曲绘：稿子在这个位置写了「这里是曲绘」，所以**整个左面板就是曲绘位**，
        //    玩家选好谱面之后才铺得出来（本地谱面库里查得到才有图）。
        //    上面再压一层自上而下由浓转淡的白纱：房名、谱面行、成绩卡这些深色小字
        //    压在图上才始终读得清，也不会把曲绘糊成一片白。
        if let Some(illu) = &self.illu {
            let ia = illu.alpha(t);
            if ia > 0.02 {
                cover_quad(
                    &d,
                    [
                        Vec2::new(panel_x, panel_y),
                        Vec2::new(panel_x + panel_w, panel_y),
                        Vec2::new(panel_x, panel_y + panel_h),
                        Vec2::new(panel_x + panel_w - panel_h * panel_slant, panel_y + panel_h),
                    ],
                    // 用整图（texture.1），不是缩略图
                    &illu.texture.1,
                    ia * a,
                );
                // 白纱**只压顶部文字那一带**（房名 / 谱面行 / 状态行），往下渐隐到全透明。
                // 以前是整块面板从 0.80 糊到 0.28，图整个发白 —— 那就是「有点白」的来源。
                let band = d.y(ILLU_VEIL_BOTTOM) - panel_y;
                theme::slant_panel_grad(
                    ui,
                    panel_x,
                    panel_y,
                    panel_w,
                    band,
                    panel_slant,
                    fade(semi_white(0.78 * ia), a),
                    fade(Color::new(1., 1., 1., 0.), a),
                );
            }
        }

        // —— 顶部黑条（只盖左面板）——
        let strip_y = panel_y;
        let strip_h = d.y(STRIP_BOTTOM) - panel_y;
        let strip_x = d.x(STRIP_LEFT);
        let strip_w = d.x(STRIP_RIGHT_TOP) - strip_x;
        theme::slant_panel(ui, strip_x, strip_y, strip_w, strip_h, panel_slant, fade(STRIP_C, a));
        let strip_right = d.x(STRIP_RIGHT_TOP - 1.5);

        // —— 返回图标：稿子顶栏没画，占黑条最左边的空位 ——
        let btn_y = d.y(BTN_TOP);
        let btn_h = d.y(BTN_BOTTOM) - btn_y;
        // 返回按钮不能压到第一个顶栏按钮（窗口比例越接近 1:1，按钮越高，
        // 这里就必须跟着收窄，否则两者会叠在一起）
        let back_s = (btn_h * BACK_SCALE).min((d.x(BTN_X0) - d.x(BACK_X)) * 0.85);
        theme::back_button(
            ui,
            &mut self.back,
            t,
            Rect::new(d.x(BACK_X), btn_y + (btn_h - back_s) * 0.5, back_s, back_s),
        );

        // —— 主要动作（左下那块白底 ▶）：先定下来，下面排按钮时要把它排除掉 ——
        let main = items
            .iter()
            .find(|it| matches!(it.action, RoomAction::Start | RoomAction::Ready | RoomAction::CancelReady));

        // —— 其余动作：收成同样形状的小方块贴在顶栏尾部（稿子这儿是空的）——
        // 先数清楚有几个，好给它们**预留**位置：功能按钮宁可变窄也不能被挤掉，
        // 少一个按钮就等于少一条出路（「离开房间」被挤掉的话人就卡在房里了）。
        //
        // 顶栏主按钮 + 主动作之外的动作全在这儿；**同一个动作只能画一次**，
        // 否则两次 `build` 会互相盖掉命中区，看起来有点不动的按钮。
        let extras: Vec<&ActItem> = items
            .iter()
            .filter(|it| {
                !matches!(
                    it.action,
                    RoomAction::Library
                        | RoomAction::Preview
                        | RoomAction::Start
                        | RoomAction::Ready
                ) && main.is_none_or(|m| m.action != it.action)
            })
            .collect();

        // —— 顶栏主要按钮：稿子顺序「打开谱面库 / 预览」——
        // 宽度按文字量（其它语言也放得下），宽度总量超预算时等比收窄。
        let btn_sz = d.fs(BTN_FS);
        let gap_px = BTN_GAP;
        let full_px = BTN_BOTTOM - BTN_TOP;
        let avail_px = d.px(strip_right - d.x(BTN_X0));
        let reserve_px = (full_px * TAIL_SCALE + gap_px) * extras.len() as f32;
        let budget_px = (avail_px - reserve_px).max(avail_px * 0.5);
        let mut mains: Vec<(RoomAction, &str, f32)> = [RoomAction::Library, RoomAction::Preview]
            .into_iter()
            .filter_map(|action| {
                let item = items.iter().find(|it| it.action == action)?;
                let tw = d.px(self.label_w(ui, item.label.as_str(), btn_sz));
                Some((action, item.label.as_str(), (tw + BTN_PAD * 2.).max(BTN_MIN_W)))
            })
            .collect();
        let used: f32 = mains.iter().map(|it| it.2).sum::<f32>() + gap_px * (mains.len().saturating_sub(1)) as f32;
        if used > budget_px && !mains.is_empty() {
            let room = (budget_px - gap_px * (mains.len() - 1) as f32).max(BTN_MIN_W);
            let k = room / mains.iter().map(|it| it.2).sum::<f32>();
            for it in &mut mains {
                it.2 = (it.2 * k).max(BTN_MIN_W * 0.8);
            }
        }
        let mut bx = d.x(BTN_X0);
        for (action, label, w_px) in &mains {
            let w = d.w(*w_px);
            Self::skew_btn(
                ui,
                t,
                self.actions.get(*action),
                Rect::new(bx, btn_y, w, btn_h),
                label,
                btn_sz,
                BTN_FILL,
                WHITE,
                BTN_SLOPE,
            );
            bx += w + d.w(gap_px);
        }

        // —— 顶栏尾部的小方块（预留的位置一定放得下；真放不下就再压一点）——
        if !extras.is_empty() {
            let avail = (strip_right - bx).max(0.);
            let full = d.w(full_px * TAIL_SCALE);
            let gap = d.w(gap_px);
            let want = full * extras.len() as f32 + gap * (extras.len() - 1) as f32;
            let sq = if want > avail && avail > 0.02 {
                ((avail - gap * (extras.len() - 1) as f32) / extras.len() as f32).max(0.01)
            } else {
                full
            };
            // 兜底：万一（超长语言 + 极端比例）还是超了，把整条尾巴往左推回黑条里，
            // 宁可和主按钮贴近一点，也不能把按钮挤出黑条甚至挤出屏幕。
            let total = sq * extras.len() as f32 + gap * (extras.len() - 1) as f32;
            let mut sx = bx.min(strip_right - total).max(d.x(BTN_X0));
            for item in extras {
                Self::skew_btn(
                    ui,
                    t,
                    self.actions.get(item.action),
                    Rect::new(sx, btn_y + (btn_h - sq) * 0.5, sq, sq),
                    short_label(item.action),
                    btn_sz,
                    BTN_FILL,
                    WHITE,
                    BTN_SLOPE,
                );
                sx += sq + gap;
            }
        }

        // —— 房名（基线 y 93.86，em 19.32px）——
        let title = match ctx.room_id {
            Some(id) => mtl!("mp-room-tag", "id" => id.to_owned()),
            None => mtl!("multiplayer").into_owned(),
        };
        let text_w = panel_right_bottom - d.x(SUB_X) - 0.02;
        theme::text_left_bold(
            ui,
            d.x(TITLE_X),
            d.base_bold(TITLE_BASE_Y, TITLE_FS),
            d.fs(TITLE_FS),
            ON_PANEL,
            title.as_str(),
            text_w,
        );

        // —— 谱面:xxx（基线 y 105.81，em 8.66px）——
        let (state_text, chart_name) = chart_parts(ctx.room, ctx.view);
        let sub = match &chart_name {
            Some(n) if !n.is_empty() => format!("{}: {}", mtl!("mp-chart-label"), n),
            _ => state_text.clone(),
        };
        theme::text_left(
            ui,
            d.x(SUB_X),
            d.base(SUB_BASE_Y, SUB_FS),
            d.fs(SUB_FS),
            ON_PANEL_DIM,
            &sub,
            text_w,
        );

        // —— 进度 / 同步状态：紧贴谱面行下方（曲绘区上沿），不压任何稿件元素 ——
        if let Some(dl) = ctx.download.as_deref_mut() {
            let dr = Rect::new(d.x(SUB_X), d.y(112.), d.w(150.), d.h(26.));
            theme::card_rect(ui, dr, card());
            dl.render_inline(ui, dr, t);
        } else if ctx.syncing {
            theme::text_left(
                ui,
                d.x(SUB_X),
                d.base(118., SUB_FS),
                d.fs(SUB_FS),
                ON_PANEL_DIM,
                &mtl!("mp-syncing-chart"),
                d.w(150.),
            );
            theme::progress_bar(ui, Rect::new(d.x(SUB_X), d.y(122.), d.w(120.), d.h(3.)), None, t, accent);
        } else if ctx.busy {
            theme::progress_bar(ui, Rect::new(d.x(SUB_X), d.y(112.), d.w(120.), d.h(3.)), None, t, accent);
        }

        // —— 左下角：最好成绩卡（画法与选曲页一致，见 scene::song）——
        self.render_score_card(ui, ctx, &d);

        // —— 主要动作：稿子那块白色平行四边形 + 里面「外黑内白」的 ▶ ——
        if let Some(item) = main {
            let (px, py, pw, ph) = PLAY_BOX;
            let box_r = Rect::new(d.x(px), d.y(py), d.w(pw), d.h(ph));
            let (tx, ty, tw, th) = PLAY_TRI;
            let tri = Rect::new(d.x(tx), d.y(ty), d.w(tw), d.h(th));
            let label = (item.action != RoomAction::Start).then(|| item.label.as_str());
            let label_cy = d.base(PLAY_LABEL_BASE_Y, PLAY_LABEL_FS);
            let label_sz = d.fs(PLAY_LABEL_FS);
            let play_icon = theme::tool_icon(theme::ToolIcon::Play);
            let btn = self.actions.get(item.action);
            quad_btn(btn, ui, t, box_r, |ui, r| {
                let (cx, cy) = (r.center().x, r.center().y);
                let k = r.w / box_r.w.max(0.0001);
                design_quad(&d, PLAY_QUAD, [fade(WHITE, a); 4], Vec2::new(cx, cy), k);
                if let Some(label) = label {
                    ui.text(label)
                        .pos(box_r.center().x, label_cy)
                        .anchor(0.5, 0.5)
                        .no_baseline()
                        .size(label_sz)
                        .color(ON_PANEL)
                        .max_width(box_r.w * 0.92)
                        .draw();
                }
                // 开始按钮的图标直接用主菜单那枚现成的
                // （assets/icon_old(home)/resume.png，启动时已装进工具条图标表）。
                // 以前是拿 triangle_right 拼的 —— 那个内三角是贴着右边画的，
                // 拼出来是个歪掉的图形，不是设计稿上那个「黑边白心」的 ▶。
                let ir = scaled(tri, Vec2::new(cx, cy), k);
                match &play_icon {
                    Some(tex) => ui.fill_rect(ir, (**tex, ir, ScaleType::Fit)),
                    // 图标没装载成功才退回手画的三角形
                    None => theme::triangle_right(ui, ir, Color::new(0.02, 0.02, 0.03, 1.), PLAY_TRI_INNER, Some(WHITE)),
                }
            });
        }

        // —— 右栏：标题 + 人数 ——
        theme::text_left_bold(
            ui,
            d.x(R_TITLE.0),
            d.base_bold(R_TITLE.1, R_TITLE.2),
            d.fs(R_TITLE.2),
            ON_PANEL,
            &mtl!("user-list"),
            d.w(120.),
        );
        let count = mtl!("mp-n-players", "n" => user_count(ctx.room) as u64);
        theme::text_right(
            ui,
            d.x(R_RIGHT),
            d.base(R_TITLE.1, R_COUNT_FS),
            d.fs(R_COUNT_FS),
            ON_PANEL,
            &count,
            d.w(110.),
        );

        // —— 用户列表（到细线 y 177.47 为止）——
        let list_top = d.y(R_ROW_CY - R_ROW_H * 0.5);
        let list = Rect::new(
            d.x(R_X),
            list_top,
            d.x(R_RIGHT) - d.x(R_X),
            d.y(R_SPLIT_Y) - list_top,
        );
        // 用户列表底：平行四边形。斜度取输入框那档（也是左面板右边缘的斜率），
        // 形状正好收在细线左边不会压到左面板；返回值是内缩后的内容区。
        let veil_slope = d.slope(R_INPUT_SLOPE);
        let list_inner = theme::parallelogram(ui, list, veil_slope, fade(CHAT_VEIL, a), fade(CHAT_VEIL, a));
        self.render_user_rows(ui, t, list_inner, ctx, accent);
        theme::h_line(ui, d.x(R_X), d.y(R_SPLIT_Y), d.x(R_RIGHT) - d.x(R_X));

        // —— 聊天区（细线 → 加粗线）——
        let chat = Rect::new(
            d.x(R_X),
            d.y(R_SPLIT_Y) + 0.006,
            d.x(R_RIGHT) - d.x(R_X),
            (d.y(R_CHAT_BOTTOM) - d.y(R_SPLIT_Y) - 0.012).max(0.06),
        );
        // 聊天日志底：同样是平行四边形
        let chat_inner = theme::parallelogram(ui, chat, veil_slope, fade(CHAT_VEIL, a), fade(CHAT_VEIL, a));
        ui.scope(|ui| {
            ui.dx(chat_inner.x + 0.004);
            ui.dy(chat_inner.y + 0.004);
            ctx.messages.render(
                ui,
                Rect::new(0., 0., (chat_inner.w - 0.008).max(0.05), (chat_inner.h - 0.008).max(0.05)),
            );
        });

        // —— 加粗线（稿子 2.5px）压在输入框上方 ——
        ui.fill_rect(
            Rect::new(d.x(R_THICK_X), d.y(R_CHAT_BOTTOM), d.x(R_RIGHT) - d.x(R_THICK_X), d.h(R_THICK_W)),
            Color::new(0., 0., 0., 0.85),
        );

        // —— 输入框：稿子里的控件统一是「上边右移」的平行四边形，输入框也用同一个斜度，
        //    和右边的「发送」拼成一对。
        //
    //    两处以前是错的：
        //    1. 底是圆角矩形（`render_input` 自带深色圆角底），和稿子的斜角控件不是一套；
        //    2. 命中区包在 `if CHAT_ENABLED` 里 —— 而 chat 不是默认 feature，
        //       于是白框画出来了、却点不动（「输入框用不了」）。现在底色自己画、
        //       命中区无条件登记。
        let input = Rect::new(
            d.x(R_X),
            d.y(R_INPUT_TOP),
            d.x(R_INPUT_RIGHT) - d.x(R_X),
            d.y(R_INPUT_BOTTOM) - d.y(R_INPUT_TOP),
        );
        {
            let slope = d.slope(R_INPUT_SLOPE);
            let size = d.fs(R_INPUT_FS);
            let pad = d.w(9.);
            let typed = ctx.chat_text;
            let empty = typed.trim().is_empty();
            let hint = mtl!("chat-placeholder").into_owned();
            quad_btn(&mut self.chat_btn, ui, t, input, |ui, r| {
                theme::skew_panel(ui, r.x, r.y, r.w, r.h, slope, WHITE, WHITE);
                ui.text(if empty { hint.as_str() } else { typed })
                    .pos(r.x + pad, r.center().y)
                    .anchor(0., 0.5)
                    .no_baseline()
                    .size(size)
                    .color(if empty {
                        Color::new(0.45, 0.45, 0.48, 1.)
                    } else {
                        Color::new(0.07, 0.07, 0.09, 1.)
                    })
                    .max_width(r.w - pad * 2.)
                    .draw();
            });
        }
        // 发送按钮是稿子画的梯形，直接按四角铺，别用对称的平行四边形去近似
        let send = Rect::new(
            d.x(R_SEND_X),
            d.y(R_INPUT_TOP),
            d.x(R_SEND_RIGHT) - d.x(R_SEND_X),
            d.y(R_INPUT_BOTTOM) - d.y(R_INPUT_TOP),
        );
        let send_label = mtl!("chat-send").into_owned();
        let send_sz = d.fs(R_SEND_FS);
        quad_btn(&mut self.chat_send_btn, ui, t, send, |ui, r| {
            let k = r.w / send.w.max(0.0001);
            design_quad(&d, R_SEND_QUAD, [fade(SEND_FILL, a); 4], send.center(), k);
            ui.text(send_label.as_str())
                .pos(r.center().x, r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(send_sz)
                .color(Color::new(0., 0., 0., 1.))
                .max_width(r.w * 0.9)
                .draw();
        });

        // —— 内联确认条（房主要开始游戏了 → 准备 / 暂不）——
        if ctx.prompt {
            let h = d.h(26.);
            let cr = Rect::new(d.x(SUB_X), d.y(222.), d.w(248.), h);
            theme::card_rect(ui, cr, tag_accent(accent));
            theme::text_left(
                ui,
                cr.x + CARD_PAD * 0.6,
                cr.y + h * 0.34,
                d.fs(SUB_FS),
                text(),
                &mtl!("preview-interrupted-content"),
                cr.w - CARD_PAD,
            );
            let bh = h * 0.42;
            let bw = (0.22 * SCALE).min((cr.w - CARD_PAD) / 2.2);
            let by = cr.bottom() - bh - h * 0.12;
            let rr = Rect::new(cr.right() - CARD_PAD * 0.5 - bw, by, bw, bh);
            let ll = Rect::new(rr.x - 0.02 - bw, by, bw, bh);
            theme::button(ui, &mut self.prompt_later, t, ll, mtl!("preview-not-now"), d.fs(9.), secondary(), text());
            theme::button(ui, &mut self.prompt_ready, t, rr, mtl!("preview-ready"), d.fs(9.), primary(accent), WHITE);
        }
    }

    /// 左下角的最好成绩卡：等级图标 + 等级牌 + 谱面 ID + 最好成绩 + 准确率。
    ///
    /// 数值与画法都照 [crate::scene::song] 的选曲页：同一个 icon_index、
    /// 同一个 {:07} 成绩格式、同一个 {:.2}% 准确率。
    fn render_score_card(&mut self, ui: &mut Ui, ctx: &Render, d: &Design) {
        let a = ui.alpha;
        // —— 卡片本体：稿子的四角是个梯形，且左下角落在画布外（可见左边缘 = 屏边）——
        let points = [
            Vec2::new(d.x(CARD_TL.0), d.y(CARD_TL.1)),
            Vec2::new(d.x(CARD_TR.0), d.y(CARD_TR.1)),
            Vec2::new(d.x(CARD_BL.0), d.y(CARD_BL.1)),
            Vec2::new(d.x(CARD_BR.0), d.y(CARD_BR.1)),
        ];
        theme::quad(
            points,
            [
                fade(CARD_LEFT_C, a),
                fade(CARD_RIGHT_C, a),
                fade(CARD_LEFT_C, a),
                fade(CARD_RIGHT_C, a),
            ],
        );
        // 描边（稿子 stroke #000 0.5）：上、左、下三道
        let edge = fade(Color::new(0., 0., 0., 0.75), a);
        let s = d.h(0.5).max(0.0018);
        let (tl, tr, bl, br) = (points[0], points[1], points[2], points[3]);
        theme::quad(
            [
                Vec2::new(tl.x, tl.y),
                Vec2::new(tr.x, tr.y),
                Vec2::new(tl.x, tl.y + s),
                Vec2::new(tr.x, tr.y + s),
            ],
            [edge; 4],
        );
        theme::quad(
            [
                Vec2::new(tl.x, tl.y),
                Vec2::new(tl.x + s, tl.y),
                Vec2::new(bl.x, bl.y),
                Vec2::new(bl.x + s, bl.y),
            ],
            [edge; 4],
        );
        theme::quad(
            [
                Vec2::new(bl.x, bl.y - s),
                Vec2::new(br.x, br.y - s),
                Vec2::new(bl.x, bl.y),
                Vec2::new(br.x, br.y),
            ],
            [edge; 4],
        );

        // 用 refresh() 缓存好的那份（每帧去扫本地谱面库 + format! 太浪费）
        let record = self.chart_record;
        let badge = self.chart_badge.as_str();

        // —— 成绩三件套：**没有记录也照画** ——
        // 选曲页（scene/song.rs）那边 record 为 None 时是 score = 0 / accuracy = 0 /
        // icon_index(0, false) = F，三样都会画出来；这里以前没记录就留白 + 破折号，
        // 看着像坏了，改成和选曲页完全一致。
        let (score, accuracy, full_combo) = match record {
            Some((s, a, f)) => (s.max(0), a, f),
            None => (0, 0., false),
        };

        // —— 等级图标（稿子 14.53,267.37 起 26.75×32.26）——
        let (ix, iy, iw, ih) = SCORE_ICON;
        let icon_r = Rect::new(d.x(ix), d.y(iy), d.w(iw), d.h(ih));
        match crate::scene::TEX_RANK_ICONS.with(|it| it.borrow().clone()) {
            Some(icons) => {
                let idx = icon_index(score as u32, full_combo);
                ui.fill_rect(icon_r, (*icons[idx], icon_r, ScaleType::Fit));
            }
            // 图标没装载成功：留一块淡底，至少位置还在
            None => ui.fill_path(&icon_r.rounded(R_ROW), Color::new(0., 0., 0., 0.10)),
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
        let step = row_step(ui.top);
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
    /// slope 是**稿子上的斜率**，由 [Design::slope] 折算到世界坐标，
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
        let slope = slope * (DH / (DW * ui.top));
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
            if r.w <= 0.01 {
                continue;
            }
            let icon = theme::tool_icon(item.action.icon());
            let active = matches!(item.action, RoomAction::Start | RoomAction::Ready);
            theme::tool_button(ui, self.actions.get(item.action), t, r, icon.as_ref(), &item.label, active, accent);
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

    /// 竖屏聊天框：日志与聊天是同一个消息流，跟输入行同框。
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
        let a = ui.alpha;
        let list_inner = theme::parallelogram(ui, list, P_SLOPE, fade(card_soft(), a), fade(card_soft(), a));
        ui.scope(|ui| {
            ui.dx(list_inner.x + PANEL_INSET);
            ui.dy(list_inner.y + PANEL_INSET);
            ctx.messages.render(
                ui,
                Rect::new(0., 0., (list_inner.w - PANEL_INSET * 2.).max(0.05), (list_inner.h - PANEL_INSET * 2.).max(0.04)),
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
fn cover_quad(d: &Design, pts: [Vec2; 4], tex: &SafeTexture, alpha: f32) {
    if alpha <= 0.004 {
        return;
    }
    let vp = prpr::ext::get_viewport();
    let (vw, vh) = (vp.2 as f32, vp.3 as f32);
    let (minx, maxx) = pts.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), p| (a.min(p.x), b.max(p.x)));
    let (miny, maxy) = pts.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), p| (a.min(p.y), b.max(p.y)));
    let px_w = (maxx - minx) * vw / 2.;
    let px_h = (maxy - miny) * vh / (2. * d.top);
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
