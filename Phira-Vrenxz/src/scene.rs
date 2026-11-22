prpr_l10n::tl_file!("import" itl);

mod chart_order;
pub use chart_order::ChartOrder;

mod chapter;
pub use chapter::ChapterScene;

mod lan_scene;
pub use lan_scene::LanScene;

mod debug;
pub use debug::DEBUG_LISTENER;

// Debug 场景请求
pub static DEBUG_REQUEST_CRASH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static DEBUG_REQUEST_LOADING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static DEBUG_REQUEST_GAME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static DEBUG_REQUEST_ENDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn request_debug_crash() {
    DEBUG_REQUEST_CRASH.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub fn take_debug_crash() -> bool {
    DEBUG_REQUEST_CRASH.swap(false, std::sync::atomic::Ordering::SeqCst)
}

pub fn request_debug_loading() {
    DEBUG_REQUEST_LOADING.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub fn take_debug_loading() -> bool {
    DEBUG_REQUEST_LOADING.swap(false, std::sync::atomic::Ordering::SeqCst)
}

pub fn request_debug_game() {
    DEBUG_REQUEST_GAME.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub fn take_debug_game() -> bool {
    DEBUG_REQUEST_GAME.swap(false, std::sync::atomic::Ordering::SeqCst)
}

pub fn request_debug_ending() {
    DEBUG_REQUEST_ENDING.store(true, std::sync::atomic::Ordering::SeqCst);
}

pub fn take_debug_ending() -> bool {
    DEBUG_REQUEST_ENDING.swap(false, std::sync::atomic::Ordering::SeqCst)
}

pub(crate) mod event;
pub use event::EventScene;

mod main;
pub use main::{
    home_background_index, MainScene, BACKGROUND_UPDATED, BGM_VOLUME_UPDATED, BUILTIN_HOME_BACKGROUNDS,
    HOME_VIDEO_UPDATED,
};

mod song;
pub use song::{compress_folder, Downloading, SongScene, RECORD_ID, LAST_MP_FINISH};
#[cfg(feature = "video")]
mod unlock;
#[cfg(feature = "video")]
pub use unlock::UnlockScene;

mod profile;
pub use profile::ProfileScene;

mod startup;
pub use startup::StartupLoadingScene;

mod studio_logo;
pub use studio_logo::StudioLogoScene;

mod setup_scene;

// 剧情代码已注释（2026-10-05）：mod story;
// pub use story::StoryScene;
pub use setup_scene::SetupScene;

use crate::{
    client::{Client, UserManager},
    data::LocalChart,
    dir, get_data, get_data_mut,
    page::Fader,
    save_data,
};
use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use once_cell::sync::{Lazy, OnceCell};
use prpr::{
    config::{HpMode, Mods},
    core::{BOLD_FONT, PGR_FONT},
    ext::{semi_white, unzip_into, RectExt, SafeTexture},
    fs::{self, FileSystem},
    info::{ChartFormat, ChartInfo},
    parse::ParseWarnings,
    scene::{show_error, show_message, FullLoadingView, GameScene},
    task::Task,
    ui::{Dialog, RectButton, Scroll, Scroller, Ui},
};
use std::{
    any::Any,
    cell::RefCell,
    fs::File,
    io::{BufReader, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tracing::{error, info, warn};
use uuid::Uuid;
use sasa::Music;

/// 开屏 → 主页共用的那首 BGM：开屏场景切走时把它放这里，主场景 enter 时取走继续播。
/// `Music` 一旦 drop 就会停，所以不能靠两个场景各存一份。
thread_local! {
    static CARRIED_BGM: RefCell<Option<Music>> = const { RefCell::new(None) };
}

pub fn carry_bgm(bgm: Music) {
    CARRIED_BGM.with(|it| *it.borrow_mut() = Some(bgm));
}

pub fn take_carried_bgm() -> Option<Music> {
    CARRIED_BGM.with(|it| it.borrow_mut().take())
}

/// 加载「开屏 / 主页」共用的 BGM 字节：
/// 自定义音乐（设置里合并后的那一项）> `bgm/NewSplashSceneBGM.ogg`。
/// `custom_startup_bgm_path` 是旧字段，只有旧数据还在用时才兜底读取。
pub fn load_shared_bgm_bytes() -> Result<Vec<u8>> {
    let custom = get_data()
        .custom_bgm_path
        .clone()
        .or_else(|| get_data().custom_startup_bgm_path.clone());
    if let Some(path) = custom {
        match std::fs::read(&path) {
            Ok(bytes) => return Ok(bytes),
            Err(err) => warn!("failed to read custom BGM {path}: {err}; falling back to splash BGM"),
        }
    }
    let path = assets_root().join("bgm/NewSplashSceneBGM.ogg");
    std::fs::read(&path).with_context(|| format!("read {}", path.display()))
}

thread_local! {
    pub static TEX_BACKGROUND: RefCell<Option<SafeTexture>> = const { RefCell::new(None) };
    pub static TEX_ICON_BACK: RefCell<Option<SafeTexture>> = const { RefCell::new(None) };
    /// 设置页左上角返回键使用的 proceed 图标（绘制时水平镜像）。
    pub static TEX_ICON_PROCEED: RefCell<Option<SafeTexture>> = const { RefCell::new(None) };
    /// 设置页左上角返回键整块贴图（icons/左上角2.png）


    pub static TEX_RANK_ICONS: RefCell<Option<[SafeTexture; 8]>> = const { RefCell::new(None) };
}

pub static ASSET_CHART_INFO: Lazy<Mutex<Option<ChartInfo>>> = Lazy::new(Mutex::default);
pub static TERMS: OnceCell<Option<(String, String)>> = OnceCell::new();
type LoadTosTask = Task<Result<Option<(String, String)>>>;
pub static LOAD_TOS_TASK: Lazy<Mutex<Option<LoadTosTask>>> = Lazy::new(Mutex::default);
pub static JUST_ACCEPTED_TOS: Lazy<AtomicBool> = Lazy::new(AtomicBool::default);
pub static JUST_LOADED_TOS: Lazy<AtomicBool> = Lazy::new(AtomicBool::default);

pub static TOS_VERIFIED: Lazy<AtomicBool> = Lazy::new(AtomicBool::default);

#[derive(Clone)]
#[allow(dead_code)]
pub struct AssetsChartFileSystem(pub String, pub String);

#[async_trait]
impl FileSystem for AssetsChartFileSystem {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>> {
        if path == ":info" {
            return Ok(serde_yaml::to_string(&ASSET_CHART_INFO.lock().unwrap().clone())?.into_bytes());
        }
        #[cfg(closed)]
        {
            use crate::load_res;
            if path == ":music" {
                return Ok(load_res(&format!("res/song/{}/music", self.0)).await);
            }
            if path == ":illu" {
                return Ok(load_res(&format!("res/song/{}/cover", self.0)).await);
            }
            if path == ":chart" {
                return Ok(load_res(&format!("res/song/{}/{}", self.0, self.1)).await);
            }
        }
        bail!("not found");
    }

    async fn exists(&mut self, _path: &str) -> Result<bool> {
        Ok(false)
    }

    fn list_root(&self) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn clone_box(&self) -> Box<dyn FileSystem> {
        Box::new(self.clone())
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

pub fn fs_from_path(path: &str) -> Result<Box<dyn FileSystem + Send + Sync + 'static>> {
    if let Some(name) = path.strip_prefix(':') {
        let (name, diff) = name.split_once(':').unwrap();
        Ok(Box::new(AssetsChartFileSystem(name.to_owned(), diff.to_owned())))
    } else if let Some(name) = path.strip_prefix("builtin:") {
        // 内置谱面：直接从内置谱面包（zip）里读，不再解压到 `assets/Level/`。
        // `name` 形如 `谱师目录/谱面.pez`，即压缩包内根目录（`Level/`）下的相对路径，
        // 该 `.pez` 本身又是一个 zip，所以再套一层 ZipFileSystem。
        let zip = open_builtin_level_zip()?;
        Ok(Box::new(
            zip.open_entry_as_zip(name).with_context(|| format!("failed to open builtin chart {name}"))?,
        ))
    } else {
        fs::fs_from_file(Path::new(&format!("{}/{path}", dir::charts()?)))
    }
}

/// 内置资源所在目录：
/// - 桌面端：工作目录下的 `assets`；
/// - Android：Java 侧已把资源 zip 复制到 `<getFilesDir()>/assets`，用绝对路径，
///   不依赖进程工作目录。
pub(crate) fn assets_root() -> PathBuf {
    #[cfg(target_os = "android")]
    if let Some(dir) = crate::writable_data_dir() {
        return PathBuf::from(dir).join("assets");
    }
    PathBuf::from("assets")
}

/// 内置谱面包的文件名。
/// 桌面端是 `assets/Level.zip`；Android 侧 Java 把 APK 里的 `Expansion_package.zip`
/// 复制到可写目录（两者内容一致，压缩包内根目录都是 `Level/`），因此按顺序探测。
pub(crate) const BUILTIN_LEVEL_ARCHIVES: &[&str] = &["Level.zip", "Expansion_package.zip"];

/// 在 `assets` 目录里找内置谱面包（zip）。不做解压，只做存在性检查。
pub(crate) fn builtin_level_zip_in(assets_dir: &Path) -> Option<PathBuf> {
    BUILTIN_LEVEL_ARCHIVES.iter().map(|it| assets_dir.join(it)).find(|it| it.is_file())
}

/// 内置谱面包是否存在。
pub(crate) fn builtin_level_zip_present() -> bool {
    builtin_level_zip_in(&assets_root()).is_some()
}

/// 打开内置谱面包的 zip 文件系统（惰性读文件，只载入中央目录）。
pub(crate) fn open_builtin_level_zip() -> Result<fs::ZipFileSystem> {
    let assets_dir = assets_root();
    let path = builtin_level_zip_in(&assets_dir).ok_or_else(|| {
        anyhow!(
            "内置谱面包不存在（{}），内置谱面不可用",
            BUILTIN_LEVEL_ARCHIVES
                .iter()
                .map(|it| assets_dir.join(it).display().to_string())
                .collect::<Vec<_>>()
                .join(" / ")
        )
    })?;
    fs::ZipFileSystem::open(&path).with_context(|| format!("无法打开内置谱面包 {}", path.display()))
}

pub fn confirm_dialog(title: impl Into<String>, content: impl Into<String>, res: Arc<AtomicBool>) {
    Dialog::plain(title.into(), content.into())
        .buttons(vec![ttl!("cancel").into_owned(), ttl!("confirm").into_owned()])
        .listener(move |_dialog, id| {
            if id == 1 {
                res.store(true, Ordering::SeqCst);
            }
            false
        })
        .show();
}

pub fn check_read_tos_and_policy(change_just_accepted: bool, strict: bool) -> bool {
    if let Some(value) = dispatch_tos_task() {
        return value;
    }
    if get_data().terms_modified.is_some() && !strict {
        return true;
    }

    if get_data().terms_modified.is_some() && TOS_VERIFIED.load(Ordering::Relaxed) {
        return true;
    }
    match TERMS.get() {
        Some(Some((terms, modified))) => {

            if get_data().terms_modified.as_deref() == Some(modified.as_str()) {
                return true;
            }
            let content = ttl!("tos-and-policy-desc") + "\n\n" + terms.as_str();
            let lines = content.split('\n').collect::<Vec<_>>();
            let pages = lines.chunks(50).map(|it| it.join("\n")).collect::<Vec<_>>();
            let pages_len = pages.len();
            let mut page = 0;
            let gen_buttons = move |page: usize| {
                let mut btns = vec![ttl!("tos-deny").into_owned()];
                let mut btn_ids = vec![0u8];
                if page != 0 {
                    btns.push(ttl!("tos-prev-page").into_owned());
                    btn_ids.push(1);
                }
                if page != pages_len - 1 {
                    btns.push(ttl!("tos-next-page").into_owned());
                    btn_ids.push(2);
                } else {
                    btns.push(ttl!("tos-accept").into_owned());
                    btn_ids.push(3);
                }
                (btns, btn_ids)
            };
            let (btns, mut btn_ids) = gen_buttons(page);
            Dialog::plain(ttl!("tos-and-policy"), &pages[page])
                .buttons(btns)
                .listener(move |dialog, pos| match pos {
                    -2 | -1 => true,
                    _ => {
                        match btn_ids[pos as usize] {
                            0 => {
                                show_message(ttl!("warn-deny-tos-policy")).warn();
                                return false;
                            }
                            1 => {
                                page -= 1;
                            }
                            2 => {
                                page += 1;
                            }
                            3 => {
                                get_data_mut().terms_modified = Some(modified.clone());
                                let _ = save_data();
                                if change_just_accepted {
                                    JUST_ACCEPTED_TOS.store(true, Ordering::Relaxed);
                                }
                                return false;
                            }
                            _ => unreachable!(),
                        }
                        let (btns, new_btn_ids) = gen_buttons(page);
                        btn_ids = new_btn_ids;
                        dialog.set_buttons(btns);
                        dialog.set_message(&pages[page]);
                        true
                    }
                })
                .show();
        }
        Some(None) => {
            error!("unreachable")
        }
        None => {
            if !strict {
                warn!("loading data to read because `check_..` was called, this would result a delay and shouldn't happen");
            }
            load_tos_and_policy(true);
        }
    }
    false
}

pub fn dispatch_tos_task() -> Option<bool> {
    let mut tos_task = LOAD_TOS_TASK.lock().unwrap();
    if let Some(task) = &mut *tos_task {
        if let Some(result) = task.take() {
            *tos_task = None;
            drop(tos_task);
            match result {
                Ok(Some(res)) => {

                    info!("terms and policy loaded");
                    get_data_mut().terms_modified = None;
                    let _ = save_data();
                    let _ = TERMS.set(Some(res));
                }
                Ok(None) => {

                    info!("terms and policy unchanged");
                    if get_data().terms_modified.is_some() {
                        TOS_VERIFIED.store(true, Ordering::Relaxed);
                        JUST_ACCEPTED_TOS.store(true, Ordering::Relaxed);
                        return Some(true);
                    }
                }
                Err(e) => {
                    show_error(e.context(ttl!("fetch-tos-policy-failed")));
                    return Some(false);
                }
            }
            return None;
        }
    }
    drop(tos_task);
    None
}
/// use the return value to add a loading screen
pub fn load_tos_and_policy(show_loading: bool) {
    if TERMS.get().is_some() {
        return;
    }
    let mut guard = LOAD_TOS_TASK.lock().unwrap();
    if guard.is_none() {
        let modified = get_data().terms_modified.clone();
        let loading = show_loading.then(|| FullLoadingView::begin_text(ttl!("loading_tos_policy")));
        *guard = Some(Task::new(async move {

            let ret = Client::fetch_terms(modified.as_deref()).await.context("failed to fetch terms");
            drop(loading);
            JUST_LOADED_TOS.store(true, Ordering::Relaxed);
            ret
        }));
    }
}

#[inline]
pub fn confirm_delete(res: Arc<AtomicBool>) {
    confirm_dialog(ttl!("del-confirm"), ttl!("del-confirm-content"), res)
}

pub fn gen_custom_dir() -> Result<(PathBuf, Uuid)> {
    let dir = dir::custom_charts()?;
    let dir = Path::new(&dir);
    let mut id = Uuid::new_v4();
    while dir.join(id.to_string()).exists() {
        id = Uuid::new_v4();
    }
    let dir = dir.join(id.to_string());
    std::fs::create_dir(&dir)?;

    Ok((dir, id))
}

pub fn parse_warnings_to_string(w: &ParseWarnings) -> Option<String> {
    let mut warnings = vec![];
    if w.has_new_speed_events {
        warnings.push(format!("- {}", itl!("warning-new-speed-event")));
    }
    if w.has_attach_ui {
        warnings.push(format!("- {}", itl!("warning-attach-ui")));
    }
    if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("\n"))
    }
}

async fn lint_chart(fs: &mut dyn FileSystem, info: &ChartInfo) -> Result<ParseWarnings> {
    let bytes = GameScene::load_chart_bytes(fs, info).await.context("Failed to load chart")?;
    let format = GameScene::infer_chart_format(info, &bytes);
    if format != ChartFormat::Rpe {
        return Ok(ParseWarnings::default());
    }
    let source = String::from_utf8_lossy(&bytes);
    prpr::parse::lint(&source).await
}

fn find_named_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_named_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name().is_some_and(|it| it == name) {
            return Some(path);
        }
    }
    None
}

fn has_audio_file(dir: &Path) -> bool {
    std::fs::read_dir(dir).ok().is_some_and(|entries| {
        entries.flatten().any(|entry| {
            let path = entry.path();
            path.is_file()
                && path
                    .extension()
                    .and_then(|it| it.to_str())
                    .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "mp3" | "ogg" | "wav" | "flac" | "aac"))
        })
    })
}

