prpr_l10n::tl_file!("import" itl);

mod chart_order;
pub use chart_order::ChartOrder;

mod chapter;
pub use chapter::ChapterScene;

pub(crate) mod event;
pub use event::EventScene;

mod main;
pub use main::{MainScene, BGM_VOLUME_UPDATED};

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

mod login_scene;
pub use login_scene::LoginScene;

mod setup_scene;
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
    config::Mods,
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

thread_local! {
    pub static TEX_BACKGROUND: RefCell<Option<SafeTexture>> = const { RefCell::new(None) };
    pub static TEX_ICON_BACK: RefCell<Option<SafeTexture>> = const { RefCell::new(None) };
    /// 谱面成绩等级图标（`assets/rank/*.png`，顺序同 [prpr::judge::icon_index]）。
    ///
    /// 与 [TEX_ICON_BACK] 一样在启动时装载：选曲页是通过 `ChartItem` 传进来的，
    /// 但多人房间页没有那条链路，左下角的「最好成绩」又必须和选曲页长得一样，
    /// 所以这里留一份全局的。
    pub static TEX_RANK_ICONS: RefCell<Option<[SafeTexture; 8]>> = const { RefCell::new(None) };
}

pub static ASSET_CHART_INFO: Lazy<Mutex<Option<ChartInfo>>> = Lazy::new(Mutex::default);
pub static TERMS: OnceCell<Option<(String, String)>> = OnceCell::new();
type LoadTosTask = Task<Result<Option<(String, String)>>>;
pub static LOAD_TOS_TASK: Lazy<Mutex<Option<LoadTosTask>>> = Lazy::new(Mutex::default);
pub static JUST_ACCEPTED_TOS: Lazy<AtomicBool> = Lazy::new(AtomicBool::default);
pub static JUST_LOADED_TOS: Lazy<AtomicBool> = Lazy::new(AtomicBool::default);
/// Set once the policy has been verified against the server this session and
/// found unchanged, so a still-accepted policy isn't re-fetched or re-prompted.
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

pub async fn import_chart_to(dir: &Path, local_path: String, file: File) -> Result<(LocalChart, ParseWarnings)> {
    let dir = prpr::dir::Dir::new(dir)?;
    unzip_into(BufReader::new(file), &dir, true)?;
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
