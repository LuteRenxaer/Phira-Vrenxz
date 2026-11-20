prpr_l10n::tl_file!("common" ttl crate::);

#[rustfmt::skip]
#[cfg(closed)]
mod inner;

mod anim;
mod bgshot;
mod tips;
mod censor;
mod charts_view;
mod client;
mod data;
pub(crate) mod data_transfer;
mod extension;
mod icons;
mod images;
mod lanzou;
mod lan;
mod login;
mod migrate;
mod mp;
mod pack;
mod page;
mod plugin;
mod plugins;
mod ppsr_features;
mod popup;
mod rate;
mod recorder;
mod resource;
mod score_upload;
mod scene;
mod splash_ui;
mod crash_extra;
mod tabs;
mod tags;
mod threed;
mod uml;
mod xcsim;
mod xcsim_theme;

use anyhow::Context;
use anyhow::Result;
use data::Data;
use macroquad::prelude::*;
use prpr::{
    build_conf,
    core::{init_assets, MAIN_FONT, PGR_FONT},
    ext::SafeTexture,
    log,
    scene::{show_error, show_message, CrashCode},
    time::TimeManager,
    ui::{cleanup_audio, FontArc, TextPainter},
    Main,
};
use prpr_l10n::set_prefered_locale;
#[cfg(not(feature = "hykb"))]
use prpr_l10n::{GLOBAL, LANGS};
use scene::{SetupScene, StudioLogoScene};
use std::{
    any::Any,
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicIsize, Ordering},
        mpsc, Mutex,
    },
};
use tracing::{error, info, warn};

#[cfg(target_os = "android")]
use jni::{
    objects::{JClass, JString},
    sys::jint,
    EnvUnowned,
};

#[cfg(target_os = "windows")]
pub static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
static MESSAGES_TX: Mutex<Option<mpsc::Sender<bool>>> = Mutex::new(None);
static DATA_PATH: Mutex<Option<String>> = Mutex::new(None);
static CACHE_DIR: Mutex<Option<String>> = Mutex::new(None);
pub static mut DATA: Option<Data> = None;

/// UI 显示版本（协议版本仍用 CARGO_PKG_VERSION）
pub const APP_VERSION: &str = "1.3.3PRO";
static CRASH_SCENE_SHOWN: AtomicBool = AtomicBool::new(false);

/// 拖拽导入谱面完成后置位，主场景 update 消费后刷新内存谱面列表（与手动导入一致）。
pub static DROPPED_IMPORT_DIRTY: AtomicBool = AtomicBool::new(false);

fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        format!("{payload:?}")
    }
}

#[cfg(target_env = "ohos")]
use napi_derive_ohos::napi;

#[cfg(closed)]
pub async fn load_res(name: &str) -> Vec<u8> {
    let bytes = load_file(name).await.unwrap();
    inner::resolve_data(bytes)
}

#[allow(unused)]
pub async fn load_res_tex(name: &str) -> SafeTexture {
    #[cfg(closed)]
    {
        let bytes = load_res(name).await;
        let image = image::load_from_memory(&bytes).unwrap();
        image.into()
    }
    #[cfg(not(closed))]
    prpr::ext::BLACK_TEXTURE.clone()
}

pub fn sync_data() {
    if get_data().language.is_none() {
        #[cfg(feature = "hykb")]
        let default_lang = "zh-CN".to_owned();
        #[cfg(not(feature = "hykb"))]
        let default_lang = LANGS[GLOBAL.order.lock().unwrap()[0]].to_owned();
        get_data_mut().language = Some(default_lang);
    }
    set_prefered_locale(get_data().language.as_ref().and_then(|it| it.parse().ok()));
    let _ = client::set_access_token_sync(get_data().tokens.as_ref().map(|it| &*it.0));
}

pub fn set_data(data: Data) {
    unsafe {
        DATA = Some(data);
    }
}

#[allow(static_mut_refs)]
pub fn get_data() -> &'static Data {
    unsafe { DATA.as_ref().unwrap() }
}

#[allow(static_mut_refs)]
pub fn get_data_mut() -> &'static mut Data {
    unsafe { DATA.as_mut().unwrap() }
}

#[cfg(target_os = "android")]
pub(crate) fn writable_data_dir() -> Option<String> {
    DATA_PATH.lock().unwrap().clone()
}

