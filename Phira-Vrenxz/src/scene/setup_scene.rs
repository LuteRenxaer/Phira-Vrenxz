//! 首启向导：把「第一次进游戏真正要决定的事」串成一步一步的流程。
//!
//! 为什么要有这一页：这几项设置原来散在三处 —— 语言选择挂在启动页（LoginScene 的第二
//! 个阶段）、登录要进主页点右上角、音量与判定延迟埋在设置页里翻两层。首次启动的玩家既
//! 不知道要点什么，也不知道顺序。这里按五步排开：
//!
//! 1. 初始设置（语言）——  原来启动页那一段，整段搬进来，不再单独弹一次；
//! 2. 登录             ——  邮箱 / 密码两个输入行直接排在版面上，提交仍走 login.rs 那一套；
//! 3. 音量             ——  音乐 / 音效 / BGM，改动直接写进 config 并落盘；
//! 4. 其他设置         ——  判定延迟 / 性能档位 / 减少动画（首启值得问的子集，不塞更多）；
//! 5. 最后确认         ——  登录状态 + 是否游玩新手教程，走完这一步才进主界面。
//!
//! 版式直接跟 App 自己的页面走（`page/settings.rs` 的行距 / 字号 / 控件，
//! `mp/page/lobby.rs` 那种"一块块排"的分块思路），**不铺装饰面片**：
//!
//! - 页头只有两行字：左边这一步叫什么、右边「第几步 / 共几步」（步骤指示，不再单独占一块）；
//! - 每一项就是一行：**左列写标签（0.55 号字，与设置页的 render_title 同号）、右列放控件**，
//!   控件在右列里左对齐，于是整页的标签与控件各成一列；行距取设置页的
//!   `ITEM_HEIGHT + 0.02`，竖屏把行距适当拉开（页面比横屏高得多，不拉开就只剩顶上一条）；
//! - 控件全部是 App 现成的：`Slider`、`ChooseButton` 下拉、`page::render_switch` 开关，
//!   以及登录那两个输入行（与 login.rs 面板里的邮箱 / 密码框同一画法）；
//! - 底色是一块平色，没有面板、没有描边、没有亮条 —— 这一页只有字和控件。
//!
//! 登录这一步为什么把控件排进版面而不是搬那张登录卡片：卡片自带圆角底板、整屏遮罩与
//! 居中定位，塞进向导就是"窗口里再开一个窗口"。所以外观完全由本页负责（两个输入行 +
//! 一颗「登录」），而**逻辑一个字没抄**：TOS 检查、Client::login 请求、错误提示、成功后
//! 写回 data.me 与落盘，仍然走 `login::Login::submit_email_login`（面板那颗按钮走的是
//! 同一个方法）。已经登录时这一步只留「账号名 + 一行已登录」。
//!
//! 为什么每一步都能前进：向导只在首次启动出现，绝不能把玩家卡在这里 —— 登录失败、离线、
//! 没有账号都只影响「有没有账号」，不影响「能不能进游戏」；上一步 / 下一步始终可用（第一步
//! 的「上一步」是置灰而不是消失，按钮位置不跳变），最后一步点完直接进加载页。
//!
//! 本地核对版面：`PHIRA_SHOT=<路径>` 每 300 帧把帧缓冲存成 PNG，`PHIRA_SETUP_STEP=1..5`
//! 直接落在某一步（五步要靠点击才能走到）。这两个环境变量在正常运行时都不设。
//!
//! 词条说明：本次改动不动 locales/，所以页面文案全部复用现有词条（见文件末尾的几个
//! tl_file! 分组）。个别词条在部分语言里本来就缺（例如 tutorial 只有中 / 英 / 日有），
//! prpr-l10n 找不到时会把 key 原样显示出来。

use crate::{
    get_data, get_data_mut,
    login::Login,
    mp::theme,
    page::render_switch,
    popup::ChooseButton,
    save_data,
    scene::{BGM_VOLUME_UPDATED, StartupLoadingScene},
    sync_data,
};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::{DRectButton, FontArc, Slider, Ui, PREFER_REDUCED_MOTION},
};
use prpr_l10n::{LanguageIdentifier, LANG_IDENTS, LANG_NAMES};
use std::sync::atomic::Ordering;

#[cfg(feature = "hykb")]
use crate::icons::Icons;
#[cfg(feature = "hykb")]
use prpr::task::Task;
#[cfg(feature = "hykb")]
use std::sync::Arc;

