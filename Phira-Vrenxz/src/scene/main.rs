use super::{import_chart, L10N_LOCAL, SongScene};
use async_trait::async_trait;
use prpr::fs::FileSystem;
use prpr::config::Config;
use prpr::scene::{LoadingScene, GameScene, EndingScene, GameMode};
use prpr::ext::poll_future;
use std::pin::Pin;
use std::future::Future;

// 空的 FileSystem 实现，用于 Debug 场景
struct DummyFileSystem;

#[async_trait]
impl FileSystem for DummyFileSystem {
    async fn load_file(&mut self, path: &str) -> anyhow::Result<Vec<u8>> {
        // 返回一些简单的测试数据
        if path == "info.json" {
            return Ok(r#"{
                "name": "示例",
                "difficulty": 114514,
                "level": "X X XXX Lv.114514",
                "charter": "Debug",
                "composer": "Debug",
                "illustrator": "Debug",
                "music": "Ambient #4002.wav",
                "chart": "chart.json",
                "illustration": ""
            }"#.as_bytes().to_vec());
        }
        if path == "chart.json" {
            return Ok(r#"{
                "format": 3,
                "jdge": {
                    "curve": "linear"
                },
                "BPMList": [[0, 120]],
                "judgeLineList": []
            }"#.as_bytes().to_vec());
        }
        Err(anyhow::anyhow!("Dummy file system: {}", path))
    }

    async fn exists(&mut self, path: &str) -> anyhow::Result<bool> {
        Ok(path == "info.json" || path == "chart.json")
    }

    fn list_root(&self) -> anyhow::Result<Vec<String>> {
        Ok(vec!["info.json".to_string(), "chart.json".to_string()])
    }

    fn clone_box(&self) -> Box<dyn FileSystem> {
        Box::new(DummyFileSystem)
    }

    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
use crate::{
    charts_view::NEED_UPDATE,
    data::{FontEntry, LocalChart},
    dir, get_data, get_data_mut,
    mp::{MpSession, MultiplayerScene, MP_SESSION},
    icons::Icons,
    page::{ExportInfo, HomePage, LibraryPage, NextPage, Page, PhigrosLibraryPage, ResPackItem, SharedState, SettingsPage, ExtensionPage, ResPackPage, MessagePage, EventPage, FavoritesPage, WorkbenchPage},
    save_data,
    scene::{confirm_dialog, import_chart_to, parse_warnings_to_string, TEX_BACKGROUND, TEX_ICON_BACK, TEX_ICON_PROCEED},
};
use prpr::scene::{CrashScene, CrashCode};
use anyhow::{anyhow, Context, Result};
use futures_util::join;
use image::GenericImageView;
use macroquad::prelude::*;
use once_cell::sync::Lazy;
use prpr::{
    core::ResPackInfo,
    ext::{unzip_into, RectExt, SafeTexture, ScaleType},
    info::ChartInfo,
    parse::ParseWarnings,
    scene::{return_file, show_error, show_message, take_file, NextScene, Scene},
    task::Task,
    time::TimeManager,
    ui::{back_sound, button_hit, Dialog, FontArc, RectButton, Ui, UI_AUDIO},
};
#[cfg(feature = "video")]
use prpr::core::{Anim, Video};
use sasa::{AudioClip, Music};
use std::{
    any::Any,
    cell::RefCell,
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    mem,
    path::{Component, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread_local,
    time::{Duration, Instant},
};
use tempfile::tempfile;
use uuid::Uuid;

const LOW_PASS: f32 = 0.95;

pub static BGM_VOLUME_UPDATED: AtomicBool = AtomicBool::new(false);

thread_local! {
    static RESPACK_ITEM: RefCell<Option<ResPackItem>> = RefCell::default();
}

#[inline]
fn position_file() -> Result<String> {
    Ok(format!("{}/mp-pos", dir::root()?))
}

async fn load_mp_background() -> Result<SafeTexture> {
    let bytes = load_file("backgrounds/mp_bg.png").await?;
    let img = image::load_from_memory(&bytes)?;
    let img = img.thumbnail(256, 256);
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let mut pixels: Vec<[u8; 3]> = rgb.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
    fastblur::gaussian_blur(&mut pixels, w, h, 24.0);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for p in &pixels {
        rgba.extend_from_slice(&[
            (p[0] as f32 * 0.8) as u8,
            (p[1] as f32 * 0.8) as u8,
            (p[2] as f32 * 0.85) as u8,
            255,
        ]);
    }
    let tex = Texture2D::from_image(&Image {
        width: w as _,
        height: h as _,
        bytes: rgba,
    });
    tex.set_filter(FilterMode::Linear);
    Ok(SafeTexture::from(tex))
}

pub const BUILTIN_HOME_BACKGROUNDS: [&str; 6] = [
    "background.jpg",
    "bg1.png",
    "bg2.png",
    "bg3.png",
    "background_old.jpg",
    "loginbg.png",
];

fn default_home_background() -> &'static str {
    if get_data().config.old_home {
        "backgrounds/background_old.jpg"
    } else {
        "backgrounds/background.jpg"
    }
}

/// 设置页那个下拉该停在第几项：显式选过就停在选的那张，没选过就跟随 `old_home`，
/// 这样下拉显示的就是当前真正在用的那张（所见即所得）。
/// 配置里存了个不认识的名字（手改过 data.json）时同样退回默认，免得下拉显示成空。
pub fn home_background_index() -> usize {
    let selected = get_data().config.home_background.clone();
    let default = if get_data().config.old_home { 4 } else { 0 };
    match selected {
        Some(name) => BUILTIN_HOME_BACKGROUNDS
            .iter()
            .position(|it| *it == name)
            .unwrap_or(default),
        None => default,
    }
}

/// 兜底的警告只打一次：反复切换背景、反复重载不该把日志刷满。
static HOME_BACKGROUND_FALLBACK_WARNED: AtomicBool = AtomicBool::new(false);

fn warn_home_background_fallback(msg: String) {
    if !HOME_BACKGROUND_FALLBACK_WARNED.swap(true, Ordering::Relaxed) {
        warn!("failed to load home background: {msg}, falling back to the default one");
    }
}

/// 装载主页背景（启动装载与设置页改完之后的重新装载共用这一份）。
///
/// 优先级（从高到低）：自定义图片（`custom_background_path`）> 内置选择
/// （`config.home_background`）> `old_home` 决定的默认 —— 选了自定义图就完全
/// 不管内置选择。
///
/// 兜底：自定义图读不出来 / 选了的内置图不存在或解码失败，都退回默认并只警告一次，
/// 绝不让主页因为这个起不来。
/// 同步从 assets 读贴图（不走 macroquad async load_texture，避免在 Task 里重载时黑屏）。
fn load_texture_sync(path: &str) -> Result<SafeTexture> {
    let full = crate::scene::assets_root().join(path);
    let data = std::fs::read(&full).with_context(|| format!("read {}", full.display()))?;
    let img = image::load_from_memory(&data).with_context(|| format!("decode {path}"))?;
    let rgba = img.to_rgba8();
    let (w, h) = img.dimensions();
    Ok(Texture2D::from_rgba8(w as u16, h as u16, &rgba).into())
}

/// 只读背景文件字节（后台线程安全，不碰 OpenGL）。
/// 主线程拿到字节后再用 from_rgba8 创建贴图。
pub async fn load_home_background_bytes() -> Result<Vec<u8>> {
    let custom = get_data().custom_background_path.clone();
    if let Some(custom_path) = custom {
        match std::fs::read(&custom_path) {
            Ok(data) => return Ok(data),
            Err(err) => warn_home_background_fallback(format!("cannot read {custom_path}: {err}")),
        }
    }
    let selected = get_data().config.home_background.clone();
    if let Some(name) = selected {
        let path = format!("backgrounds/{name}");
        let full = crate::scene::assets_root().join(&path);
        match std::fs::read(&full) {
            Ok(data) => return Ok(data),
            Err(err) => warn_home_background_fallback(format!("cannot read {path}: {err:#}")),
        }
    }
    let fallback = default_home_background();
    let full = crate::scene::assets_root().join(fallback);
    Ok(std::fs::read(&full)?)
}

pub async fn load_home_background() -> Result<SafeTexture> {
    // 先把配置拷出来，别让 RefCell 的借用跨过 await
    let custom = get_data().custom_background_path.clone();
    if let Some(custom_path) = custom {
        match std::fs::read(&custom_path) {
            Ok(data) => match image::load_from_memory(&data) {
                Ok(img) => {
                    info!("Loaded custom background from {}", custom_path);
                    let rgba = img.to_rgba8();
                    let (w, h) = img.dimensions();
                    return Ok(Texture2D::from_rgba8(w as u16, h as u16, &rgba).into());
                }
                Err(err) => warn_home_background_fallback(format!("cannot decode {custom_path}: {err}")),
            },
            Err(err) => warn_home_background_fallback(format!("cannot read {custom_path}: {err}")),
        }
    }

    let selected = get_data().config.home_background.clone();
    if let Some(name) = selected {
        let path = format!("backgrounds/{name}");
        match load_texture_sync(&path) {
            Ok(tex) => return Ok(tex),
            Err(err) => warn_home_background_fallback(format!("cannot load {path}: {err:#}")),
        }
    }

    load_texture_sync(default_home_background()).map_err(Into::into)
}

/// 设置页改了主页背景（内置下拉 / 自定义图 / 重置设置）之后置位：
/// 主场景下一帧重新装载并换掉手里那份背景贴图，做到“选完立刻生效”。
pub static BACKGROUND_UPDATED: AtomicBool = AtomicBool::new(false);
pub static HOME_VIDEO_UPDATED: AtomicBool = AtomicBool::new(false);

pub struct MainScene {
    state: SharedState,

