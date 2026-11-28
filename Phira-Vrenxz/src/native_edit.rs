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

/// IME 组合窗样式：只给一个锚点（候选窗就贴在这一点下面）
const CFS_POINT: u32 = 0x0002;

#[repr(C)]
#[derive(Clone, Copy)]
struct CompositionForm {
    style: u32,
    current_pos: [i32; 2],
    area: [i32; 4],
}

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
    fn ClientToScreen(h: Handle, pt: *mut i32) -> i32;
    fn SetForegroundWindow(h: Handle) -> i32;
    fn GetForegroundWindow() -> Handle;
    fn GetParent(h: Handle) -> Handle;
    fn SetParent(h: Handle, parent: Handle) -> Handle;
    fn GetClassNameW(h: Handle, buf: *mut u16, max: i32) -> i32;
}

/// 把键盘焦点真正交给某个字段：先直接 SetFocus；拿不到（游戏窗口被点过、
/// 或不在前台）就把主窗口提到前台再试一次。返回最终是否拿到了焦点。
fn force_focus(h: Handle, parent: Handle) -> bool {
    unsafe {
        if GetFocus() != h {
            SetFocus(h);
        }
        if GetFocus() != h {
            if GetForegroundWindow() != parent {
                SetForegroundWindow(parent);
            }
            SetFocus(h);
        }
        GetFocus() == h
    }
}

#[link(name = "imm32")]
extern "system" {
    fn ImmGetContext(h: Handle) -> Handle;
    fn ImmSetCompositionWindow(himc: Handle, form: *const CompositionForm) -> i32;
    fn ImmReleaseContext(h: Handle, himc: Handle) -> i32;
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

/// 游戏内输入对话框的「输入框在窗口里的像素位置」(x, y, w, h)。
/// 隐形输入点挪到那儿去，系统输入法的候选窗才会贴到输入框旁边，
/// 而不是钉在窗口左上角。
static DIALOG_PLACE: Mutex<Option<(i32, i32, i32, i32)>> = Mutex::new(None);

/// 由 prpr 每帧告知输入框的像素位置（客户区坐标，左上原点）
pub fn dialog_place(x: f32, y: f32, w: f32, h: f32) {
    if let Ok(mut g) = DIALOG_PLACE.lock() {
        *g = Some((x as i32, y as i32, w.max(2.) as i32, h.max(2.) as i32));
    }
}

fn slot_hwnd(id: &'static str) -> Option<Handle> {
    let g = INPUTS.lock().ok()?;
    g.as_ref().and_then(|it| it.slots.get(id)).map(|it| it.h)
}
static FRAME_HAS_INPUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// 对话框（"dialog"/"dialog_multi"）当前是否占着键盘。开着的时候，页面自己在渲染里
/// 调 begin_frame/end_frame 也不能把焦点抢回去。
static DIALOG_LATCH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

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
    if DIALOG_LATCH.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
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
    // 对话框开着的时候，页面自己的帧（比如房间页 render 里的 begin_frame/end_frame）
    // 不能在对话框之后又把焦点抢走：抢走以后按键落到聊天框或被还给游戏窗口，
    // 对话框下一帧把字符队列一清，就表现为「弹窗输入不了」。
    if DIALOG_LATCH.load(std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    end_frame_unguarded();
}

fn end_frame_unguarded() {
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
                    force_focus(slot.h, parent);
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
    DIALOG_LATCH.store(true, std::sync::atomic::Ordering::Relaxed);
    begin_frame();
    let changed = edit(id, text, true, masked, multiline);
    let sel = selection(id);
    let caret = caret(id);
    let enter = if multiline { false } else { enter_pressed() };
    let escape = escape_pressed();
    // 对话框自己这一帧必须落实焦点，不能走带 latch 检查的通用版本
    end_frame_unguarded();
    // 关键：end_frame 会把所有输入点收回 (1,1)，所以必须在它之后再挪，否则
    // 系统输入法候选窗仍然锚在窗口左上角。挪到输入框上以后候选窗就贴着输入框弹。
    place_dialog_edit(id);
    // 原生框有没有真正拿到键盘焦点：拿不到就得让 prpr 退回游戏内按键处理，
    // 否则两边都不收键，表现就是「输入框输入不了」。
    let mut focused = false;
    if let (Some(h), Some(parent)) = (
        slot_hwnd(id),
        INPUTS.lock().ok().and_then(|g| g.as_ref().map(|it| it.parent)),
    ) {
        focused = force_focus(h, parent);
        // 拿不到焦点通常说明输入点挂错了父窗口（例如真正前台的不是这个 HWND）：
        // 改挂到当前前台窗口再要一次焦点。
        if !focused {
            unsafe {
                let fg = GetForegroundWindow();
                if fg != 0 && GetParent(h) != fg {
                    SetParent(h, fg);
                }
                focused = force_focus(h, fg);
            }
        }
    }
    prpr::scene::DialogInputResult { changed, sel, caret, enter, escape, focused }
}

/// 上次给 IME 设过的锚点：位置没变就不重复设，免得打字时候选窗每帧被重置。
static LAST_PLACE: Mutex<Option<(Handle, i32, i32)>> = Mutex::new(None);

fn place_dialog_edit(id: &'static str) {
    let (Some(slot_h), Some((x, y, w, h))) =
        (slot_hwnd(id), DIALOG_PLACE.lock().ok().and_then(|g| *g))
    else {
        return;
    };
    unsafe {
        SetWindowPos(slot_h, 0, x, y, 1, 1, SWP_NOZORDER | SWP_NOACTIVATE);
        let mut pt = [0i32, 0i32];
        ClientToScreen(slot_h, pt.as_mut_ptr());
        // 锚点放在输入框左侧偏下一点，候选窗会展开在它下面
        let anchor = (pt[0] + 4, pt[1] + (h / 2).max(4));
        let stale = LAST_PLACE
            .lock()
            .map(|g| match *g {
                Some((lh, lx, ly)) => lh != slot_h || lx != anchor.0 || ly != anchor.1,
                None => true,
            })
            .unwrap_or(true);
        if !stale {
            return;
        }
        let himc = ImmGetContext(slot_h);
        if himc != 0 {
            let form = CompositionForm {
                style: CFS_POINT,
                current_pos: [anchor.0, anchor.1],
                area: [0; 4],
            };
            ImmSetCompositionWindow(himc, &form);
            ImmReleaseContext(slot_h, himc);
        }
        if let Ok(mut g) = LAST_PLACE.lock() {
            *g = Some((slot_h, anchor.0, anchor.1));
        }
    }
}

/// 对话框「全选」按钮
pub fn dialog_select_all() {
    select_all("dialog");
    select_all("dialog_multi");
}

/// 对话框关闭：隐藏输入点、焦点交回游戏窗口
pub fn dialog_release() {
    DIALOG_LATCH.store(false, std::sync::atomic::Ordering::Relaxed);
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
