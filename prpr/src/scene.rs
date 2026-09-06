//! Scene management module.
#![allow(unused_macros)]

prpr_l10n::tl_file!("scene" ttl);

mod ending;
pub use ending::{EndingScene, RecordUpdateState};

mod crash;
pub use crash::{CrashCode, CrashScene};

mod game;
pub use game::{GameMode, GameScene, SimpleRecord};

mod loading;
pub use loading::{BasicPlayer, FinishedStats, LoadingScene, SaveFn, UpdateFn, UploadFn};

use crate::{
    core::BOLD_FONT,
    ext::{
        draw_image, draw_parallelogram, draw_parallelogram_ex, screen_aspect, semi_black,
        semi_white, RectExt, LocalTask, SafeTexture, ScaleType, PARALLELOGRAM_SLOPE,
    },
    judge::Judge,
    time::TimeManager,
    ui::{message_sound, BillBoard, Dialog, DRectButton, Message, MessageHandle, MessageKind, TextPainter, Ui},
};
use anyhow::{Error, Result};
use cfg_if::cfg_if;
use inputbox::{
    InputBox,
};
use macroquad::prelude::*;
use std::{
    any::Any,
    borrow::Cow,
    cell::RefCell,
    sync::{Arc, Mutex},
};
use tracing::warn;

#[derive(Default)]
pub enum NextScene {
    #[default]
    None,
    Pop,
    PopN(usize),
    PopWithResult(Box<dyn Any>),
    PopNWithResult(usize, Box<dyn Any>),
    Exit,
    Overlay(Box<dyn Scene>),
    Replace(Box<dyn Scene>),
}

thread_local! {
    pub static BILLBOARD: RefCell<(BillBoard, TimeManager)> = RefCell::new((BillBoard::new(), TimeManager::default()));
    pub static DIALOG: RefCell<Option<Dialog>> = const { RefCell::new(None) };
    pub static FULL_LOADING: RefCell<Option<FullLoadingView>> = const { RefCell::new(None) };
    pub static INPUT_DIALOG: RefCell<Option<InputDialog>> = const { RefCell::new(None) };
    /// 成就解锁横幅（独立于 BillBoard 消息：带图标与主题色的横幅，几秒后淡出）
    pub static ACH_BANNERS: RefCell<Vec<AchievementBanner>> = const { RefCell::new(Vec::new()) };
}

/// 一条成就解锁横幅
pub struct AchievementBanner {
    emoji: String,
    icon: Option<SafeTexture>,
    title: String,
    desc: String,
    color: Color,
    born: std::time::Instant,
}

/// 弹出成就解锁横幅（图标可选，未加载图标时显示 emoji 占位；颜色为 RGB 元组）
pub fn show_achievement_popup(emoji: &str, icon: Option<SafeTexture>, title: String, desc: String, color: (f32, f32, f32)) {
    let color = Color::new(color.0, color.1, color.2, 1.);
    ACH_BANNERS.with(|it| {
        let mut guard = it.borrow_mut();
        guard.push(AchievementBanner {
            emoji: emoji.to_string(),
            icon,
            title,
            desc,
            color,
            born: std::time::Instant::now(),
        });
        if guard.len() > 3 {
            guard.remove(0);
        }
    });
}

/// 每帧绘制成就横幅（淡入 → 停留 → 淡出），由顶层渲染器调用
pub fn render_achievement_banners(ui: &mut Ui) {
    const LIFETIME: f32 = 3.4;
    const OUT: f32 = 0.4;
    let now = std::time::Instant::now();
    ACH_BANNERS.with(|it| {
        let mut guard = it.borrow_mut();
        guard.retain(|b| now.duration_since(b.born).as_secs_f32() < LIFETIME);
        let n = guard.len() as f32;
        let top = ui.top.max(0.05);
        for (k, b) in guard.iter().enumerate() {
            let age = now.duration_since(b.born).as_secs_f32();
            // alpha：0.25s 淡入，末尾 0.4s 淡出
            let alpha = ((age / 0.25).min(1.) * ((LIFETIME - age) / OUT).clamp(0., 1.)).max(0.);
            if alpha <= 0. {
                continue;
            }
            let w = 0.72f32;
            let h = 0.15f32;
            let y_center = -top + 0.2 + 0.17 * (n - 1. - k as f32);
            let r = Rect::new(-w / 2., y_center - h / 2., w, h);
            // 卡片底色 + 主题色描边（半透明）
            let a = alpha;
            draw_rectangle(r.x, r.y, r.w, r.h, Color::new(0.03, 0.04, 0.09, 0.88 * a));
            let border = b.color;
            draw_rectangle_lines(r.x, r.y, r.w, r.h, 0.006, Color::new(border.r, border.g, border.b, 0.9 * a));
            // 图标 / emoji 占位
            let icon_size = h - 0.06;
            if let Some(tex) = &b.icon {
                draw_image(**tex, Rect::new(r.x + 0.03, r.y + (h - icon_size) / 2., icon_size, icon_size), ScaleType::Fit);
            } else {
                ui.text(&b.emoji)
                    .pos(r.x + 0.03 + icon_size / 2., r.y + h / 2.)
                    .anchor(0.5, 0.5)
                    .size(0.5)
                    .color(Color::new(1., 1., 1., a))
                    .draw();
            }
            let tx = r.x + 0.03 + icon_size + 0.03;
            ui.text(&b.title)
                .pos(tx, r.y + 0.04)
                .anchor(0., 0.5)
                .size(0.4)
                .color(Color::new(1., 1., 1., a))
                .draw();
            ui.text(&b.desc)
                .pos(tx, r.y + h - 0.045)
                .anchor(0., 0.5)
                .size(0.22)
                .color(Color::new(0.72, 0.75, 0.82, a))
                .draw();
        }
    });
}