/// 整页淡入时长
const FADE_IN: f32 = 0.45;
/// 行距：照 page/settings.rs 的 ITEM_HEIGHT + 0.02
const ROW_PITCH: f32 = 0.17;
/// 横屏最多把行距拉到这个程度：内容区只有 0.88 单位高，按设置页的密度铺会全挤在顶部
const ROW_PITCH_MAX_WIDE: f32 = 0.28;
/// 竖屏的行距上限：竖屏页面高得多，但仍然要有上限，否则两三行会被拉得散开
const ROW_PITCH_MAX_TALL: f32 = 0.5;
/// 标签列宽度（够放最长的那条标签，例如「减少动画效果」）
const LABEL_W: f32 = 0.52;
/// 标签列与控件列之间的间距（滑块左端的读数就落在这条缝里）
const COL_GAP: f32 = 0.06;
/// 行标签字号（= settings.rs 里 render_title 的 TITLE_SIZE）
const FS_LABEL: f32 = 0.55;
/// 页头「第几步 / 共几步」的字号（= settings.rs 的 SUBTITLE_SIZE）
const FS_COUNTER: f32 = 0.3;
/// 横条控件（下拉 / 输入框 / 按钮）的高度（= settings.rs 的 ITEM_HEIGHT × 2/3）
const CTRL_H: f32 = 0.1;
/// 下拉框宽度区间
const FIELD_MIN_W: f32 = 0.34;
const FIELD_MAX_W: f32 = 0.7;
/// 输入框宽度区间（要装得下邮箱）
const INPUT_MIN_W: f32 = 0.4;
const INPUT_MAX_W: f32 = 0.9;
/// 「登录」/「注册」两颗按钮的宽度，以及两颗之间的间距
const BTN_W: f32 = 0.26;
const BTN_GAP: f32 = 0.05;
/// 按钮文字字号（与引擎自己的 ui.button 同号）
const FS_BUTTON: f32 = 0.45;
/// 开关尺寸（= settings.rs 里那一颗：INTERACT_WIDTH 宽）
const SW_W: f32 = 0.28;
const SW_H: f32 = 0.1;
/// 滑块可见轨道的宽度上限（Slider::render 内部会把传进去的矩形左移并放大，见 slider_rect）
const SLIDER_TRACK_MAX: f32 = 0.86;
/// 设置改动的落盘延迟（与设置页同一个做法：拖动滑块时别每帧写一次 data.json）
const SAVE_DELAY: f32 = 0.5;

/// 向导的步骤。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    /// 1. 初始设置：语言
    Language,
    /// 2. 登录
    Login,
    /// 3. 音量
    Volume,
    /// 4. 其他设置
    General,
    /// 5. 最后确认：登录状态 + 新手教程
    Finish,
}

impl Step {
    const ALL: [Step; 5] = [Step::Language, Step::Login, Step::Volume, Step::General, Step::Finish];

    #[inline]
    fn index(self) -> usize {
        Self::ALL.iter().position(|it| *it == self).unwrap_or(0)
    }

    /// 页头里这一步叫什么。
    fn title(self) -> String {
        match self {
            Step::Language => l10n::login::select_language(),
            Step::Login => l10n::login::login(),
            Step::Volume => l10n::settings::audio(),
            Step::General => l10n::settings::general(),
            Step::Finish => l10n::settings::tutorial(),
        }
    }

    /// 这一步有几行。
    fn rows(self) -> usize {
        match self {
            Step::Language => 1,
            // 邮箱 / 用户名 / 密码 / 「登录」+「注册」：注册也要用户名，所以那一行一并摆出来，
            // 想注册直接填完按「注册」，不必先切一次模式
            Step::Login => 4,
            Step::Volume | Step::General => 3,
            Step::Finish => 2,
        }
    }
}

/// 登录这一步正在编辑哪个输入行。
#[derive(Clone, Copy, PartialEq, Eq)]
enum LoginField {
    Email,
    Name,
    Pwd,
}

/// 一行：标签列与控件列（两列同高，标签左对齐、控件在右列里也左对齐）。
#[derive(Clone, Copy)]
struct Row {
    label: Rect,
    ctrl: Rect,
}

pub struct SetupScene {
    /// 走完向导后交给加载页的字体
    fallback: FontArc,
    step: Step,
    enter_time: f32,

    /// 语言下拉（与设置页同款控件：按钮 + 展开列表）
    lang_btn: ChooseButton,

    /// 登录逻辑的宿主：外观一点不用它的，只借它那一套 TOS / 请求 / 错误 / 落盘
    login: Option<Login>,
    /// HYKB 构建里登录逻辑要图标，进这一步时后台加载一份
    #[cfg(feature = "hykb")]
    icons_task: Option<Task<Result<Arc<Icons>>>>,

    /// 向导自己的邮箱 / 用户名 / 密码输入行（点一下就地编辑，回车收起）
    t_email: String,
    t_name: String,
    t_pwd: String,
    active: Option<LoginField>,
    input_email: DRectButton,
    input_name: DRectButton,
    input_pwd: DRectButton,
    login_btn: DRectButton,
    register_btn: DRectButton,

    music_slider: Slider,
    sfx_slider: Slider,
    bgm_slider: Slider,
    offset_slider: Slider,
    perf_btn: ChooseButton,
    reduced_motion_btn: DRectButton,
    tutorial_btn: DRectButton,

    prev_btn: DRectButton,
    next_btn: DRectButton,

    /// 设置改动后延迟落盘（f32::INFINITY = 没有待保存的改动）
    save_time: f32,

    /// 走完向导后要切过去的下一个场景
    pending: Option<NextScene>,
}

