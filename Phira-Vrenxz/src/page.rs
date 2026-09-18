pub mod coll;
pub use coll::CollectionPage;

mod event;
pub use event::EventPage;

mod achievement;
pub use achievement::AchievementPage;

mod extension;
pub use extension::ExtensionPage;

pub mod favorites;
pub use favorites::FavoritesPage;

mod home;
pub use home::HomePage;

mod library;
pub use library::{request_export, resolve_export, take_export, ExportInfo, LibraryPage, CHOOSE_COVER, CHOSEN_COVER, FAV_UPDATED};

mod message;
pub use message::MessagePage;

mod offset;
pub use offset::OffsetPage;

mod respack;
pub use respack::{ResPackItem, ResPackPage};

mod settings;
// 首启向导复用设置页的开关控件（同一份画法，样式才不会两边不一致）
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
use anyhow::Result;
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
    Ok(format!("{}/{}", dir::cache_image_local()?, path.replace('/', "_")).into())
}

pub fn illustration_task(notify: Arc<Notify>, path: String, full: bool) -> Task<Result<(DynamicImage, Option<DynamicImage>)>> {
    Task::new(async move {
        notify.notified().await;
        let mut fs = fs_from_path(&path)?;
        let info = fs::load_info(fs.deref_mut()).await?;
        let mut img = None;
        let thumbnail = Images::local_or_else(thumbnail_path(&path)?, async {
            let image = image::load_from_memory(&fs.load_file(&info.illustration).await?)?;
            let thumbnail = Images::thumbnail(&image);
            img = Some(image);
            Ok(thumbnail)
        })
        .await?;
        if full {
            if img.is_none() {
                img = Some(image::load_from_memory(&fs.load_file(&info.illustration).await?)?);
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

/// 内置谱面列表。
/// 内置谱面包不再解压：直接打开 `assets/Level.zip`（Android 为 `Expansion_package.zip`），
/// 枚举 `Level/<谱师目录>/Level.json` 里列出的谱面，谱面信息同样从 zip 内的 `.pez` 解析。
/// 压缩包缺失或损坏时返回空列表（并给出提示），不会 panic。
pub fn load_builtin_task() -> Task<Result<Vec<ChartItem>>> {
    Task::new(async move {
        let tex = BLACK_TEXTURE.clone();
        let mut charts = Vec::new();
        let mut zip = match open_builtin_level_zip() {
            Ok(it) => it,
            Err(err) => {
                warn!("failed to open builtin level package: {err:#}");
                // 压缩包存在却打不开（损坏）时给用户明确提示；只是缺失（开发环境）只记日志。
                if crate::scene::builtin_level_zip_present() {
                    show_error(err);
                }
                return Ok(charts);
            }
        };
        // 每个谱师一个目录；根目录下的散文件（如 提示.txt）不作为谱面
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
            // 解析谱师信息
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
            // 找到 level 字段
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
                // 压缩包里的压缩包（.pez / .zip 谱面）
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
                ui.text(format!("v{}", env!("CARGO_PKG_VERSION")))
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
        // 注入成就自定义图标（achievements_icon/1..20.png，按顺序对应成就；加载失败则回退 emoji）
        for order in 1..=20u8 {
            if let Ok(tex) = macroquad::texture::load_texture(&format!("achievements_icon/{order}.png")).await {
                crate::achievement::set_achievement_icon(order, tex.into());
            }
        }
        // 注入多成就汇总图标（加载失败则回退 emoji）
        if let Ok(tex) = macroquad::texture::load_texture("achievements_icon/Many_achievements.png").await {
            crate::achievement::set_many_icon(tex.into());
        }
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
    fn pause(&mut self) -> Result<()> {
        Ok(())
    }
    fn resume(&mut self) -> Result<()> {
        Ok(())
    }
    fn next_page(&mut self) -> NextPage {
        NextPage::None
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
