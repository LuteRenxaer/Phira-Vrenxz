//! 调试工具：只渲染主页背景，并把渲染结果存成 PNG。
//!
//! 主页背景指 `MainScene::render` 里画的那一层 `splash_ui::Backdrop`：不透明底色
//! + 左侧主图（含竖图时的模糊补边）+ 右侧半圆黑场 / TriGrid + 飘动粒子。
//! 字标、菜单、图标、用户名、FPS 等 UI 一律跳过。
//!
//! 环境变量：
//! - `PHIRA_BG_SHOT`：输出 PNG 的路径。不设置时本模块完全不生效。
//! - `PHIRA_BG_SHOT_SIZE`：画面尺寸，形如 `2560x1440`（默认沿用 1600x740 窗口）。
//! - `PHIRA_BG_SHOT_DELAY`：主页首次开始绘制后等待多少秒再截（默认 3，等过渡/淡入结束）。
//!
//! 截图写盘后进程直接退出（走 TerminateProcess，避免 gilrs 线程析构的退出 panic）。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;

struct Shot {
    path: String,
    delay: f32,
}

static SHOT: OnceLock<Shot> = OnceLock::new();
static WINDOW: OnceLock<(i32, i32)> = OnceLock::new();
static DONE: AtomicBool = AtomicBool::new(false);
/// 主页第一帧的时间戳（毫秒）；0 表示还没画过主页。
static FIRST_MS: AtomicU64 = AtomicU64::new(0);
static LAST_LOG: AtomicU64 = AtomicU64::new(u64::MAX);

/// 读取环境变量；窗口创建前和 `the_main` 各调用一次（OnceLock 保证幂等）。
pub fn init() {
    if SHOT.get().is_some() {
        return;
    }
    let Ok(path) = std::env::var("PHIRA_BG_SHOT") else {
        return;
    };
    let delay = std::env::var("PHIRA_BG_SHOT_DELAY")
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .unwrap_or(3.0);
    if let Some(size) = std::env::var("PHIRA_BG_SHOT_SIZE")
        .ok()
        .and_then(|v| parse_size(&v))
    {
        let _ = WINDOW.set(size);
    }
    let _ = SHOT.set(Shot {
        path: path.clone(),
        delay,
    });
    tracing::info!("bg shot mode: {path} after {delay}s of home page");
}

fn parse_size(s: &str) -> Option<(i32, i32)> {
    let (w, h) = s.split_once(['x', 'X'])?;
    let w = w.trim().parse::<i32>().ok()?;
    let h = h.trim().parse::<i32>().ok()?;
    (w > 0 && h > 0).then_some((w, h))
}

pub fn active() -> bool {
    SHOT.get().is_some() && !DONE.load(Ordering::Relaxed)
}

/// 截图用的窗口尺寸（未配置时为 None，沿用默认窗口）。
pub fn window_size() -> Option<(i32, i32)> {
    WINDOW.get().copied()
}

/// 每帧在背景画完之后调用：主页稳定后把当前帧存成 PNG，然后退出。
pub fn capture_after_backdrop() {
    let Some(shot) = SHOT.get() else {
        return;
    };
    if DONE.load(Ordering::Relaxed) {
        return;
    }

    let now_ms = (macroquad::time::get_time() * 1000.0) as u64;
    let first = FIRST_MS.load(Ordering::Relaxed);
    if first == 0 {
        FIRST_MS.store(now_ms.max(1), Ordering::Relaxed);
        return;
    }
    if now_ms.saturating_sub(first) < (shot.delay.max(0.0) * 1000.0) as u64 {
        return;
    }

    let art_ready = crate::splash_ui::key_art().is_some();
    // 每秒打一行状态，便于确认卡在哪个条件上。
    let sec = now_ms / 1000;
    if LAST_LOG.swap(sec, Ordering::Relaxed) != sec {
        eprintln!("[bgshot] t={sec}s art_ready={art_ready}");
    }
    // 主图是异步装载的，没装载好之前不截，避免存到一张空底板。
    if !art_ready {
        return;
    }

    DONE.store(true, Ordering::Relaxed);
    let img = macroquad::texture::get_screen_data();
    if let Some(dir) = std::path::Path::new(&shot.path).parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    img.export_png(&shot.path);
    eprintln!(
        "[bgshot] saved {} ({}x{})",
        shot.path, img.width, img.height
    );
    hard_exit();
}

#[cfg(target_os = "windows")]
fn hard_exit() -> ! {
    use std::ffi::c_void;
    extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn TerminateProcess(process: *mut c_void, code: u32) -> i32;
    }
    // 图片已经落盘；直接结束进程，避开退出时的线程析构 panic。
    unsafe {
        TerminateProcess(GetCurrentProcess(), 0);
    }
    std::process::exit(0)
}

#[cfg(not(target_os = "windows"))]
fn hard_exit() -> ! {
    std::process::exit(0)
}