impl SetupScene {
    pub fn new(fallback: FontArc) -> Self {
        // 语言下拉的初值 = 当前生效的语言（sync_data 启动时已经把语言定下来了），
        // 找不到就用第一个 —— 与设置页里那一颗下拉的算法一致。
        let lang_index = get_data()
            .language
            .as_ref()
            .and_then(|it| it.parse::<LanguageIdentifier>().ok())
            .and_then(|ident| LANG_IDENTS.iter().position(|it| *it == ident))
            .unwrap_or_default();
        // 本地核对版面：PHIRA_SETUP_STEP=1..5 直接落在某一步（正常运行为空）。
        // 五步要靠点击才能走到，没有这个入口就只能核对第一步的版面。
        let step = std::env::var("PHIRA_SETUP_STEP")
            .ok()
            .and_then(|it| it.parse::<usize>().ok())
            .and_then(|n| Step::ALL.get(n.saturating_sub(1)).copied())
            .unwrap_or(Step::Language);
        Self {
            fallback,
            step,
            enter_time: f32::NAN,
            lang_btn: ChooseButton::new()
                .with_options(LANG_NAMES.iter().map(|it| it.to_string()).collect())
                .with_selected(lang_index),
            login: None,
            #[cfg(feature = "hykb")]
            icons_task: None,
            t_email: String::new(),
            t_name: String::new(),
            t_pwd: String::new(),
            active: None,
            input_email: DRectButton::new(),
            input_name: DRectButton::new(),
            input_pwd: DRectButton::new(),
            login_btn: DRectButton::new(),
            register_btn: DRectButton::new(),
            music_slider: Slider::new(0.0..2.0, 0.05),
            sfx_slider: Slider::new(0.0..2.0, 0.05),
            bgm_slider: Slider::new(0.0..2.0, 0.05),
            offset_slider: Slider::new(-0.5..0.5, 0.005),
            // 档位选项与设置页完全一致（含「自定义」）：少一项的话 ChooseButton::render
            // 会按越界的下标取选项直接 panic
            perf_btn: ChooseButton::new()
                .with_options(l10n::settings::perf_tiers())
                .with_selected(get_data().config.performance.min(5) as usize),
            reduced_motion_btn: DRectButton::new(),
            tutorial_btn: DRectButton::new(),
            prev_btn: DRectButton::new(),
            next_btn: DRectButton::new(),
            save_time: f32::INFINITY,
            pending: None,
        }
    }

    /// 版面：把内容区分成 n 行，每行再分「标签列 / 控件列」。
    ///
    /// 行距的下界就是设置页的密度，上界取决于方向：横屏内容区只有 0.88 单位高，
    /// 按设置页的密度铺会全挤在顶部，所以允许拉到 0.28；竖屏页面高得多，上限 0.5 且整列
    /// 竖直居中，免得只有两三行还全飘在页面顶上。
    fn rows(body: Rect, n: usize, wide: bool) -> Vec<Row> {
        let n = n.max(1);
        let max_pitch = if wide { ROW_PITCH_MAX_WIDE } else { ROW_PITCH_MAX_TALL };
        let pitch = ((body.h - ROW_PITCH) / n as f32).clamp(ROW_PITCH, max_pitch);
        let total = pitch * n as f32;
        let y0 = body.y + (body.h - total) / 2.;
        (0..n)
            .map(|i| {
                let y = y0 + i as f32 * pitch;
                Row {
                    label: Rect::new(body.x, y, LABEL_W, pitch),
                    ctrl: Rect::new(
                        body.x + LABEL_W + COL_GAP,
                        y,
                        (body.w - LABEL_W - COL_GAP).max(0.2),
                        pitch,
                    ),
                }
            })
            .collect()
    }

    /// 控件在右列里的位置：左对齐、竖直居中。
    fn ctrl_rect(r: Rect, w: f32, h: f32) -> Rect {
        Rect::new(r.x, r.center().y - h / 2., w, h)
    }

    /// 滑块的矩形。
    ///
    /// `Slider::render` 会自己把传进来的矩形左移 `0.1 + 宽度的 20%` 再把宽度乘 1.2
    /// （见 prpr::ui::Slider::render），读数画在轨道左端、加减号挂在两端。这里先把
    /// 「可见轨道」左对齐到控件列里（左侧留出读数与减号的位置），再反推它要的矩形。
    fn slider_rect(r: Rect) -> Rect {
        let left = r.x + 0.14;
        let track = (r.w - 0.14 - 0.132).clamp(0.24, SLIDER_TRACK_MAX);
        let w = track / 1.2;
        Rect::new(left + 0.166 + 0.2 * w, r.center().y - 0.04, w, 0.08)
    }

    /// 一行的标签（左列、竖直居中、设置页同款字号）。
    fn label(ui: &mut Ui, r: Rect, s: &str) {
        theme::text_left(ui, r.x, r.center().y, FS_LABEL, theme::text_dim(), s, r.w);
    }

