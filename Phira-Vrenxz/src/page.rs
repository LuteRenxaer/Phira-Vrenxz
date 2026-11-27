mod challenge;
pub use challenge::ChallengePage;

/// 谱面合集 l10n 容器（合集页已移除，仅保留文案供 charts_view 占位卡片使用）
pub mod coll;
pub mod phigros_library;
pub use phigros_library::PhigrosLibraryPage;
pub mod replay_manager;
pub use replay_manager::ReplayManager;

mod data_manage;
pub use data_manage::DataManagePage;

mod event;
pub use event::EventPage;

mod extension;
pub use extension::ExtensionPage;

pub mod favorites;
pub use favorites::FavoritesPage;

mod home;
pub use home::HomePage;

mod library;
pub use library::{
    request_export, request_export_filtered, resolve_export, take_export, ExportConfig, ExportInfo, LibraryPage,
    CHOOSE_COVER, CHOSEN_COVER, FAV_UPDATED,
};

mod message;
pub use message::MessagePage;

mod offset;
pub use offset::OffsetPage;


mod respack;
pub use respack::{ResPackItem, ResPackPage};

mod settings;

pub(crate) use settings::render_switch;
pub use settings::SettingsPage;
use tokio::sync::Notify;

use crate::{
    client::{Chart, ChartRef, File},
    data::BriefChartInfo,
    dir, get_data,
    images::Images,
    scene::{fs_from_path, open_builtin_level_zip},
};
use anyhow::{Context, Result};
use image::DynamicImage;
use macroquad::prelude::*;
use prpr::{
    core::{Resource, BOLD_FONT},
    ext::{semi_black, semi_white, SafeTexture, ScaleType, BLACK_TEXTURE},
    fs::{self, FileSystem, ZipFileSystem},
    scene::{show_error, NextScene, Scene},
    task::Task,
    time::TimeManager,
    ui::{FontArc, IntoShading, Shading, TextPainter, Ui},
};
use std::{
    any::Any,
    borrow::Cow,
    cell::RefCell,
    ops::DerefMut,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tracing::warn;

pub fn thumbnail_path(path: &str) -> Result<PathBuf> {
    // `:`（builtin 谱面 local_path 形如 builtin:xxx/yyy.pez）在 Windows 路径中非法，
    // 与 `/` 一并替换为 `_`，保证缓存文件名合法且与 serve_cover_cache 一致。
    Ok(format!("{}/{}", dir::cache_image_local()?, path.replace('/', "_").replace(':', "_")).into())
}

pub fn illustration_task(notify: Arc<Notify>, path: String, full: bool) -> Task<Result<(DynamicImage, Option<DynamicImage>)>> {
    Task::new(async move {
        notify.notified().await;
        let mut fs = fs_from_path(&path)
            .with_context(|| format!("[debug] fs_from_path failed: path={path:?}"))?;
        let info = fs::load_info(fs.deref_mut())
            .await
            .with_context(|| format!("[debug] load_info failed: path={path:?}"))?;
        let thumb_p = thumbnail_path(&path)
            .with_context(|| format!("[debug] thumbnail_path failed: path={path:?}"))?;
        let mut img = None;
        let thumbnail = Images::local_or_else(thumb_p.clone(), async {
            let image = image::load_from_memory(
                &fs.load_file(&info.illustration)
                    .await
                    .with_context(|| format!("[debug] load illustration file failed: path={path:?}, illustration={:?}, thumb={:?}", info.illustration, thumb_p))?,
            )
            .with_context(|| format!("[debug] decode illustration failed: path={path:?}, illustration={:?}", info.illustration))?;
            let thumbnail = Images::thumbnail(&image);
            img = Some(image);
            Ok(thumbnail)
        })
        .await
        .with_context(|| format!("[debug] local_or_else failed: thumb={thumb_p:?}"))?;
        if full {
            if img.is_none() {
                img = Some(
                    image::load_from_memory(
                        &fs.load_file(&info.illustration)
                            .await
                            .with_context(|| format!("[debug] full: load illustration file failed: path={path:?}, illustration={:?}", info.illustration))?,
                    )
                    .with_context(|| format!("[debug] full: decode illustration failed: path={path:?}, illustration={:?}", info.illustration))?,
                );
            }
        } else {
            img = None;
        }
        Ok((thumbnail, img))
    })
}

pub fn local_illustration(path: String, def: SafeTexture, full: bool) -> Illustration {
    let notify = Arc::new(Notify::new());
    Illustration {
        texture: (def.clone(), def),
        notify: Arc::clone(&notify),
        task: Some(illustration_task(notify, path, full)),
        loaded: Arc::default(),
        load_time: f32::NAN,
    }
}

pub fn load_local() -> Vec<ChartItem> {
    let tex = BLACK_TEXTURE.clone();
    get_data()
        .charts
        .iter()
        .map(|it| ChartItem {
            info: it.info.clone(),
            local_path: Some(it.local_path.clone()),
            illu: local_illustration(it.local_path.clone(), tex.clone(), false),
            chart_type: ChartType::Imported,
            level_author: None,
            xcsim_preview_url: None,
            xcsim_illustration_url: None,
        })
        .collect()
}

pub fn load_builtin_task() -> Task<Result<Vec<ChartItem>>> {
    Task::new(async move {
        let tex = BLACK_TEXTURE.clone();
        let mut charts = Vec::new();
        let mut zip = match open_builtin_level_zip() {
            Ok(it) => it,
            Err(err) => {
                warn!("failed to open builtin level package: {err:#}");

                if crate::scene::builtin_level_zip_present() {
                    show_error(err);
                }
                return Ok(charts);
            }
        };

        for entry in zip.list_dir("") {
            let Some(file_name) = entry.strip_suffix('/').filter(|it| !it.is_empty()) else {
                continue;
            };
            let file_name = file_name.to_owned();
            let Ok(content) = zip.load_file(&format!("{file_name}/Level.json")).await else {
                continue;
            };
            let Ok(level_data) = serde_json::from_slice::<Vec<serde_json::Value>>(&content) else {
                continue;
            };

            let author = level_data.iter().find_map(|v| {
                let name = v.get("name")?.as_str()?;
                let terrace = v.get("terrace")?.as_str()?;
                let link = v.get("link")?.as_str()?;
                Some(LevelAuthor {
                    name: name.to_string(),
                    terrace: terrace.to_string(),
                    link: link.to_string(),
                })
            });

            let Some(level_str) = level_data.iter().find_map(|v| v.get("level").and_then(|l| l.as_str())) else {
                continue;
            };
            for pez_name in level_str.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                let rel_path = format!("{file_name}/{pez_name}");
                let path_str = format!("builtin:{rel_path}");
                let bytes = match zip.load_file(&rel_path).await {
                    Ok(it) => it,
                    Err(err) => {
                        warn!("builtin chart {rel_path} is missing: {err:#}");
                        continue;
                    }
                };

                let mut inner = match ZipFileSystem::new(bytes) {
                    Ok(it) => it,
                    Err(err) => {
                        warn!("builtin chart {rel_path} is not an archive: {err:#}");
                        continue;
                    }
                };
                match fs::load_info(&mut inner).await {
                    Ok(info) => charts.push(ChartItem {
                        info: BriefChartInfo { id: None, ..info.into() },
                        local_path: Some(path_str.clone()),
                        illu: local_illustration(path_str, tex.clone(), false),
                        chart_type: ChartType::Imported,
                        level_author: author.clone(),
                        xcsim_preview_url: None,
                        xcsim_illustration_url: None,
                    }),
                    Err(err) => warn!("failed to load info of builtin chart {rel_path}: {err:#}"),
                }
            }
        }
        Ok(charts)
    })
}

