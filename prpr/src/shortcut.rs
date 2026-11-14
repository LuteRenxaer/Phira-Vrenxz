//! Text-field clipboard shortcuts that do not depend on macroquad's key state.
//!
//! Windows IME / some keyboard layouts swallow Ctrl+A/C/X/V from the normal
//! key event stream.  `GetAsyncKeyState` still sees the physical keys, so text
//! fields use this as a fallback (macroquad key handling stays as-is for the
//! other platforms).

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    SelectAll,
    Copy,
    Cut,
    Paste,
}

#[cfg(target_os = "windows")]
pub fn poll() -> Option<Shortcut> {
    use std::cell::RefCell;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_A, VK_C, VK_CONTROL, VK_V, VK_X,
    };

    #[inline]
    fn down(vk: u16) -> bool {
        unsafe { GetAsyncKeyState(vk as i32) as u16 & 0x8000 != 0 }
    }

    thread_local! {
        static PREV: RefCell<[bool; 4]> = const { RefCell::new([false; 4]) };
    }

    let ctrl = down(VK_CONTROL);
    let now = [down(VK_A), down(VK_C), down(VK_X), down(VK_V)];
    PREV.with(|prev| {
        let mut prev = prev.borrow_mut();
        let mut result = None;
        if ctrl {
            for (index, pressed) in now.iter().enumerate() {
                if *pressed && !prev[index] {
                    result = Some(match index {
                        0 => Shortcut::SelectAll,
                        1 => Shortcut::Copy,
                        2 => Shortcut::Cut,
                        _ => Shortcut::Paste,
                    });
                    break;
                }
            }
        }
        *prev = now;
        result
    })
}

#[cfg(not(target_os = "windows"))]
pub fn poll() -> Option<Shortcut> {
    None
}