    bgm: Option<Music>,

    /// 进入子场景（谱面 / 个人资料 / 多人等）时把主页音乐淡出，回到主场景要再淡入。
    bgm_resume_on_enter: bool,

    /// 所有页面共用的主页底板：左侧主图 + 右侧半圆黑场 + TriGrid + 粒子。
    /// 主页不再自己画一份，避免同一画面叠两层。
    backdrop: crate::splash_ui::Backdrop,
    btn_back: RectButton,
    icon_back: SafeTexture,

    pages: Vec<Box<dyn Page>>,

    import_task: Option<Task<Result<(LocalChart, ParseWarnings)>>>,
    pending_debug_scene: Option<&'static str>,
    debug_scene_task: Option<Pin<Box<dyn Future<Output = Result<NextScene>>>>>,
    pending_debug_scene_result: Option<NextScene>,


    /// 设置页改了背景之后的重新装载任务（见 [`BACKGROUND_UPDATED`]）：
    /// 装好再替换 `background`，中途出错就保持原来那张。
    bg_reload: Option<Task<Result<Vec<u8>>>>,

    /// 左侧主图的重新装载任务：自定义背景 / 内置背景改完之后，
    /// 重新解码并替换全局的 key art（所有页面下一帧就全部换掉）。
    key_art_reload: Option<Task<Result<Vec<u8>>>>,

    /// 主页动态背景视频（导入 MP4 等格式）
    #[cfg(feature = "video")]
    home_video: Option<Video>,
    #[cfg(feature = "video")]
    home_video_start: f64,
    #[cfg(feature = "video")]
    home_video_path: Option<String>,
    #[cfg(feature = "video")]
    home_video_rx: Option<mpsc::Receiver<Vec<u8>>>,

    /// 谱面库页面用的图标集：房间页的「谱面库」按钮会把谱面库直接推到主场景上，
    /// 那时需要一份 `Arc<Icons>`（首页自己那份在 `HomePage` 里拿不到）。
    lib_icons: Option<Arc<Icons>>,

    /// 全局图标集（从 HomePage 获取）
    icons: Option<Arc<Icons>>,

    mp_btn: RectButton,
    mp_icon: SafeTexture,
    mp_btn_pos: Vec2,
    mp_move: Option<(u64, Vec2, Vec2)>,
    mp_moved: bool,
    mp_save_pos_at: Option<Instant>,

    /// 入场时间（从开屏 Replace 进入时记录）；用于入场黑色渐隐过渡（开屏黑色淡入的接力）
    enter_at: f32,

    batch_import_confirm: Arc<AtomicBool>,
    batch_import: Option<(String, ExportInfo)>,
    batch_import_task: Option<Task<Result<()>>>,

    open_debug: bool,
    batch_import_rx: Option<mpsc::Receiver<ImportChart>>,
    batch_imported_charts: Vec<ImportChart>,
    batch_import_total: usize,
}

enum ImportChart {
    Imported(Box<LocalChart>, ParseWarnings),
    Skipped(String),
}

impl MainScene {

    pub async fn new(fallback: FontArc) -> Result<Self> {
        Self::init().await?;

        // 主页 BGM 不在这里加载：正常启动时由开屏场景把正在播放的 Music 交过来；
        // 只有从 Setup / StartupLoading 直接进主界面时才在 enter() 里兜底创建。
        let mut sf = Self::new_inner(fallback).await?;
        sf.pages.push(Box::new(HomePage::new().await?));
        if std::env::var("PHIRA_PAGE").ok().as_deref() == Some("phigros") {
            sf.pages.push(Box::new(PhigrosLibraryPage::new()));
        }
        prpr::hud::selftest();
        Ok(sf)
    }

    /// 没有开屏场景可接手时（首次设置 / 解压后直进主界面），按同一份共享音源新起一首。
    fn create_shared_bgm() -> Result<Music> {
        let data = crate::scene::load_shared_bgm_bytes()?;
        let clip = AudioClip::new(data)?;
        UI_AUDIO.with(|it| {
            it.borrow_mut().create_music(
                clip,
                sasa::MusicParams {
                    amplifier: get_data().config.volume_bgm,
                    loop_mix_time: 0.,
                    command_buffer_size: 64,
                    ..Default::default()
                },
            )
        })
    }

    async fn init() -> Result<()> {
        // 显示分数默认打开
        if !get_data().config.show_score_initialized {
            get_data_mut().config.show_score = true;
            get_data_mut().config.show_score_initialized = true;
            let _ = save_data();
        }
        // 并行加载所有资源文件，减少加载时间
        let (sfx_large_data, sfx_data, switch_data, play_data, back_data, suspend_data, message_data, background_tex, icon_back_tex, proceed_tex) = join!(
            load_file("sfx/button_large.ogg"),
            load_file("sfx/button.ogg"),
            load_file("sfx/switch.ogg"),
            load_file("sfx/play.wav"),
            load_file("sfx/back.wav"),
            load_file("sfx/suspend.wav"),
            load_file("sfx/message.wav"),
            load_home_background(),
            load_texture("icon_old(home)/back.png"),
            load_texture("icons/proceed.png")
        );

        // 创建音效
        macro_rules! create_sfx {
            ($name:ident, $data:expr) => {{
                let clip = AudioClip::new($data?)?;
                let sound = UI_AUDIO.with(|it| it.borrow_mut().create_sfx(clip, None))?;
                prpr::ui::$name.with(|it| *it.borrow_mut() = Some(sound));
            }};
        }
        create_sfx!(UI_BTN_HITSOUND_LARGE, sfx_large_data);
        create_sfx!(UI_BTN_HITSOUND, sfx_data);
        create_sfx!(UI_SWITCH_SOUND, switch_data);
        create_sfx!(UI_PLAY_SOUND, play_data);
        create_sfx!(UI_BACK_SOUND, back_data);
        create_sfx!(UI_SUSPEND_SOUND, suspend_data);
        create_sfx!(UI_MESSAGE_SOUND, message_data);

        let background: SafeTexture = background_tex?;
        let icon_back: SafeTexture = icon_back_tex?.into();
        // 设置页左上角返回键贴图（自带黑底 + 白斜条 + 箭头）
        if let Ok(tex) = proceed_tex {
            TEX_ICON_PROCEED.with(|it| *it.borrow_mut() = Some(tex.into()));
        }

        TEX_BACKGROUND.with(|it| *it.borrow_mut() = Some(background));
        TEX_ICON_BACK.with(|it| *it.borrow_mut() = Some(icon_back));

        Ok(())
    }