pub struct FullLoadingView {
    keep_alive: Arc<()>,
    text: Option<Cow<'static, str>>,
}

impl FullLoadingView {
    pub fn begin() -> Arc<()> {
        Self::begin_inner(None)
    }
    pub fn begin_text(text: Cow<'static, str>) -> Arc<()> {
        Self::begin_inner(Some(text))
    }
    fn begin_inner(text: Option<Cow<'static, str>>) -> Arc<()> {
        let arc = Arc::new(());
        let ret = arc.clone();
        FULL_LOADING.replace(Some(Self { keep_alive: arc, text }));
        ret
    }
}

#[inline]
pub fn show_error(error: Error) {
    warn!("show error: {error:?}");
    Dialog::error(error).show();
}

pub struct MessageBuilder {
    content: String,
    kind: MessageKind,
    duration: f32,
}

impl MessageBuilder {
    pub fn new(content: String) -> Self {
        Self {
            content,
            kind: MessageKind::Info,
            duration: 2.,
        }
    }

    #[inline]
    pub fn kind(mut self, kind: MessageKind) -> Self {
        self.kind = kind;
        self
    }

    #[inline]
    pub fn duration(mut self, t: f32) -> Self {
        self.duration = t;
        self
    }

    #[inline]
    pub fn ok(self) -> Self {
        self.kind(MessageKind::Ok)
    }

    #[inline]
    pub fn warn(self) -> Self {
        self.kind(MessageKind::Warn)
    }

    #[inline]
    pub fn error(self) -> Self {
        self.kind(MessageKind::Error)
    }

    fn show(&mut self) -> MessageHandle {
        message_sound();
        BILLBOARD.with(|it| {
            let mut guard = it.borrow_mut();
            let (msg, handle) = Message::new(std::mem::take(&mut self.content), guard.1.now() as _, self.duration, self.kind.clone());
            guard.0.add(msg);
            handle
        })
    }

    #[inline]
    pub fn handle(mut self) -> MessageHandle {
        let handle = self.show();
        std::mem::forget(self);
        handle
    }
}

impl Drop for MessageBuilder {
    fn drop(&mut self) {
        self.show();
    }
}

#[inline]
pub fn show_message(msg: impl Into<String>) -> MessageBuilder {
    MessageBuilder::new(msg.into())
}

pub static INPUT_TEXT: Mutex<(Option<String>, Option<String>)> = Mutex::new((None, None));
/// Holds the id of the last input request the user cancelled (clicked Cancel or
/// dismissed the dialog). Consumed via [`take_input_cancelled`]; distinct from
/// [`INPUT_TEXT`] so callers can distinguish "cancelled" from "no input yet".
pub static INPUT_CANCELLED: Mutex<Option<String>> = Mutex::new(None);
#[cfg(not(target_arch = "wasm32"))]
pub static CHOSEN_FILE: Mutex<(Option<String>, Option<String>)> = Mutex::new((None, None));

pub struct InputDialog {
    id: String,
    title: String,
    prompt: String,
    text: String,
    password: bool,
    multiline: bool,
    ok_label: String,
    cancel_label: String,
    ok_btn: DRectButton,
    cancel_btn: DRectButton,
    enter_time: f32,
    cursor: usize,
    cursor_timer: f32,
    selection: Option<(usize, usize)>,
}

impl InputDialog {
    fn new(id: String, config: InputBox) -> Self {
        let password = matches!(config.mode, inputbox::InputMode::Password);
        let multiline = matches!(config.mode, inputbox::InputMode::Multiline);
        let text = config.default.to_string();
        // 清空打开前累积的字符事件，避免自动输入一长串字符
        while get_char_pressed().is_some() {}
        Self {
            id,
            title: config.title.map(|s| s.to_string()).unwrap_or_default(),
            prompt: config.prompt.map(|s| s.to_string()).unwrap_or_default(),
            cursor: text.len(),
            text,
            password,
            multiline,
            ok_label: config.ok_label.map(|s| s.to_string()).unwrap_or_else(|| "OK".to_string()),
            cancel_label: config.cancel_label.map(|s| s.to_string()).unwrap_or_else(|| "Cancel".to_string()),
            ok_btn: DRectButton::new(),
            cancel_btn: DRectButton::new(),
            enter_time: f32::NAN,
            cursor_timer: 0.,
            selection: None,
        }
    }

