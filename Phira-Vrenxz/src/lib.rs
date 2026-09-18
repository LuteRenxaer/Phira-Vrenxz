prpr_l10n::tl_file!("common" ttl crate::);

#[rustfmt::skip]
#[cfg(closed)]
mod inner;

mod anim;
mod achievement;
mod blue_archive_tips;
mod censor;
mod charts_view;
mod client;
mod data;
mod extension;
mod icons;
mod images;
mod lanzou;
mod login;
mod migrate;
mod mp;
mod page;
mod popup;
mod rate;
mod resource;
mod scene;
mod tabs;
mod tags;
mod threed;
mod uml;
mod xcsim;

use anyhow::Result;
use data::Data;
use macroquad::prelude::*;
use prpr::{
    build_conf,
    core::{init_assets, PGR_FONT},
    ext::SafeTexture,
    log,
    scene::{show_error, CrashCode},
    time::TimeManager,
    ui::{cleanup_audio, FontArc, TextPainter},
    Main,
};
use prpr_l10n::set_prefered_locale;
#[cfg(not(feature = "hykb"))]
use prpr_l10n::{GLOBAL, LANGS};
use scene::{LoginScene, SetupScene, StartupLoadingScene, StudioLogoScene};
use std::{
    any::Any,
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex,
    },
};
use tracing::{error, info};

#[cfg(target_os = "android")]
use jni::{
    objects::{JClass, JString},
    sys::jint,
    EnvUnowned,
};

static MESSAGES_TX: Mutex<Option<mpsc::Sender<bool>>> = Mutex::new(None);
static DATA_PATH: Mutex<Option<String>> = Mutex::new(None);
static CACHE_DIR: Mutex<Option<String>> = Mutex::new(None);
pub static mut DATA: Option<Data> = None;

/// Set once a panic has been routed to the crash screen, so repeated panics
/// (e.g. inside the crash scene itself) cannot push it again.
static CRASH_SCENE_SHOWN: AtomicBool = AtomicBool::new(false);

/// Extract the message from a caught panic payload.
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

/// 返回应用可写数据目录（Android = `getFilesDir()`，由 MainActivity 通过 JNI 设置）。
#[cfg(target_os = "android")]
pub(crate) fn writable_data_dir() -> Option<String> {
    DATA_PATH.lock().unwrap().clone()
}

pub fn save_data() -> Result<()> {
    // 旧版本数据刚同步进 data/ 时（见 crate::migrate）：本次运行内存里还是「上一份」数据，
    // 写回去就把同步结果盖掉了 —— 同步完成后唯一的出路是重启，重启前一律不落盘。
    if migrate::migrated() {
        return Ok(());
    }
    std::fs::write(format!("{}/data.json", dir::root()?), serde_json::to_string(get_data())?)?;
    Ok(())
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
        let s = format!("{}/{}", DATA_PATH.lock().unwrap().as_ref().map(|it| it.as_str()).unwrap_or("."), s);
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
}