pub fn save_data() -> Result<()> {

    if migrate::migrated() {
        return Ok(());
    }
    let root = match dir::root().with_context(|| "save_data: resolve root dir") {
        Ok(root) => root,
        Err(err) => {
            error!(?err, "save_data: cannot resolve data root, skip saving");
            return Ok(());
        }
    };
    let path = format!("{root}/data.json");
    let body = match serde_json::to_string(get_data()) {
        Ok(body) => body,
        Err(err) => {
            error!(?err, "save_data: cannot serialize data, skip saving");
            return Ok(());
        }
    };
    match std::fs::write(&path, body.as_bytes()).with_context(|| format!("save_data: write {path}")) {
        Ok(()) => {}
        Err(err) => {
            // 有的机器上目标文件的 ACL 会比目录更严（写不进去但目录可写），
            // 先删后建通常能绕过，成功了就继续，失败也不让游戏退出。
            if std::fs::remove_file(&path).is_ok()
                && std::fs::write(&path, body.as_bytes()).is_ok()
            {
                warn!(?err, "save_data: rewritten after removing stale file");
            } else {
                error!(?err, "save_data failed, keep running without persisting");
            }
        }
    }
    Ok(())
}

/// 轮询引擎层接收到的拖拽文件路径（桌面端支持）。
/// 每帧调用，返回本轮新拖入的文件路径列表。
#[cfg(target_os = "windows")]
fn poll_dropped_files() -> Vec<String> {
    let mut paths = Vec::new();
    let quad_ctx = unsafe { get_internal_gl() }.quad_context;
    let count = quad_ctx.dropped_file_count();
    for i in 0..count {
        if let Some(path) = quad_ctx.dropped_file_path(i) {
            if let Some(s) = path.to_str() {
                paths.push(s.to_owned());
            }
        }
    }
    quad_ctx.clear_dropped_files();
    // 调试辅助：--drop-test=<zip> 在首帧模拟一次拖拽，走完整导入+刷新链路
    #[cfg(debug_assertions)]
    {
        use std::sync::atomic::AtomicBool;
        static DROP_TEST_TAKEN: AtomicBool = AtomicBool::new(false);
        if !DROP_TEST_TAKEN.swap(true, Ordering::Relaxed) {
            for arg in std::env::args().skip(1) {
                if let Some(p) = arg.strip_prefix("--drop-test=") {
                    paths.push(p.to_owned());
                }
            }
        }
    }
    if !paths.is_empty() {
        eprintln!("[drop-debug] poll got: {paths:?}");
    }
    paths
}

/// 导入拖入的谱面压缩包（zip）到自定义谱面目录，并刷新内存列表。
async fn import_dropped_chart(path: String) {
    let err_ctx = || format!("导入谱面失败: {path}");
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(err) => {
            show_error(anyhow::Error::new(err).context(err_ctx()));
            return;
        }
    };
    match scene::import_chart(file).await {
        Ok((chart, warnings)) => {
            eprintln!("[drop-debug] import ok: {}", chart.info.name);
            {
                let data = get_data_mut();
                data.charts.push(chart);
            }
            let _ = save_data();
            DROPPED_IMPORT_DIRTY.store(true, Ordering::Relaxed);
            if let Some(w) = scene::parse_warnings_to_string(&warnings) {
                show_message(format!("已导入谱面\n{w}")).warn();
            } else {
                show_message("已导入谱面").warn();
            }
        }
        Err(err) => show_error(err.context(err_ctx())),
    }
}

#[cfg(target_os = "windows")]
extern "C" {
    fn FindWindowW(lpClassName: *const u16, lpWindowName: *const u16) -> *mut std::ffi::c_void;
    fn ShowWindow(hWnd: *mut std::ffi::c_void, nCmdShow: i32) -> bool;
}

pub fn set_fullscreen_mode(fullscreen: bool) {
    macroquad::window::set_fullscreen(fullscreen);
    #[cfg(target_os = "windows")]
    unsafe {
        let class: Vec<u16> = "Shell_TrayWnd\0".encode_utf16().collect();
        let hwnd = FindWindowW(class.as_ptr(), std::ptr::null());
        if !hwnd.is_null() {
            ShowWindow(hwnd, if fullscreen { 0 } else { 5 });
        }
    }
}

mod dir {
    use anyhow::Result;

    use crate::{CACHE_DIR, DATA_PATH};

    fn ensure(s: &str) -> Result<String> {
        // 绝对路径（如 cache_image_local 传入的 DATA_PATH/cache/image）直接使用，
        // 避免再次拼接 DATA_PATH 变成 F:\xxx/F:\xxx/... 触发 os error 123
        let s = if std::path::Path::new(s).is_absolute() {
            s.to_owned()
        } else {
            let data_path = DATA_PATH.lock().unwrap();
            let prefix = data_path.as_ref().map(|it| it.as_str()).unwrap_or(".");
            format!("{prefix}/{s}")
        };
        let path = std::path::Path::new(&s);
        if !path.exists() {
            std::fs::create_dir_all(path)?;
        }
        Ok(s)
    }

