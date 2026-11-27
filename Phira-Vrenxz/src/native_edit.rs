//! Windows 原生单行输入框（EDIT 控件），叠在游戏窗口上。
//!
//! 目的：登录/注册这类需要真正打字的字段，直接交给系统文本框处理——自带输入法、
//! 光标、选区、Ctrl+V、密码掩码，不再是游戏内自绘输入，也不会弹任何输入对话框。
//! 游戏只在它上面按 UI 坐标摆位置、读文本、写文本。

use macroquad::prelude::{screen_height, screen_width, Rect};
use std::ffi::c_void;

type Handle = isize;

#[repr(C)]
struct Rect32 {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const ES_AUTOHSCROLL: u32 = 0x0080;
const ES_PASSWORD: u32 = 0x0020;
const WS_EX_CLIENTEDGE: u32 = 0x0000_0200;

const WM_SETFONT: u32 = 0x0030;
const WM_GETTEXTLENGTH: u32 = 0x000E;
const EM_SETLIMITTEXT: u32 = 0x00C5;

const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;

#[link(name = "user32")]
extern "system" {
    fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        text: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Handle,
        menu: Handle,
        instance: Handle,
        param: *mut c_void,
    ) -> Handle;
    fn DestroyWindow(h: Handle) -> i32;
    fn ShowWindow(h: Handle, cmd: i32) -> i32;
    fn SetWindowPos(h: Handle, after: Handle, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    fn SetFocus(h: Handle);
    fn GetWindowTextW(h: Handle, buf: *mut u16, max: i32) -> i32;
    fn SetWindowTextW(h: Handle, s: *const u16) -> i32;
    fn SendMessageW(h: Handle, msg: u32, w: usize, l: isize) -> isize;
    fn GetClientRect(h: Handle, r: *mut Rect32) -> i32;
    fn IsWindow(h: Handle) -> i32;
    fn GetWindowLongW(h: Handle, index: i32) -> i32;
    fn SetWindowLongW(h: Handle, index: i32, value: i32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateFontW(
        height: i32,
        width: i32,
        esc: i32,
        orient: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike: u32,
        charset: u32,
        out_prec: u32,
        clip_prec: u32,
        quality: u32,
        pitch: u32,
        face: *const u16,
    ) -> Handle;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 区分哪个原生框，便于读取/写入各自文本
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Email,
    Password,
}

/// 一对原生输入框（账号 + 密码），挂在主窗口上
pub struct Edits {
    parent: Handle,
    email: Handle,
    pwd: Handle,
    font: Handle,
    shown: bool,
    cache: (String, String),
}

unsafe fn make_edit(parent: Handle, instance: Handle, password: bool, font: Handle, limit: u32) -> Handle {
    let mut style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL;
    if password {
        style |= ES_PASSWORD;
    }
    let class = wide("EDIT");
    let h = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        class.as_ptr(),
        std::ptr::null(),
        style,
        0,
        0,
        10,
        10,
        parent,
        0,
        instance,
        std::ptr::null_mut(),
    );
    if h != 0 {
        SendMessageW(h, WM_SETFONT, font as usize, 1);
        SendMessageW(h, EM_SETLIMITTEXT, limit as usize, 0);
    }
    h
}

impl Edits {
    /// 在主窗口上建好两个原生输入框；主窗口句柄无效时返回 None
    pub fn create(parent: Handle) -> Option<Self> {
        if parent == 0 || unsafe { IsWindow(parent) } == 0 {
            return None;
        }
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            // 父窗口加 WS_CLIPCHILDREN，避免 GL 每帧重绘把子输入框刷掉
            const GWL_STYLE: i32 = -16;
            const WS_CLIPCHILDREN: i32 = 0x0200_0000;
            let style = GetWindowLongW(parent, GWL_STYLE);
            if style & WS_CLIPCHILDREN == 0 {
                SetWindowLongW(parent, GWL_STYLE, style | WS_CLIPCHILDREN);
            }
            let face = wide("Segoe UI");
            let font = CreateFontW(
                -30,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                1, // DEFAULT_CHARSET
                0,
                0,
                0,
                0,
                face.as_ptr(),
            );
            let email = make_edit(parent, instance, false, font, 128);
            let pwd = make_edit(parent, instance, true, font, 128);
            if email == 0 || pwd == 0 {
                if email != 0 {
                    DestroyWindow(email);
                }
                if pwd != 0 {
                    DestroyWindow(pwd);
                }
                return None;
            }
            Some(Self {
                parent,
                email,
                pwd,
                font,
                shown: false,
                cache: (String::new(), String::new()),
            })
        }
    }