type IllustrationTask = Task<Result<(DynamicImage, Option<DynamicImage>)>>;

#[derive(Clone)]
pub struct Illustration {
    pub texture: (SafeTexture, SafeTexture),
    pub notify: Arc<Notify>,
    pub task: Option<IllustrationTask>,
    pub loaded: Arc<Mutex<Option<(SafeTexture, SafeTexture)>>>,
    pub load_time: f32,
}

impl Illustration {
    const TIME: f32 = 0.4;

    pub fn from_file(file: File) -> Self {
        let notify = Arc::default();
        Self {
            texture: (BLACK_TEXTURE.clone(), BLACK_TEXTURE.clone()),
            notify: Arc::clone(&notify),
            task: Some(Task::new(async move {
                notify.notified().await;
                Ok((file.load_image().await?, None))
            })),
            loaded: Arc::default(),
            load_time: f32::NAN,
        }
    }

    pub fn from_file_thumbnail(file: File) -> Self {
        let notify = Arc::default();
        Self {
            texture: (BLACK_TEXTURE.clone(), BLACK_TEXTURE.clone()),
            notify: Arc::clone(&notify),
            task: Some(Task::new(async move {
                notify.notified().await;
                Ok((file.load_thumbnail().await?, None))
            })),
            loaded: Arc::default(),
            load_time: f32::NAN,
        }
    }