    async fn new_inner(fallback: FontArc) -> Result<Self> {
        let state = SharedState::new(fallback).await?;
        let icon_user = load_texture("icons/user.png").await?;
        // 多人场景专用背景（模糊 + 压暗后的 mp_bg；失败时退回全局背景）
        let mp_background = match load_mp_background().await {
            Ok(tex) => tex,
            Err(err) => {
                warn!("failed to load multiplayer background: {err}");
                TEX_BACKGROUND.with(|it| it.borrow().clone().unwrap())
            }
        };
        // 多人工具条图标（模仿爱笔思画的方块工具按钮：上图标下小字）。
        // 装载失败的那个只是让按钮少个图标，不影响点击。
        let mut mp_tool_icons: [Option<SafeTexture>; 16] = Default::default();
        for (slot, path) in mp_tool_icons.iter_mut().zip([
            "icon_old(home)/resume.png", // Play
            "icons/select.png",         // Ready
            "icon_old(home)/close.png", // Cancel
            "icons/settings.png",       // Settings
            "icons/order.png",          // Cycle
            "icons/mod.png",            // Lock
            "icons/search.png",         // Preview
            "icons/plus.png",           // Create
            "icons/user.png",           // Join
            "icons/order.png",          // Refresh
            "icon_old(home)/close.png", // Disconnect
            "icons/menu.png",           // Library
            "icons/back.png",           // LeaveRoom
        ]) {
            match load_texture(path).await {
                Ok(tex) => *slot = Some(SafeTexture::from(tex)),
                Err(err) => warn!("failed to load multiplayer tool icon {path}: {err}"),
            }
        }
        crate::mp::theme::set_tool_icons(mp_tool_icons);
        // 多人场景专用 BGM（`MpSession::play_bgm` 以 10% 音量播放；失败时就没有 BGM）
        let mp_bgm = match load_file("bgm/mp_bgm.mp3").await {
            Ok(data) => match AudioClip::new(data) {
                Ok(clip) => Some(clip),
                Err(err) => {
                    warn!("failed to decode multiplayer bgm: {err}");
                    None
                }
            },
            Err(err) => {
                warn!("failed to load multiplayer bgm: {err}");
                None
            }
        };
        // 多人会话（跨场景存活）：多人场景只是它的一层视图壳
        MP_SESSION.with(|it| *it.borrow_mut() = Some(MpSession::new(icon_user.into(), mp_background, mp_bgm)));
        // 本地核对界面用：PHIRA_MP_DEV 非空时直接进多人场景（正常运行为空）
        if std::env::var_os("PHIRA_MP_DEV").is_some() {
            crate::mp::request_enter();
        }
        // 正常启动时开屏已经把主图和 TriGrid 装好；这里兜底一下，保证直接进 MainScene
        // （Debug / 异常路径）也能看到完整的主页背景。
        if crate::splash_ui::key_art().is_none() {
            if let Err(err) = crate::splash_ui::load_key_art().await {
                warn!("failed to load key art for main scene: {err:#}");
            }
        }
        if crate::splash_ui::tri_grid().is_none() {
            match load_texture("textures/TriGridBase.png").await {
                Ok(tex) => {
                    tex.set_filter(FilterMode::Linear);
                    crate::splash_ui::set_tri_grid(tex.into());
                }
                Err(err) => warn!("failed to load TriGridBase.png: {err}"),
            }
        }
        Ok(Self {
            state,

            bgm: None,
            bgm_resume_on_enter: false,
            backdrop: crate::splash_ui::Backdrop::new(),
            btn_back: RectButton::new(),
            icon_back: TEX_ICON_BACK.with(|it| it.borrow().clone().unwrap()),

            pages: Vec::new(),

            import_task: None,
            pending_debug_scene: None,
            debug_scene_task: None,
            pending_debug_scene_result: None,
            bg_reload: None,
            key_art_reload: None,

            #[cfg(feature = "video")]
            home_video: None,
            #[cfg(feature = "video")]
            home_video_start: 0.,
            #[cfg(feature = "video")]
            home_video_path: None,
            #[cfg(feature = "video")]
            home_video_rx: None,

            lib_icons: match Icons::new().await {
                Ok(icons) => Some(Arc::new(icons)),
                Err(err) => {
                    warn!("failed to load icons for chart library: {err:#}");
                    None
                }
            },
            icons: match Icons::new().await {
                Ok(icons) => Some(Arc::new(icons)),
                Err(_) => None,
            },

            mp_btn: RectButton::new(),
            mp_icon: SafeTexture::from(load_texture("icons/multiplayer.png").await?).with_mipmap(),
            mp_btn_pos: (|| -> Result<Vec2> {
                let s = std::fs::read_to_string(position_file()?)?;
                let (x, y) = s.split_once(',').ok_or_else(|| anyhow!("invalid"))?;
                Ok(vec2(x.parse()?, y.parse()?))
            })()
            .unwrap_or_default(),
            mp_move: None,
            mp_moved: false,
            mp_save_pos_at: None,
            enter_at: f32::NAN,

            batch_import_confirm: Arc::default(),
            batch_import: None,
            batch_import_task: None,
        open_debug: false,
            batch_import_rx: None,
            batch_imported_charts: Vec::new(),
            batch_import_total: 0,
        })
    }

    fn pop(&mut self) {
        if !self.pages.last().unwrap().can_play_bgm() && self.pages[self.pages.len() - 2].can_play_bgm() {
            if let Some(bgm) = &mut self.bgm {
                let _ = bgm.fade_in(0.5);
            }
        }
        self.state.fader.back(self.state.t);
    }

    #[cfg(feature = "video")]
    fn update_home_video(&mut self, now: f64) {
        let path = get_data().home_video_path.clone();
        if path != self.home_video_path {
            self.home_video_path = path.clone();
            self.home_video = None;
            self.home_video_rx = None;
            if let Some(path) = path {
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    if let Ok(data) = std::fs::read(&path) {
                        let _ = tx.send(data);
                    }
                });
                self.home_video_rx = Some(rx);
            }
        }

        if let Some(rx) = self.home_video_rx.take() {
            match rx.try_recv() {
                Ok(data) => match Video::new(
                    data,
                    0.,
                    ScaleType::CropCenter,
                    Anim::fixed(1.),
                    Anim::fixed(0.),
                ) {
                    Ok(video) => {
                        self.home_video_start = now;
                        self.home_video = Some(video);
                    }
                    Err(err) => warn!("failed to open home video: {err:#}"),
                },
                Err(mpsc::TryRecvError::Empty) => self.home_video_rx = Some(rx),
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
        }

        if let Some(video) = &mut self.home_video {
            if now < self.home_video_start || now - self.home_video_start >= video.duration {
                self.home_video_start = now;
                let _ = video.reset();
            }
            let _ = video.update(now - self.home_video_start);
        }
    }

    fn open_library_page(&mut self) {
        // 打开谱面库页面
        eprintln!("[Debug] Opening library page...");
    }

    fn open_settings_page(&mut self) {
        // 打开设置页面
        eprintln!("[Debug] Opening settings page...");
    }

    pub fn take_imported_respack() -> Option<ResPackItem> {
        RESPACK_ITEM.with(|it| it.borrow_mut().take())
    }
}