    pub fn cache() -> Result<String> {
        if let Some(cache) = &*CACHE_DIR.lock().unwrap() {
            ensure(cache)
        } else {
            ensure("cache")
        }
    }

    pub fn bold_font_path() -> Result<String> {
        Ok(format!("{}/bold.ttf", root()?))
    }

    pub fn cache_image_local() -> Result<String> {
        ensure(&format!("{}/image", cache()?))
    }

    pub fn root() -> Result<String> {
        ensure("data")
    }

    pub fn charts() -> Result<String> {
        ensure("data/charts")
    }

    pub fn collections() -> Result<String> {
        ensure("data/collections")
    }

    pub fn custom_charts() -> Result<String> {
        ensure("data/charts/custom")
    }

    pub fn downloaded_charts() -> Result<String> {
        ensure("data/charts/download")
    }

    pub fn respacks() -> Result<String> {
        ensure("data/respack")
    }

    pub fn fonts() -> Result<String> {
        ensure("data/fonts")
    }
}

async fn the_main() -> Result<()> {
    log::register();
    bgshot::init();

    std::panic::set_hook(Box::new(|info| {
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_owned()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            format!("{:?}", info.payload())
        };
        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        crash_extra::record_panic(&message, location.clone());
        let bt = std::backtrace::Backtrace::force_capture();
        error!(
            "panic occurred: {} at {}\n{}",
            message,
            location.clone().unwrap_or_else(|| "unknown".to_string()),
            bt
        );
        // 桌面端 exe 没有控制台，日志看不到；把 panic 现场单独落盘，方便排查 6010 这类崩溃。
        let _ = std::panic::catch_unwind(|| {
            if let Ok(root) = dir::root() {
                let path = std::path::Path::new(&root).join("crash_panic.log");
                let text = format!(
                    "panic: {message}\nat: {}\n{bt}\n\n",
                    location.unwrap_or_else(|| "unknown".to_string()),
                    bt = std::backtrace::Backtrace::force_capture()
                );
                use std::io::Write as _;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
                    let _ = f.write_all(text.as_bytes());
                }
            }
        });
    }));

    #[cfg(target_env = "ohos")]
    {
        *DATA_PATH.lock().unwrap() = Some("/data/storage/el2/base".to_owned());
        *CACHE_DIR.lock().unwrap() = Some("/data/storage/el2/base/cache".to_owned());
        prpr::core::DPI_VALUE.store(250, std::sync::atomic::Ordering::Relaxed);
    };

    init_assets();
    #[cfg(target_os = "android")]
    {
        // 兜底：Android 首次启动若 assets 复制不完整，直接写入内置背景，避免初始化失败。
        // assets 位于 DATA_PATH/assets（与 scene::assets_root 一致），不在 DATA_PATH/data 下。
        let bg = crate::scene::assets_root().join("backgrounds/background.jpg");
        if !bg.exists() {
            if let Some(parent) = bg.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&bg, include_bytes!("../../assets/backgrounds/background.jpg"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        // 双击 exe 直接启动时 cwd 可能不在项目根（如 target\\release），
        // 从 cwd / exe 目录向上多级探测真正的 data 目录，避免读到空的 data.json 与谱面。
        if DATA_PATH.lock().unwrap().is_none() {
            let mut bases: Vec<std::path::PathBuf> = Vec::new();
            if let Ok(cwd) = std::env::current_dir() {
                bases.push(cwd);
            }
            if let Some(exe_dir) = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            {
                bases.push(exe_dir);
            }
            let mut seen = std::collections::HashSet::new();
            'outer: for base in bases {
                let mut cur = base;
                for _ in 0..=5 {
                    let data_dir = cur.join("data");
                    let has_charts_dir = data_dir.join("charts").is_dir()
                        && std::fs::read_dir(data_dir.join("charts"))
                            .map(|mut it| it.next().is_some())
                            .unwrap_or(false);
                    let has_charts_in_json = std::fs::read_to_string(data_dir.join("data.json"))
                        .ok()
                        .and_then(|s| {
                            serde_json::from_str::<serde_json::Value>(&s)
                                .ok()
                                .and_then(|v| v.get("charts").and_then(|c| c.as_array()).map(|a| a.len()))
                        })
                        .map(|n| n > 0)
                        .unwrap_or(false);
                    let is_real = has_charts_dir || has_charts_in_json;
                    if !seen.contains(&data_dir) && is_real {
                        let root_s = cur.to_string_lossy().into_owned();
                        *DATA_PATH.lock().unwrap() = Some(root_s.clone());
                        // 顺带切换 cwd 到数据根，修复 assets/cache 等其它相对路径依赖
                        if let Err(err) = std::env::set_current_dir(&cur) {
                            error!("failed to set current dir to {root_s}: {err:?}");
                        }
                        if let Ok(cwd) = std::env::current_dir() {
                            prpr::scene::set_loading_art_root(cwd.join("data/charts"));
                        }
                        break 'outer;
                    }
                    seen.insert(data_dir);
                    if !cur.pop() {
                        break;
                    }
                }
            }
        }
    }

    #[cfg(target_os = "android")]
    {
        if let Some(dir) = DATA_PATH.lock().unwrap().clone() {
            if let Err(err) = std::env::set_current_dir(&dir) {
                error!("failed to set current dir to {dir}: {err:?}");
            }
        }
    }

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();

    #[cfg(target_os = "ios")]
    {
        use objc2_foundation::{NSSearchPathDirectory, NSSearchPathDomainMask, NSSearchPathForDirectoriesInDomains};

        let directories = NSSearchPathForDirectoriesInDomains(NSSearchPathDirectory::LibraryDirectory, NSSearchPathDomainMask::UserDomainMask, true);
        let path = directories.firstObject().unwrap().to_string();
        *DATA_PATH.lock().unwrap() = Some(path);
        *CACHE_DIR.lock().unwrap() = Some("Caches".to_owned());
    }

    let dir = dir::root()?;
    prpr::hud::set_root(dir.clone());
    let mut data: Data = std::fs::read_to_string(format!("{dir}/data.json"))
        .map_err(anyhow::Error::new)
        .and_then(|s| Ok(serde_json::from_str(&s)?))
        .unwrap_or_default();
    data.init().await?;
    set_data(data);
    plugins::init();
    prpr::scene::GLOBAL_OVERLAY.with(|f| *f.borrow_mut() = Some(crate::plugins::render_global));
    sync_data();
    save_data()?;


    tokio::spawn(censor::preload());

    let rx = {
        let (tx, rx) = mpsc::channel();
        *MESSAGES_TX.lock().unwrap() = Some(tx);
        rx
    };

    unsafe { get_internal_gl() }
        .quad_context
        .display_mut()
        .set_pause_resume_listener(on_pause_resume);

    #[cfg(target_os = "windows")]
    if let Some(hwnd) = unsafe { get_internal_gl() }
        .quad_context
        .display()
        .native_window_handle()
    {
        crate::MAIN_HWND.store(hwnd, Ordering::Relaxed);
        info!("cached main hwnd: {hwnd:#x}");
    } else {
        info!("failed to obtain main hwnd via native_window_handle");
    }

    let pgr_font = FontArc::try_from_vec(load_file("fonts/phigros.ttf").await?)?;
    PGR_FONT.with(move |it| *it.borrow_mut() = Some(TextPainter::new(pgr_font, None)));

    // 主字体：优先使用玩家导入的自定义字体，否则回退到内置字体
    let builtin_font = FontArc::try_from_vec(load_file("fonts/font.ttf").await?)?;
    let (main_font, fallback) = {
        let data = get_data();
        let custom = data
            .font_id
            .checked_sub(1)
            .and_then(|i| data.fonts.get(i))
            .and_then(|f| dir::fonts().ok().map(|root| format!("{root}/{}", f.file)))
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| FontArc::try_from_vec(bytes).ok());
        match custom {
            Some(font) => (font, Some(builtin_font.clone())),
            None => (builtin_font.clone(), None),
        }
    };
    MAIN_FONT.with(move |it| *it.borrow_mut() = Some(TextPainter::new(main_font, fallback)));

    let first_scene: Box<dyn prpr::scene::Scene> = {

        // 与 Phigros 一致：开屏（StudioLogoScene）为唯一启动场景，LoginScene 已删除并合并入开屏，
        // 主界面在开屏播放期间后台加载完成，结束后直接进入；仅首次设置保留前置 SetupScene。
        let inner: Option<Box<dyn prpr::scene::Scene>> = if !get_data().initial_setup_done {
            Some(Box::new(SetupScene::new(builtin_font.clone())))
        } else {
            None
        };


        let studio = StudioLogoScene::new(inner, builtin_font).await;
        #[cfg(not(target_os = "android"))]
        {
            Box::new(studio)
        }
        #[cfg(target_os = "android")]
        {
            Box::new(studio)
        }
    };
    let mut main = Main::new(first_scene, TimeManager::default(), None).await?;

    let tm = TimeManager::default();
    let mut fps_time = -1;

    const FPS_BUF_SIZE: usize = 60;
    let mut fps_times = VecDeque::<f32>::with_capacity(FPS_BUF_SIZE);
    let mut last_frame_start = f32::NAN;
    let mut fps_time_sum = 0.;

    let mut paused = false;

    #[cfg(target_os = "windows")]
    if get_data().config.fullscreen_mode {
        set_fullscreen_mode(true);
    }

    #[cfg(target_os = "windows")]
    if get_data().config.console_enabled {
        crate::set_console_enabled(true);
    }

    'app: loop {
        let frame_start = tm.real_time();
        if !last_frame_start.is_nan() {
            if fps_times.len() == FPS_BUF_SIZE {
                fps_time_sum -= fps_times.pop_front().unwrap();
            }
            let frame_time = frame_start as f32 - last_frame_start;
            fps_times.push_back(frame_time);
            fps_time_sum += frame_time;
        }
        last_frame_start = frame_start as f32;
        // Catch panics on the main thread so they enter the crash screen
        // instead of unwinding through the C boundary and aborting the app.
        let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
            let signal = if paused {
                rx.recv_timeout(std::time::Duration::from_secs(1)).ok()
            } else {
                rx.try_recv().ok()
            };
            if let Some(msg) = signal {
                paused = msg;
                if msg {
                    main.pause()?;
                } else {
                    main.resume()?;
                }
            }
            if !paused {
                #[cfg(target_os = "windows")]
                {
                    // 拖拽导入谱面：把拖入窗口的 zip 导入自定义谱面
                    let dropped = poll_dropped_files();
                    if !dropped.is_empty() {
                        for path in dropped {
                            tokio::spawn(import_dropped_chart(path));
                        }
                    }
                }
                if is_key_pressed(KeyCode::F11) {
                    let data = get_data_mut();
                    data.config.fullscreen_mode = !data.config.fullscreen_mode;
                    set_fullscreen_mode(data.config.fullscreen_mode);
                    let _ = save_data();
                }
                if is_key_pressed(KeyCode::F10) {
                    recorder::recorder().lock().unwrap().toggle();
                }
                if recorder::take_render_on_next_play() {
                    recorder::recorder().lock().unwrap().start();
                }
                main.update()?;
                {
                    let data = get_data();
                    let avg = if fps_times.is_empty() {
                        0.
                    } else {
                        1. / (fps_time_sum / fps_times.len() as f32)
                    };
                    prpr::scene::set_fps_overlay(
                        data.config.show_fps,
                        data.config.fps_position,
                        avg,
                    );
                }
                let render_res = MAIN_FONT.with(|it| {
                    let mut guard = it.borrow_mut();
                    match guard.as_mut() {
                        Some(painter) => main.render(painter),
                        None => Ok(()),
                    }
                });
                render_res?;
                recorder::recorder().lock().unwrap().capture_frame();
            }
            prpr::ext::flush_pending_texture_deletions();
            Ok(())
        }));
        let res = match res {
            Ok(res) => res,
            Err(payload) => {
                let message = panic_message(&*payload);
                error!("caught panic on main thread: {message}");
                if !CRASH_SCENE_SHOWN.swap(true, Ordering::SeqCst) {
                    let crash_code = crash_extra::make_crash_code(&message);
                    let entered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        main.enter_crash_scene(crash_code, ttl!("crash-unexpected").into_owned())
                    }));
                    match entered {
                        Ok(Ok(())) => {}
                        Ok(Err(err)) => error!("failed to enter crash scene: {err:?}"),
                        Err(payload) => error!("crash scene itself panicked: {}", panic_message(&*payload)),
                    }
                }
                Ok(())
            }
        };
        if let Err(err) = res {
            error!("uncaught error: {err:?}");
            show_error(err);
        }
        if main.should_exit() {
            break 'app;
        }

        let t = tm.real_time();

        let fps_now = t as i32;
        if fps_now != fps_time {
            fps_time = fps_now;
            if fps_times.len() == FPS_BUF_SIZE {
                let actual_fps = 1. / (fps_time_sum / FPS_BUF_SIZE as f32);
                let current_fps = 1. / (t - frame_start);
                info!("FPS {} (capped at {})", current_fps as u32, actual_fps as u32);
            }
        }

        if let Some(limit) = Some(get_data().config.fps_limit).filter(|v| *v > 0) {
            let target = 1. / limit as f64;
            let elapsed = tm.real_time() - frame_start as f64;
            if elapsed < target {
                std::thread::sleep(std::time::Duration::from_secs_f64(target - elapsed));
            }
        }
        next_frame().await;
    }

    prpr::ui::cleanup_audio();
    Ok(())
}