/// A bare ADOFAI file may not carry its audio.  Keep the chart playable by
/// adding a short silent track instead of failing to open the chart later.
fn write_silent_wav(path: &Path) -> Result<()> {
    const RATE: u32 = 8000;
    const SAMPLES: u32 = 1600; // 0.2 s, mono 16-bit
    let data_len = SAMPLES * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.resize(out.len() + data_len as usize, 0);
    std::fs::write(path, out)?;
    Ok(())
}

fn find_adofai(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_adofai(&path) {
                return Some(found);
            }
        } else if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("adofai")) {
            return Some(path);
        }
    }
    None
}

fn looks_like_adofai(path: &Path) -> bool {
    if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("adofai")) {
        return true;
    }
    let Ok(source) = std::fs::read_to_string(path) else {
        return false;
    };
    let source = source.trim_start_matches('\u{feff}');
    let Ok(value) = serde_json::from_str::<serde_json::Value>(source) else {
        return false;
    };
    value.get("settings").is_some() && (value.get("angleData").is_some() || value.get("pathData").is_some())
}

async fn import_adofai_from(
    root: &Path,
    adofai_path: &Path,
    local_path: String,
    source_root: &Path,
    key_mode: crate::adofai::KeyMode,
) -> Result<(LocalChart, ParseWarnings)> {
    let source = std::fs::read_to_string(adofai_path)
        .with_context(|| format!("cannot read ADOFAI chart {}", adofai_path.display()))?;
    let fallback = adofai_path.file_stem().and_then(|it| it.to_str()).unwrap_or("ADOFAI");
    let converted = crate::adofai::convert_with_key_mode(&source, fallback, key_mode)?;

    std::fs::write(root.join("chart.json"), &converted.chart_json)?;

    // A bare .adofai file is normally next to its audio/background.  For a
    // zip import these already live in `root`, so skip copying in that case.
    let mut info = converted.info;
    for (field, name) in [("music", converted.song.as_deref()), ("illustration", converted.background.as_deref())] {
        let Some(name) = name else { continue };
        let dst = root.join(name);
        let src = source_root
            .join(name)
            .exists()
            .then(|| source_root.join(name))
            .or_else(|| adofai_path.parent().map(|it| it.join(name)).filter(|it| it.exists()))
            .or_else(|| find_named_file(root, name));
        if let Some(src) = src {
            if src != dst {
                if let Some(parent) = dst.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&src, &dst).with_context(|| format!("cannot copy ADOFAI asset {name}"))?;
            }
        }
        if dst.is_file() {
            match field {
                "music" => info.music = name.to_owned(),
                _ => info.illustration = name.to_owned(),
            }
        }
    }
    if !has_audio_file(root) {
        bail!(
            "ADOFAI 缺少音频文件（{}）。请把 .adofai 和 settings.songFilename 指向的音频放在同一目录，或一起打包成 zip 后再导入。",
            info.music
        );
    }

    std::fs::write(root.join("info.yml"), serde_yaml::to_string(&info)?)?;
    Ok((
        LocalChart {
            info: info.into(),
            local_path,
            record: None,
            mods: Mods::default(),
            hp: HpMode::Off,
            played_unlock: false,
        },
        ParseWarnings::default(),
    ))
}