impl Scene for MainScene {
    fn is_main(&self) -> bool {
        true
    }

    fn on_result(&mut self, _tm: &mut TimeManager, result: Box<dyn Any>) -> Result<()> {
        self.pages.last_mut().unwrap().on_result(result, &mut self.state)
    }

    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        // 记录入场时间：从开屏 Replace 进来时黑色过渡在此接力渐隐（仅首次 enter 生效）
        self.enter_at = tm.now() as f32;
        let mut just_created = false;
        if self.bgm.is_none() {
            // 正常路径：接手开屏场景正在播的那首，切场景时不断音。
            if let Some(bgm) = crate::scene::take_carried_bgm() {
                self.bgm = Some(bgm);
            } else {
                // 首次设置 / 解压后直接进主界面的路径：没有开屏音乐可接，按同一份源新起一首。
                match Self::create_shared_bgm() {
                    Ok(bgm) => {
                        just_created = true;
                        self.bgm = Some(bgm);
                    }
                    Err(err) => warn!("failed to create home BGM: {err:#}"),
                }
            }
        }
        // 从谱面 / 个人资料 / 多人等子场景回来时，主页音乐之前被 fade_out 掉了；
        // 这里统一恢复，否则会一直停着。首次从开屏接手时 bgm 还在播放，不会重复淡入。
        if let Some(bgm) = &mut self.bgm {
            let should_resume = self.bgm_resume_on_enter || just_created || bgm.paused();
            self.bgm_resume_on_enter = false;
            if should_resume {
                let _ = bgm.fade_in(if just_created { 1.3 } else { 1.0 });
            }
        }
        self.state.update(tm);
        self.pages.last_mut().unwrap().enter(&mut self.state)?;
        Ok(())
    }

    fn resume(&mut self, tm: &mut TimeManager) -> Result<()> {
        if let Some(bgm) = &mut self.bgm {
            bgm.play()?;
        }
        self.state.update(tm);
        self.pages.last_mut().unwrap().resume()?;
        Ok(())
    }

    fn pause(&mut self, tm: &mut TimeManager) -> Result<()> {
        if let Some(bgm) = &mut self.bgm {
            bgm.pause()?;
        }
        self.state.update(tm);
        self.pages.last_mut().unwrap().pause()?;
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if crate::plugins::touch(touch) { return Ok(true); }
        if self.state.fader.transiting() {
            return Ok(false);
        }
        if self.import_task.is_some() {
            return Ok(true);
        }

        if get_data().config.mp_enabled && self.pages.len() == 1 {
            if self.mp_btn.touch(touch) && !self.mp_moved {
                // 进入多人场景（与主菜单平级的整屏场景）
                crate::mp::request_enter();
                self.mp_move = None;
                self.mp_moved = false;
                return Ok(true);
            }
            if let Some((id, pos, btn_pos)) = self.mp_move {
                if touch.id == id {
                    if matches!(touch.phase, TouchPhase::Cancelled | TouchPhase::Ended) {
                        self.mp_move = None;
                        self.mp_moved = false;
                        return Ok(true);
                    }
                    let new_pos = touch.position;
                    if !self.mp_moved && (new_pos - pos).length() > 0.03 {
                        self.mp_moved = true;
                    }
                    if self.mp_moved {
                        self.mp_btn_pos = new_pos - pos + btn_pos;
                        self.mp_save_pos_at = Some(Instant::now() + Duration::from_secs(1));
                    }
                }
                return Ok(true);
            } else if self.mp_btn.touching() {
                self.mp_move = Some((touch.id, touch.position, self.mp_btn_pos));
                return Ok(true);
            }
        }

        let s = &mut self.state;
        s.update(tm);
        if prpr::hud::edit_active() {
            return Ok(prpr::hud::editor_touch(touch));
        }
        if self.pages.last_mut().unwrap().touch(touch, s)? {
            return Ok(true);
        }
        if self.btn_back.touch(touch) && self.pages.len() > 1 {
            back_sound();
            if !self.pages.last_mut().unwrap().on_back_pressed(&mut self.state) {
                if self.pages.len() == 2 {
                    if let Some(bgm) = &mut self.bgm {
                        bgm.set_low_pass(0.)?;
                    }
                }
                self.pop();
            }
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        /// HUD 编辑器请求切页：重建页面栈（根页面保留、不重建，因此不需要异步）。
        /// Vrenxz 的 icons 是 Option<Arc<Icons>>（加载失败时可能为 None），无独立
        /// TabsPage，PageId::Tabs 跳转时用谱面库页面代替。
        fn goto_hud_page(
            pages: &mut Vec<Box<dyn Page>>,
            icons: Option<Arc<Icons>>,
            state: &mut SharedState,
            p: prpr::hud::PageId,
        ) -> Result<()> {
            use prpr::hud::PageId;
            pages.truncate(1);
            prpr::hud::set_cur_page(p);
            match p {
                PageId::Home => {}
                PageId::Library => {
                    if let Some(icons) = &icons {
                        let page = LibraryPage::new(Arc::clone(icons), state.icons.clone())?;
                        pages.push(Box::new(page));
                    }
                }
                PageId::Settings => {
                    if let Some(icons) = &icons {
                        let page = SettingsPage::new(icons.settings.clone(), icons.lang.clone());
                        pages.push(Box::new(page));
                    }
                }
                PageId::Favorites => {
                    if let Some(icons) = &icons {
                        let page = FavoritesPage::new(Arc::clone(icons), state.icons.clone(), None, None);
                        pages.push(Box::new(page));
                    }
                }
                PageId::Message => {
                    if let Some(icons) = &icons {
                        let page = MessagePage::new(Arc::clone(icons), state.icons.clone());
                        pages.push(Box::new(page));
                    }
                }
                PageId::Respack => {
                    if let Some(icons) = &icons {
                        let page = ResPackPage::new(Arc::clone(icons))?;
                        pages.push(Box::new(page));
                    }
                }
                PageId::Tabs => {
                    // Vrenxz 无独立 TabsPage：跳谱面库即可编辑其顶部 tab 栏槽位。
                    if let Some(icons) = &icons {
                        let page = LibraryPage::new(Arc::clone(icons), state.icons.clone())?;
                        pages.push(Box::new(page));
                    }
                }
                PageId::Game | PageId::Loading | PageId::Ending => {}
            }
            pages.last_mut().unwrap().enter(state)?;
            Ok(())
        }
        // 按 F10 打开制谱工作台（RPE 谱面编辑）
        if is_key_pressed(KeyCode::F10) && self.pages.len() == 1 {
            match WorkbenchPage::new() {
                Ok(page) => {
                    self.pages.push(Box::new(page));
                    self.state.fader.sub(self.state.t);
                }
                Err(e) => eprintln!("[Workbench] open failed: {e:#}"),
            }
        }

        // 按 F3 打开 Debug 窗口
        if is_key_pressed(KeyCode::F3) {
            // 直接打开独立的 Debug 控制台窗口
            if let Err(e) = std::process::Command::new("cmd")
                .args(&["/K", r"F:\RustProject\phirLie\debug_console.bat"])
                .spawn()
            {
                eprintln!("[Debug] Failed to open debug window: {e}");
            }

            // 剧情代码已注释（2026-10-05）：F3 不再进入剧情页面 StoryScene
            // if let Some(icons) = self.icons.clone() {
            //     let rank_icons = crate::scene::TEX_RANK_ICONS
            //         .with(|it| it.borrow().clone())
            //         .unwrap_or_else(|| core::array::from_fn(|_| prpr::ext::BLACK_TEXTURE.clone()));
            //     self.pending_debug_scene_result = Some(NextScene::Overlay(Box::new(
            //         crate::scene::StoryScene::new(icons, rank_icons),
            //     )));
            // }
        }

        // 插件管理已并入设置页「通用」列表（原 F9 独立页已移除）

        // 检查 Debug 命令
        crate::scene::DEBUG_LISTENER.with(|listener| {
            if let Some(cmd) = listener.borrow_mut().update(tm) {
                match cmd.as_str() {
                    "opensong" => {
                        eprintln!("[Debug] Opening song select...");
                        // 请求打开谱面库
                        crate::mp::request_open_library();
                    }
                    "render" => {
                        eprintln!("[Debug] Opening render tool...");
                        std::process::Command::new("explorer")
                            .arg("F:/phirLie/target/release")
                            .spawn()
                            .ok();
                    }
                    "up:main" => {
                        eprintln!("[Debug] UI preview: Main Menu");
                        // 回到主页
                        if self.pages.len() > 1 {
                            self.pop();
                        }
                    }
                    "up:settings" => {
                        eprintln!("[Debug] UI preview: Settings");
                        // 打开设置页面
                        if let Some(icons) = &self.icons {
                            let page = SettingsPage::new(icons.settings.clone(), icons.lang.clone());
                            self.pages.push(Box::new(page));
                            self.state.fader.sub(self.state.t);
                        }
                    }

                    "up:favorites" => {
                        eprintln!("[Debug] UI preview: Favorites");
                        // 打开收藏页面
                        if let Some(icons) = &self.icons {
                            let page = FavoritesPage::new(
                                Arc::clone(icons),
                                self.state.icons.clone(),
                                None,
                                None,
                            );
                            self.pages.push(Box::new(page));
                            self.state.fader.sub(self.state.t);
                        }
                    }
                    "up:respack" => {
                        eprintln!("[Debug] UI preview: Resource Pack");
                        // 打开资源包页面
                        if let Some(icons) = &self.icons {
                            match ResPackPage::new(Arc::clone(icons)) {
                                Ok(page) => {
                                    self.pages.push(Box::new(page));
                                    self.state.fader.sub(self.state.t);
                                }
                                Err(e) => eprintln!("[Debug] Failed to open respack page: {}", e),
                            }
                        }
                    }
                    "up:extension" => {
                        eprintln!("[Debug] UI preview: Extensions");
                        // 打开扩展页面
                        match ExtensionPage::new() {
                            Ok(page) => {
                                self.pages.push(Box::new(page));
                                self.state.fader.sub(self.state.t);
                            }
                            Err(e) => eprintln!("[Debug] Failed to open extension page: {}", e),
                        }
                    }
                    "up:message" => {
                        eprintln!("[Debug] UI preview: Messages");
                        // 打开消息页面
                        if let Some(icons) = &self.icons {
                            let page = MessagePage::new(
                                Arc::clone(icons),
                                self.state.icons.clone(),
                            );
                            self.pages.push(Box::new(page));
                            self.state.fader.sub(self.state.t);
                        }
                    }
                    "up:event" => {
                        eprintln!("[Debug] UI preview: Events");
                        // 打开活动页面
                        if let Some(icons) = &self.icons {
                            let page = EventPage::new(
                                Arc::clone(icons),
                                self.state.icons.clone(),
                            );
                            self.pages.push(Box::new(page));
                            self.state.fader.sub(self.state.t);
                        }
                    }
                    "up:collection" => {
                        eprintln!("[Debug] UI preview: Collection");
                        eprintln!("[Debug] Collection page is async and not Send, not implemented yet");
                    }
                    "up:crash" => {
                        eprintln!("[Debug] Scene preview: Crash Scene");
                        crate::scene::request_debug_crash();
                    }
                    "up:loading" => {
                        eprintln!("[Debug] Scene preview: Loading Scene");
                        // 用第一个本地谱面来创建 LoadingScene
                        let charts = &get_data().charts;
                        if let Some(first_chart) = charts.first() {
                            let local_path = first_chart.local_path.clone();
                            let mods = prpr::config::Mods::empty();
                            match SongScene::global_launch(
                                None,
                                &local_path,
                                mods,
                                prpr::config::HpMode::Off,
                                GameMode::Normal,
                                None,
                                None,
                                None,
                                false,
                                false,
                            ) {
                                Ok(scene_task) => {
                                    self.debug_scene_task = scene_task;
                                    eprintln!("[Debug] Scene task created");
                                }
                                Err(e) => {
                                    eprintln!("[Debug] Failed to create scene task: {}", e);
                                }
                            }
                        } else {
                            eprintln!("[Debug] No local charts found");
                        }
                    }
                    "up:game" => {
                        eprintln!("[Debug] Scene preview: Game Scene");
                        self.pending_debug_scene = Some("game");
                    }
                    "up:ending" => {
                        eprintln!("[Debug] Scene preview: Ending Scene");
                        self.pending_debug_scene = Some("ending");
                    }
                    "coco" => {
                        eprintln!("[Debug] Console command...");
                    }
                    "gi" => eprintln!("[Debug] Game: Phira-Vrenxz v1.3.15"),
                    _ => eprintln!("[Debug] Unknown command: {}", cmd),
                }
            }
        });

        UI_AUDIO.with(|it| it.borrow_mut().recover_if_needed())?;
        if let Some(p) = prpr::hud::take_goto() {
            use prpr::hud::PageId;
            if matches!(p, PageId::Game | PageId::Loading | PageId::Ending) {
                // HUD 编辑：场景示例预览。不新建场景，直接以第一个本地谱面进入原有真实场景
                // （loading/game/ending），仅把曲名/难度等文本替换为示例值供 HUD 拖拽布局；
                // 槽位键名与真实场景一致，保存后可直接作用于实玩。
                prpr::hud::set_cur_page(p);
                if p == PageId::Loading {
                    // 加载页预览不进谱面：直接用示例数据停在加载页（load_task 为空，不会转入游戏）。
                    match SongScene::global_launch_loading_preview() {
                        Ok(task) => {
                            self.debug_scene_task = task;
                            eprintln!("[HUD] Loading preview task created");
                        }
                        Err(e) => {
                            eprintln!("[HUD] Failed to create loading preview: {e}");
                        }
                    }
                    return Ok(());
                }
                let charts = &get_data().charts;
                let first = charts.iter().find(|it| !it.local_path.is_empty());
                let local_path = match first {
                    Some(chart) => chart.local_path.clone(),
                    None => {
                        eprintln!("[HUD] No local charts, skip scene preview");
                        prpr::hud::set_cur_page(PageId::Home);
                        return Ok(());
                    }
                };
                if p == PageId::Game {
                    match SongScene::global_launch_game_hud_preview() {
                        Ok(task) => {
                            self.debug_scene_task = task;
                            eprintln!("[HUD] Game preview task created");
                        }
                        Err(e) => {
                            eprintln!("[HUD] Failed to create game preview: {e}");
                        }
                    }
                    return Ok(());
                }
                let scene_task = match p {
                    PageId::Game => SongScene::global_launch_hud_preview(&local_path),
                    PageId::Ending => SongScene::global_launch_ending_preview(&local_path),
                    _ => unreachable!(),
                };
                match scene_task {
                    Ok(task) => {
                        self.debug_scene_task = task;
                        eprintln!("[HUD] Scene preview task created: {p:?}");
                    }
                    Err(e) => {
                        eprintln!("[HUD] Failed to create scene preview task: {e}");
                    }
                }
            } else {
                goto_hud_page(&mut self.pages, self.icons.clone(), &mut self.state, p)?;
            }
        }
        // 旧版本（PhirLie / 旧版 Phira-Vrenxz）数据检测与同步：只在主菜单里跑一次，与多人模式无关
        crate::migrate::tick(tm);
        // Android 深链接（phira://）：把待处理的房间动作交给多人会话并进入多人场景
        if let Some(link) = crate::mp::take_pending_room_link() {
            // TEMP DEBUG (to be removed)
            eprintln!("[DBG] pending room link taken: {link:?}");
            if !get_data().config.mp_enabled {
                get_data_mut().config.mp_enabled = true;
            }
            MP_SESSION.with(|it| {
                if let Some(session) = it.borrow_mut().as_mut() {
                    session.set_deep_link(link);
                }
            });
            crate::mp::request_enter();
        }
        let s = &mut self.state;
        s.update(tm);
        if s.fader.transiting() {
            let pos = self.pages.len() - 2;
            self.pages[pos].update(s)?;
        }
        self.pages.last_mut().unwrap().update(s)?;
        if self.pages.last_mut().unwrap().take_pop_request() {
            // 页面已经播完自己的退场动画，直接弹栈；不再等 fader，
            // 避免只滑走返回键但设置面板还留在屏幕上。
            if self.pages.len() > 1 {
                self.pages.pop().unwrap().exit()?;
                self.pages.last_mut().unwrap().enter(s)?;
            }
        }
        if !s.fader.transiting() {
            // 房间页的「谱面库」按钮：退出多人场景时留下的一次性请求 —— 直接把谱面库
            // 页面压上来（只在主菜单首页时处理，避免在别的页面上乱插一层）。
            let requested_library = crate::mp::take_open_library_request();
            let injected = if requested_library && self.pages.len() == 1 {
                match &self.lib_icons {
                    Some(icons) => match LibraryPage::new(Arc::clone(icons), s.icons.clone()) {
                        Ok(page) => Some(Box::new(page) as Box<dyn Page>),
                        Err(err) => {
                            warn!("failed to open chart library: {err:#}");
                            None
                        }
                    },
                    None => None,
                }
            } else {
                None
            };
            let next = match injected {
                // 房间页点了「谱面库」：直接把谱面库当作 Overlay 压上来（走原来那条分支）
                Some(page) => NextPage::Overlay(page),
                None => self.pages.last_mut().unwrap().next_page(),
            };
            match next {
                NextPage::Overlay(mut sub) => {
                    if self.pages.len() == 1 {
                        if let Some(bgm) = &mut self.bgm {
                            bgm.set_low_pass(LOW_PASS)?;
                        }
                    }
                    sub.enter(s)?;
                    self.pages.push(sub);
                    s.fader.sub(s.t);
                }
                NextPage::Pop => {
                    self.pop();
                }
                NextPage::None => {}
            }
        } else if let Some(true) = s.fader.done(s.t) {
            self.pages.pop().unwrap().exit()?;
            self.pages.last_mut().unwrap().enter(s)?;
        }
        if let Some(bgm) = &mut self.bgm {
            if BGM_VOLUME_UPDATED.fetch_and(false, Ordering::Relaxed) {
                bgm.set_amplifier(get_data().config.volume_bgm)?;
            }
        }
        // 设置页换了主页背景：重新装载（异步），装好之后同时更新全局那份和自己的那份。
        if BACKGROUND_UPDATED.swap(false, Ordering::Relaxed) {
            self.bg_reload = Some(Task::new(load_home_background_bytes()));
            self.key_art_reload = Some(Task::new(crate::splash_ui::load_key_art_bytes()));
        }
        if let Some(task) = &mut self.bg_reload {
            if let Some(res) = task.take() {
                self.bg_reload = None;
                match res {
                    Ok(bytes) => {
                        // 主线程创建贴图（OpenGL 上下文只在主线程可用）
                        match image::load_from_memory(&bytes) {
                            Ok(img) => {
                                let rgba = img.to_rgba8();
                                let (w, h) = img.dimensions();
                                let tex: SafeTexture = Texture2D::from_rgba8(w as u16, h as u16, &rgba).into();
                                TEX_BACKGROUND.with(|it| *it.borrow_mut() = Some(tex));
                            }
                            Err(err) => warn!("failed to decode home background: {err:#}"),
                        }
                    }
                    Err(err) => warn!("failed to reload home background: {err:#}"),
                }
            }
        }
        // 左侧主图：解出来的图直接替换全局 key art，主页 / 设置 / 谱面库 / 个人资料等
        // 所有共用 Backdrop 的页面会立刻跟着换。
        if let Some(task) = &mut self.key_art_reload {
            if let Some(res) = task.take() {
                self.key_art_reload = None;
                match res {
                    Ok(bytes) => {
                        if let Err(err) = crate::splash_ui::install_key_art(&bytes) {
                            warn!("failed to install key art: {err:#}");
                        }
                    }
                    Err(err) => warn!("failed to reload key art: {err:#}"),
                }
            }
        }
        #[cfg(feature = "video")]
        self.update_home_video(macroquad::time::get_time() as f64);
        if let Some(task) = &mut self.import_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err.context(itl!("import-failed")));
                    }
                    Ok((chart, warnings)) => {
                        if let Some(warn) = parse_warnings_to_string(&warnings) {
                            Dialog::plain(itl!("warning"), warn).show();
                        }
                        show_message(itl!("import-success")).ok();
                        get_data_mut().charts.push(chart);
                        save_data()?;
                        self.state.reload_local_charts();
                        NEED_UPDATE.store(true, Ordering::Relaxed);
                    }
                }
                self.import_task = None;
            }
        }

        // 拖拽导入完成后与手动导入一致刷新内存谱面列表
        if crate::DROPPED_IMPORT_DIRTY.swap(false, Ordering::Relaxed) {
            eprintln!("[drop-debug] reload triggered");
            self.state.reload_local_charts();
            NEED_UPDATE.store(true, Ordering::Relaxed);
        }

        // 处理 Debug Scene task
        if let Some(task) = &mut self.debug_scene_task {
            if let Some(result) = poll_future(task.as_mut()) {
                match result {
                    Ok(scene) => {
                        eprintln!("[Debug] Scene loaded successfully");
                        self.pending_debug_scene_result = Some(scene);
                    }
                    Err(e) => {
                        eprintln!("[Debug] Failed to load scene: {}", e);
                    }
                }
                self.debug_scene_task = None;
            }
        }

        if let Some((id, file)) = take_file() {
            match id.as_str() {
                "_import_auto" => {
                    let new_id = match File::open(&file).map(BufReader::new).map(zip::ZipArchive::new) {
                        Ok(Ok(zip)) => {
                            if zip.file_names().any(|name| name.ends_with("click.png")) {
                                "_import_respack"
                            } else {
                                "_import"
                            }
                        }
                        _ => "_import",
                    };
                    return_file(new_id.to_owned(), file);
                }
                "_import" => {
                    let export_info = (|| -> Result<Option<(ExportInfo, usize)>> {
                        let file = File::open(&file)?;
                        let mut archive = zip::ZipArchive::new(file)?;
                        let export_info = match archive.by_name("export.json") {
                            Err(zip::result::ZipError::FileNotFound) => {
                                return Ok(None);
                            }
                            Err(err) => {
                                return Err(err.into());
                            }
                            Ok(file) => serde_json::from_reader(file)?,
                        };
                        let mut count = 0;
                        for i in 0..archive.len() {
                            let file = archive.by_index(i)?;
                            if file.enclosed_name().is_some_and(|it| it.extension().is_some_and(|ext| ext == "zip")) {
                                count += 1;
                            }
                        }
                        Ok(Some((export_info, count)))
                    })();
                    match export_info {
                        Err(err) => {
                            show_error(err.context(itl!("import-failed")));
                        }
                        Ok(None) => {
                            self.import_task = Some(Task::new(async move {
                                let file = File::open(&file).context("cannot open file")?;
                                import_chart(file).await
                            }));
                        }
                        Ok(Some((info, count))) => {
                            self.batch_import = Some((file, info));
                            self.batch_import_total = count;
                            confirm_dialog(itl!("batch-import"), itl!("batch-import-confirm", "count" => count), self.batch_import_confirm.clone());
                        }
                    };
                }
                "_import_font" => {
                    let result: Result<(String, String)> = (|| {
                        let bytes = std::fs::read(&file).context("cannot read font file")?;
                        FontArc::try_from_vec(bytes.clone()).context("invalid font file")?;
                        let name = PathBuf::from(&file)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("font")
                            .to_string();
                        let root = dir::fonts()?;
                        let filename = format!("{}.ttf", Uuid::new_v4());
                        std::fs::write(format!("{root}/{filename}"), &bytes)?;
                        Ok((name, filename))
                    })();
                    match result {
                        Err(err) => {
                            show_error(err.context(itl!("import-font-failed")));
                        }
                        Ok((name, filename)) => {
                            get_data_mut().fonts.push(FontEntry { name, file: filename });
                            save_data()?;
                            show_message(itl!("import-font-success")).ok();
                        }
                    }
                }
                "_import_respack" => {
                    let root = dir::respacks()?;
                    let dir = prpr::dir::Dir::new(&root)?;
                    let mut dir_id: Option<String> = None;
                    let item: Result<ResPackItem> = (|| {
                        let config = {
                            let mut zip = zip::ZipArchive::new(BufReader::new(File::open(&file)?))?;
                            let config: ResPackInfo =
                                serde_yaml::from_reader(zip.by_name("info.yml").context("missing info.yml")?).context("invalid info.yml")?;
                            config.verify()?;
                            let mut buffer = Vec::new();
                            for file in [
                                "click.png",
                                "click_mh.png",
                                "drag.png",
                                "drag_mh.png",
                                "flick.png",
                                "flick_mh.png",
                                "hold.png",
                                "hold_mh.png",
                                "hit_fx.png",
                            ] {
                                let mut entry = zip.by_name(file).with_context(|| format!("missing file: {file}"))?;
                                buffer.clear();
                                entry.read_to_end(&mut buffer)?;
                                image::load_from_memory(&buffer).with_context(|| format!("failed to load image: {file}"))?;
                            }

                            for audio in ["click", "drag", "flick", "ending"] {
                                for ext in [".ogg", ".wav", ".mp3"] {
                                    let mut entry = match zip.by_name(format!("{audio}{ext}").as_str()) {
                                        Err(zip::result::ZipError::FileNotFound) => continue,
                                        Err(err) => return Err(err.into()),
                                        Ok(file) => file,
                                    };
                                    buffer.clear();
                                    entry.read_to_end(&mut buffer)?;
                                    AudioClip::new(mem::take(&mut buffer)).with_context(|| format!("failed to load audio: {audio}"))?;
                                    break;
                                }
                            }
                            config
                        };

                        let mut uuid = Uuid::new_v4();
                        while dir.exists(uuid.to_string())? {
                            uuid = Uuid::new_v4();
                        }
                        let id = uuid.to_string();
                        dir.create_dir_all(&id)?;
                        let dir = dir.open_dir(&id)?;
                        dir_id = Some(id.clone());
                        unzip_into(BufReader::new(File::open(file)?), &dir, false).context("failed to unzip")?;
                        get_data_mut().respacks.push(id.clone());
                        save_data()?;
                        Ok(ResPackItem::new(Some(format!("{root}/{id}").into()), config.name))
                    })();
                    match item {
                        Err(err) => {
                            show_error(err.context(itl!("import-respack-failed")));
                            if let Some(id) = &dir_id {
                                dir.remove_dir_all(id)?;
                            }
                        }
                        Ok(item) => {
                            RESPACK_ITEM.with(|it| *it.borrow_mut() = Some(item));
                            show_message(itl!("import-respack-success"));
                        }
                    }
                }
                _ => return_file(id, file),
            }
        }
        if self.batch_import_confirm.swap(false, Ordering::Relaxed) {
            if let Some((file, _info)) = self.batch_import.take() {
                let (tx, rx) = mpsc::channel();
                self.batch_import_rx = Some(rx);
                self.batch_imported_charts.clear();
                self.batch_import_task = Some(Task::new(async move {
                    let mut archive = zip::ZipArchive::new(BufReader::new(File::open(&file)?))?;
                    let charts_dir = dir::charts()?;
                    for i in 0..archive.len() {
                        let mut file = archive.by_index(i)?;
                        let Some(name) = file.enclosed_name() else {
                            continue;
                        };
                        if name.extension().is_none_or(|it| it != "zip") {
                            continue;
                        }
                        let [Component::Normal(dir), Component::Normal(name)] = name.components().collect::<Vec<_>>()[..] else {
                            continue;
                        };
                        let mut to_tempfile = || -> std::io::Result<_> {
                            let mut tf = tempfile()?;
                            std::io::copy(&mut file, &mut tf)?;
                            tf.seek(SeekFrom::Start(0))?;
                            Ok(tf)
                        };
                        match dir.to_str() {
                            Some("custom") => {
                                let tf = to_tempfile()?;
                                let (chart, warnings) = import_chart(tf)
                                    .await
                                    .with_context(|| itl!("batch-import-failed-chart", "chart" => name.display().to_string()))?;
                                let _ = tx.send(ImportChart::Imported(Box::new(chart), warnings)).ok();
                            }
                            Some("download") => {
                                let Some(id) = name.to_str().and_then(|it| it.strip_suffix(".zip")).and_then(|it| it.parse::<i32>().ok()) else {
                                    warn!("invalid batch import download id: {:?}", name);
                                    continue;
                                };
                                let local_path = format!("download/{id}");
                                let path = PathBuf::from(format!("{charts_dir}/{local_path}"));
                                if std::fs::exists(&path)? {
                                    let info: ChartInfo = serde_yaml::from_reader(File::open(path.join("info.yml"))?)?;
                                    let _ = tx.send(ImportChart::Skipped(info.name));
                                    continue;
                                }
                                std::fs::create_dir(&path)?;
                                let tf = to_tempfile()?;
                                let (chart, warnings) = import_chart_to(&path, local_path, tf)
                                    .await
                                    .with_context(|| itl!("batch-import-failed-chart", "chart" => name.display().to_string()))?;
                                let _ = tx.send(ImportChart::Imported(Box::new(chart), warnings)).ok();
                            }
                            _ => {
                                warn!("invalid batch import dir: {:?}", dir);
                            }
                        }
                    }
                    Ok(())
                }));
            }
        }

        if let Some(rx) = &mut self.batch_import_rx {
            match rx.try_recv() {
                Ok(chart) => {
                    self.batch_imported_charts.push(chart);
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    warn!("import thread panicked");
                    self.batch_import_rx = None;
                }
            }
        }

        if let Some(task) = &mut self.batch_import_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        let charts = dir::charts()?;
                        for chart in self.batch_imported_charts.drain(..) {
                            if let ImportChart::Imported(chart, _) = chart {
                                let path = format!("{charts}{}", chart.local_path);
                                let _ = std::fs::remove_dir_all(path);
                            }
                        }
                        show_error(err.context(itl!("batch-import-failed")));
                    }
                    Ok(()) => {
                        let mut warning_messages = vec![];
                        let data = get_data_mut();
                        let mut count = 0;
                        let mut skipped = String::new();
                        for chart in self.batch_imported_charts.drain(..) {
                            match chart {
                                ImportChart::Imported(chart, warnings) => {
                                    if let Some(warn) = parse_warnings_to_string(&warnings) {
                                        warning_messages.push(format!("{}\n{warn}", chart.info.name));
                                    }
                                    data.charts.push(*chart);
                                    count += 1;
                                }
                                ImportChart::Skipped(name) => {
                                    if !skipped.is_empty() {
                                        skipped.push_str(", ");
                                    }
                                    skipped.push_str(&name);
                                }
                            }
                        }
                        save_data()?;
                        self.state.reload_local_charts();
                        NEED_UPDATE.store(true, Ordering::Relaxed);

                        let mut message = itl!("batch-import-success", "count" => count);
                        if !skipped.is_empty() {
                            message.push('\n');
                            message += &itl!("batch-import-downloaded-skipped", "charts" => skipped);
                        }

                        if !warning_messages.is_empty() {
                            message += "\n\n";
                            message += &warning_messages.join("\n\n");
                        }
                        Dialog::simple(message).show();
                    }
                }
                self.batch_import_task = None;
                self.batch_import_rx = None;
            }
        }

        if self.mp_save_pos_at.is_some_and(|it| it < Instant::now()) {
            std::fs::write(position_file()?, format!("{},{}", self.mp_btn_pos.x, self.mp_btn_pos.y))?;
            self.mp_save_pos_at = None;
        }

        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        // HUD 编辑层：每帧先清空上一帧槽位记录，避免拖拽残影。
        prpr::hud::begin_frame();

        // 所有页面共用同一份主页底板（左侧主图 + 右侧黑场 / TriGrid + 粒子）。
        // 这样谱面库 / 设置 / 课题模式 / 资源包 / 消息 / 收藏 / 个人资料看到的背景
        // 与主页完全一致；自定义背景只换左侧那张主图。
        set_camera(&ui.camera());
        #[cfg(feature = "video")]
        let draw_video = self.home_video.is_some();
        #[cfg(not(feature = "video"))]
        let draw_video = false;
        if draw_video {
            #[cfg(feature = "video")]
            if let Some(video) = &self.home_video {
                let top = ui.top;
                let full = Rect::new(-1., -top, 2., 2. * top);
                ui.fill_rect(full, BLACK);
                push_camera_state();
                set_camera(&ui.camera());
                let elapsed = macroquad::time::get_time() as f64 - self.home_video_start;
                video.render(elapsed, 1. / top, WHITE);
                pop_camera_state();
                // 视频之上先压暗保证 UI 可读，再叠穹顶/TriGrid/粒子。
                ui.fill_rect(full, Color::new(0., 0., 0., 0.28));
                self.backdrop.draw_dome(ui, macroquad::time::get_time() as f32, 1.0);
            }
        } else {
            self.backdrop.draw(
                ui,
                crate::splash_ui::key_art().as_ref(),
                macroquad::time::get_time() as f32,
                1.0,
            );
        }

        // 只渲染背景截图模式：背景画完就截帧，页面 / 字标 / 图标 / FPS 一律不画。
        if crate::bgshot::active() {
            crate::bgshot::capture_after_backdrop();
            return Ok(());
        }

        let s = &mut self.state;
        s.update(tm);

        if s.fader.transiting() {
            let pos = self.pages.len() - 2;
            let old = s.fader.distance;
            s.fader.distance *= -0.6;
            self.pages[pos].render(ui, s)?;
            s.fader.distance = old;
        }
        s.fader.sub = true;
        s.fader.reset();
        self.pages.last_mut().unwrap().render(ui, s)?;
        s.fader.sub = false;

        if s.fader.transiting() {
            let pos = self.pages.len() - 2;
            s.fader.reset();
            s.fader.render_title(ui, s.t, &self.pages[pos].label());
        }
        s.fader.for_sub(|f| f.render_title(ui, s.t, &self.pages.last().unwrap().label()));

        if self.pages.len() >= 2 {
            // 页面可以自己接管返回键（自己画、自己给落点），这时不再画默认图标
            let custom = self.pages.last().unwrap().back_button_rect(ui);
            let r = custom.unwrap_or_else(|| ui.back_rect());
            self.btn_back.set(ui, r);
            if custom.is_none() {
                let dy = (match self.pages.len() {
                    1 => 1.,
                    2 => s.fader.for_sub(|f| f.progress(s.t)),
                    _ => 0.,
                } * r.h)
                    .clamp(0., r.h);
                let ir = Rect::new(r.x, r.y + dy, r.w, r.h);
                ui.fill_rect(Rect::new(r.x, r.y + dy, r.w, r.h - dy), (*self.icon_back, ir, ScaleType::Fit));
            }
        }

        self.pages.last_mut().unwrap().render_top(ui, s)?;

        if prpr::hud::edit_active() {
            prpr::hud::editor_render(ui);
        }

        if get_data().config.mp_enabled && self.pages.len() == 1 {
            let r = 0.06;
            self.mp_btn_pos.y = self.mp_btn_pos.y.clamp(-ui.top, ui.top);
            self.mp_btn_pos.x = self.mp_btn_pos.x.clamp(-1., 1.);
            ui.fill_circle(self.mp_btn_pos.x, self.mp_btn_pos.y, r, ui.background());
            let r = Rect::new(self.mp_btn_pos.x, self.mp_btn_pos.y, 0., 0.).feather(r);
            self.mp_btn.set(ui, r);
            let r = r.feather(-0.02);
            ui.fill_rect(r, (*self.mp_icon, r));
        }

        if self.import_task.is_some() {
            ui.full_loading(itl!("importing"), s.t);
        }
        if self.batch_import_task.is_some() {
            let current = self.batch_imported_charts.len();
            let total = self.batch_import_total;
            ui.full_loading(itl!("batch-importing", "current" => current, "total" => total), s.t);
        }



        // TEMP-SHOT-HOOK: 本机核对用，交付前移除 —— PHIRA_SHOT=<路径> 时每 120 帧把帧缓冲写成 PNG
        {
            use once_cell::sync::Lazy;
            use std::sync::atomic::AtomicU32;
            static SHOT: Lazy<Option<String>> = Lazy::new(|| std::env::var("PHIRA_SHOT").ok());
            static N: AtomicU32 = AtomicU32::new(0);
            if let Some(path) = SHOT.as_ref() {
                if N.fetch_add(1, Ordering::Relaxed) % 120 == 0 {
                    macroquad::texture::get_screen_data().export_png(path);
                }
            }
        }

        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        // Debug 场景请求
        if crate::scene::take_debug_crash() {
            return NextScene::Overlay(Box::new(CrashScene::new(
                CrashCode::ManualCrash,
                "Debug Crash Scene".to_string(),
            )));
        }

        // 处理 pending debug scene result
        if let Some(scene) = self.pending_debug_scene_result.take() {
            eprintln!("[Debug] Returning pending debug scene");
            return scene;
        }

        // 进入多人场景：主菜单按钮 / 谱面库选谱 / 深链接都会置一次性请求。
        // 多人场景与主菜单平级（压栈 Overlay），返回时 Pop 回主菜单。
        let enter_mp = crate::mp::take_enter_request() && crate::mp::session_ready();
        let res = if enter_mp {
            NextScene::Overlay(Box::new(MultiplayerScene::new()))
        } else {
            self.pages.last_mut().unwrap().next_scene(&mut self.state)
        };
        if !matches!(res, NextScene::None) {
            if let Some(bgm) = &mut self.bgm {
                let _ = bgm.fade_out(1.5);
                self.bgm_resume_on_enter = true;
            }
        }
        res
    }
}

static STRIPE_MATERIAL: Lazy<Material> = Lazy::new(|| {
    load_material(
        shader::VERTEX,
        shader::FRAGMENT,
        MaterialParams {
            uniforms: vec![("time".to_owned(), UniformType::Float1)],
            ..Default::default()
        },
    )
    .unwrap()
});

mod shader {
    pub const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying lowp vec4 color;
varying lowp vec2 pos0;
varying lowp vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    color = color0 / 255.0;
    pos0 = position.xy;
    uv = texcoord;
}"#;

    pub const FRAGMENT: &str = r#"#version 100
precision highp float;

varying lowp vec4 color;
varying lowp vec2 pos0;
varying lowp vec2 uv;

uniform sampler2D Texture;
uniform float time;

void main() {
    float angle = 0.66;
    float w = sin(angle) * pos0.y + cos(angle) * pos0.x - time;
    float t = mod(w, 0.02);
    float p = step(t, 0.012) * 0.07;
    gl_FragColor = texture2D(Texture, uv);
    gl_FragColor += (vec4(1.0) - gl_FragColor) * p;
}"#;
}