    pub fn from_done(tex: SafeTexture) -> Self {
        Self {
            texture: (tex.clone(), tex),
            notify: Arc::default(),
            task: None,
            loaded: Arc::default(),
            load_time: f32::NAN,
        }
    }

    pub fn notify(&self) {
        self.notify.notify_one();
    }

    pub fn settle(&mut self, t: f32) {
        if let Some(task) = &mut self.task {
            if let Some(illu) = task.take() {
                match illu {
                    Err(err) => {
                        warn!(?err, "failed to load illustration");
                    }
                    Ok(illu) => {
                        self.texture = Images::into_texture(illu);
                    }
                };
                *self.loaded.lock().unwrap() = Some(self.texture.clone());
                self.task = None;
                self.load_time = t;
            } else if let Some(loaded) = self.loaded.lock().unwrap().clone() {
                self.texture = loaded;
                self.load_time = t;
                self.task = None;
            }
        } else if self.load_time.is_nan() {
            self.load_time = t;
        }
    }

    pub fn alpha(&self, t: f32) -> f32 {
        if self.load_time.is_nan() {
            0.
        } else if get_data().prefer_reduced_motion {
            1.
        } else {
            ((t - self.load_time) / Self::TIME).min(1.)
        }
    }

    pub fn shading(&self, r: Rect, t: f32) -> impl Shading {
        (*self.texture.0, r, ScaleType::CropCenter, semi_white(self.alpha(t))).into_shading()
    }
}

#[derive(Clone)]
pub struct LevelAuthor {
    pub name: String,
    pub terrace: String,
    pub link: String,
}

#[derive(Clone)]
pub struct ChartItem {
    pub info: BriefChartInfo,
    pub local_path: Option<String>,
    pub illu: Illustration,
    pub chart_type: ChartType,
    pub level_author: Option<LevelAuthor>,
    pub xcsim_preview_url: Option<String>,
    pub xcsim_illustration_url: Option<String>,
}
impl ChartItem {
    pub fn to_bare_ref(&self) -> ChartRef {
        ChartRef::new_bare(self.info.id, self.local_path.as_deref())
    }

