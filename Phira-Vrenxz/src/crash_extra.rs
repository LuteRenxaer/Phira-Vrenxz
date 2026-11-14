//! Panic classification + crash log (ported from Phira-Firefly).
//!
//! The crash page itself already exists in `prpr`; this module only turns a raw
//! panic into a readable code/reason and appends a searchable `crash.log`
//! (including a backtrace) under the data directory.

use prpr::scene::CrashCode;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

const CODE_RENDER: u32 = 6001;
const CODE_AUDIO: u32 = 6002;
const CODE_RESOURCE: u32 = 6003;
const CODE_MEMORY: u32 = 6004;
const CODE_NETWORK: u32 = 6005;
const CODE_UNKNOWN: u32 = 6010;

fn classify(message: &str) -> (u32, &'static str) {
    let lower = message.to_lowercase();
    if [
        "texture", "render", "opengl", "gpu", "shader", "draw", "framebuffer",
    ]
    .iter()
    .any(|it| lower.contains(it))
    {
        (CODE_RENDER, "游戏渲染引擎发生错误，建议更新显卡驱动或降低画质后重试。")
    } else if ["audio", "sound", "music", "playback", "sfx", "sasa"]
        .iter()
        .any(|it| lower.contains(it))
    {
        (CODE_AUDIO, "音频系统发生错误，请检查音频设备是否被其他程序占用。")
    } else if [
        "load", "file", "assets", "resource", "missing", "not found", "不存在",
    ]
    .iter()
    .any(|it| lower.contains(it))
    {
        (CODE_RESOURCE, "游戏资源加载失败，可能是文件损坏或缺失。")
    } else if ["memory", "alloc", "out of memory", "failed to allocate"]
        .iter()
        .any(|it| lower.contains(it))
    {
        (CODE_MEMORY, "系统内存不足，请关闭其他程序后重试。")
    } else if [
        "network", "connection", "http", "request", "timeout", "dns", "socket",
    ]
    .iter()
    .any(|it| lower.contains(it))
    {
        (CODE_NETWORK, "网络连接异常，无法与服务器通信。")
    } else {
        (CODE_UNKNOWN, "无法归类的游戏程序错误。")
    }
}

fn log_path() -> String {
    crate::dir::root()
        .map(|root| format!("{root}/crash.log"))
        .unwrap_or_else(|_| "crash.log".to_string())
}

/// Called from the panic hook; never panics or allocates the crash page.
pub fn record_panic(message: &str, location: Option<String>) {
    let (code, reason) = classify(message);
    let backtrace = std::backtrace::Backtrace::force_capture();
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|it| it.as_secs())
        .unwrap_or(0);
    let text = format!(
        "========== Phira-Vrenxz crash ==========
time: {timestamp}
code: {code}
reason: {reason}
location: {}
panic: {message}

backtrace:
{backtrace}
",
        location.as_deref().unwrap_or("unknown")
    );
    let path = log_path();
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut file| file.write_all(text.as_bytes()));
}

/// Build the code/reason pair shown by the existing crash page.
pub fn make_crash_code(message: &str) -> CrashCode {
    let (code, reason) = classify(message);
    CrashCode::Custom {
        code,
        reason: format!("{reason}\n{message}"),
    }
}