fn build_global_window_conf() -> Conf {
    let mut conf = build_conf();
    conf.window_title = "Phira-Vrenxz".to_owned();
    conf.icon = Some(miniquad::conf::Icon {
        small: *include_bytes!("../icon/small"),
        medium: *include_bytes!("../icon/medium"),
        big: *include_bytes!("../icon/big"),
    });

    #[cfg(target_os = "windows")]
    {
        let data = dir::root()
            .ok()
            .and_then(|r| std::fs::read_to_string(std::path::Path::new(&r).join("data.json")).ok())
            .and_then(|s| serde_json::from_str::<Data>(&s).ok());
        conf.fullscreen = data.as_ref().is_some_and(|d| d.config.fullscreen_mode);
        conf.platform.swap_interval = Some(
            if data.as_ref().is_some_and(|d| d.config.vsync && !d.config.eff_vsync_off()) { 1 } else { 0 },
        );
    }

    // 只渲染背景截图模式：按环境变量指定画面尺寸，并强制窗口化。
    if let Some((w, h)) = bgshot::window_size() {
        conf.window_width = w;
        conf.window_height = h;
        conf.fullscreen = false;
    }

    conf
}

#[cfg(target_os = "windows")]
extern "system" {
    fn AllocConsole() -> i32;
    fn FreeConsole() -> i32;
}