    pub fn from_remote(chart: &Chart) -> Self {
        ChartItem {
            info: chart.to_info(),
            illu: Illustration::from_file_thumbnail(chart.illustration.clone()),
            local_path: None,
            chart_type: ChartType::Downloaded,
            level_author: None,
            xcsim_preview_url: None,
            xcsim_illustration_url: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChartType {
    Downloaded,
    Imported,
    Integrated,
    XCSim,
    LocalServer,
}

pub struct Fader {
    pub distance: f32,
    start_time: f32,
    pub time: f32,
    index: usize,
    back: bool,
    pub sub: bool,
}

impl Fader {
    const DELTA: f32 = 0.04;

    pub fn new() -> Self {
        Self {
            distance: 0.2,
            start_time: f32::NAN,
            time: 0.7,
            index: 0,
            back: false,
            sub: false,
        }
    }

    #[inline]
    pub fn with_time(mut self, time: f32) -> Self {
        self.time = time;
        self
    }

    #[inline]
    pub fn with_distance(mut self, distance: f32) -> Self {
        self.distance = distance;
        self
    }

    #[inline]
    pub fn reset(&mut self) {
        self.index = 0;
    }

    #[inline]
    pub fn sub(&mut self, t: f32) {
        self.start_time = t;
        self.back = false;
    }

    #[inline]
    pub fn for_sub<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.sub = true;
        let res = f(self);
        self.sub = false;
        res
    }

    #[inline]
    pub fn back(&mut self, t: f32) {
        self.start_time = t;
        self.back = true;
    }

    pub fn progress_scaled(&self, t: f32, scale: f32) -> f32 {
        if self.start_time.is_nan() {
            0.
        } else {
            let p = if get_data().prefer_reduced_motion {
                1.
            } else {
                ((t - self.start_time) / self.time * scale).clamp(0., 1.)
            };
            let p = (1. - p).powi(3);
            let p = if self.back { p } else { 1. - p };
            if self.sub {
                1. - p
            } else {
                -p
            }
        }
    }

    pub fn progress(&self, t: f32) -> f32 {
        self.progress_scaled(t, 1.)
    }

    pub fn roll_back(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    pub fn render<R>(&mut self, ui: &mut Ui, t: f32, f: impl FnOnce(&mut Ui) -> R) -> R {
        let p = self.progress(t - self.index as f32 * Self::DELTA);
        let (dy, alpha) = (p * self.distance, 1. - p.abs());
        self.index += 1;
        ui.scope(|ui| {
            ui.dy(dy);
            ui.alpha(alpha, f)
        })
    }

    #[inline]
    pub fn transiting(&self) -> bool {
        !self.start_time.is_nan()
    }

    pub fn done(&mut self, t: f32) -> Option<bool> {
        if !self.start_time.is_nan() && (t - self.start_time > self.time || get_data().prefer_reduced_motion) {
            self.start_time = f32::NAN;
            Some(self.back)
        } else {
            None
        }
    }

    pub fn render_title(&mut self, ui: &mut Ui, t: f32, s: &str) {
        let tp = ui.back_rect().center().y;
        let h = ui.text("L").size(1.2).no_baseline().measure_using(&BOLD_FONT).h;
        ui.scissor(Rect::new(-1., tp - h / 2., 2., h), |ui| {
            let p = self.progress_scaled(t, 1.6);
            let tp = tp + h * p - h / 2.;
            let mut x = -0.87;
            if s == "PHIRA-VRENXZ" {
                x -= ui.back_rect().w;
            }
            for c in s.chars() {
                x += ui
                    .text(c.to_string())
                    .pos(x, tp)
                    .anchor(0., 0.)
                    .size(1.2)
                    .color(WHITE)
                    .draw_using(&BOLD_FONT)
                    .w
                    + 0.012;
            }
            if s == "PHIRA-VRENXZ" {
                ui.text(format!("v{}", crate::APP_VERSION))
                    .pos(x + 0.02, tp + h - 0.02)
                    .anchor(0., 1.)
                    .color(semi_white(0.4))
                    .size(0.5)
                    .draw_using(&BOLD_FONT);
            }
        });
    }
}

pub struct SFader {
    time: f32,
    next_scene: Option<NextScene>,
}

impl SFader {
    const TIME: f32 = 0.35;

    pub fn new() -> Self {
        Self {
            time: f32::NAN,
            next_scene: None,
        }
    }

    pub fn transiting(&self) -> bool {
        !self.time.is_nan()
    }

    pub fn goto(&mut self, t: f32, scene: impl Scene + 'static) {
        self.time = t;
        self.next_scene = Some(NextScene::Overlay(Box::new(scene)));
    }

    pub fn next(&mut self, t: f32, next: NextScene) {
        self.time = t;
        self.next_scene = Some(next);
    }

    pub fn enter(&mut self, t: f32) {
        self.time = t;
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32) {
        if self.time.is_nan() {
            return;
        }
        let p = if get_data().prefer_reduced_motion {
            1.
        } else {
            ((t - self.time) / Self::TIME).min(1.)
        };
        if p >= 1. && self.next_scene.is_none() {
            self.time = f32::NAN;
        } else {
            ui.fill_rect(ui.screen_rect(), semi_black(if self.next_scene.is_some() { p } else { 1. - p }));
        }
    }

    pub fn next_scene(&mut self, t: f32) -> Option<NextScene> {
        if t >= self.time + Self::TIME {
            self.next_scene.take()
        } else {
            None
        }
    }
}

pub struct SharedState {
    pub t: f32,
    pub rt: f32,
    pub fader: Fader,
    pub charts_local: Vec<ChartItem>,
    pub charts_builtin: Vec<ChartItem>,

    pub icons: [SafeTexture; 8],
}

thread_local! {
    static FALLBACK: RefCell<Option<FontArc>> = RefCell::default();
    pub static BOLD_FONT_CKSUM: RefCell<Option<String>> = RefCell::default();
}

fn sha256(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}
fn load_font_with_cksum(data: Vec<u8>) -> Result<(FontArc, String)> {
    let cksum = sha256(&data);
    Ok((FontArc::try_from_vec(data)?, cksum))
}

fn set_bold_font((font, cksum): (FontArc, String)) {
    BOLD_FONT.with(move |it| *it.borrow_mut() = Some(TextPainter::new(font, FALLBACK.with(|it| it.borrow().clone()))));
    BOLD_FONT_CKSUM.with(move |it| *it.borrow_mut() = Some(cksum));
}

impl SharedState {
    pub async fn new(fallback: FontArc) -> Result<Self> {
        FALLBACK.with(|it| *it.borrow_mut() = Some(fallback));
        let path: PathBuf = dir::bold_font_path()?.into();
        let mut font = None;
        if path.exists() {
            font = std::fs::read(&path).ok().and_then(|it| load_font_with_cksum(it).ok());
        }
        let loaded = match font {
            Some(it) => it,
            None => load_font_with_cksum(load_file("fonts/bold.ttf").await?)?,
        };
        set_bold_font(loaded);
        let icons = Resource::load_icons().await?;
        // 多人房间页的「最好成绩」要用同一套等级图标（见 scene::TEX_RANK_ICONS）
        crate::scene::TEX_RANK_ICONS.with(|it| *it.borrow_mut() = Some(icons.clone()));
        Ok(Self {
            t: 0.,
            rt: 0.,
            fader: Fader::new(),
            charts_local: Vec::new(),
            charts_builtin: Vec::new(),

            icons,
        })
    }

    pub fn update(&mut self, tm: &mut TimeManager) {
        self.t = tm.now() as _;
        self.rt = tm.real_time() as _;
    }

    pub fn render_fader<R>(&mut self, ui: &mut Ui, f: impl FnOnce(&mut Ui) -> R) -> R {
        self.fader.render(ui, self.t, f)
    }

    pub fn reload_local_charts(&mut self) {
        self.charts_local = load_local();
    }
}

#[derive(Default)]
#[allow(dead_code)]
pub enum NextPage {
    #[default]
    None,
    Overlay(Box<dyn Page>),
    Pop,
}

pub trait Page {
    fn label(&self) -> Cow<'static, str>;

    /// 按具体类型访问页面（默认不可 downcast；插件页等需要被主场景按类型请求行为时覆盖）
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }

    fn can_play_bgm(&self) -> bool {
        true
    }
    fn on_result(&mut self, _result: Box<dyn Any>, _s: &mut SharedState) -> Result<()> {
        Ok(())
    }
    fn enter(&mut self, _s: &mut SharedState) -> Result<()> {
        Ok(())
    }
    fn update(&mut self, s: &mut SharedState) -> Result<()>;
    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool>;
    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()>;
    fn render_top(&mut self, _ui: &mut Ui, _s: &mut SharedState) -> Result<()> {
        Ok(())
    }
    /// 页面自己画左上角返回键时返回它的落点矩形（MainScene 据此设置点击区域，并不再画默认图标）
    fn back_button_rect(&self, _ui: &Ui) -> Option<Rect> {
        None
    }
    fn pause(&mut self) -> Result<()> {
        Ok(())
    }
    fn resume(&mut self) -> Result<()> {
        Ok(())
    }
    fn next_page(&mut self) -> NextPage {
        NextPage::None
    }
    /// 页面自己完成退出动画后请求 MainScene 弹栈。
    fn take_pop_request(&mut self) -> bool {
        false
    }
    fn next_scene(&mut self, _s: &mut SharedState) -> NextScene {
        NextScene::None
    }
    fn exit(&mut self) -> Result<()> {
        Ok(())
    }
    fn on_back_pressed(&mut self, _s: &mut SharedState) -> bool {
        false
    }
}
