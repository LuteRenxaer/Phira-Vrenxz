//! Windows 原生单行输入点（EDIT 控件），挂在游戏主窗口上。
//!
//! 每个逻辑字段一个稳定 id（"login_email" / "setup_pwd" / "mp_chat" …），首次用到时
//! 惰性建一个 1px 宽的隐藏 EDIT，之后复用。它只负责键盘：系统输入法、光标、选区、
//! Ctrl+A/C/V、密码掩码全由系统处理；画面仍然由游戏自己画（原来的深色圆角输入框），
//! 游戏再用 [`draw_selection`] 把选区和光标画回字段上。

use macroquad::prelude::Rect;
use prpr::ui::Ui;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::Mutex;

type Handle = isize;

const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const ES_AUTOHSCROLL: u32 = 0x0080;
const ES_PASSWORD: u32 = 0x0020;

const WM_SETFONT: u32 = 0x0030;
const WM_GETTEXTLENGTH: u32 = 0x000E;
const EM_SETLIMITTEXT: u32 = 0x00C5;
const EM_SETSEL: u32 = 0x00B1;
const EM_GETSEL: u32 = 0x00B0;

const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;

const VK_RETURN: i32 = 0x0D;
const VK_ESCAPE: i32 = 0x1B;

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
    fn GetFocus() -> Handle;
    fn GetWindowTextW(h: Handle, buf: *mut u16, max: i32) -> i32;
    fn SetWindowTextW(h: Handle, s: *const u16) -> i32;
    fn SendMessageW(h: Handle, msg: u32, w: usize, l: isize) -> isize;
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

fn get_text(h: Handle) -> String {
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

fn put_text(h: Handle, s: &str) {
    let w = wide(s);
    unsafe {
        SetWindowTextW(h, w.as_ptr());
    }
}

fn create_edit(parent: Handle, instance: Handle, password: bool, font: Handle, multiline: bool) -> Handle {
    let mut style = WS_CHILD | WS_VISIBLE | ES_AUTOHSCROLL;
    if password {
        style |= ES_PASSWORD;
    }
    if multiline {
        // ES_MULTILINE | ES_WANTRETURN：回车在框内换行，不走对话框的"确认"
        style |= 0x0000_0004 | 0x0000_1000 | 0x0000_0040;
    }
    let class = wide("EDIT");
    let h = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            style,
            -200,
            -200,
            1,
            1,
            parent,
            0,
            instance,
            std::ptr::null_mut(),
        )
    };
    if h != 0 {
        unsafe {
            SendMessageW(h, WM_SETFONT, font as usize, 1);
            SendMessageW(h, EM_SETLIMITTEXT, 4096, 0);
        }
    }
    h
}

struct Slot {
    h: Handle,
    cache: String,
    shown: bool,
    multiline: bool,
}

struct Inputs {
    parent: Handle,
    font: Handle,
    slots: HashMap<&'static str, Slot>,
    used: Vec<&'static str>,
    focus_req: Option<&'static str>,
    enter_down: bool,
    escape_down: bool,
}

impl Inputs {
    fn new(parent: Handle) -> Option<Self> {
        if parent == 0 || unsafe { IsWindow(parent) } == 0 {
            return None;
        }
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            // 父窗口加 WS_CLIPCHILDREN，避免 GL 每帧重绘把子窗口刷掉
            const GWL_STYLE: i32 = -16;
            const WS_CLIPCHILDREN: i32 = 0x0200_0000;
            let style = GetWindowLongW(parent, GWL_STYLE);
            if style & WS_CLIPCHILDREN == 0 {
                SetWindowLongW(parent, GWL_STYLE, style | WS_CLIPCHILDREN);
            }
            let face = wide("Segoe UI");
            let font = CreateFontW(
                -30, 0, 0, 0, 400, 0, 0, 0, 1, 0, 0, 0, 0, face.as_ptr(),
            );
            Some(Self {
                parent,
                font,
                slots: HashMap::new(),
                used: Vec::new(),
                focus_req: None,
                enter_down: false,
                escape_down: false,
            })
        }
    }

    fn slot(&mut self, id: &'static str, masked: bool, multiline: bool) -> &mut Slot {
        if !self.slots.contains_key(id) {
            let h = create_edit(self.parent, self.instance(), masked, self.font, multiline);
            self.slots.insert(id, Slot { h, cache: String::new(), shown: false, multiline });
        }
        self.slots.get_mut(id).unwrap()
    }

    fn instance(&self) -> Handle {
        unsafe { GetModuleHandleW(std::ptr::null()) }
    }
}

impl Drop for Inputs {
    fn drop(&mut self) {
        unsafe {
            for slot in self.slots.values() {
                if IsWindow(slot.h) != 0 {
                    DestroyWindow(slot.h);
                }
            }
        }
    }
}