async fn import_adofai_path(path: &Path, source_root: &Path, key_mode: crate::adofai::KeyMode) -> Result<(LocalChart, ParseWarnings)> {
    let (root, id) = gen_custom_dir()?;
    let result = import_adofai_from(&root, path, format!("custom/{id}"), source_root, key_mode).await;
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&root);
    }
    result
}

/// Import a chart from a filesystem path.  Supports the normal Phira zip /
/// `.pez` archives as well as a bare ADOFAI `.adofai` JSON file (or a folder
/// containing one plus its audio/background).
pub async fn import_chart_path(path: &Path) -> Result<(LocalChart, ParseWarnings)> {
    import_chart_path_with_mode(path, crate::adofai::KeyMode::K4).await
}

pub async fn import_chart_path_with_mode(
    path: &Path,
    key_mode: crate::adofai::KeyMode,
) -> Result<(LocalChart, ParseWarnings)> {
    if path.is_dir() {
        if let Some(adofai) = find_adofai(path) {
            return import_adofai_path(&adofai, path, key_mode).await;
        }
        bail!("folder does not contain an .adofai chart");
    }
    if looks_like_adofai(path) {
        return import_adofai_path(path, path.parent().unwrap_or_else(|| Path::new(".")), key_mode).await;
    }
    let file = File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
    import_chart(file).await
}