    fn to_px(ui_top: f32, r: Rect, scale_x: f32, scale_y: f32) -> (i32, i32, i32, i32) {
        let sw = screen_width();
        let sh = screen_height();
        let x = (r.x + 1.) * 0.5 * sw;
        let y = (r.y + ui_top) / (2. * ui_top) * sh;
        let w = r.w * 0.5 * sw;
        let h = r.h / (2. * ui_top) * sh;
        (
            (x * scale_x) as i32,
            (y * scale_y) as i32,
            (w * scale_x).max(8.) as i32,
            (h * scale_y).max(8.) as i32,
        )
    }

    /// 按 UI 坐标摆放；show=false 时整组隐藏
    pub fn layout(&mut self, email: Rect, pwd: Rect, ui_top: f32, show: bool) {
        unsafe {
            let mut cr = Rect32 { left: 0, top: 0, right: 0, bottom: 0 };
            if GetClientRect(self.parent, &mut cr) == 0 {
                return;
            }
            let cw = (cr.right - cr.left).max(1) as f32;
            let ch = (cr.bottom - cr.top).max(1) as f32;
            let sx = cw / screen_width().max(1.);
            let sy = ch / screen_height().max(1.);
            if show != self.shown {
                ShowWindow(self.email, if show { SW_SHOW } else { SW_HIDE });
                ShowWindow(self.pwd, if show { SW_SHOW } else { SW_HIDE });
                if !show {
                    // 收起时把焦点交回游戏窗口，否则键盘事件会落到隐藏的输入框里
                    SetFocus(self.parent);
                }
                self.shown = show;
            }
            if !show {
                return;
            }
            let (ex, ey, ew, eh) = Self::to_px(ui_top, email, sx, sy);
            let (px, py, pw, ph) = Self::to_px(ui_top, pwd, sx, sy);
            SetWindowPos(self.email, 0, ex, ey, ew, eh, SWP_NOZORDER | SWP_NOACTIVATE);
            SetWindowPos(self.pwd, 0, px, py, pw, ph, SWP_NOZORDER | SWP_NOACTIVATE);
        }
    }

    fn get(h: Handle) -> String {
        unsafe {
            let len = SendMessageW(h, WM_GETTEXTLENGTH, 0, 0) as usize;
            let mut buf: Vec<u16> = vec![0; len + 2];
            let n = GetWindowTextW(h, buf.as_mut_ptr(), buf.len() as i32);
            if n <= 0 {
                return String::new();
            }
            String::from_utf16_lossy(&buf[..n as usize])
        }
    }

    fn put(h: Handle, s: &str) {
        let w = wide(s);
        unsafe {
            SetWindowTextW(h, w.as_ptr());
        }
    }

    fn handle(&self, f: Field) -> Handle {
        match f {
            Field::Email => self.email,
            Field::Password => self.pwd,
        }
    }

    /// 双向同步：系统框里改了文本就写回游戏，游戏里清了文本就写进系统框
    pub fn sync(&mut self, f: Field, text: &mut String) {
        let h = self.handle(f);
        let win = Self::get(h);
        let cached = match f {
            Field::Email => &self.cache.0,
            Field::Password => &self.cache.1,
        };
        if win != *cached {
            *text = win.clone();
            match f {
                Field::Email => self.cache.0 = win,
                Field::Password => self.cache.1 = win,
            }
        } else if *text != win {
            Self::put(h, text);
            match f {
                Field::Email => self.cache.0 = text.clone(),
                Field::Password => self.cache.1 = text.clone(),
            }
        }
    }

    /// 重设文本（例如登录成功后清空密码、切换服务器时回填）
    pub fn reset(&mut self, f: Field, text: &str) {
        Self::put(self.handle(f), text);
        match f {
            Field::Email => self.cache.0 = text.to_string(),
            Field::Password => self.cache.1 = text.to_string(),
        }
    }

    pub fn focus(&self, f: Field) {
        unsafe { SetFocus(self.handle(f)) };
    }

    pub fn hide(&mut self) {
        if self.shown {
            unsafe {
                ShowWindow(self.email, SW_HIDE);
                ShowWindow(self.pwd, SW_HIDE);
            }
            self.shown = false;
        }
    }
}

impl Drop for Edits {
    fn drop(&mut self) {
        unsafe {
            if IsWindow(self.email) != 0 {
                DestroyWindow(self.email);
            }
            if IsWindow(self.pwd) != 0 {
                DestroyWindow(self.pwd);
            }
        }
    }
}