static INPUTS: Mutex<Option<Inputs>> = Mutex::new(None);
static FRAME_HAS_INPUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 一帧开始时调用，清空"本帧用到"的字段列表
pub fn begin_frame() {
    FRAME_HAS_INPUT.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut g) = INPUTS.lock() {
        if let Some(it) = g.as_mut() {
            it.used.clear();
            it.focus_req = None;
        }
    }
}

/// 每帧最后调用一次。若这一帧根本没有任何输入字段（游玩、结算、过场……），
/// 就把所有隐形输入点收起来、把键盘焦点还给游戏窗口——否则残留的输入点会
/// 吞掉游玩按键。
pub fn end_of_frame() {
    if FRAME_HAS_INPUT.swap(false, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    if let Ok(mut g) = INPUTS.lock() {
        let Some(inp) = g.as_mut() else { return };
        let parent = inp.parent;
        for slot in inp.slots.values_mut() {
            if slot.shown {
                unsafe { ShowWindow(slot.h, SW_HIDE) };
                slot.shown = false;
            }
        }
        unsafe {
            if GetFocus() != parent {
                SetFocus(parent);
            }
        }
    }
}

/// 同步某个字段：系统框里改了文本就写回游戏，游戏里清了文本就写进系统框。
/// `focus` 表示这一帧要不要把键盘焦点交给它。返回文本是否被系统侧改过。
pub fn edit(id: &'static str, text: &mut String, focus: bool, masked: bool, multiline: bool) -> bool {
    let Ok(mut g) = INPUTS.lock() else { return false };
    if g.is_none() {
        let parent = crate::MAIN_HWND.load(std::sync::atomic::Ordering::Relaxed);
        *g = Inputs::new(parent);
    }
    let Some(inp) = g.as_mut() else { return false };
    let slot = inp.slot(id, masked, multiline);
    let win = get_text(slot.h);
    let win = if multiline { win.replace("\r\n", "\n") } else { win };
    let changed = win != slot.cache;
    if changed {
        *text = win.clone();
        slot.cache = win;
    } else if *text != slot.cache {
        let s = if multiline { text.replace('\n', "\r\n") } else { text.clone() };
        put_text(slot.h, &s);
        slot.cache = text.clone();
    }
    if !inp.used.contains(&id) {
        inp.used.push(id);
    }
    if focus {
        inp.focus_req = Some(id);
    }
    changed
}

/// 一帧结束时调用：用到的字段显示成 1px 隐形输入点，其余隐藏；焦点按请求分配
pub fn end_frame() {
    if let Ok(mut g) = INPUTS.lock() {
        let Some(inp) = g.as_mut() else { return };
        let used: Vec<&'static str> = inp.used.clone();
        let parent = inp.parent;
        for (id, slot) in inp.slots.iter_mut() {
            let on = used.contains(id);
            if on != slot.shown {
                unsafe { ShowWindow(slot.h, if on { SW_SHOW } else { SW_HIDE }) };
                slot.shown = on;
            }
            if on {
                unsafe {
                    SetWindowPos(slot.h, 0, 1, 1, 1, 1, SWP_NOZORDER | SWP_NOACTIVATE);
                }
            }
        }
        match inp.focus_req {
            Some(id) => {
                if let Some(slot) = inp.slots.get(id) {
                    unsafe { SetFocus(slot.h) };
                }
            }
            None => unsafe {
                // 没有输入目标时把焦点还给游戏窗口，否则键盘事件全被隐藏框吃掉
                if GetFocus() != parent {
                    SetFocus(parent);
                }
            },
        }
    }
}

/// 通用输入对话框用：同步文本、取选区、取回车边沿，一帧内自洽（begin/edit/end）
pub fn dialog_input(text: &mut String, masked: bool, multiline: bool) -> prpr::scene::DialogInputResult {
    let id: &'static str = if multiline { "dialog_multi" } else { "dialog" };
    begin_frame();
    let changed = edit(id, text, true, masked, multiline);
    let sel = selection(id);
    let caret = caret(id);
    let enter = if multiline { false } else { enter_pressed() };
    let escape = escape_pressed();
    end_frame();
    prpr::scene::DialogInputResult { changed, sel, caret, enter, escape }
}

/// 对话框「全选」按钮
pub fn dialog_select_all() {
    select_all("dialog");
    select_all("dialog_multi");
}

/// 对话框关闭：隐藏输入点、焦点交回游戏窗口
pub fn dialog_release() {
    begin_frame();
    end_frame();
}

/// 某个字段的光标位置（按字符计）
pub fn caret(id: &'static str) -> usize {
    let g = match INPUTS.lock() {
        Ok(g) => g,
        Err(_) => return 0,
    };
    let Some(slot) = g.as_ref().and_then(|it| it.slots.get(id)) else { return 0 };
    let mut start: u32 = 0;
    let mut end: u32 = 0;
    unsafe {
        SendMessageW(
            slot.h,
            EM_GETSEL,
            &mut start as *mut u32 as usize,
            &mut end as *mut u32 as isize,
        );
    }
    start as usize
}

/// 某个字段当前的选区（按字符计，非空才算）
pub fn selection(id: &'static str) -> Option<(usize, usize)> {
    let g = INPUTS.lock().ok()?;
    let inp = g.as_ref()?;
    let slot = inp.slots.get(id)?;
    let mut start: u32 = 0;
    let mut end: u32 = 0;
    unsafe {
        SendMessageW(
            slot.h,
            EM_GETSEL,
            &mut start as *mut u32 as usize,
            &mut end as *mut u32 as isize,
        );
    }
    let (s, e) = (start as usize, end as usize);
    if s >= e {
        None
    } else {
        Some((s, e))
    }
}

/// 选中整段（Ctrl+A、双击、切换账号回填后由游戏层调用）
pub fn select_all(id: &'static str) {
    if let Ok(g) = INPUTS.lock() {
        if let Some(slot) = g.as_ref().and_then(|it| it.slots.get(id)) {
            unsafe { SendMessageW(slot.h, EM_SETSEL, 0, -1) };
        }
    }
}

/// 该字段是否处于全选状态
pub fn is_all_selected(id: &'static str, len: usize) -> bool {
    matches!(selection(id), Some((0, e)) if e >= len)
}

/// 读一次物理回车键的"刚按下"边沿。系统框拿走焦点后 macroquad 收不到 Enter，
/// 所以这里直接问系统键状态。
pub fn enter_pressed() -> bool {
    unsafe {
        let down = GetAsyncKeyState(VK_RETURN) < 0;
        let Ok(mut g) = INPUTS.lock() else { return false };
        let Some(inp) = g.as_mut() else { return false };
        let edge = down && !inp.enter_down;
        inp.enter_down = down;
        edge
    }
}

/// 读一次物理 Esc 的"刚按下"边沿（同样是为了绕过被系统框吃掉的键盘事件）
pub fn escape_pressed() -> bool {
    unsafe {
        let down = GetAsyncKeyState(VK_ESCAPE) < 0;
        let Ok(mut g) = INPUTS.lock() else { return false };
        let Some(inp) = g.as_mut() else { return false };
        let edge = down && !inp.escape_down;
        inp.escape_down = down;
        edge
    }
}

/// 把选区/光标画回游戏自绘的字段上：选区铺半透明蓝，光标是竖线（带闪烁）
pub fn draw_caret_and_selection(
    ui: &mut Ui,
    r: Rect,
    text: &str,
    masked: bool,
    sel: Option<(usize, usize)>,
    size: f32,
    t: f32,
) {
    let display: Vec<char> = if masked {
        std::iter::repeat('*').take(text.chars().count()).collect()
    } else {
        text.chars().collect()
    };
    let n = display.len();
    let w_of = |ui: &mut Ui, chars: &[char]| -> f32 {
        if chars.is_empty() {
            return 0.;
        }
        let s: String = chars.iter().collect();
        ui.text(s)
            .pos(r.x + 0.03, r.center().y)
            .anchor(0., 0.5)
            .no_baseline()
            .size(size)
            .max_width(r.w - 0.06)
            .measure()
            .w
    };
    match sel {
        Some((s, e)) if s < e && n > 0 => {
            let s = s.min(n);
            let e = e.min(n);
            if s >= e {
                return;
            }
            let x0 = w_of(ui, &display[..s]);
            let w = w_of(ui, &display[..e]) - x0;
            ui.fill_rect(
                Rect::new(r.x + 0.03 + x0, r.y + 0.022, w.max(0.005), r.h - 0.044),
                macroquad::color::Color::new(0.36, 0.60, 1.0, 0.45),
            );
        }
        _ => {
            // 光标：只在没有选区时画，闪烁 0.6s 亮 / 0.4s 灭
            if (t * 2.).fract() > 0.7 {
                return;
            }
            let caret = match sel {
                Some((s, _)) => s.min(n),
                None => n,
            };
            let x = w_of(ui, &display[..caret]);
            ui.fill_rect(
                Rect::new(r.x + 0.03 + x, r.y + 0.024, 0.0035, r.h - 0.048),
                macroquad::color::WHITE,
            );
        }
    }
}

#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(vk: i32) -> i16;
}