async fn the_main() -> Result<()> {
    log::register();

    // 设置全局 panic hook，捕获所有线程的 panic
    std::panic::set_hook(Box::new(|info| {
        let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_owned()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            format!("{:?}", info.payload())
        };
        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        error!("panic occurred: {} at {}", message, location.unwrap_or_else(|| "unknown".to_string()));
    }));

    #[cfg(target_env = "ohos")]
    {
        *DATA_PATH.lock().unwrap() = Some("/data/storage/el2/base".to_owned());
        *CACHE_DIR.lock().unwrap() = Some("/data/storage/el2/base/cache".to_owned());
        prpr::core::DPI_VALUE.store(250, std::sync::atomic::Ordering::Relaxed);
    };

    init_assets();

    // Android：把进程工作目录切到应用私有目录（init_assets 之后执行，避免被覆盖）。
    // Java 侧已把 APK 内置资源 zip（Expansion/Ontology）复制到 <getFilesDir()>/assets，
    // Rust 相对路径 "assets/..."（内置谱面加载等）需指向该可写目录。
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
    let mut data: Data = std::fs::read_to_string(format!("{dir}/data.json"))
        .map_err(anyhow::Error::new)
        .and_then(|s| Ok(serde_json::from_str(&s)?))
        .unwrap_or_default();
    data.init().await?;
    set_data(data);
    sync_data();
    save_data()?;

    // 初始化成就系统
    {
        let data_dir = std::path::PathBuf::from(dir::root()?);
        if let Err(e) = achievement::init_manager(&data_dir) {
            tracing::warn!(?e, "初始化成就系统失败");
        }
    }



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

    let pgr_font = FontArc::try_from_vec(load_file("fonts/phigros.ttf").await?)?;
    PGR_FONT.with(move |it| *it.borrow_mut() = Some(TextPainter::new(pgr_font, None)));

    let font = FontArc::try_from_vec(load_file("fonts/font.ttf").await?)?;
    let mut painter = TextPainter::new(font.clone(), None);

    let first_scene: Box<dyn prpr::scene::Scene> = {
        // 三选一：有启动画面就走启动页（它点完把玩家交给首启向导）；关掉启动画面时，
        // 首次启动仍然直接进向导 —— 「第一次该问的几件事」不该因为关了一个显示开关就被跳过。
        let inner: Box<dyn prpr::scene::Scene> = if get_data().show_startup_screen {
            Box::new(LoginScene::new(font))
        } else if !get_data().initial_setup_done {
            Box::new(SetupScene::new(font))
        } else {
            Box::new(StartupLoadingScene::new(font))
        };
        Box::new(StudioLogoScene::new(inner).await)
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
                if is_key_pressed(KeyCode::F11) {
                    let data = get_data_mut();
                    data.config.fullscreen_mode = !data.config.fullscreen_mode;
                    set_fullscreen_mode(data.config.fullscreen_mode);
                    let _ = save_data();
                }
                main.update()?;
                main.render(&mut painter)?;
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
                    let crash_code = CrashCode::from_panic_message(&message);
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



        next_frame().await;
    }
    // 退出前清理音频，防止退出后音频还在播放
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

    conf
}

#[cfg(target_os = "windows")]
extern "system" {
    fn AllocConsole() -> i32;
    fn FreeConsole() -> i32;
}

#[cfg(target_os = "windows")]
static CONSOLE_ALLOCATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 切换调试控制台（仅 Windows 有效；其它平台为无操作，保证设置页可编译）。
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

/// 通过传入的 JNIEnv 把 Java String 转为 Rust String。
///
/// 不能直接 `path.to_string()`（jni 的 Display 需要 `JavaVM::singleton` 已初始化，
/// 而 setDataPath 等会在 miniquad 引擎启动前被调用，那时 singleton 尚未就绪，
/// 会得到占位字符串 `<JNI Not Initialized>`）。这里用 JNI 函数表直接读取。
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

/// 初始化 rustls-platform-verifier（Android 证书校验需要先 init，并且 APK 里
/// 必须带 rustls-platform-verifier-android 的 Java 类）。在拥有 JNI env 的
/// 早期 native 调用里执行一次即可（ndk_context 的 Activity 已由 initializeContext 设置）。
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

/// 深链接（phira://room/join|create/<...>?server=...）多人启动参数。
/// Java 侧保证三个参数都不为 null（空字符串表示未提供），这里只暂存，
/// 等玩家进入多人场景、连上服务器后再由多人会话自动加入/创建房间。
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

/// Credentials obtained from the native HYKB (好游快爆) login SDK.
pub struct HykbCredential {
    /// SDK result code: 0 on success, otherwise an error / user cancellation.
    pub code: i32,
    pub uid: i64,
    pub nick: String,
    pub access_token: String,
}

impl HykbCredential {
    /// Map the SDK result code to an error, or yield the credential on success.
    /// Centralizes the code → user-facing message translation shared by every
    /// HYKB login/bind entry point.
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

/// Slot for the pending HYKB login result. The native callback fulfills it.
static HYKB_TX: Mutex<Option<tokio::sync::oneshot::Sender<HykbCredential>>> = Mutex::new(None);

/// Call a no-arg `void` method on the Android host activity (the HYKB shell).
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