#[cfg(target_os = "windows")]
static CONSOLE_ALLOCATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_console_enabled(enabled: bool) {
    #[cfg(target_os = "windows")]
    {
        use std::sync::atomic::Ordering;
        let was_allocated = CONSOLE_ALLOCATED.load(Ordering::SeqCst);
        if enabled && !was_allocated {
            unsafe { AllocConsole(); }
            CONSOLE_ALLOCATED.store(true, Ordering::SeqCst);
        } else if !enabled && was_allocated {
            unsafe { FreeConsole(); }
            CONSOLE_ALLOCATED.store(false, Ordering::SeqCst);
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = enabled;
}

#[no_mangle]
pub extern "C" fn quad_main() {
    // 窗口尺寸必须在创建窗口前确定，所以这里先解析只渲染背景截图的环境变量。
    bgshot::init();
    macroquad::Window::from_config(build_global_window_conf(), async {
        if let Err(err) = the_main().await {
            error!(?err, "global error");
        }
    });
    cleanup_audio();
}

fn on_pause_resume(pause: bool) {
    if let Some(tx) = MESSAGES_TX.lock().unwrap().as_mut() {
        let _ = tx.send(pause);
    }
}

#[cfg(target_os = "android")]
fn android_jstring_to_string(env: &mut jni::EnvUnowned, s: &jni::objects::JString) -> String {
    use jni::Outcome;
    match env
        .with_env_no_catch(|env| -> jni::errors::Result<String> {
            let s = env.get_string(s)?;
            Ok(s.to_string())
        })
        .into_outcome()
    {
        Outcome::Ok(s) => s,
        _ => String::new(),
    }
}

#[cfg(target_os = "android")]
fn init_rustls_platform_verifier(mut env: EnvUnowned) {
    use jni::Outcome;
    let outcome = env
        .with_env_no_catch(|env| -> jni::errors::Result<()> {
            let ctx_ptr = ndk_context::android_context().context();
            if ctx_ptr.is_null() {
                return Ok(());
            }
            let ctx = unsafe { jni::objects::JObject::from_raw(env, ctx_ptr as jni::sys::jobject) };
            rustls_platform_verifier::android::init_with_env(env, ctx)?;
            Ok(())
        })
        .into_outcome();
    match outcome {
        Outcome::Ok(_) => {
            tracing::info!("rustls-platform-verifier initialized");
        }
        Outcome::Err(err) => {
            tracing::error!("rustls-platform-verifier init failed: {err:?}");
        }
        Outcome::Panic(_) => {}
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_initializeEnvironment(env: EnvUnowned, _class: JClass) {
    unsafe {
        inputbox::backend::Android::initialize_raw(env.as_raw()).unwrap();
    }
    init_rustls_platform_verifier(env);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_prprActivityOnPause(_env: EnvUnowned, _class: JClass) {
    if let Some(tx) = MESSAGES_TX.lock().unwrap().as_mut() {
        let _ = tx.send(true);
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_prprActivityOnResume(_env: EnvUnowned, _class: JClass) {
    if let Some(tx) = MESSAGES_TX.lock().unwrap().as_mut() {
        let _ = tx.send(false);
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_prprActivityOnDestroy(_env: EnvUnowned, _class: JClass) {
    std::process::exit(0);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_setDataPath(mut env: EnvUnowned, _class: JClass, path: JString) {
    let path = android_jstring_to_string(&mut env, &path);
    *DATA_PATH.lock().unwrap() = Some(path);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_setTempDir(mut env: EnvUnowned, _class: JClass, path: JString) {
    let path = android_jstring_to_string(&mut env, &path);
    std::env::set_var("TMPDIR", path.clone());
    *CACHE_DIR.lock().unwrap() = Some(path);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_setDpi(_env: EnvUnowned, _class: JClass, dpi: jint) {
    prpr::core::DPI_VALUE.store(dpi as _, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_setChosenFile(mut env: EnvUnowned, _class: JClass, file: JString) {
    use prpr::scene::CHOSEN_FILE;
    CHOSEN_FILE.lock().unwrap().1 = Some(android_jstring_to_string(&mut env, &file));
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_markImport(_env: EnvUnowned, _class: JClass) {
    use prpr::scene::CHOSEN_FILE;

    CHOSEN_FILE.lock().unwrap().0 = Some("_import".to_owned());
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_markImportRespack(_env: EnvUnowned, _class: JClass) {
    use prpr::scene::CHOSEN_FILE;

    CHOSEN_FILE.lock().unwrap().0 = Some("_import_respack".to_owned());
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_markAutoImport(_env: EnvUnowned, _class: JClass) {
    use prpr::scene::CHOSEN_FILE;

    CHOSEN_FILE.lock().unwrap().0 = Some("_import_auto".to_owned());
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_setStartupArgs(
    mut env: EnvUnowned,
    _class: JClass,
    join: JString,
    create: JString,
    server: JString,
) {
    let join = android_jstring_to_string(&mut env, &join);
    let create = android_jstring_to_string(&mut env, &create);
    let server = android_jstring_to_string(&mut env, &server);
    crate::mp::set_pending_room_link(crate::mp::PendingRoomLink {
        join: if join.is_empty() { None } else { Some(join) },
        create: if create.is_empty() { None } else { Some(create) },
        server: if server.is_empty() { None } else { Some(server) },
    });
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_setInputText(mut env: EnvUnowned, _class: JClass, text: JString) {
    use prpr::scene::INPUT_TEXT;
    INPUT_TEXT.lock().unwrap().1 = Some(android_jstring_to_string(&mut env, &text));
}

pub struct HykbCredential {

    pub code: i32,
    pub uid: i64,
    pub nick: String,
    pub access_token: String,
}

impl HykbCredential {

    #[cfg(feature = "hykb")]
    pub fn ok_or_err(self) -> Result<Self> {
        if self.code == 0 {
            Ok(self)
        } else {

            force_logout();
            anyhow::bail!("{}", crate::ttl!("hykb-login-cancelled"))
        }
    }
}

static HYKB_TX: Mutex<Option<tokio::sync::oneshot::Sender<HykbCredential>>> = Mutex::new(None);

#[cfg(all(target_os = "android", feature = "hykb"))]
fn call_activity_void(method: &'static jni::strings::JNIStr) {
    use jni::{jni_sig, objects::JObject, vm::JavaVM};

    JavaVM::singleton()
        .unwrap()
        .attach_current_thread(|env| -> jni::errors::Result<()> {
            let ctx = unsafe { JObject::from_raw(env, ndk_context::android_context().context() as _) };
            env.call_method(ctx, method, jni_sig!("()V"), &[])?;
            Ok(())
        })
        .unwrap();
}

/// Ask the Android shell to pop the HYKB account picker (`MainActivity.hykbSwitchAccount`).
/// Used by the explicit login / switch-account flow.
#[cfg(all(target_os = "android", feature = "hykb"))]
fn request_hykb_login() {
    call_activity_void(jni::jni_str!("hykbSwitchAccount"));
}

#[cfg(not(all(target_os = "android", feature = "hykb")))]
fn request_hykb_login() {}

/// Ask the Android shell to sign in using the cached HYKB account without
/// popping the picker (`MainActivity.hykbLogin`). The credentials the SDK
/// reports flow back through `HYKB_TX`, so the caller can verify them against
/// the restored Phira session. Used by the silent startup restore.
#[cfg(all(target_os = "android", feature = "hykb"))]
fn request_hykb_login_silent() {
    call_activity_void(jni::jni_str!("hykbLogin"));
}

#[cfg(not(all(target_os = "android", feature = "hykb")))]
fn request_hykb_login_silent() {}

/// Tell the native HYKB SDK to sign out (`MainActivity.hykbLogout`). Called when the
/// player logs out from their profile.
#[cfg(all(target_os = "android", feature = "hykb"))]
pub fn hykb_logout() {
    call_activity_void(jni::jni_str!("hykbLogout"));
}

#[cfg(not(all(target_os = "android", feature = "hykb")))]
pub fn hykb_logout() {}

/// Tear down the local session: sign out of the native HYKB SDK, clear the
/// stored account and tokens, then re-sync. Shared by every path that must
/// reject a login — a failed/cancelled HYKB verification, a uid mismatch, or
/// the player logging out from their profile.
pub fn force_logout() {
    hykb_logout();
    get_data_mut().me = None;
    get_data_mut().tokens = None;
    let _ = save_data();
    sync_data();
}

/// Trigger the native HYKB login and await its credentials.
#[allow(unused)]
pub async fn obtain_hykb_credential() -> Result<HykbCredential> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    *HYKB_TX.lock().unwrap() = Some(tx);
    request_hykb_login();
    let cred = rx.await.map_err(|_| anyhow::anyhow!("hykb login cancelled"))?;
    Ok(cred)
}

/// Silently restore the HYKB session from the cached account and await its
/// credentials. Unlike [`obtain_hykb_credential`], this does not pop the account
/// picker; used by the blocking startup check to verify the restored session.
#[allow(unused)]
pub async fn obtain_hykb_credential_silent() -> Result<HykbCredential> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    *HYKB_TX.lock().unwrap() = Some(tx);
    request_hykb_login_silent();
    let cred = rx.await.map_err(|_| anyhow::anyhow!("hykb login cancelled"))?;
    Ok(cred)
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_quad_1native_QuadNative_hykbLoginCallback(
    mut env: EnvUnowned,
    _class: JClass,
    code: jint,
    uid: jni::sys::jlong,
    nick: JString,
    access_token: JString,
) {
    let nick = android_jstring_to_string(&mut env, &nick);
    let access_token = android_jstring_to_string(&mut env, &access_token);
    if let Some(tx) = HYKB_TX.lock().unwrap().take() {
        let _ = tx.send(HykbCredential {
            code: code as i32,
            uid: uid as i64,
            nick,
            access_token,
        });
    } else if code == 2005 {

    }
}

#[cfg(target_env = "ohos")]
#[napi]
pub fn set_input_text(text: String) {
    use prpr::scene::INPUT_TEXT;
    INPUT_TEXT.lock().unwrap().1 = Some(text);
}

#[cfg(target_env = "ohos")]
#[napi]
pub fn set_chosen_file(file: String) {
    use prpr::scene::CHOSEN_FILE;
    CHOSEN_FILE.lock().unwrap().1 = Some(file);
}

#[cfg(target_env = "ohos")]
#[napi]
pub fn mark_auto_import() {
    use prpr::scene::CHOSEN_FILE;
    CHOSEN_FILE.lock().unwrap().0 = Some("_import_auto".to_owned());
}

#[cfg(target_env = "ohos")]
#[napi]
pub fn on_foreground() {
    if let Some(tx) = MESSAGES_TX.lock().unwrap().as_mut() {
        let _ = tx.send(false);
    }
}

#[cfg(target_env = "ohos")]
#[napi]
pub fn on_background() {
    if let Some(tx) = MESSAGES_TX.lock().unwrap().as_mut() {
        let _ = tx.send(true);
    }
}

#[cfg(target_os = "android")]
pub extern "C" fn android_main(app: &mut macroquad::Window) {
    quad_main();
}

#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_quad_1native_QuadNative_preprocessInput(
    _: *mut std::ffi::c_void,
    _: *const std::ffi::c_void,
    #[allow(dead_code)] motionEvent: ndk_sys::AInputEvent,
    #[allow(dead_code)] f: jni::sys::jfloat,
    #[allow(dead_code)] f2: jni::sys::jfloat,
    #[allow(dead_code)] z: jni::sys::jboolean,
    #[allow(dead_code)] z2: jni::sys::jboolean,
) {

}