pub async fn import_chart_to(dir: &Path, local_path: String, file: File) -> Result<(LocalChart, ParseWarnings)> {
    let root = dir.to_path_buf();
    let dir = prpr::dir::Dir::new(&root)?;
    unzip_into(BufReader::new(file), &dir, true)?;
    if !root.join("info.yml").exists() && !root.join("info.txt").exists() && !root.join("info.csv").exists() {
        if let Some(adofai_path) = find_adofai(&root) {
            return import_adofai_from(&root, &adofai_path, local_path, &root, crate::adofai::KeyMode::K4).await;
        }
    }
    let mut fs = fs_from_path(&local_path)?;
    let mut info = fs::load_info(fs.as_mut()).await.with_context(|| itl!("info-fail"))?;
    fs::fix_info(fs.as_mut(), &mut info).await.with_context(|| itl!("invalid-chart"))?;
    let warnings = if info.use_rpe_170_speed.is_none() || info.use_attach_ui_fix.is_none() {
        if info.use_attach_ui_fix.is_none() {
            info.use_attach_ui_fix = Some(true);
        }
        lint_chart(fs.as_mut(), &info).await?
    } else {
        ParseWarnings::default()
    };
    dir.create("info.yml")?.write_all(serde_yaml::to_string(&info)?.as_bytes())?;
    Ok((
        LocalChart {
            info: info.into(),
            local_path,
            record: None,
            mods: Mods::default(),
            hp: HpMode::Off,
            played_unlock: false,
        },
        warnings,
    ))
}

