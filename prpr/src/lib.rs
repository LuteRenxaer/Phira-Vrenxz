pub mod bin;
pub mod config;
pub mod core;
pub mod dir;
pub mod ext;
pub mod fs;
pub mod info;
pub mod judge;
pub mod parallel;
pub mod parse;
pub mod particle;
pub mod scene;
pub mod task;
pub mod time;
pub mod ui;

#[cfg(feature = "log")]
pub mod log;

#[rustfmt::skip]
#[cfg(closed)]
pub mod inner;

pub use scene::Main;

pub fn build_conf() -> macroquad::window::Conf {
    macroquad::window::Conf {
        window_title: "Phira-Vrenxz".to_string(),
        window_width: 973,
        window_height: 608,
        ..Default::default()
    }
}

/// 运行时切换垂直同步间隔（0 = 关闭 vsync / 1 = 锁定到刷新率）。
///
/// miniquad 只在启动时读取 `Conf::swap_interval`，运行中切换需要直接调用
/// `wglSwapIntervalEXT`（Windows）；其它平台目前无可行的运行时切换 API，记录警告。
#[cfg(target_os = "windows")]
pub fn set_swap_interval(interval: i32) {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const u8) -> *mut c_void;
    }
    unsafe {
        let module = LoadLibraryA(b"opengl32.dll\0".as_ptr());
        if module.is_null() {
            tracing::warn!("set_swap_interval: failed to load opengl32.dll");
            return;
        }
        let addr = GetProcAddress(module, b"wglSwapIntervalEXT\0".as_ptr());
        if addr.is_null() {
            tracing::warn!("set_swap_interval: wglSwapIntervalEXT not available");
            return;
        }
        let f: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(addr);
        f(interval);
    }
}

/// 非 Windows 平台暂不支持运行时切换 vsync
#[cfg(not(target_os = "windows"))]
pub fn set_swap_interval(_interval: i32) {
    tracing::warn!("set_swap_interval: runtime vsync switching not supported on this platform");
}