    fn confirm(&self) {
        INPUT_TEXT.lock().unwrap().1 = Some(self.text.clone());
        set_ime_enabled(false);
        android_show_keyboard(false);
    }

    fn cancel(&self) {
        *INPUT_CANCELLED.lock().unwrap() = Some(self.id.clone());
        set_ime_enabled(false);
        android_show_keyboard(false);
    }

    fn update_keyboard(&mut self) -> bool {
        let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
        if ctrl && is_key_pressed(KeyCode::A) {
            self.selection = Some((0, self.text.len()));
            self.cursor = self.text.len();
            return true;
        }
        // 复制 (Ctrl+C)
        if ctrl && is_key_pressed(KeyCode::C) {
            if let Some((s, e)) = self.selection {
                let text = self.text[s..e].to_string();
                unsafe { get_internal_gl() }.quad_context.clipboard_set(&text);
            }
            return true;
        }
        // 剪切 (Ctrl+X)
        if ctrl && is_key_pressed(KeyCode::X) {
            if let Some((s, e)) = self.selection.take() {
                let text = self.text[s..e].to_string();
                unsafe { get_internal_gl() }.quad_context.clipboard_set(&text);
                self.text.replace_range(s..e, "");
                self.cursor = s;
            }
            return true;
        }
        // 粘贴 (Ctrl+V)
        if ctrl && is_key_pressed(KeyCode::V) {
            if let Some(clip) = unsafe { get_internal_gl() }.quad_context.clipboard_get() {
                if let Some((s, e)) = self.selection.take() {
                    self.text.replace_range(s..e, "");
                    self.cursor = s;
                }
                self.text.insert_str(self.cursor, &clip);
                self.cursor += clip.len();
            }
            return true;
        }
        let mut input = String::new();
        while let Some(c) = get_char_pressed() {
            if c == '\r' || c == '\n' {
                if self.multiline {
                    input.push('\n');
                } else {
                    self.confirm();
                    return false;
                }
                continue;
            }
            if c == '\u{8}' || c == '\u{7f}' {
                continue;
            }
            if c.is_control() && c != '\t' {
                continue;
            }
            input.push(c);
        }
        if !input.is_empty() {
            if let Some((s, e)) = self.selection.take() {
                self.text.replace_range(s..e, "");
                self.cursor = s;
            }
            let reversed: String = input.chars().rev().collect();
            self.text.insert_str(self.cursor, &reversed);
            self.cursor += input.len();
        }
        if is_key_pressed(KeyCode::Backspace) {
            if let Some((s, e)) = self.selection.take() {
                self.text.replace_range(s..e, "");
                self.cursor = s;
            } else if self.cursor > 0 {
                let mut idx = self.cursor - 1;
                while idx > 0 && !self.text.is_char_boundary(idx) {
                    idx -= 1;
                }
                self.text.replace_range(idx..self.cursor, "");
                self.cursor = idx;
            }
        }
        if is_key_pressed(KeyCode::Delete) {
            if let Some((s, e)) = self.selection.take() {
                self.text.replace_range(s..e, "");
                self.cursor = s;
            } else if self.cursor < self.text.len() {
                let mut idx = self.cursor + 1;
                while idx < self.text.len() && !self.text.is_char_boundary(idx) {
                    idx += 1;
                }
                self.text.replace_range(self.cursor..idx, "");
            }
        }
        if is_key_pressed(KeyCode::Left) {
            let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
            let anchor = self.cursor;
            let mut idx = if self.cursor > 0 { self.cursor - 1 } else { 0 };
            while idx > 0 && !self.text.is_char_boundary(idx) {
                idx -= 1;
            }
            if shift {
                // Shift+← 扩展选区：选中从选区另一端起的高亮
                let end = self.selection.and_then(|(s, _)| if s == self.cursor { None } else { Some(self.cursor) }).unwrap_or(anchor);
                self.cursor = idx;
                let (s, e) = (end.min(self.cursor), end.max(self.cursor));
                if s != e {
                    self.selection = Some((s, e));
                } else {
                    self.selection = None;
                }
            } else {
                self.selection = None;
                self.cursor = idx;
            }
        }
        if is_key_pressed(KeyCode::Right) {
            let shift = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
            let anchor = self.cursor;
            let mut idx = self.cursor + 1;
            while idx < self.text.len() && !self.text.is_char_boundary(idx) {
                idx += 1;
            }
            if shift {
                let end = self.selection.and_then(|(_, e)| if e == self.cursor { None } else { Some(self.cursor) }).unwrap_or(anchor);
                self.cursor = idx.min(self.text.len());
                let (s, e) = (end.min(self.cursor), end.max(self.cursor));
                if s != e {
                    self.selection = Some((s, e));
                } else {
                    self.selection = None;
                }
            } else {
                self.selection = None;
                self.cursor = idx.min(self.text.len());
            }
        }
        let shift_down = is_key_down(KeyCode::LeftShift) || is_key_down(KeyCode::RightShift);
        if is_key_pressed(KeyCode::Home) {
            let end = if shift_down { self.cursor } else { 0 };
            self.cursor = 0;
            if shift_down && end != 0 {
                self.selection = Some((0, end));
            } else if !shift_down {
                self.selection = None;
            }
        }
        if is_key_pressed(KeyCode::End) {
            let end = if shift_down { self.cursor } else { self.text.len() };
            self.cursor = self.text.len();
            if shift_down && end != self.text.len() {
                self.selection = Some((end, self.text.len()));
            } else if !shift_down {
                self.selection = None;
            }
        }
        if is_key_pressed(KeyCode::Enter) && !self.multiline {
            self.confirm();
            return false;
        }
        if is_key_pressed(KeyCode::Escape) {
            self.cancel();
            return false;
        }
        true
    }