pub async fn import_chart(file: File) -> Result<(LocalChart, ParseWarnings)> {
    let (dir, id) = gen_custom_dir()?;
    match import_chart_to(&dir, format!("custom/{id}"), file).await {
        Err(err) => {
            std::fs::remove_dir_all(dir)?;
            Err(err)
        }
        Ok(val) => Ok(val),
    }
}

pub struct LdbDisplayItem<'a> {
    pub player_id: i32,
    pub rank: u32,
    pub score: String,
    pub alt: Option<String>,
    pub btn: &'a mut RectButton,
}

#[allow(clippy::too_many_arguments)]
pub fn render_ldb<'a>(
    ui: &mut Ui,
    title: &str,
    w: f32,
    rt: f32,
    scroll: &mut Scroll,
    fader: &mut Fader,
    icon_user: &SafeTexture,
    iter: Option<impl Iterator<Item = LdbDisplayItem<'a>>>,
) {
    use macroquad::prelude::*;

    let pad = 0.03;
    let width = w - pad;
    ui.dy(0.01);
    let r = ui.text(title).size(0.9).draw_using(&BOLD_FONT);
    ui.dy(r.h + 0.05);
    let sh = ui.top * 2. - r.h - 0.08;
    let Some(iter) = iter else {
        ui.loading(width / 2., sh / 2., rt, WHITE, ());
        return;
    };
    let off = scroll.y_scroller.offset;
    scroll.size((width, sh));
    scroll.render(ui, |ui| {
        render_release_to_refresh(ui, width / 2., off);
        let s = 0.14;
        let mut h = 0.;
        ui.dx(0.02);
        fader.reset();
        let me = get_data().me.as_ref().map(|it| it.id);
        fader.for_sub(|f| {
            for item in iter {
                f.render(ui, rt, |ui| {
                    if me == Some(item.player_id) {
                        ui.fill_path(&Rect::new(-0.02, 0., width, s).feather(-0.01).rounded(0.02), ui.background());
                    }
                    let r = s / 2. - 0.02;
                    ui.text(format!("#{}", item.rank))
                        .pos((0.18 - r) / 2., s / 2.)
                        .anchor(0.5, 0.5)
                        .no_baseline()
                        .size(0.52)
                        .draw_using(&PGR_FONT);
                    let ct = (0.18, s / 2.);
                    ui.avatar(ct.0, ct.1, r, rt, UserManager::opt_avatar(item.player_id, icon_user));
                    item.btn.set(ui, Rect::new(ct.0 - r, ct.1 - r, r * 2., r * 2.));
                    let mut rt = width - 0.04;
                    if let Some(alt) = item.alt {
                        let r = ui
                            .text(alt)
                            .pos(rt, s / 2.)
                            .anchor(1., 0.5)
                            .no_baseline()
                            .size(0.4)
                            .color(semi_white(0.6))
                            .draw_using(&BOLD_FONT);
                        rt -= r.w + 0.01;
                    } else {
                        rt -= 0.01;
                    }
                    let r = ui
                        .text(item.score)
                        .pos(rt, s / 2.)
                        .anchor(1., 0.5)
                        .no_baseline()
                        .size(0.6)
                        .draw_using(&PGR_FONT);
                    rt -= r.w + 0.03;
                    let lt = 0.25;
                    if let Some((name, color)) = UserManager::name_and_color(item.player_id) {
                        ui.text(name)
                            .pos(lt, s / 2.)
                            .anchor(0., 0.5)
                            .no_baseline()
                            .max_width(rt - lt - 0.01)
                            .size(0.5)
                            .color(color)
                            .draw();
                    }
                });
                ui.dy(s);
                h += s;
            }
        });
        (width, h)
    });
}

pub fn render_release_to_refresh(ui: &mut Ui, cx: f32, off: f32) {
    let p = (-off / Scroller::EXTEND).clamp(0., 1.);
    ui.text(ttl!("release-to-refresh"))
        .pos(cx, -0.2 + p * 0.07)
        .anchor(0.5, 0.)
        .size(0.8)
        .color(semi_white(p * 0.8))
        .draw();
}

#[cfg(test)]
mod tests {

}