    /// 纯色按钮：一块圆角底 + 居中文字，没有描边、没有投影、没有角标
    /// （与引擎对话框里那种按钮同一种做法）。
    fn flat_button(ui: &mut Ui, btn: &mut DRectButton, t: f32, r: Rect, label: &str, fill: Color, fg: Color) {
        btn.build(ui, t, r, |ui, path| {
            ui.fill_path(&path, fill);
            ui.text(label)
                .pos(r.center().x, r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(FS_BUTTON)
                .color(fg)
                .max_width(r.w * 0.92)
                .draw();
        });
    }

    /// 页头：左边这一步叫什么，右边「第几步 / 共几步」。就两行字，没有底板。
    fn render_header(&self, ui: &mut Ui, f: &theme::Frame) {
        let cy = f.header.center().y;
        let counter = format!("{} / {}", self.step.index() + 1, Step::ALL.len());
        theme::text_left(ui, f.header.x, cy, FS_LABEL, theme::text(), &self.step.title(), f.header.w * 0.7);
        theme::text_right(ui, f.header.right(), cy, FS_COUNTER, theme::text_muted(), &counter, f.header.w * 0.25);
    }

    /// 内容区：这一页没有底板，控件直接排在底色上。
    fn render_body(&mut self, ui: &mut Ui, t: f32, body: Rect, wide: bool) {
        // 已登录时登录这一步只剩「账号名 + 一行已登录」，占一行
        let n = if self.step == Step::Login && get_data().me.is_some() {
            1
        } else {
            self.step.rows()
        };
        let rows = Self::rows(body, n, wide);
        self.render_rows(ui, t, &rows);
    }

    fn render_rows(&mut self, ui: &mut Ui, t: f32, rows: &[Row]) {
        let accent = ui.accent();
        match self.step {
            Step::Language => {
                let w = Self::field_w(rows[0].ctrl);
                Self::label(ui, rows[0].label, &l10n::settings::lang());
                self.lang_btn.render(ui, Self::ctrl_rect(rows[0].ctrl, w, CTRL_H), t);
            }
            Step::Login => match &get_data().me {
                // 已登录：账号名 + 一行「已登录」，这一步到这里就够了
                Some(me) => {
                    let name = me.name.clone();
                    let logged_in = l10n::login::logged_in();
                    let r = rows[0].label;
                    let cy = r.center().y;
                    // 名字可能很长：这两行占整行宽度，不被标签列截断
                    let w = rows[0].ctrl.right() - r.x;
                    theme::text_left(ui, r.x, cy - 0.055, FS_LABEL, theme::text(), &name, w);
                    theme::text_left(ui, r.x, cy + 0.055, FS_COUNTER, theme::text_muted(), &logged_in, w);
                }
                // 未登录：邮箱 / 用户名 / 密码三个输入行 + 「登录」「注册」两颗按钮，全部摆在面上
                None => {
                    // 字段显示文本先算好（渲染要可变借 self.input_*，不能同时借 self）
                    let d_email = self.field_display(LoginField::Email);
                    let d_name = self.field_display(LoginField::Name);
                    let d_pwd = self.field_display(LoginField::Pwd);
                    let email = l10n::login::email();
                    let name = l10n::login::username();
                    let pwd = l10n::login::password();
                    let w = Self::input_w(rows[0].ctrl);
                    Self::label(ui, rows[0].label, &email);
                    self.input_email
                        .render_input(ui, Self::ctrl_rect(rows[0].ctrl, w, CTRL_H), t, &d_email, &email, 0.5);
                    Self::label(ui, rows[1].label, &name);
                    self.input_name
                        .render_input(ui, Self::ctrl_rect(rows[1].ctrl, w, CTRL_H), t, &d_name, &name, 0.5);
                    Self::label(ui, rows[2].label, &pwd);
                    self.input_pwd
                        .render_input(ui, Self::ctrl_rect(rows[2].ctrl, w, CTRL_H), t, &d_pwd, &pwd, 0.5);
                    // 两颗按钮并排、左对齐在控件列里：有请求在途时都压暗并停止接收点击
                    let busy = self.login.as_ref().is_some_and(|it| it.busy());
                    let login_fill = if busy {
                        theme::color_alpha(accent, 0.5)
                    } else {
                        theme::primary(accent)
                    };
                    let register_fill = if busy {
                        theme::color_alpha(theme::secondary(), theme::secondary().a * 0.5)
                    } else {
                        theme::secondary()
                    };
                    let login_label = l10n::login::login();
                    let register_label = l10n::login::register();
                    let r = rows[3].ctrl;
                    let (login_r, register_r) = Self::button_pair(r);
                    Self::flat_button(ui, &mut self.login_btn, t, login_r, &login_label, login_fill, WHITE);
                    Self::flat_button(ui, &mut self.register_btn, t, register_r, &register_label, register_fill, theme::text());
                }
            },
            Step::Volume => {
                {
                    let v = get_data_mut().config.volume_music;
                    Self::label(ui, rows[0].label, &l10n::settings::music());
                    self.music_slider.render(ui, Self::slider_rect(rows[0].ctrl), t, v, format!("{:.2}", v));
                }
                {
                    let v = get_data_mut().config.volume_sfx;
                    Self::label(ui, rows[1].label, &l10n::settings::sfx());
                    self.sfx_slider.render(ui, Self::slider_rect(rows[1].ctrl), t, v, format!("{:.2}", v));
                }
                {
                    let v = get_data_mut().config.volume_bgm;
                    Self::label(ui, rows[2].label, &l10n::settings::bgm());
                    self.bgm_slider.render(ui, Self::slider_rect(rows[2].ctrl), t, v, format!("{:.2}", v));
                }
            }
            Step::General => {
                {
                    // 判定延迟按毫秒显示（滑块本体画的就是传进去的这串文字）
                    let v = get_data_mut().config.offset;
                    Self::label(ui, rows[0].label, &l10n::settings::cali());
                    self.offset_slider
                        .render(ui, Self::slider_rect(rows[0].ctrl), t, v, format!("{:.0}ms", v * 1000.));
                }
                {
                    let w = Self::field_w(rows[1].ctrl);
                    Self::label(ui, rows[1].label, &l10n::settings::perf());
                    self.perf_btn.render(ui, Self::ctrl_rect(rows[1].ctrl, w, CTRL_H), t);
                }
                {
                    let on = get_data().prefer_reduced_motion;
                    Self::label(ui, rows[2].label, &l10n::settings::reduced_motion());
                    render_switch(ui, Self::ctrl_rect(rows[2].ctrl, SW_W, SW_H), t, &mut self.reduced_motion_btn, on);
                }
            }
            Step::Finish => {
                {
                    // 第一行：登录状态（只读）
                    let (value, color) = match &get_data().me {
                        Some(me) => (me.name.clone(), theme::text()),
                        None => (l10n::home::not_logged_in(), theme::text_muted()),
                    };
                    Self::label(ui, rows[0].label, &l10n::login::login());
                    theme::text_left(ui, rows[0].ctrl.x, rows[0].ctrl.center().y, FS_LABEL, color, &value, rows[0].ctrl.w);
                }
                {
                    // 第二行：要不要玩新手教程（结果落进 data.play_tutorial）
                    let on = get_data().play_tutorial;
                    Self::label(ui, rows[1].label, &l10n::settings::tutorial());
                    render_switch(ui, Self::ctrl_rect(rows[1].ctrl, SW_W, SW_H), t, &mut self.tutorial_btn, on);
                }
            }
        }
    }

    /// 并排两颗按钮的矩形（左对齐在控件列里，竖直居中）。
    fn button_pair(r: Rect) -> (Rect, Rect) {
        let h = CTRL_H;
        let y = r.center().y - h / 2.;
        (
            Rect::new(r.x, y, BTN_W, h),
            Rect::new(r.x + BTN_W + BTN_GAP, y, BTN_W, h),
        )
    }

    /// 下拉框宽度：跟着控件列走，但夹在一个区间里（太宽没必要，太窄装不下档位名）。
    fn field_w(ctrl: Rect) -> f32 {
        ctrl.w.clamp(FIELD_MIN_W, FIELD_MAX_W)
    }

    /// 输入框宽度：比下拉再宽一点（邮箱要看得见）。
    fn input_w(ctrl: Rect) -> f32 {
        ctrl.w.clamp(INPUT_MIN_W, INPUT_MAX_W)
    }

    /// 底部两颗按钮：上一步 / 下一步（最后一步是「确定」）。
    fn render_nav(&mut self, ui: &mut Ui, t: f32, f: &theme::Frame) {
        let accent = ui.accent();
        let last = self.step == Step::Finish;
        let labels = vec![
            l10n::library::prev_page(),
            if last { l10n::common::confirm() } else { l10n::library::next_page() },
        ];
        let (_, rects) = theme::button_bar(
            ui,
            &labels,
            f.bar.x,
            f.bar.right(),
            f.bar.bottom(),
            f.bar.h,
            theme::BAR_ROW_GAP,
            theme::BAR_COL_GAP,
            theme::BarAlign::Fill,
        );
        // 第一步没有「上一步」：置灰而不是让它消失（按钮位置跳变会让人以为点错了），
        // 同时显式撤掉命中区 —— DRectButton 没有 disabled 状态，不登记就等于点不动。
        if self.step == Step::Language {
            self.prev_btn.invalidate();
            // 置灰 = 底色淡一半 + 文字换成弱化色（theme::color_alpha 是直接给 alpha 赋值，
            // 不是乘上去，所以这里自己换算成「原来的一半」）
            let dim = theme::secondary();
            Self::flat_button(
                ui,
                &mut self.prev_btn,
                t,
                rects[0],
                &labels[0],
                Color { a: dim.a * 0.5, ..dim },
                theme::text_muted(),
            );
        } else {
            Self::flat_button(ui, &mut self.prev_btn, t, rects[0], &labels[0], theme::secondary(), theme::text());
        }
        Self::flat_button(ui, &mut self.next_btn, t, rects[1], &labels[1], theme::primary(accent), WHITE);
    }

    /// 取 / 造登录逻辑的宿主（只造一次；切走再切回来还接着用，请求也不会中断）。
    #[cfg(not(feature = "hykb"))]
    fn ensure_login(&mut self, _t: f32) {
        if self.login.is_none() {
            self.login = Some(Login::new_bare());
        }
    }

    /// HYKB 构建：登录逻辑要图标，所以先异步把图标集加载回来再建宿主
    /// （加载失败只报一条错，玩家仍然可以点「下一步」继续）。
    #[cfg(feature = "hykb")]
    fn ensure_login(&mut self, _t: f32) {
        if self.login.is_some() {
            return;
        }
        if self.icons_task.is_none() {
            self.icons_task = Some(Task::new(async { Ok(Arc::new(Icons::new().await?)) }));
            return;
        }
        let Some(task) = &mut self.icons_task else { return };
        let Some(res) = task.take() else { return };
        self.icons_task = None;
        match res {
            Ok(icons) => self.login = Some(Login::new(icons)),
            Err(err) => prpr::scene::show_error(err),
        }
    }

    /// 提交登录。
    ///
    /// 输入与外观都是向导自己的；TOS 检查、Client::login 请求、错误提示、成功后写回
    /// data.me 并落盘统统走 `Login::submit_email_login` —— 与登录面板那颗按钮同一条路，
    /// 这里不复制任何网络实现。失败不改变步骤，玩家照样能点「下一步」。
    fn submit_login(&mut self, t: f32) {
        self.finish_editing();
        if self.login.as_ref().is_some_and(|it| it.busy()) {
            return; // 已经有一个请求在跑，别重复提交
        }
        self.ensure_login(t);
        let email = self.t_email.clone();
        let pwd = self.t_pwd.clone();
        if let Some(login) = &mut self.login {
            login.submit_email_login(email, pwd);
        }
    }

    /// 提交注册，与 `submit_login` 同一套路，只是交给 `Login::submit_email_register`：
    /// TOS 检查、输入校验的提示、Client::register 请求都在 login.rs 那边。
    fn submit_register(&mut self, t: f32) {
        self.finish_editing();
        if self.login.as_ref().is_some_and(|it| it.busy()) {
            return; // 已经有一个请求在跑，别重复提交
        }
        self.ensure_login(t);
        let email = self.t_email.clone();
        let name = self.t_name.clone();
        let pwd = self.t_pwd.clone();
        if let Some(login) = &mut self.login {
            login.submit_email_register(email, name, pwd);
        }
    }

    /// 点输入行 → 激活就地编辑，并叫出软键盘。
    fn activate(&mut self, f: LoginField) {
        self.active = Some(f);
        unsafe { get_internal_gl() }.quad_context.show_keyboard(true);
    }

    /// 结束编辑并收起软键盘（提交、离开这一步时都要收）。
    fn finish_editing(&mut self) {
        if self.active.take().is_some() {
            unsafe { get_internal_gl() }.quad_context.show_keyboard(false);
        }
    }

    fn field_mut(&mut self, f: LoginField) -> &mut String {
        match f {
            LoginField::Email => &mut self.t_email,
            LoginField::Name => &mut self.t_name,
            LoginField::Pwd => &mut self.t_pwd,
        }
    }

    /// 输入行里显示的文字：密码用星号掩码，正在编辑的那个字段末尾补一个光标。
    fn field_display(&self, f: LoginField) -> String {
        let raw = match f {
            LoginField::Email => &self.t_email,
            LoginField::Name => &self.t_name,
            LoginField::Pwd => &self.t_pwd,
        };
        let shown = if f == LoginField::Pwd {
            "*".repeat(raw.chars().count())
        } else {
            raw.clone()
        };
        if self.active == Some(f) {
            format!("{shown}|")
        } else {
            shown
        }
    }

    /// 逐帧把键盘输入写进正在编辑的那个字段。
    ///
    /// 这一段与 login.rs 面板里的内联编辑是同一套做法（输入法一次上屏的一批字符按反序
    /// 到达，反转后才是正确顺序；退格删一个；回车收起）：引擎只有 get_char_pressed
    /// 这一个入口，没有现成的文本控件可用。
    fn update_typing(&mut self) {
        let Some(f) = self.active else { return };
        let mut batch = String::new();
        loop {
            match get_char_pressed() {
                None => break,
                Some(c) => {
                    if c == '\r' || c == '\n' {
                        self.finish_editing();
                        return;
                    }
                    if c == '\u{8}' || c == '\u{7f}' || c.is_control() {
                        continue;
                    }
                    batch.push(c);
                }
            }
        }
        if !batch.is_empty() {
            let text = self.field_mut(f);
            for c in batch.chars().rev() {
                text.push(c);
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            self.field_mut(f).pop();
        }
    }

    /// 记一笔「设置改了」，由 update 延迟落盘。
    fn mark_dirty(&mut self, t: f32) {
        self.save_time = t;
    }

    /// 进入某一步。
    fn goto(&mut self, t: f32, step: Step) {
        self.finish_editing();
        self.step = step;
        if step == Step::Login {
            // 提前把登录逻辑的宿主准备好（HYKB 构建要等图标加载）
            self.ensure_login(t);
        }
    }

    /// 上一步。
    fn step_prev(&mut self, t: f32) {
        let i = self.step.index();
        if i > 0 {
            self.goto(t, Step::ALL[i - 1]);
        }
    }

    /// 下一步；最后一步就是「完成」：落盘 + 记住已经走过向导 + 去加载页。
    fn step_next(&mut self, t: f32) {
        let i = self.step.index();
        if i + 1 < Step::ALL.len() {
            self.goto(t, Step::ALL[i + 1]);
            return;
        }
        {
            let data = get_data_mut();
            data.initial_setup_done = true;
            // 语言这一步走过了（哪怕没改）：原来的 has_chosen_language 是「别再弹语言
            // 选择」的标记，这里继续沿用，启动页那一段语言选择也才算真正退休。
            data.has_chosen_language = true;
        }
        let _ = save_data();
        self.pending = Some(NextScene::Replace(Box::new(StartupLoadingScene::new(self.fallback.clone()))));
    }
}

impl Scene for SetupScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.enter_time.is_nan() {
            self.enter_time = tm.now() as f32;
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        let t = tm.now() as f32;
        // 下拉菜单的展开层最优先：它开着的时候点别处只应该把它收起来
        // （与设置页的处理顺序一致，见 page/settings.rs 的 top_touch）
        if self.lang_btn.top_touch(touch, t) {
            return Ok(true);
        }
        if self.perf_btn.top_touch(touch, t) {
            return Ok(true);
        }
        if self.prev_btn.touch(touch, t) {
            self.step_prev(t);
            return Ok(true);
        }
        if self.next_btn.touch(touch, t) {
            self.step_next(t);
            return Ok(true);
        }

        match self.step {
            Step::Language => {
                if self.lang_btn.touch(touch, t) {
                    return Ok(true);
                }
            }
            Step::Login => {
                // 已登录时这一步只有两行字，没有可点的东西
                if get_data().me.is_some() {
                    return Ok(false);
                }
                if self.input_email.touch(touch, t) {
                    self.activate(LoginField::Email);
                    return Ok(true);
                }
                if self.input_name.touch(touch, t) {
                    self.activate(LoginField::Name);
                    return Ok(true);
                }
                if self.input_pwd.touch(touch, t) {
                    self.activate(LoginField::Pwd);
                    return Ok(true);
                }
                if self.login_btn.touch(touch, t) {
                    self.submit_login(t);
                    return Ok(true);
                }
                if self.register_btn.touch(touch, t) {
                    self.submit_register(t);
                    return Ok(true);
                }
            }
            Step::Volume => {
                let config = &mut get_data_mut().config;
                if self.music_slider.touch(touch, t, &mut config.volume_music).is_some() {
                    self.mark_dirty(t);
                    return Ok(true);
                }
                if self.sfx_slider.touch(touch, t, &mut config.volume_sfx).is_some() {
                    self.mark_dirty(t);
                    return Ok(true);
                }
                let old = config.volume_bgm;
                if self.bgm_slider.touch(touch, t, &mut config.volume_bgm).is_some() {
                    if (config.volume_bgm - old).abs() > 0.001 {
                        // 主界面 BGM 可能正在放：标一下让主场景把音量跟过来（设置页也是这么做的）
                        BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
                    }
                    self.mark_dirty(t);
                    return Ok(true);
                }
            }
            Step::General => {
                {
                    let config = &mut get_data_mut().config;
                    if self.offset_slider.touch(touch, t, &mut config.offset).is_some() {
                        self.mark_dirty(t);
                        return Ok(true);
                    }
                }
                if self.perf_btn.touch(touch, t) {
                    return Ok(true);
                }
                if self.reduced_motion_btn.touch(touch, t) {
                    let data = get_data_mut();
                    data.prefer_reduced_motion ^= true;
                    // 这一项是内存里的全局开关（不少动画直接读它），改完立刻生效
                    PREFER_REDUCED_MOTION.store(data.prefer_reduced_motion, Ordering::Relaxed);
                    self.mark_dirty(t);
                    return Ok(true);
                }
            }
            Step::Finish => {
                if self.tutorial_btn.touch(touch, t) {
                    get_data_mut().play_tutorial ^= true;
                    self.mark_dirty(t);
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = tm.now() as f32;
        if self.enter_time.is_nan() {
            self.enter_time = t;
        }
        self.lang_btn.update(t);
        self.perf_btn.update(t);
        self.update_typing();
        // 登录请求在后台推进：切到别的步骤也继续跑，失败了只弹一条错误，不挡着「下一步」
        if let Some(login) = &mut self.login {
            login.update(t)?;
        }
        // 登录成功后手输的密码就没用了，顺手清掉（与 login.rs 里「拿到结果就清密码」一致）
        if self.active.is_none() && get_data().me.is_some() && !self.t_pwd.is_empty() {
            self.t_pwd.clear();
        }

        // 语言下拉：选中即生效（与设置页同一套做法：写 language + sync_data 换语言）
        if self.lang_btn.changed() {
            let data = get_data_mut();
            data.language = Some(LANG_IDENTS[self.lang_btn.selected()].to_string());
            data.has_chosen_language = true;
            sync_data();
            self.mark_dirty(t);
        }
        // 性能档位：选中即写进 config（自定义档的子项保持原样，细节仍在设置页里调）
        if self.perf_btn.changed() {
            get_data_mut().config.performance = self.perf_btn.selected().min(5) as u8;
            self.mark_dirty(t);
        }

        // 设置改动延迟落盘：拖动滑块时每帧写一次 data.json 太重（设置页也是攒 0.5s 写一次）
        if self.save_time.is_finite() && t > self.save_time + SAVE_DELAY {
            let _ = save_data();
            self.save_time = f32::INFINITY;
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        let t = tm.now() as f32;
        if self.enter_time.is_nan() {
            self.enter_time = t;
        }
        // 页头 / 内容区 / 底部操作条的位置由 theme::frame 算（与多人页共用同一套边距）
        let f = theme::frame(ui, theme::BAR_BTN_H);

        // 底色：一块平色（不铺渐变、不铺面片）。用引擎自己的 ui.background() ——
        // 这一页没有面片，底色就是唯一的"底"：压得太深，下拉 / 输入框那种半透明黑的
        // 控件就看不见了（它们本来就是靠"比底色再暗一点"来划出边界的）。
        set_camera(&ui.bg_camera());
        let bg = ui.background();
        ui.fill_rect(ui.screen_rect(), bg);
        set_camera(&ui.camera());

        let alpha = ((t - self.enter_time) / FADE_IN).clamp(0., 1.);
        ui.alpha(alpha, |ui| {
            self.render_header(ui, &f);
            self.render_body(ui, t, f.body, f.wide);
            self.render_nav(ui, t, &f);
        });

        // 下拉列表最后画：它要压在页面所有东西之上
        self.lang_btn.render_top(ui, t, 1.);
        self.perf_btn.render_top(ui, t, 1.);

        // 本地核对版面用（与 scene::mp::scene 里的同名后门同一套）：PHIRA_SHOT=<路径> 时每 300
        // 帧把**真正的帧缓冲**存成 PNG —— 截图走游戏自己，不依赖系统截屏。间隔不能太短：
        // export_png 会先截断再写，外部每隔几十毫秒去拷一次就总拷到半截文件。
        {
            use once_cell::sync::Lazy;
            use std::sync::atomic::{AtomicU32, Ordering};
            static SHOT: Lazy<Option<String>> = Lazy::new(|| std::env::var("PHIRA_SHOT").ok());
            static N: AtomicU32 = AtomicU32::new(0);
            if let Some(path) = SHOT.as_ref() {
                if N.fetch_add(1, Ordering::Relaxed) % 300 == 0 {
                    macroquad::texture::get_screen_data().export_png(path);
                }
            }
        }
        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        self.pending.take().unwrap_or_default()
    }
}

/// 本页文案：本次改动不动 locales/，所以只复用现有词条。
///
/// 为什么不直接 import 那几个 tl_file! 宏：tl! 展开出来的代码是在**调用处**解析
/// L10N_LOCAL 这个名字（macro_rules 的混合卫生），而每个 tl_file! 又只在它自己所在的
/// 模块里生成那一份 L10N_LOCAL —— 于是跨文件取词条只能"在词条所在的模块里包一层函数"。
/// 下面每个子模块对应一个 .ftl，函数名写清楚取的是哪一条。
mod l10n {
    /// login.ftl：选择语言与登录
    pub(crate) mod login {
        prpr_l10n::tl_file!("login" tll);

        /// 第一步的标题：选择语言
        pub(crate) fn select_language() -> String {
            tll!("startup-select-language").into_owned()
        }
        /// 「登录」（步骤标题 / 账号那一行的标签 / 那颗按钮）
        pub(crate) fn login() -> String {
            tll!("login").into_owned()
        }
        /// 邮箱输入行的标签
        pub(crate) fn email() -> String {
            tll!("email").into_owned()
        }
        /// 用户名输入行的标签（注册要它）
        pub(crate) fn username() -> String {
            tll!("username").into_owned()
        }
        /// 密码输入行的标签
        pub(crate) fn password() -> String {
            tll!("password").into_owned()
        }
        /// 「注册」（第二颗按钮）
        pub(crate) fn register() -> String {
            tll!("register").into_owned()
        }
        /// 「已登录」（用既有的 action-success 词条的 login 分支）
        pub(crate) fn logged_in() -> String {
            tll!("action-success", "action" => "login")
        }
    }

    /// settings.ftl：音量 / 延迟 / 性能 / 减少动画 / 教程
    pub(crate) mod settings {
        prpr_l10n::tl_file!("settings" tsl);

        /// 语言（下拉那一行的标签）
        pub(crate) fn lang() -> String {
            tsl!("item-lang").into_owned()
        }
        /// 音量步骤的标题：音频
        pub(crate) fn audio() -> String {
            tsl!("audio").into_owned()
        }
        /// 其余设置步骤的标题：通用
        pub(crate) fn general() -> String {
            tsl!("general").into_owned()
        }
        /// 音乐音量
        pub(crate) fn music() -> String {
            tsl!("item-music").into_owned()
        }
        /// 音效音量
        pub(crate) fn sfx() -> String {
            tsl!("item-sfx").into_owned()
        }
        /// 界面 BGM 音量
        pub(crate) fn bgm() -> String {
            tsl!("item-bgm").into_owned()
        }
        /// 判定延迟
        pub(crate) fn cali() -> String {
            tsl!("item-cali").into_owned()
        }
        /// 性能优化档位
        pub(crate) fn perf() -> String {
            tsl!("item-perf").into_owned()
        }
        /// 性能档位的选项：顺序与条数跟设置页里那颗下拉完全一致
        pub(crate) fn perf_tiers() -> Vec<String> {
            vec![
                tsl!("item-perf-off").into_owned(),
                tsl!("item-perf-light").into_owned(),
                tsl!("item-perf-medium").into_owned(),
                tsl!("item-perf-full").into_owned(),
                tsl!("item-perf-ultra").into_owned(),
                tsl!("item-perf-custom").into_owned(),
            ]
        }
        /// 减少动画效果
        pub(crate) fn reduced_motion() -> String {
            tsl!("item-prefer-reduced-motion").into_owned()
        }
        /// 新手教程
        pub(crate) fn tutorial() -> String {
            tsl!("tutorial").into_owned()
        }
    }

    /// common.ftl：最后一步的主按钮
    pub(crate) mod common {
        prpr_l10n::tl_file!("common" tcl);

        /// 「确定」（= 完成向导）
        pub(crate) fn confirm() -> String {
            tcl!("confirm").into_owned()
        }
    }

    /// home.ftl：登录状态
    pub(crate) mod home {
        prpr_l10n::tl_file!("home" thl);

        /// 「未登录」
        pub(crate) fn not_logged_in() -> String {
            thl!("not-logged-in").into_owned()
        }
    }

    /// library.ftl：翻页按钮（唯一在 15 种语言里都齐全的一组"上一页 / 下一页"词条）
    pub(crate) mod library {
        prpr_l10n::tl_file!("library" tyl);

        /// 「上一页」（= 上一步）
        pub(crate) fn prev_page() -> String {
            tyl!("prev-page").into_owned()
        }
        /// 「下一页」（= 下一步）
        pub(crate) fn next_page() -> String {
            tyl!("next-page").into_owned()
        }
    }
}