    fn touch(&mut self, touch: &Touch, t: f32) -> bool {
        if self.ok_btn.touch(touch, t) {
            self.confirm();
            return false;
        }
        if self.cancel_btn.touch(touch, t) {
            self.cancel();
            return false;
        }
        true
    }

    fn render(&mut self, ui: &mut Ui, t: f32) {
        if self.enter_time.is_nan() {
            self.enter_time = t;
        }
        self.cursor_timer += get_frame_time();

        let p = ((t - self.enter_time) / 0.2).clamp(0., 1.);
        let ease = 1. - (1. - p).powi(3);
        ui.fill_rect(ui.screen_rect(), semi_black(0.55 * ease));

        let w = 0.9;
        let h = if self.multiline { 0.58 } else { 0.42 };
        let scale = 0.92 + 0.08 * ease;
        let wr = Rect::new(-w * scale / 2., -h * scale / 2., w * scale, h * scale);
        let l = wr.h * PARALLELOGRAM_SLOPE;

        // 平行四边形底板（与“提示弹窗”一致的样式）
        draw_parallelogram_ex(
            wr,
            None,
            Color::new(0.20, 0.23, 0.29, 0.97 * ease),
            Color::new(0.09, 0.11, 0.15, 0.97 * ease),
            true,
        );
        draw_parallelogram(wr, None, Color::new(1., 1., 1., 0.09 * ease), false);

        let pad = 0.05;
        let cx = wr.x + l + pad;
        let cw = wr.w - l - pad * 2.;

        // 顶边高光（平行四边形上边缘）
        ui.fill_rect(Rect::new(wr.x + l, wr.y, wr.w - l, 0.004), Color::new(1., 1., 1., 0.12 * ease));

        ui.text(&self.title)
            .pos(cx, wr.y + pad)
            .size(0.48)
            .color(Color::new(1., 1., 1., ease))
            .draw_using(&BOLD_FONT);

        let mut y = wr.y + pad + 0.075;
        if !self.prompt.is_empty() {
            let r = ui
                .text(&self.prompt)
                .pos(cx, y)
                .size(0.32)
                .color(semi_white(0.65 * ease))
                .max_width(cw)
                .multiline()
                .draw();
            y = r.bottom() + 0.03;
        }

        let input_h = if self.multiline { 0.2 } else { 0.075 };
        let input_r = Rect::new(cx, y, cw, input_h);
        let il = input_r.h * PARALLELOGRAM_SLOPE;
        // 输入框：平行四边形暗底 + 蓝色细描边
        draw_parallelogram(
            input_r.feather(0.0025),
            None,
            Color::new(0.4, 0.6, 1., 0.35 * ease),
            false,
        );
        draw_parallelogram(input_r, None, Color::new(0.08, 0.09, 0.13, 0.98 * ease), false);

        let display: String = if self.password {
            self.text.chars().map(|_| '*').collect()
        } else {
            self.text.clone()
        };
        let before_cursor: String = if self.password {
            self.text[..self.cursor].chars().map(|_| '*').collect()
        } else {
            self.text[..self.cursor].to_string()
        };

        let text_size = 0.38;
        let text_x = input_r.x + il + 0.015;
        let text_y = input_r.center().y;
        let text_max_w = input_r.w - il - 0.06;

        if let Some((s, e)) = self.selection {
            if s < e {
                let before_sel: String = if self.password {
                    self.text[..s].chars().map(|_| '*').collect()
                } else {
                    self.text[..s].to_string()
                };
                let sel_text: String = if self.password {
                    self.text[s..e].chars().map(|_| '*').collect()
                } else {
                    self.text[s..e].to_string()
                };
                let off_x = ui.text(&before_sel).size(text_size).measure().w;
                let sel_w = ui.text(&sel_text).size(text_size).measure().w;
                ui.fill_rect(
                    Rect::new(text_x + off_x, input_r.y + 0.01, sel_w, input_r.h - 0.02),
                    Color::new(0.2, 0.4, 0.8, 0.5 * ease),
                );
            }
        }

        let placeholder = !self.prompt.is_empty() && self.text.is_empty();
        if placeholder {
            ui.text(&self.prompt)
                .pos(text_x, text_y)
                .anchor(0., 0.5)
                .max_width(text_max_w)
                .size(text_size)
                .color(semi_white(0.35 * ease))
                .draw();
        } else {
            ui.text(&display)
                .pos(text_x, text_y)
                .anchor(0., 0.5)
                .max_width(text_max_w)
                .size(text_size)
                .color(Color::new(1., 1., 1., ease))
                .draw();
        }

        if !placeholder && (self.cursor_timer % 1.0) < 0.5 {
            let cw0 = ui.text(&before_cursor).size(text_size).measure().w;
            let cursor_x = text_x + cw0;
            ui.fill_rect(
                Rect::new(cursor_x, input_r.y + 0.012, 0.0025, input_r.h - 0.024),
                Color::new(0.4, 0.6, 1., ease),
            );
        }

        let bh = 0.065;
        let bw = 0.16;
        let gap = 0.025;
        let by = wr.bottom() - bh - pad;
        let right_edge = wr.right() - l - pad;
        let ok_r = Rect::new(right_edge - bw, by, bw, bh);
        let cancel_r = Rect::new(ok_r.x - gap - bw, by, bw, bh);

        self.cancel_btn.inner.set(ui, cancel_r);
        let cv = if self.cancel_btn.inner.touching() { 0.3 } else { 0.16 };
        draw_parallelogram(cancel_r, None, Color::new(cv, cv, cv + 0.05, 0.92 * ease), true);
        let ss = 0.05;
        draw_parallelogram(
            Rect::new(cancel_r.x + cancel_r.w * (1. - ss), cancel_r.y, cancel_r.w * ss, cancel_r.h),
            None,
            Color::new(1., 1., 1., 0.9 * ease),
            false,
        );
        let ct = cancel_r.center();
        ui.text(&self.cancel_label)
            .pos(ct.x, ct.y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.34)
            .color(semi_white(0.85 * ease))
            .max_width(cancel_r.w)
            .draw();

        self.ok_btn.inner.set(ui, ok_r);
        let ob = if self.ok_btn.inner.touching() {
            Color::new(0.45, 0.65, 1., 0.95 * ease)
        } else {
            Color::new(0.3, 0.5, 0.95, 0.95 * ease)
        };
        draw_parallelogram(ok_r, None, ob, true);
        draw_parallelogram(
            Rect::new(ok_r.x + ok_r.w * (1. - ss), ok_r.y, ok_r.w * ss, ok_r.h),
            None,
            Color::new(1., 1., 1., 0.9 * ease),
            false,
        );
        let ct = ok_r.center();
        ui.text(&self.ok_label)
            .pos(ct.x, ct.y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.34)
            .color(Color::new(1., 1., 1., ease))
            .max_width(ok_r.w)
            .draw_using(&BOLD_FONT);
    }
}

#[cfg(windows)]
#[link(name = "imm32")]
extern "system" {
    fn GetActiveWindow() -> *mut std::ffi::c_void;
    fn ImmGetContext(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ImmSetOpenStatus(himc: *mut std::ffi::c_void, fopen: i32);
    fn ImmReleaseContext(hwnd: *mut std::ffi::c_void, himc: *mut std::ffi::c_void) -> i32;
}

#[cfg(windows)]
fn set_ime_enabled(enabled: bool) {
    unsafe {
        let hwnd = GetActiveWindow();
        if hwnd.is_null() {
            return;
        }
        let himc = ImmGetContext(hwnd);
        if himc.is_null() {
            return;
        }
        ImmSetOpenStatus(himc, if enabled { 1 } else { 0 });
        ImmReleaseContext(hwnd, himc);
    }
}

// 注意：set_ime_enabled 只在 Windows 上操作系统 IME 状态；
// Android 的软键盘只能在“确实打开/关闭输入框”时弹出/收起，
// 不能在这里统一挂钩（否则退出谱面等恢复 IME 的调用也会误弹键盘）。
#[cfg(not(windows))]
fn set_ime_enabled(_enabled: bool) {}

/// Android：打开/关闭软键盘（真正弹输入框时才调用）。
#[cfg(target_os = "android")]
pub fn android_show_keyboard(show: bool) {
    unsafe { get_internal_gl() }.quad_context.show_keyboard(show);
}
#[cfg(not(target_os = "android"))]
pub fn android_show_keyboard(_show: bool) {}

#[inline]
pub fn request_input(id: impl Into<String>, mut config: InputBox) {
    let id = id.into();
    *INPUT_TEXT.lock().unwrap() = (Some(id.clone()), None);
    *INPUT_CANCELLED.lock().unwrap() = None;
    if config.title.is_none() {
        config = config.title(ttl!("input"));
    }
    if config.prompt.is_none() {
        config = config.prompt(ttl!("input-msg"));
    }
    if config.cancel_label.is_none() {
        config = config.cancel_label(ttl!("cancel"));
    }
    if config.ok_label.is_none() {
        config = config.ok_label(ttl!("confirm"));
    }
    INPUT_DIALOG.with(|it| *it.borrow_mut() = Some(InputDialog::new(id, config)));
    set_ime_enabled(true);
    android_show_keyboard(true);
}

pub fn take_input() -> Option<(String, String)> {
    let mut w = INPUT_TEXT.lock().unwrap();
    w.0.clone().zip(std::mem::take(&mut w.1))
}

/// Returns the id of a cancelled input request once, clearing it.
pub fn take_input_cancelled() -> Option<String> {
    INPUT_CANCELLED.lock().unwrap().take()
}

pub fn return_input(id: String, text: String) {
    *INPUT_TEXT.lock().unwrap() = (Some(id), Some(text));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn request_file(id: impl Into<String>) {
    let id: String = id.into();
    #[cfg(target_env = "ohos")]
    let is_photo = id == "avatar";
    *CHOSEN_FILE.lock().unwrap() = (Some(id), None);
    cfg_if! {
        if #[cfg(target_os = "android")] {
            unsafe {
                let env = miniquad::native::attach_jni_env();
                let ctx = ndk_context::android_context().context();
                let class = (**env).GetObjectClass.unwrap()(env, ctx);
                let method = (**env).GetMethodID.unwrap()(env, class, c"chooseFile".as_ptr() as _, c"()V".as_ptr() as _);
                (**env).CallVoidMethod.unwrap()(env, ctx, method);
            }
        } else if #[cfg(target_os = "ios")] {
            use objc2::{available, define_class, rc::Retained, runtime::ProtocolObject, MainThreadMarker, MainThreadOnly};
            use objc2_foundation::{NSArray, NSObject, NSObjectProtocol, NSString, NSURL};
            use objc2_ui_kit::{UIDocumentPickerDelegate, UIDocumentPickerViewController};

            thread_local! {
                static DELEGATE: RefCell<Option<Retained<PickerDelegate>>> = const { RefCell::new(None) };
            }

            define_class! {



                #[unsafe(super = NSObject)]
                #[thread_kind = MainThreadOnly]
                struct PickerDelegate;


                unsafe impl NSObjectProtocol for PickerDelegate {}


                unsafe impl UIDocumentPickerDelegate for PickerDelegate {

                    #[unsafe(method(documentPicker:didPickDocumentsAtURLs:))]
                    fn did_pick_documents_at_urls(&self, controller: &UIDocumentPickerViewController, urls: &NSArray<NSURL>) {
                        use objc2_foundation::{NSData, NSDataReadingOptions, NSTemporaryDirectory};

                        let url = urls.firstObject().unwrap();
                        let need_close = unsafe { url.startAccessingSecurityScopedResource() };

                        let data = match NSData::dataWithContentsOfURL_options_error(&url, NSDataReadingOptions::Uncached) {
                            Ok(data) => data,
                            Err(err) => {
                                let message = err.localizedDescription().to_string();
                                show_error(Error::msg(message).context(ttl!("read-file-failed")));
                                return;
                            }
                        };
                        if need_close {
                            unsafe { url.stopAccessingSecurityScopedResource() };
                        }

                        let dir = NSTemporaryDirectory();
                        let path = format!("{}{}", dir, uuid::Uuid::new_v4());
                        data.writeToFile_atomically(&NSString::from_str(&path), true);
                        CHOSEN_FILE.lock().unwrap().1 = Some(path);
                    }
                }
            }

            impl PickerDelegate {
                fn new(mtm: MainThreadMarker) -> Retained<Self> {
                    let this = Self::alloc(mtm).set_ivars(());
                    unsafe { objc2::msg_send![super(this), init] }
                }
            }

            let mtm = MainThreadMarker::new().unwrap();

            let picker = UIDocumentPickerViewController::alloc(mtm);
            let picker = if available!(ios = 14.0.0) {
                use objc2_uniform_type_identifiers::UTType;

                let ext = |e: &str| UTType::typeWithFilenameExtension(&NSString::from_str(e)).unwrap();
                let types = NSArray::from_retained_slice(&[
                    ext("zip"),
                    ext("pez"),
                    ext("jpg"),
                    ext("png"),
                    ext("jpeg"),
                    ext("json"),
                    ext("mp3"),
                    ext("ogg"),
                ]);
                UIDocumentPickerViewController::initForOpeningContentTypes(picker, &types)
            } else {
                #[allow(deprecated)]
                {
                    use objc2_ui_kit::UIDocumentPickerMode;

                    let ext = NSString::from_str;
                    let types = NSArray::from_retained_slice(&[ext("public.image"), ext("public.archive")]);
                    UIDocumentPickerViewController::initWithDocumentTypes_inMode(picker, &types, UIDocumentPickerMode::Import)
                }
            };
            let dlg_obj = PickerDelegate::new(mtm);
            picker.setDelegate(Some(ProtocolObject::from_ref(&*dlg_obj)));
            DELEGATE.with(|it| *it.borrow_mut() = Some(dlg_obj));

            inputbox::backend::IOS::get_top_view_controller(mtm)
                .unwrap()
                .presentViewController_animated_completion(&picker, true, None);
        } else if #[cfg(target_env = "ohos")] {
            miniquad::native::call_request_callback(format!(r#"{{"action": "chooseFile", "isPhoto": {}}}"#, is_photo));
        } else {
            CHOSEN_FILE.lock().unwrap().1 = rfd::FileDialog::new().pick_file().map(|it| it.display().to_string());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn take_file() -> Option<(String, String)> {
    let mut w = CHOSEN_FILE.lock().unwrap();
    w.0.clone().zip(std::mem::take(&mut w.1))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn return_file(id: String, file: String) {
    *CHOSEN_FILE.lock().unwrap() = (Some(id), Some(file));
}

pub trait Scene {
    fn enter(&mut self, _tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        Ok(())
    }
    fn pause(&mut self, _tm: &mut TimeManager) -> Result<()> {
        Ok(())
    }
    fn resume(&mut self, _tm: &mut TimeManager) -> Result<()> {
        Ok(())
    }
    fn on_result(&mut self, _tm: &mut TimeManager, _result: Box<dyn Any>) -> Result<()> {
        Ok(())
    }
    fn touch(&mut self, _tm: &mut TimeManager, _touch: &Touch) -> Result<bool> {
        Ok(false)
    }
    fn update(&mut self, tm: &mut TimeManager) -> Result<()>;
    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()>;
    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        NextScene::None
    }
}

pub trait RenderTargetChooser {
    fn choose(&mut self) -> Option<RenderTarget>;
}
impl RenderTargetChooser for Option<RenderTarget> {
    fn choose(&mut self) -> Option<RenderTarget> {
        *self
    }
}
impl<F: FnMut() -> Option<RenderTarget>> RenderTargetChooser for F {
    fn choose(&mut self) -> Option<RenderTarget> {
        self()
    }
}

pub struct Main {
    pub scenes: Vec<Box<dyn Scene>>,
    times: Vec<f64>,
    target_chooser: Box<dyn RenderTargetChooser>,
    tm: TimeManager,
    paused: bool,
    last_update_time: f64,
    should_exit: bool,
    pub top_level: bool,
    touches: Option<Vec<Touch>>,
    pub viewport: Option<(i32, i32, i32, i32)>,
}

impl Main {
    pub async fn new(mut scene: Box<dyn Scene>, mut tm: TimeManager, mut target_chooser: impl RenderTargetChooser + 'static) -> Result<Self> {
        simulate_mouse_with_touch(false);
        scene.enter(&mut tm, target_chooser.choose())?;
        let last_update_time = tm.now();
        macro_rules! load_tex_or_fallback {
            ($flat:literal, $org:literal) => {
                match load_image($org).await {
                    Ok(img) => SafeTexture::from(Texture2D::from_image(&img)),
                    Err(_) => SafeTexture::from(Texture2D::from_image(&load_image($flat).await?)),
                }
            };
        }
        // 整理后的布局把图标放在 assets/icons/ 下（兼容旧的 assets/ 平铺）
        let icons = [
            load_tex_or_fallback!("info.png", "icons/info.png"),
            load_tex_or_fallback!("warn.png", "icons/warn.png"),
            load_tex_or_fallback!("ok.png", "icons/ok.png"),
            load_tex_or_fallback!("error.png", "icons/error.png"),
        ];
        BILLBOARD.with(|it| it.borrow_mut().0.set_icons(icons));
        Ok(Self {
            scenes: vec![scene],
            times: Vec::new(),
            target_chooser: Box::new(target_chooser),
            tm,
            paused: false,
            last_update_time,
            should_exit: false,
            top_level: true,
            touches: None,
            viewport: None,
        })
    }

    pub fn update(&mut self) -> Result<()> {
        self.update_with_mutate(|_| {})
    }

    pub fn update_with_mutate(&mut self, f: impl Fn(&mut Touch)) -> Result<()> {
        if self.paused {
            return Ok(());
        }
        match self.scenes.last_mut().unwrap().next_scene(&mut self.tm) {
            NextScene::None => {}
            NextScene::Pop => {
                self.scenes.pop();
                self.tm.seek_to(self.times.pop().unwrap());
                self.scenes.last_mut().unwrap().enter(&mut self.tm, self.target_chooser.choose())?;
            }
            NextScene::PopN(num) => {
                for _ in 0..num {
                    self.scenes.pop();
                    self.tm.seek_to(self.times.pop().unwrap());
                }
                self.scenes.last_mut().unwrap().enter(&mut self.tm, self.target_chooser.choose())?;
            }
            NextScene::PopWithResult(result) => {
                self.scenes.pop();
                self.tm.seek_to(self.times.pop().unwrap());
                self.scenes.last_mut().unwrap().on_result(&mut self.tm, result)?;
                self.scenes.last_mut().unwrap().enter(&mut self.tm, self.target_chooser.choose())?;
            }
            NextScene::PopNWithResult(num, result) => {
                for _ in 0..num {
                    self.scenes.pop();
                    self.tm.seek_to(self.times.pop().unwrap());
                }
                self.scenes.last_mut().unwrap().on_result(&mut self.tm, result)?;
                self.scenes.last_mut().unwrap().enter(&mut self.tm, self.target_chooser.choose())?;
            }
            NextScene::Exit => {
                self.should_exit = true;
            }
            NextScene::Overlay(mut scene) => {
                self.times.push(self.tm.now());
                scene.enter(&mut self.tm, self.target_chooser.choose())?;
                self.scenes.push(scene);
            }
            NextScene::Replace(mut scene) => {
                scene.enter(&mut self.tm, self.target_chooser.choose())?;
                *self.scenes.last_mut().unwrap() = scene;
            }
        }
        Judge::on_new_frame();
        let mut touches = Judge::get_touches();
        touches.iter_mut().for_each(f);
        if !(touches.is_empty() || FULL_LOADING.with(|it| it.borrow().is_some())) {
            let now = self.tm.now();
            let delta = (now - self.last_update_time) / touches.len() as f64;
            let start_time = self.tm.start_time;
            let mut last_err = None;
            DIALOG.with(|it| -> Result<()> {
                let mut index = 1;
                touches.retain_mut(|touch| {
                    let t = self.last_update_time + (index + 1) as f64 * delta;
                    index += 1;
                    let mut guard = it.borrow_mut();
                    if let Some(dialog) = guard.as_mut() {
                        if !dialog.touch(touch, t as _) {
                            drop(guard);
                            *it.borrow_mut() = None;
                        }
                        false
                    } else {
                        drop(guard);
                        let input_consumed = INPUT_DIALOG.with(|it| {
                            let mut guard = it.borrow_mut();
                            if let Some(dlg) = guard.as_mut() {
                                if !dlg.touch(touch, t as _) {
                                    drop(guard);
                                    *it.borrow_mut() = None;
                                }
                                true
                            } else {
                                false
                            }
                        });
                        if input_consumed {
                            false
                        } else {
                            self.tm.seek_to(t);
                            match self.scenes.last_mut().unwrap().touch(&mut self.tm, touch) {
                                Ok(val) => !val,
                                Err(err) => {
                                    warn!(?err, "failed to handle touch");
                                    last_err = Some(err);
                                    false
                                }
                            }
                        }
                    }
                });
                Ok(())
            })?;
            if let Some(err) = last_err {
                return Err(err);
            }
            self.tm.start_time = start_time;
        }
        self.touches = Some(touches);
        self.last_update_time = self.tm.now();
        DIALOG.with(|it| {
            if let Some(dialog) = it.borrow_mut().as_mut() {
                dialog.update(self.last_update_time as _);
            }
        });
        self.scenes.last_mut().unwrap().update(&mut self.tm)?;
        Ok(())
    }

    pub fn render(&mut self, painter: &mut TextPainter) -> Result<()> {
        if self.paused {
            return Ok(());
        }
        let mut ui = Ui::new(painter, self.viewport);
        ui.set_touches(self.touches.take().unwrap());
        ui.scope(|ui| self.scenes.last_mut().unwrap().render(&mut self.tm, ui))?;
        if self.top_level {
            push_camera_state();
            set_camera(&ui.camera());
            let mut gl = unsafe { get_internal_gl() };
            gl.flush();


            BILLBOARD.with(|it| {
                let mut guard = it.borrow_mut();
                let t = guard.1.now() as f32;
                guard.0.render(&mut ui, t);
            });
            render_achievement_banners(&mut ui);
            DIALOG.with(|it| {
                if let Some(dialog) = it.borrow_mut().as_mut() {
                    dialog.render(&mut ui, self.tm.now() as _);
                }
            });
            INPUT_DIALOG.with(|it| {
                let mut guard = it.borrow_mut();
                if let Some(dlg) = guard.as_mut() {
                    if !dlg.update_keyboard() {
                        drop(guard);
                        *it.borrow_mut() = None;
                    } else {
                        dlg.render(&mut ui, self.tm.now() as _);
                    }
                }
            });
            let remove = FULL_LOADING.with(|it| {
                if let Some(loading) = it.borrow_mut().as_mut() {
                    if Arc::strong_count(&loading.keep_alive) > 1 {
                        if let Some(text) = loading.text.as_ref() {
                            ui.full_loading(text.clone(), self.tm.now() as _);
                        } else {
                            ui.full_loading_simple(self.tm.now() as _);
                        }
                        return false;
                    } else {
                        return true;
                    }
                }
                false
            });
            if remove {
                FULL_LOADING.take();
            }
            pop_camera_state();
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        self.paused = true;
        self.scenes.last_mut().unwrap().pause(&mut self.tm)
    }

    pub fn resume(&mut self) -> Result<()> {
        self.paused = false;
        self.scenes.last_mut().unwrap().resume(&mut self.tm)
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    /// Push the crash screen on top of the current scene after a panic was
    /// caught on the main thread, so the app shows a proper crash UI instead of
    /// aborting. Any dialog that was open when the panic happened is dismissed
    /// so it cannot keep rendering over the crash screen.
    pub fn enter_crash_scene(&mut self, code: CrashCode, title: String) -> Result<()> {
        DIALOG.with(|it| *it.borrow_mut() = None);
        let mut scene = CrashScene::new(code, title);
        scene.enter(&mut self.tm, self.target_chooser.choose())?;
        self.scenes.push(Box::new(scene));
        Ok(())
    }
}

fn draw_background(tex: Texture2D) {
    let asp = screen_aspect();
    let top = 1. / asp;
    draw_image(tex, Rect::new(-1., -top, 2., top * 2.), ScaleType::CropCenter);
    draw_rectangle(-1., -top, 2., top * 2., Color::new(0., 0., 0., 0.3));
}

pub type LocalSceneTask = LocalTask<Result<NextScene>>;
