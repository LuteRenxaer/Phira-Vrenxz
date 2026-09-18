prpr_l10n::tl_file!("song");

#[cfg(feature = "video")]
use super::UnlockScene;
use super::{
    confirm_delete, confirm_dialog, fs_from_path, gen_custom_dir, import_chart_to, render_ldb, LdbDisplayItem, ProfileScene, ASSET_CHART_INFO,
};
use crate::{
    charts_view::NEED_UPDATE,
    client::{
        basic_client_builder, recv_raw, Chart, ChartRef, ChartRefChartInfo, Client, Collection, CollectionUpdate, Permissions, Ptr, Record, User,
        UserManager, CLIENT_TOKEN,
    },
    data::{BriefChartInfo, LocalChart},
    dir, get_data, get_data_mut,
    icons::Icons,
    page::{
        local_illustration, request_export, resolve_export, take_export, thumbnail_path, ChartItem, ChartType, Fader, Illustration, SFader,
        FAV_UPDATED,
    },
    popup::Popup,
    rate::RateDialog,
    save_data,
    tags::TagsDialog,
};
use ::rand::{thread_rng, Rng};
use anyhow::{bail, Context, Error, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use core::f32;
use futures_util::StreamExt;
use inputbox::{InputBox, InputMode};
use macroquad::prelude::*;
use once_cell::sync::Lazy;
use phira_mp_common::{ClientCommand, CompactPos, JudgeEvent, TouchFrame};
use prpr::{
    config::Mods,
    core::{Tweenable, BOLD_FONT},
    ext::{
        open_url, poll_future, rect_shadow, semi_black, semi_white, unzip_into, JoinToString, LocalTask, RectExt, SafeTexture, ScaleType,
        BLACK_TEXTURE,
    },
    fs::{self},
    info::ChartInfo,
    judge::{icon_index, Judge},
    scene::{
        request_file, request_input, return_file, return_input, show_error, show_message, take_file, take_input, BasicPlayer, GameMode, LoadingScene,
        LocalSceneTask, NextScene, RecordUpdateState, SaveFn, Scene, SimpleRecord, UpdateFn, UploadFn, FinishedStats,
    },
    task::Task,
    time::TimeManager,
    ui::{back_sound, button_hit, play_sound, render_chart_info, ChartInfoEdit, DRectButton, Dialog, LoadingParams, LongTouchState, RectButton, Scroll, Ui, UI_AUDIO},
};
use regex::Regex;
use reqwest::Method;
use sanitize_filename::sanitize;
use sasa::{AudioClip, Frame, Music, MusicParams};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    any::Any,
    borrow::Cow,
    collections::{hash_map, BTreeMap, HashMap, VecDeque},
    fs::File,
    io::{BufWriter, Cursor, Seek, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicI32, Ordering},
        mpsc, Arc, Mutex, Weak,
    },
    thread_local,
};
use tap::Tap;
use tokio::net::TcpStream;
use tracing::{error, warn};
use uuid::Uuid;
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

type LocalTuple = (String, ChartInfo, AudioClip, Illustration);

static CONFIRM_CKSUM: AtomicBool = AtomicBool::new(false);
static UPLOAD_NOT_SAVED: AtomicBool = AtomicBool::new(false);
static CONFIRM_OVERWRITE: AtomicBool = AtomicBool::new(false);
static CONFIRM_UPLOAD: AtomicBool = AtomicBool::new(false);
static CONFIRM_AUTOCOMPLETE: AtomicBool = AtomicBool::new(false);
static SKIP_AUTOCOMPLETE: AtomicBool = AtomicBool::new(false);
pub static RECORD_ID: AtomicI32 = AtomicI32::new(-1);

/// 最近一局“自然完成且成绩有效”的结算（由引擎 SaveFn 在谱面正常打完时写入，
/// 多人面板据此上报 client.played 而非误判 abort；单人游玩不会消费，保留不影响）。
pub static LAST_MP_FINISH: Mutex<Option<prpr::scene::FinishedStats>> = Mutex::new(None);

/// Matches any `@name#id (role)` or `@name#id` or `@name (role)` or `@name`.
/// Parentheses may be ASCII `()` or fullwidth `（）`; whitespace before `(` is optional.
/// Groups: 1=name, 2=id (optional), 3=role (optional)
/// 选曲页「开始」按钮里，图标边长占按钮高度的比例（比原版小一圈）。
const PLAY_ICON_RATIO: f32 = 0.38;

/// 选曲页这边白闪最多兜底多久（加载页迟迟不出现时别一直白着）；
/// 正常情况是加载页滑入完成后由它自己淡掉，见 [prpr::scene::LAUNCH_FLASH]。
const LAUNCH_FLASH_MAX: f32 = 1.5;

static MENTION_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"@([^\s#@(（]+)(?:#(\d+))?(?:\s*[（(]([^)）]+)[)）])?").unwrap());

/// Parse all `@name#id` resolved collaborator mentions and return `(id, role)` pairs.
fn parse_collaborators(intro: &str) -> BTreeMap<i32, Option<String>> {
    use std::collections::btree_map::Entry;

    let mut result = BTreeMap::new();
    for (id, role) in MENTION_RE.captures_iter(intro).filter_map(|cap| {
        let id: i32 = cap.get(2)?.as_str().parse().ok()?;
        let role = cap.get(3).map(|m| m.as_str().to_owned());
        Some((id, role))
    }) {
        match result.entry(id) {
            Entry::Vacant(e) => {
                e.insert(role);
            }
            Entry::Occupied(mut e) => {
                if e.get().is_none() && role.is_some() {
                    e.insert(role);
                }
            }
        }
    }
    result
}

/// Find all unresolved `@name` or `@name (role)` mentions (those missing `#id`).
/// Returns `(start, end, name)` byte-offset pairs into `intro`.
/// Processing right-to-left preserves earlier offsets during replacement.
fn find_unresolved_mentions(intro: &str) -> Vec<(usize, usize, String)> {
    MENTION_RE
        .captures_iter(intro)
        .filter_map(|cap| {
            if cap.get(2).is_some() {

                return None;
            }
            let m = cap.get(0)?;
            let name = cap.get(1)?.as_str().to_owned();
            Some((m.start(), m.end(), name))
        })
        .collect()
}

fn fade_in_time() -> Option<f32> {
    if get_data().prefer_reduced_motion {
        None
    } else {
        Some(0.3)
    }
}

fn edit_transit() -> Option<f32> {
    if get_data().prefer_reduced_motion {
        None
    } else {
        Some(0.32)
    }
}

fn create_music(clip: AudioClip) -> Result<Music> {
    let mut music = UI_AUDIO.with(|it| {
        it.borrow_mut().create_music(
            clip,
            MusicParams {
                amplifier: 0.7,
                loop_mix_time: 0.,
                ..Default::default()
            },
        )
    })?;
    music.play()?;
    Ok(music)
}

fn with_effects((mut frames, sample_rate): (Vec<Frame>, u32), range: Option<(f32, f32)>) -> Result<AudioClip> {
    if let Some((begin, end)) = range {
        frames.drain(((end * sample_rate as f32) as usize).min(frames.len())..);
        frames.drain(..((begin * sample_rate as f32) as usize).min(frames.len()));
    }
    let len = (0.8 * sample_rate as f64) as usize;
    let len = len.min(frames.len() / 2);
    for (i, frame) in frames[..len].iter_mut().enumerate() {
        let s = i as f32 / len as f32;
        frame.0 *= s;
        frame.1 *= s;
    }
    let st = frames.len() - len;
    for (i, frame) in frames[st..].iter_mut().rev().enumerate() {
        let s = i as f32 / len as f32;
        frame.0 *= s;
        frame.1 *= s;
    }
    Ok(AudioClip::from_raw(frames, sample_rate))
}

async fn load_local_tuple(local_path: &str, def_illu: SafeTexture, info: ChartInfo) -> Result<LocalTuple> {
    let dir = prpr::dir::Dir::new(format!("{}/{local_path}", dir::charts()?))?;
    let bytes = dir.read(&info.music)?;
    let (frames, sample_rate) = AudioClip::decode(bytes)?;
    let length = frames.len() as f32 / sample_rate as f32;
    if info.preview_end.unwrap_or(info.preview_start + 1.) > length {
        tl!(bail "edit-preview-invalid");
    }
    // 钳制预览范围到有效区间，防止越界
    let preview_begin = info.preview_start.clamp(0., length.max(0.));
    let preview_end = info.preview_end.unwrap_or(preview_begin + 15.).clamp(preview_begin, length.max(preview_begin));
    let preview = with_effects((frames, sample_rate), Some((preview_begin, preview_end)))?;
    let illu = local_illustration(local_path.to_owned(), def_illu, true);
    illu.notify.notify_one();

    Ok((local_path.to_owned(), info, preview, illu))
}

pub struct Downloading {
    info: BriefChartInfo,
    local_path: Option<String>,
    loading_last: f32,
    cancel_download_btn: DRectButton,
    status: Arc<Mutex<Cow<'static, str>>>,
    prog: Arc<Mutex<Option<f32>>>,
    atomicity: Arc<Mutex<()>>,
    task: Task<Result<(LocalChart, LocalTuple)>>,
}

impl Downloading {
    pub fn touch(&mut self, touch: &Touch, t: f32) -> bool {
        self.cancel_download_btn.touch(touch, t)
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32) {
        ui.fill_rect(ui.screen_rect(), semi_black(0.6));
        ui.loading(0., -0.06, t, WHITE, (*self.prog.lock().unwrap(), &mut self.loading_last));
        ui.text(self.status.lock().unwrap().clone())
            .pos(0., 0.02)
            .anchor(0.5, 0.)
            .size(0.6)
            .draw();
        let size = 0.7;
        let r = ui.text(tl!("dl-cancel")).pos(0., 0.12).anchor(0.5, 0.).size(size).measure().feather(0.02);
        self.cancel_download_btn.render_text(ui, r, t, tl!("dl-cancel"), 0.6, true);
    }

    /// 内联渲染（多人房间页的「谱面下载」状态行）：在给定矩形内画谱面名、状态文字、
    /// 进度条与「取消」按钮，**不铺满全屏、不加暗色遮罩**。
    ///
    /// 「取消」按钮的命中区就在本函数里登记（`cancel_download_btn`），因此
    /// [`Self::touch`] 与该按钮的显示天然同源。
    pub fn render_inline(&mut self, ui: &mut Ui, r: Rect, t: f32) {
        let accent = ui.accent();
        let status = self.status.lock().unwrap().clone();
        let text_w = (r.w - 0.42).max(0.1);
        ui.text(&self.info.name)
            .pos(r.x + 0.03, r.y + r.h * 0.28)
            .anchor(0., 0.5)
            .no_baseline()
            .max_width(text_w)
            .size(0.38)
            .color(semi_white(0.92))
            .draw();
        ui.text(status)
            .pos(r.x + 0.03, r.y + r.h * 0.6)
            .anchor(0., 0.5)
            .no_baseline()
            .max_width(text_w)
            .size(0.31)
            .color(semi_white(0.6))
            .draw();
        // 进度条（进度未知时画一条满宽的浅色底，靠状态文字表达“进行中”）
        let bar = Rect::new(r.x + 0.03, r.y + r.h * 0.82, text_w, 0.012);
        ui.fill_path(&bar.rounded(0.006), semi_black(0.35));
        if let Some(p) = *self.prog.lock().unwrap() {
            let w = bar.w * p.clamp(0., 1.);
            if w > 0.002 {
                let shading = Color { a: 0.85, ..accent };
                ui.fill_path(&Rect::new(bar.x, bar.y, w, bar.h).rounded(0.006), shading);
            }
        }
        let cr = Rect::new(r.right() - 0.36, r.y + r.h * 0.18, 0.32, r.h * 0.64);
        // 不带阴影：多人页的按钮一律是平底 + 描边（`build` 已经负责按压动画与命中区）
        self.cancel_download_btn.build(ui, t, cr, |ui, path| {
            ui.fill_path(&path, semi_black(0.4));
            ui.text(tl!("dl-cancel"))
                .pos(cr.center().x, cr.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.36)
                .color(semi_white(0.9))
                .max_width(cr.w - 0.02)
                .draw();
        });
    }

    pub fn check(&mut self) -> Result<Option<Option<LocalTuple>>> {
        if let Some(res) = self.task.take() {
            match res {
                Err(err) => {
                    let path = format!("{}/{}", dir::downloaded_charts()?, self.info.id.unwrap());
                    let path = Path::new(&path);
                    if path.exists() {
                        std::fs::remove_dir_all(path)?;
                    }
                    show_error(err.context(tl!("dl-failed")));
                    Ok(Some(None))
                }
                Ok((chart, tuple)) => {
                    self.info = chart.info.clone();
                    if let Some(local_path) = &self.local_path {

                        SongScene::global_update_chart_info(local_path, self.info.clone())?;
                    } else {
                        NEED_UPDATE.store(true, Ordering::Relaxed);
                        self.local_path = Some(chart.local_path.clone());
                        get_data_mut().charts.push(chart);
                    }
                    save_data()?;
                    show_message(tl!("dl-success")).ok();
                    Ok(Some(Some(tuple)))
                }
            }
        } else {
            Ok(None)
        }
    }
}

enum SideContent {
    Edit,
    Leaderboard,
    Info,
    Mods,
}

impl SideContent {
    fn width(&self) -> f32 {
        match self {
            Self::Edit => 0.9,
            Self::Leaderboard => 0.94,
            Self::Info => 0.75,
            Self::Mods => 0.8,
        }
    }
}

#[derive(Deserialize)]
struct StableR {
    status: i8,
}

#[derive(Deserialize)]
struct LdbItem {
    #[serde(flatten)]
    pub inner: Record,
    pub rank: u32,
    #[serde(skip, default)]
    pub btn: RectButton,
}

pub struct SongScene {
    illu: Illustration,

    first_in: bool,

    back_btn: RectButton,
    play_btn: DRectButton,

    icons: Arc<Icons>,

    next_scene: Option<NextScene>,

    preview: Option<Music>,
    preview_task: Option<Task<Result<AudioClip>>>,

    load_task: Option<Task<Result<Option<Arc<Chart>>>>>,
    entity: Option<Chart>,
    info: BriefChartInfo,
    local_path: Option<String>,

    downloading: Option<Downloading>,
    loading_last: f32,

    rank_icons: [SafeTexture; 8],
    record: Option<SimpleRecord>,

    fetch_best_task: Option<Task<Result<SimpleRecord>>>,

    menu: Popup,
    menu_btn: RectButton,
    need_show_menu: bool,
    should_delete: Arc<AtomicBool>,
    menu_options: Vec<&'static str>,

    info_edit: Option<ChartInfoEdit>,
    edit_btn: RectButton,
    edit_scroll: Scroll,

    mods: Mods,
    mod_btn: RectButton,
    mod_scroll: Scroll,
    mod_btns: Vec<(DRectButton, bool)>,

    side_content: SideContent,
    side_enter_time: f32,

    save_task: Option<Task<Result<LocalTuple>>>,
    upload_task: Option<Task<Result<BriefChartInfo>>>,

    ldb: Option<(Option<u32>, Vec<LdbItem>)>,
    ldb_task: Option<Task<Result<Vec<LdbItem>>>>,
    ldb_btn: RectButton,
    ldb_scroll: Scroll,
    ldb_fader: Fader,
    ldb_type_btn: DRectButton,
    ldb_std: bool,

    info_btn: RectButton,
    info_scroll: Scroll,

    fav_btn: RectButton,
    fav_long_touch: LongTouchState,
    fav_menu: Popup,
    fav_menu_options: Vec<Uuid>,
    need_show_fav_menu: bool,

    review_task: Option<Task<Result<String>>>,
    chart_should_delete: Arc<AtomicBool>,
    should_review_approve: Arc<AtomicBool>,

    edit_tags_task: Option<Task<Result<()>>>,
    tags: TagsDialog,

    rate_dialog: RateDialog,
    rate_task: Option<Task<Result<()>>>,

    should_update: Arc<AtomicBool>,

    my_rating_task: Option<Task<Result<i16>>>,
    my_rate_score: Option<i16>,

    stabilize_task: Option<Task<Result<()>>>,
    should_stabilize: Arc<AtomicBool>,
    should_stabilize_approve: Arc<AtomicBool>,
    should_stabilize_approve_ranked: Arc<AtomicBool>,

    scene_task: LocalTask<Result<NextScene>>,

    uploader_btn: RectButton,

    sf: SFader,
    fade_start: f32,

    background: Arc<Mutex<Option<SafeTexture>>>,
    tr_start: f32,

    open_web_btn: DRectButton,
    level_author_btn: DRectButton,


    overwrite_from: Option<String>,
    overwrite_task: Option<Task<Result<LocalTuple>>>,

    update_cksum_passed: Option<bool>,
    update_cksum_task: Option<Task<Result<bool>>>,
    chart_type: ChartType,
    level_author: Option<crate::page::LevelAuthor>,
    xcsim_preview_url: Option<String>,
    xcsim_illustration_url: Option<String>,

    is_fav: Option<bool>,
    toggle_fav_task: Option<Task<Result<(Collection, bool)>>>,

    confirm_cancel_edit: Arc<AtomicBool>,

    collaborators: BTreeMap<i32, Option<String>>,
    autocomplete_task: Option<Task<Result<String>>>,

    export_task: Option<mpsc::Receiver<Result<()>>>,
}

impl SongScene {
    pub fn new(mut chart: ChartItem, local_path: Option<String>, icons: Arc<Icons>, rank_icons: [SafeTexture; 8], mods: Mods) -> Self {
        let is_xcsim = chart.chart_type == ChartType::XCSim;
        if let Some(path) = &local_path {
            if let Some(id_str) = path.strip_prefix("download/") {
                let id_str = id_str.strip_prefix("xcsim_").unwrap_or(id_str);
                if let Ok(id) = id_str.parse::<i32>() {
                    chart.info.id = Some(id);
                }
            }
        }
        let illu = if let Some(path) = &chart.local_path {
            let illu = local_illustration(path.clone(), chart.illu.texture.1.clone(), true);
            illu.notify.notify_one();
            illu
        } else if let Some(id) = chart.info.id {
            if is_xcsim {
                if let Some(illu_url) = &chart.xcsim_illustration_url {
                    let illu_url = crate::xcsim::rehost_url(illu_url);
                    Illustration {
                        texture: chart.illu.texture.clone(),
                        notify: Arc::default(),
                        task: Some(Task::new({
                            let illu_url = illu_url.clone();
                            async move {
                                let bytes = reqwest::get(&illu_url).await?.bytes().await?;
                                let image = image::load_from_memory(&bytes)?;
                                Ok((image, None))
                            }
                        })),
                        loaded: Arc::default(),
                        load_time: f32::NAN,
                    }
                } else {
                    chart.illu
                }
            } else {
                Illustration {
                    texture: chart.illu.texture.clone(),
                    notify: Arc::default(),
                    task: Some(Task::new({
                        async move {
                            let chart = Ptr::<Chart>::new(id).load().await?;
                            let image = chart.illustration.load_image().await?;
                            Ok((image, None))
                        }
                    })),
                    loaded: Arc::default(),
                    load_time: f32::NAN,
                }
            }
        } else {
            chart.illu
        };
        let record = get_data()
            .charts
            .iter()
            .find(|it| Some(&it.local_path) == local_path.as_ref())
            .and_then(|it| it.record.clone())
            .or_else(|| local_path.as_ref().and_then(|path| get_data().local_records.get(path).cloned().flatten()));
        let fetch_best_task = if !is_xcsim && get_data().me.is_some() {
            chart.info.id.map(|id| Task::new(Client::best_record(id)))
        } else {
            None
        };
        let id = chart.info.id;
        let offline_mode = get_data().config.offline_mode;
        let icon_star = icons.star.clone();
        Self {
            illu,

            first_in: true,

            back_btn: RectButton::new(),
            play_btn: DRectButton::new(),

            icons,

            next_scene: None,

            preview: None,
            preview_task: Some(Task::new({
                let local_path = local_path.clone();
                let xcsim_preview_url = chart.xcsim_preview_url.clone();
                async move {
                    if let Some(path) = local_path {
                        let mut fs = fs_from_path(&path)?;
                        let info = fs::load_info(fs.as_mut()).await?;
                        with_effects(
                            AudioClip::decode(fs.load_file(&info.music).await?)?,
                            Some((info.preview_start, info.preview_end.unwrap_or(info.preview_start + 15.))),
                        )
                    } else if is_xcsim {
                        if let Some(preview_url) = xcsim_preview_url {
                            let preview_url = crate::xcsim::rehost_url(&preview_url);
                            let bytes = reqwest::get(&preview_url).await?.bytes().await?;
                            with_effects(AudioClip::decode(bytes.to_vec())?, None)
                        } else {
                            bail!(tl!("xcsim-no-preview").to_string());
                        }
                    } else {
                        let chart = Ptr::<Chart>::new(id.unwrap()).fetch().await?;
                        with_effects(AudioClip::decode(chart.preview.fetch().await?.to_vec())?, None)
                    }
                }
            })),

            load_task: if offline_mode || is_xcsim {
                None
            } else {
                id.map(|it| Task::new(async move { Ptr::new(it).fetch_opt().await }))
            },
            entity: None,
            info: chart.info,
            local_path,

            downloading: None,
            loading_last: 0.,

            rank_icons,
            record,

            fetch_best_task,

            menu: Popup::new(),
            menu_btn: RectButton::new(),
            need_show_menu: false,
            should_delete: Arc::new(AtomicBool::default()),
            menu_options: Vec::new(),

            info_edit: None,
            edit_btn: RectButton::new(),
            edit_scroll: Scroll::new(),

            mods,
            mod_btn: RectButton::new(),
            mod_scroll: Scroll::new(),
            mod_btns: Vec::new(),

            side_content: SideContent::Edit,
            side_enter_time: f32::INFINITY,

            save_task: None,
            upload_task: None,

            ldb: None,
            ldb_task: None,
            ldb_btn: RectButton::new(),
            ldb_scroll: Scroll::new(),
            ldb_fader: Fader::new().with_distance(0.12),
            ldb_type_btn: DRectButton::new(),
            ldb_std: false,

            info_btn: RectButton::new(),
            info_scroll: Scroll::new(),

            fav_btn: RectButton::new(),
            fav_long_touch: LongTouchState::default(),
            fav_menu: Popup::new().tap_mut(|it| it.set_auto_dismiss(false)),
            fav_menu_options: Vec::new(),
            need_show_fav_menu: false,

            review_task: None,
            chart_should_delete: Arc::default(),
            should_review_approve: Arc::default(),

            edit_tags_task: None,
            tags: TagsDialog::new(false),

            rate_dialog: RateDialog::new(icon_star, false),
            rate_task: None,

            should_update: Arc::default(),

            my_rating_task: if offline_mode || is_xcsim {
                None
            } else {
                id.map(|id| {
                    Task::new(async move {
                        #[derive(Deserialize)]
                        struct Resp {
                            score: i16,
                        }
                        let resp: Resp = recv_raw(Client::get(format!("/chart/{id}/rate"))).await?.json().await?;
                        Ok(resp.score)
                    })
                })
            },
            my_rate_score: None,

            stabilize_task: None,
            should_stabilize: Arc::default(),
            should_stabilize_approve: Arc::default(),
            should_stabilize_approve_ranked: Arc::default(),

            scene_task: None,

            uploader_btn: RectButton::new(),

            sf: SFader::new(),
            fade_start: 0.,

            tr_start: f32::NAN,
            background: Arc::default(),

            open_web_btn: DRectButton::new(),
            level_author_btn: DRectButton::new(),

            overwrite_from: None,
            overwrite_task: None,

            update_cksum_passed: None,
            update_cksum_task: None,
            chart_type: chart.chart_type,
            level_author: chart.level_author,
            xcsim_preview_url: chart.xcsim_preview_url,
            xcsim_illustration_url: chart.xcsim_illustration_url,

            is_fav: None,
            toggle_fav_task: None,

            confirm_cancel_edit: Arc::default(),

            collaborators: BTreeMap::new(),
            autocomplete_task: None,

            export_task: None,
        }
    }

    fn start_download(&mut self) -> Result<()> {
        let chart = self.info.clone();
        if self.chart_type == ChartType::XCSim {
            self.loading_last = 0.;
            self.downloading = Some(Self::global_start_download_xcsim(chart, self.local_path.clone())?);
            return Ok(());
        }
        let Some(entity) = self.entity.clone() else {
            show_message(tl!("still-loading")).error();
            return Ok(());
        };
        self.loading_last = 0.;
        self.downloading = Some(Self::global_start_download(chart, entity, self.local_path.clone())?);
        Ok(())
    }

    pub fn global_start_download_xcsim(chart: BriefChartInfo, local_path: Option<String>) -> Result<Downloading> {
        let progress = Arc::new(Mutex::new(None));
        let status = Arc::new(Mutex::new(Cow::Owned(tl!("xcsim-downloading").to_string())));
        let status_shared = Arc::clone(&status);
        let atomicity = Arc::new(Mutex::new(()));
        Ok(Downloading {
            info: chart.clone(),
            local_path,
            loading_last: 0.,
            cancel_download_btn: DRectButton::new(),
            prog: progress,
            status: status_shared,
            atomicity: atomicity.clone(),
            task: Task::new({
                let path = format!("{}/{}", dir::downloaded_charts()?, Uuid::new_v4());
                async move {
                    let path = std::path::Path::new(&path);
                    tokio::fs::create_dir(path).await?;

                    let id = chart.id.ok_or_else(|| anyhow::anyhow!(tl!("xcsim-missing-id").to_string()))?;
                    let access_token = crate::xcsim::account().access_token.clone();
                    *status.lock().unwrap() = Cow::Owned(tl!("xcsim-downloading-from").to_string());
                    crate::xcsim::download_chart(access_token.as_deref(), id, path).await?;

                    *status.lock().unwrap() = Cow::Owned(tl!("xcsim-saving").to_string());
                    let dir = prpr::dir::Dir::new(path)?;
                    let mut info: ChartInfo = serde_yaml::from_reader(dir.open("info.yml")?)?;
                    info.id = Some(id);
                    serde_yaml::to_writer(dir.create("info.yml")?, &info)?;

                    let local_path = format!("download/xcsim_{}", id);
                    let to_path = format!("{}/{local_path}", dir::charts()?);
                    let to_path = Path::new(&to_path);
                    {
                        let _guard = atomicity.lock().unwrap();
                        if to_path.exists() {
                            if to_path.is_file() {
                                std::fs::remove_file(to_path)?;
                            } else {
                                std::fs::remove_dir_all(to_path)?;
                            }
                        }
                        std::fs::rename(path, to_path)?;
                    }

                    let tuple = load_local_tuple(&local_path, BLACK_TEXTURE.clone(), info).await?;

                    Ok((
                        LocalChart {
                            info: chart,
                            local_path,
                            record: None,
                            mods: Mods::default(),
                            played_unlock: false,
                        },
                        tuple,
                    ))
                }
            }),
        })
    }

    pub fn global_start_download(chart: BriefChartInfo, entity: Chart, local_path: Option<String>) -> Result<Downloading> {
        let progress = Arc::new(Mutex::new(None));
        let prog_wk = Arc::downgrade(&progress);
        let status = Arc::new(Mutex::new(tl!("dl-status-fetch")));
        let status_shared = Arc::clone(&status);
        let atomicity = Arc::new(Mutex::new(()));
        Ok(Downloading {
            info: chart.clone(),
            local_path,
            loading_last: 0.,
            cancel_download_btn: DRectButton::new(),
            prog: progress,
            status: status_shared,
            atomicity: atomicity.clone(),
            task: Task::new({
                let path = format!("{}/{}", dir::downloaded_charts()?, Uuid::new_v4());
                async move {
                    let path = std::path::Path::new(&path);
                    tokio::fs::create_dir(path).await?;
                    let dir = prpr::dir::Dir::new(path)?;

                    let chart = chart;
                    async fn download(mut file: impl Write, url: &str, prog_wk: &Weak<Mutex<Option<f32>>>) -> Result<()> {
                        let Some(prog) = prog_wk.upgrade() else { return Ok(()) };
                        *prog.lock().unwrap() = None;
                        let req = basic_client_builder().build().unwrap().get(url);
                        let req = if let Some(token) = CLIENT_TOKEN.load().as_ref() {
                            req.header("Authorization", format!("Bearer {token}"))
                        } else {
                            req
                        };
                        let res = req.send().await.with_context(|| tl!("request-failed"))?.error_for_status()?;
                        let size = res.content_length();
                        let mut stream = res.bytes_stream();
                        let mut count = 0;
                        while let Some(chunk) = stream.next().await {
                            let chunk = chunk?;
                            file.write_all(&chunk)?;
                            count += chunk.len() as u64;
                            if let Some(size) = size {
                                *prog.lock().unwrap() = Some(count.min(size) as f32 / size as f32);
                            }
                            if prog_wk.strong_count() == 1 {

                                break;
                            }
                        }
                        Ok(())
                    }

                    *status.lock().unwrap() = tl!("dl-status-chart");
                    let mut bytes = Vec::new();
                    download(Cursor::new(&mut bytes), &entity.file.url, &prog_wk).await?;
                    *status.lock().unwrap() = tl!("dl-status-extract");
                    if prog_wk.strong_count() != 0 {
                        unzip_into(Cursor::new(bytes), &dir, false)?;
                    }
                    *status.lock().unwrap() = tl!("dl-status-saving");
                    if let Some(prog) = prog_wk.upgrade() {
                        *prog.lock().unwrap() = None;
                    }
                    let mut info: ChartInfo = serde_yaml::from_reader(dir.open("info.yml")?)?;
                    info.id = Some(entity.id);
                    info.created = Some(entity.created);
                    info.updated = Some(entity.updated);
                    info.chart_updated = Some(entity.chart_updated);
                    info.uploader = Some(entity.uploader.id);
                    serde_yaml::to_writer(dir.create("info.yml")?, &info)?;

                    if prog_wk.strong_count() == 0 {

                        drop(dir);
                        tokio::fs::remove_dir_all(&path).await?;
                    }

                    let local_path = format!("download/{}", chart.id.unwrap());
                    let to_path = format!("{}/{local_path}", dir::charts()?);
                    let to_path = Path::new(&to_path);
                    {
                        let _guard = atomicity.lock().unwrap();
                        if to_path.exists() {
                            if to_path.is_file() {
                                std::fs::remove_file(to_path)?;
                            } else {
                                std::fs::remove_dir_all(to_path)?;
                            }
                        }
                        std::fs::rename(path, to_path)?;
                    }

                    let tuple = load_local_tuple(&local_path, BLACK_TEXTURE.clone(), info).await?;

                    Ok((
                        LocalChart {
                            info: entity.to_info(),
                            local_path,
                            record: None,
                            mods: Mods::default(),
                            played_unlock: false,
                        },
                        tuple,
                    ))
                }
            }),
        })
    }

    fn load_ldb(&mut self) {
        if get_data().config.offline_mode || self.chart_type == ChartType::XCSim {
            return;
        }
        let Some(id) = self.info.id else { return };
        self.ldb = None;
        let std = self.ldb_std;
        self.ldb_task = Some(Task::new(async move {
            Ok(recv_raw(Client::get(format!("/record/list15/{id}")).query(&[("std", std)]))
                .await?
                .json()
                .await?)
        }));
    }

    fn update_record(&mut self, new_rec: SimpleRecord) -> Result<()> {
        let rec = get_data_mut()
            .charts
            .iter_mut()
            .find(|it| Some(&it.local_path) == self.local_path.as_ref())
            .map(|it| &mut it.record)
            .or_else(|| {
                self.local_path
                    .clone()
                    .map(|path| get_data_mut().local_records.entry(path).or_insert(None))
            });
        let Some(rec) = rec else {
            if let Some(rec) = &mut self.record {
                rec.update(&new_rec);
            } else {
                self.record = Some(new_rec);
            }
            return Ok(());
        };
        if let Some(rec) = rec {
            if rec.update(&new_rec) {
                save_data()?;
            }
        } else {
            *rec = Some(new_rec);
            save_data()?;
        }
        self.record = rec.clone();
        Ok(())
    }

    fn update_menu(&mut self) {
        self.menu_options.clear();
        if self.local_path.as_ref().is_some_and(|it| !it.starts_with(':')) {
            self.menu_options.push("delete");
        }
        if self.info.id.is_some() && self.chart_type != ChartType::XCSim {
            self.menu_options.push("rate");
        }
        if let Some(local_path) = &self.local_path {
            self.menu_options.push("exercise");
            self.menu_options.push("offset");
            if get_data()
                .charts
                .iter()
                .find(|it| it.local_path == *local_path)
                .is_some_and(|it| it.played_unlock)
            {
                self.menu_options.push("unlock");
            }
        }
        let perms = get_data().me.as_ref().map(|it| it.perms()).unwrap_or_default();
        let is_uploader = get_data()
            .me
            .as_ref()
            .is_some_and(|it| Some(it.id) == self.info.uploader.as_ref().map(|it| it.id));
        if self.info.id.is_some() && (perms.contains(Permissions::REVIEW) || perms.contains(Permissions::REVIEW_PECJAM)) {
            if self.entity.as_ref().is_some_and(|it| !it.reviewed && !it.stable_request) {
                self.menu_options.push("review-approve");
                self.menu_options.push("review-deny");
            }
            self.menu_options.push("review-edit-tags");
        }
        if self.info.id.is_some() && is_uploader && self.entity.as_ref().is_some_and(|it| !it.stable && !it.stable_request) {
            self.menu_options.push("stabilize");
        }
        if self.info.id.is_some() && self.entity.as_ref().is_some_and(|it| it.stable_request) && perms.contains(Permissions::STABILIZE_CHART) {
            self.menu_options.push("stabilize-approve");
            self.menu_options.push("stabilize-approve-ranked");
            self.menu_options.push("stabilize-comment");
            self.menu_options.push("stabilize-deny");
        }
        if self.info.id.is_some()
            && self.entity.as_ref().is_some_and(|it| {
                if it.stable {
                    perms.contains(Permissions::DELETE_STABLE)
                } else {
                    is_uploader || perms.contains(Permissions::DELETE_UNSTABLE)
                }
            })
        {
            self.menu_options.push("review-del");
        }
        if self.local_path.as_ref().is_some_and(|it| !it.starts_with(':')) {
            self.menu_options.push("export");
        }
        self.menu.set_options(self.menu_options.iter().map(|it| tl!(*it).into_owned()).collect());
    }

    fn launch(&mut self, mode: GameMode, force_unlock: bool) -> Result<()> {
        let local_path = self.local_path.as_ref().unwrap();
        let is_unlock = force_unlock
            || (mode == GameMode::Normal
                && get_data()
                    .charts
                    .iter()
                    .find(|it| it.local_path == *local_path)
                    .is_some_and(|it| it.info.has_unlock && !it.played_unlock));
        let is_xcsim = self.chart_type == ChartType::XCSim;

        self.scene_task =
            Self::global_launch(self.info.id, local_path, self.mods, mode, None, Some(self.background.clone()), self.record.clone(), is_unlock, is_xcsim)?;

        Ok(())
    }

    #[must_use = "futures do nothing unless you `.await` or poll them"]
    #[allow(clippy::too_many_arguments)]
    pub fn global_launch(
        id: Option<i32>,
        local_path: &str,
        mods: Mods,
        mode: GameMode,
        client: Option<Arc<phira_mp_client::Client>>,
        background_output: Option<Arc<Mutex<Option<SafeTexture>>>>,
        record: Option<SimpleRecord>,
        is_unlock: bool,
        is_xcsim: bool,
    ) -> Result<LocalSceneTask> {
        Self::global_launch_preview(
            id,
            local_path,
            mods,
            mode,
            client,
            background_output,
            record,
            is_unlock,
            is_xcsim,
            false,
            None,
        )
    }

    /// 与 [`SongScene::global_launch`] 相同的启动流程，但以“谱面预览”模式播放（autoplay 试听等）：
    /// `preview_mode` 下谱面自然播完不进入结算页（引擎层直接弹回）；`interrupt` 为外部写入的
    /// 打断信号（如多人模式房主点了开始），置位后预览立即结束。普通游玩请用
    /// [`SongScene::global_launch`]（等价于 `preview_mode = false, interrupt = None`）。
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    #[allow(clippy::too_many_arguments)]
    pub fn global_launch_preview(
        id: Option<i32>,
        local_path: &str,
        mods: Mods,
        mode: GameMode,
        client: Option<Arc<phira_mp_client::Client>>,
        background_output: Option<Arc<Mutex<Option<SafeTexture>>>>,
        record: Option<SimpleRecord>,
        is_unlock: bool,
        is_xcsim: bool,
        preview_mode: bool,
        interrupt: Option<Arc<AtomicBool>>,
    ) -> Result<LocalSceneTask> {
        let mut fs = fs_from_path(local_path)?;
        let can_rated = id.is_some() || local_path.starts_with(':');
        #[cfg(feature = "video")]
        let local_path = local_path.to_owned();
        // 全局禁用成绩上传
        let rated = false;
        if !rated && can_rated && mode == GameMode::Normal {
            show_message(tl!("warn-unrated")).warn();
        }
        let update_fn = client.and_then(|mut client| {
            let live = client.blocking_state().unwrap().live;
            let token = get_data().tokens.as_ref().map(|it| it.0.clone()).unwrap();
            let addr = get_data().config.mp_address.clone();
            let mut reconnect_task: Option<Task<Result<phira_mp_client::Client>>> = None;
            let update_fn: Option<UpdateFn> = if live {
                Some(Box::new({
                    let mut touch_ids: HashMap<u64, i8> = HashMap::new();
                    let mut touch_last_update: HashMap<i8, f32> = HashMap::new();
                    let mut touches: VecDeque<TouchFrame> = VecDeque::new();
                    let mut judges: VecDeque<JudgeEvent> = VecDeque::new();
                    let mut last_send_touch_time: f32 = 0.;
                    move |t, res, judge| {
                        if client.ping_fail_count() >= 1 && reconnect_task.is_none() {
                            warn!("lost connection, auto re-connect");
                            let token = token.clone();
                            let addr = addr.clone();
                            reconnect_task = Some(Task::new(async move {
                                let client = phira_mp_client::Client::new(TcpStream::connect(addr).await?).await?;
                                client.authenticate(token).await?;
                                Ok(client)
                            }));
                        }
                        if let Some(task) = &mut reconnect_task {
                            if let Some(res) = task.take() {
                                match res {
                                    Err(err) => {
                                        warn!(?err, "failed to reconnect");
                                    }
                                    Ok(new) => {
                                        warn!("reconnected!");
                                        client = new.into();
                                    }
                                }
                                reconnect_task = None;
                            }
                        }
                        let points: Vec<_> = Judge::get_touches()
                            .into_iter()
                            .filter_map(|it| {
                                if matches!(it.phase, TouchPhase::Stationary) {
                                    return None;
                                }
                                let len = touch_ids.len();
                                let mut id = match touch_ids.entry(it.id) {
                                    hash_map::Entry::Occupied(val) => *val.get(),
                                    hash_map::Entry::Vacant(place) => *place.insert(len.try_into().ok()?),
                                };
                                if matches!(it.phase, TouchPhase::Moved) && touch_last_update.get(&id).is_some_and(|it| *it as f64 + 1. / 20. >= t) {
                                    return None;
                                }
                                touch_last_update.insert(id, t as f32);
                                if matches!(it.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                                    touch_ids.remove(&it.id);
                                    id = !id;
                                }
                                Some((id, CompactPos::new(it.position.x, it.position.y * res.aspect_ratio)))
                            })
                            .collect();
                        if !points.is_empty() {
                            touches.push_back(TouchFrame { time: t as f32, points });
                        }
                        if last_send_touch_time as f64 + 1. < t || touches.len() > 20 {
                            if touches.is_empty() {
                                touches.push_back(TouchFrame {
                                    time: t as f32,
                                    points: Vec::new(),
                                });
                            }
                            let frames = Arc::new(touches.drain(..).collect());
                            client.blocking_send(ClientCommand::Touches { frames }).unwrap();
                            last_send_touch_time = t as f32;
                        }
                        judges.extend(judge.judgements.borrow_mut().drain(..).map(|it| JudgeEvent {
                            time: it.0 as f32,
                            line_id: it.1 as i32,
                            note_id: it.2 as i32,
                            judgement: {
                                use phira_mp_common::Judgement::*;
                                use prpr::judge::Judgement as OJ;
                                match it.3 {
                                    Ok(OJ::Perfect) => Perfect,
                                    Ok(OJ::Good) => Good,
                                    Ok(OJ::Bad) => Bad,
                                    Ok(OJ::Miss) => Miss,
                                    Err(true) => HoldPerfect,
                                    Err(false) => HoldGood,
                                }
                            },
                        }));
                        if judges.len() > 10 || judges.front().is_some_and(|it| it.time + 0.6 < t as f32) {
                            let judges = Arc::new(judges.drain(..).collect());
                            client.blocking_send(ClientCommand::Judges { judges }).unwrap();
                        }
                    }
                }))
            } else {
                None
            };
            update_fn
        });

        let save_fn: Option<SaveFn> = Some(Box::new({
            let local_path = local_path.to_string();
            move |stats: FinishedStats| -> Result<()> {
                // 自然完成且成绩有效（引擎仅在 record 有效时调用本回调）：
                // 记录本局成绩供多人模式上报读取（每局开局清空、取走后置空），
                // 使“正常打完”上报 client.played 而非被误判为 abort。
                *LAST_MP_FINISH.lock().unwrap() = Some(stats);
                let new_rec = SimpleRecord {
                    score: stats.score as i32,
                    accuracy: stats.accuracy,
                    full_combo: stats.full_combo,
                };
                let rec = get_data_mut()
                    .charts
                    .iter_mut()
                    .find(|it| it.local_path == local_path)
                    .map(|it| &mut it.record)
                    .or_else(|| Some(get_data_mut().local_records.entry(local_path.clone()).or_insert(None)))
                    .unwrap();
                if let Some(rec) = rec {
                    if rec.update(&new_rec) {
                        save_data()?;
                    }
                } else {
                    *rec = Some(new_rec);
                    save_data()?;
                }
                Ok(())
            }
        }));

        Ok(Some(Box::pin(async move {
            let mut info = fs::load_info(fs.as_mut()).await?;
            info.id = id;
            info.arcaea_judgement = is_xcsim;
            let mut config = get_data().config.clone();
            config.player_name = get_data()
                .me
                .as_ref()
                .map(|it| it.name.clone())
                .unwrap_or_else(|| tl!("guest").into_owned());
            config.res_pack_path = {
                let id = get_data().respack_id;
                if id == 0 {
                    None
                } else {
                    Some(format!("{}/{}", dir::respacks()?, get_data().respacks[id - 1]))
                }
            };
            let chart_updated = info.chart_updated;
            config.mods = mods;

            if let Some(output) = &background_output {
                let background = match async {
                    let image = image::load_from_memory(&fs.load_file(&info.illustration).await?)
                        .context("Failed to decode image")?;
                    let (w, h) = (image.width(), image.height());
                    let size = w as usize * h as usize;
                    let rgba = image.to_rgba8();
                    let mut blurred = Vec::with_capacity(size * 4);
                    for pixel in rgba.chunks_exact(4) {
                        blurred.extend_from_slice(&pixel[0..3]);
                        blurred.push(255);
                    }
                    let bg = Texture2D::from_image(&Image { width: w as _, height: h as _, bytes: blurred });
                    Ok::<_, anyhow::Error>(SafeTexture::from(bg))
                }.await {
                    Ok(bg) => Some(bg),
                    Err(err) => {
                        warn!("Failed to load background for output: {:?}", err);
                        None
                    }
                };
                *output.lock().unwrap() = background;
            }

            let player = get_data().me.as_ref().map(|it| BasicPlayer {
                avatar: UserManager::get_avatar(it.id).flatten(),
                id: it.id,
                rks: it.rks,
                historic_best: record.map_or(0, |it| it.score as u32),
            });
            let upload_fn: Option<UploadFn> = None;

            if is_unlock {
                #[cfg(not(feature = "video"))]
                {
                    warn!("this build does not support unlock video.");
                    LoadingScene::new_preview(
                        mode,
                        info,
                        config,
                        fs,
                        player,
                        None,
                        upload_fn,
                        update_fn,
                        save_fn,
                        None,
                        preview_mode,
                        interrupt,
                    )
                    .await
                    .map(|it| NextScene::Overlay(Box::new(it)))
                }
                #[cfg(feature = "video")]
                {
                    let chart = get_data_mut().charts.iter_mut().find(|it| it.local_path == local_path).unwrap();
                    if !chart.played_unlock {
                        chart.played_unlock = true;
                        save_data()?;
                    }

                    UnlockScene::new(
                        mode,
                        info,
                        config,
                        fs,
                        player,
                        upload_fn,
                        update_fn,
                        save_fn,
                    )
                    .await
                    .map(|it| NextScene::Overlay(Box::new(it)))
                }
            } else {

                LoadingScene::new_preview(
                    mode,
                    info,
                    config,
                    fs,
                    player,
                    None,
                    upload_fn,
                    update_fn,
                    save_fn,
                    None,
                    preview_mode,
                    interrupt,
                )
                .await
                .map(|it| NextScene::Overlay(Box::new(it)))
            }
        })))
    }

    fn is_owner(&self) -> bool {
        self.info.id.is_none()
            || (self.info.created.is_some() && self.info.uploader.as_ref().map(|it| it.id) == get_data().me.as_ref().map(|it| it.id))
    }

    fn hide_side(&mut self, rt: f32) {
        self.side_enter_time = -rt;
    }

    fn side_chart_info(&mut self, ui: &mut Ui, rt: f32) -> Result<()> {
        let h = 0.11;
        let pad = 0.03;
        let width = self.side_content.width() - pad;

        let is_owner = self.is_owner();
        let online = self.info.id.is_some();
        let vpad = 0.02;
        let hpad = 0.01;
        let dx = width / if is_owner { 3. } else { 2. };
        let mut r = Rect::new(hpad, ui.top * 2. - h + vpad, dx - hpad * 2., h - vpad * 2.);
        if ui.button("cancel", r, tl!("edit-cancel")) {
            if self.info_edit.as_ref().is_some_and(|it| it.updated) {
                confirm_dialog(tl!("warn"), tl!("cancel-not-saved"), self.confirm_cancel_edit.clone());
            } else {
                self.hide_side(rt);
            }
        }
        if is_owner {
            r.x += dx;
            if ui.button(
                "upload",
                r,
                if self.info.id.is_none() {
                    tl!("edit-upload")
                } else {
                    tl!("edit-update")
                },
            ) {
                if self.info_edit.as_ref().unwrap().updated && !UPLOAD_NOT_SAVED.load(Ordering::SeqCst) {
                    Dialog::simple(tl!("upload-not-saved"))
                        .buttons(vec![ttl!("cancel").into_owned(), ttl!("confirm").into_owned()])
                        .listener(|_dialog, pos| {
                            if pos == 1 {
                                UPLOAD_NOT_SAVED.store(true, Ordering::SeqCst);
                            }
                            false
                        })
                        .show();
                } else {
                    let path = self.local_path.as_ref().unwrap();
                    if get_data().me.is_none() {
                        show_message(tl!("upload-login-first"));
                    } else if path.starts_with(':') {
                        show_message(tl!("upload-builtin"));
                    } else {
                        self.update_cksum_passed = None;
                        Dialog::plain(tl!("upload-rules"), tl!("upload-rules-content"))
                            .buttons(vec![ttl!("cancel").into_owned(), ttl!("confirm").into_owned()])
                            .listener(|_dialog, pos| {
                                if pos == 1 {
                                    CONFIRM_UPLOAD.store(true, Ordering::SeqCst);
                                }
                                pos == -2
                            })
                            .show();
                    }
                }
            }
        }
        r.x += dx;
        if ui.button("save", r, tl!("edit-save")) {
            self.try_save_with_autocomplete();
        }

        ui.ensure_touches()
            .retain(|it| !matches!(it.phase, TouchPhase::Started) || self.edit_scroll.contains(it));

        self.edit_scroll.size((width, ui.top * 2. - h));
        self.edit_scroll.render(ui, |ui| {
            let (w, mut h) = render_chart_info(ui, self.info_edit.as_mut().unwrap(), width);
            h += 0.06;
            ui.dy(h);
            let mut r = Rect::new(0.04, 0., 0.23, 0.07);
            if ui.button("edit_tags", r, tl!("edit-tags")) {
                self.tags.set(self.info_edit.as_ref().unwrap().info.tags.clone());
                self.tags.enter(rt);
            }
            if is_owner && online {
                r.x += r.w + 0.01;
                if ui.button("overwrite", r, tl!("edit-overwrite")) {
                    request_file("overwrite");
                }
            }
            (w, h + 0.1)
        });
        Ok(())
    }

    fn side_ldb(&mut self, ui: &mut Ui, rt: f32) {
        let pad = 0.03;
        let width = self.side_content.width() - pad;
        ui.dy(0.03);
        self.ldb_type_btn.render_text(
            ui,
            Rect::new(width - 0.24, 0.01, 0.23, 0.08),
            rt,
            if self.ldb_std { tl!("ldb-std") } else { tl!("ldb-score") },
            0.6,
            true,
        );
        render_ldb(
            ui,
            &tl!("ldb"),
            self.side_content.width(),
            rt,
            &mut self.ldb_scroll,
            &mut self.ldb_fader,
            &self.icons.user,
            self.ldb.as_mut().map(|it| {
                it.1.iter_mut().map(|it| LdbDisplayItem {
                    player_id: it.inner.player.id,
                    rank: it.rank,
                    score: if self.ldb_std {
                        format!("{:07}", it.inner.std_score.unwrap_or(0.) as i64)
                    } else if self.chart_type == ChartType::XCSim {
                        format!("{:08}", it.inner.score)
                    } else {
                        format!("{:07}", it.inner.score)
                    },
                    alt: Some(if self.ldb_std {
                        format!("{}ms", (it.inner.std.unwrap_or(0.) * 1000.) as i32)
                    } else {
                        format!("{:.2}%", it.inner.accuracy * 100.)
                    }),
                    btn: &mut it.btn,
                })
            }),
        );
    }

    fn side_info(&mut self, ui: &mut Ui, rt: f32) {
        let pad = 0.03;
        ui.dx(pad);
        ui.dy(0.03);
        let width = self.side_content.width() - pad;
        self.info_scroll.size((width - pad, ui.top * 2. - 0.06));
        self.info_scroll.render(ui, |ui| {
            let mut h = 0.;
            macro_rules! dy {
                ($e:expr) => {{
                    let dy = $e;
                    h += dy;
                    ui.dy(dy);
                }};
            }
            let mw = width - pad * 3.;
            if self.info.id.is_some() && self.chart_type != ChartType::XCSim {
                let r = Rect::new(0.03, 0., mw, 0.12).nonuniform_feather(-0.03, -0.01);
                self.open_web_btn.render_text(ui, r, rt, ttl!("open-in-web"), 0.6, true);
                dy!(r.h + 0.04);
            }
            if let Some(uploader) = &self.info.uploader {
                let c = 0.06;
                let s = 0.05;
                let r = ui.avatar(c, c, s, rt, UserManager::opt_avatar(uploader.id, &self.icons.user));
                self.uploader_btn.set(ui, Rect::new(c - s, c - s, s * 2., s * 2.));
                if let Some((name, color)) = UserManager::name_and_color(uploader.id) {
                    ui.text(name)
                        .pos(r.right() + 0.02, r.center().y)
                        .anchor(0., 0.5)
                        .no_baseline()
                        .max_width(width - 0.15)
                        .size(0.6)
                        .color(color)
                        .draw();
                }
                dy!(0.14);
            }
            if !self.collaborators.is_empty() {
                dy!(ui.text(tl!("info-collaborators")).size(0.4).color(semi_white(0.7)).draw().h + 0.02);
                for (collab_id, role) in &self.collaborators {
                    let c = 0.06;
                    let s = 0.05;
                    let r = ui.avatar(c, c, s, rt, UserManager::opt_avatar(*collab_id, &self.icons.user));
                    if let Some((name, color)) = UserManager::name_and_color(*collab_id) {
                        let name_r = ui
                            .text(name)
                            .pos(r.right() + 0.02, r.center().y - if role.is_some() { 0.01 } else { 0. })
                            .anchor(0., 0.5)
                            .no_baseline()
                            .max_width(width - 0.15)
                            .size(0.5)
                            .color(color)
                            .draw();
                        if let Some(role_text) = role {
                            ui.text(role_text.as_str())
                                .pos(r.right() + 0.02, name_r.bottom() + 0.005)
                                .size(0.35)
                                .color(semi_white(0.6))
                                .draw();
                        }
                    }
                    dy!(0.14);
                }
            }

            if let Some(author) = &self.level_author {
                dy!(ui.text(tl!("info-charter")).size(0.4).color(semi_white(0.7)).draw().h + 0.02);
                // 谱师名字
                dy!(ui.text(&author.name).pos(pad, 0.).size(0.7).color(WHITE).draw().h + 0.01);
                // 平台
                dy!(ui.text(tl!("author-platform", "platform" => author.terrace.as_str())).pos(pad, 0.).size(0.45).color(semi_white(0.7)).draw().h + 0.01);
                // 链接按钮
                let link_r = Rect::new(pad, 0., mw, 0.08);
                self.level_author_btn.render_text(ui, link_r, rt, tl!("author-view-profile"), 0.5, true);
                dy!(link_r.h + 0.03);
            }

            let mut item = |title: Cow<'_, str>, content: Cow<'_, str>| {
                dy!(ui.text(title).size(0.4).color(semi_white(0.7)).draw().h + 0.02);
                dy!(ui.text(content).pos(pad, 0.).size(0.6).multiline().max_width(mw).draw().h + 0.03);
            };
            item(tl!("info-name"), self.info.name.as_str().into());
            item(tl!("info-composer"), self.info.composer.as_str().into());
            item(tl!("info-charter"), self.info.charter.as_str().into());
            item(tl!("info-difficulty"), format!("{} ({:.1})", self.info.level, self.info.difficulty).into());
            item(tl!("info-desc"), self.info.intro.as_str().into());
            if let Some(entity) = &self.entity {
                item(tl!("info-rating"), entity.rating.map_or(Cow::Borrowed("NaN"), |r| format!("{:.2} / 5.00", r * 5.).into()));
                item(
                    tl!("info-type"),
                    format!(
                        "{}{}",
                        if entity.reviewed { tl!("reviewed") } else { tl!("unreviewed") },
                        match (entity.stable, entity.ranked) {
                            (true, true) => ttl!("chart-ranked"),
                            (true, false) => ttl!("chart-special"),
                            (false, _) => ttl!("chart-unstable"),
                        }
                    )
                    .into(),
                );
                item(tl!("info-tags"), entity.tags.iter().map(|it| format!("#{it}")).join(" ").into());
            }
            if let Some(id) = self.info.id {
                item("ID".into(), id.to_string().into());
            }
            (width, h)
        });
    }

    fn side_mods(&mut self, ui: &mut Ui, rt: f32) {
        let pad = 0.03;
        ui.dx(pad);
        ui.dy(0.03);
        let width = self.side_content.width() - pad;
        self.mod_scroll.size((width - pad, ui.top * 2. - 0.06));
        self.mod_scroll.render(ui, |ui| {
            const ITEM_HEIGHT: f32 = 0.15;
            let mut h = 0.;
            macro_rules! dy {
                ($e:expr) => {{
                    let dy = $e;
                    h += dy;
                    ui.dy(dy);
                }};
            }
            let title_r = ui.text(tl!("mods")).size(0.9).draw_using(&BOLD_FONT);
            dy!(title_r.h + 0.02);
            ui.fill_rect(Rect::new(0., title_r.h + 0.01, width * 0.3, 0.003), semi_white(0.3));
            dy!(0.01);
            let rh = ITEM_HEIGHT * 3. / 5.;
            let rr = Rect::new(width - 0.24, (ITEM_HEIGHT - rh) / 2., 0.2, rh);
            let mut index = 0;
            let mut item = |title: Cow<'_, str>, subtitle: Option<Cow<'_, str>>, flag: Mods| {
                const TITLE_SIZE: f32 = 0.6;
                const SUBTITLE_SIZE: f32 = 0.35;
                const LEFT: f32 = 0.03;
                const PAD: f32 = 0.01;
                const SUB_MAX_WIDTH: f32 = 0.46;
                if let Some(subtitle) = subtitle {
                    let r1 = ui.text(Cow::clone(&title)).size(TITLE_SIZE).measure();
                    let r2 = ui
                        .text(Cow::clone(&subtitle))
                        .size(SUBTITLE_SIZE)
                        .max_width(SUB_MAX_WIDTH)
                        .no_baseline()
                        .measure();
                    let h = r1.h + PAD + r2.h;
                    ui.text(subtitle)
                        .pos(LEFT, (ITEM_HEIGHT + h) / 2. - r2.h)
                        .size(SUBTITLE_SIZE)
                        .max_width(SUB_MAX_WIDTH)
                        .multiline()
                        .color(semi_white(0.6))
                        .draw();
                    ui.text(title).pos(LEFT, (ITEM_HEIGHT - h) / 2.).no_baseline().size(TITLE_SIZE).draw();
                } else {
                    ui.text(title)
                        .pos(LEFT, ITEM_HEIGHT / 2.)
                        .anchor(0., 0.5)
                        .no_baseline()
                        .size(TITLE_SIZE)
                        .draw();
                }
                if self.mod_btns.len() <= index {
                    self.mod_btns.push(Default::default());
                }
                let (btn, clicked) = &mut self.mod_btns[index];
                if *clicked {
                    *clicked = false;
                    self.mods.toggle_mod(flag);
                }
                let on = self.mods.contains(flag);
                let oh = rr.h;
                btn.build(ui, rt, rr, |ui, path| {
                    let ct = rr.center();
                    ui.fill_path(&path, if on { WHITE } else { ui.background() });
                    ui.text(if on { ttl!("switch-on") } else { ttl!("switch-off") })
                        .pos(ct.x, ct.y)
                        .anchor(0.5, 0.5)
                        .no_baseline()
                        .size(0.5 * (1. - (1. - rr.h / oh).powf(1.3)))
                        .max_width(rr.w)
                        .color(if on { Color::new(0.3, 0.3, 0.3, 1.) } else { WHITE })
                        .draw();
                });
                dy!(ITEM_HEIGHT);
                index += 1;
            };
            item(tl!("mods-autoplay"), Some(tl!("mods-autoplay-sub")), Mods::AUTOPLAY);
            item(tl!("mods-flip-x"), Some(tl!("mods-flip-x-sub")), Mods::FLIP_X);
            item(tl!("mods-flip-y"), Some(tl!("mods-flip-y-sub")), Mods::FLIP_Y);
            item(tl!("mods-fade-in"), Some(tl!("mods-fade-in-sub")), Mods::FADE_IN);
            item(tl!("mods-fade-out"), Some(tl!("mods-fade-out-sub")), Mods::FADE_OUT);
            item(tl!("mods-nightcore"), Some(tl!("mods-nightcore-sub")), Mods::NIGHTCORE);
            item(tl!("mods-rainbow"), Some(tl!("mods-rainbow-sub")), Mods::RAINBOW);
            item(tl!("mods-instant-death-ap"), Some(tl!("mods-instant-death-ap-sub")), Mods::INSTANT_DEATH_AP);
            item(tl!("mods-instant-death-fc"), Some(tl!("mods-instant-death-fc-sub")), Mods::INSTANT_DEATH_FC);
            item(tl!("mods-no-shader"), Some(tl!("mods-no-shader-sub")), Mods::NO_SHADER);

            // 花样 mod
            item(tl!("mods-ghost"), Some(tl!("mods-ghost-sub")), Mods::GHOST);
            item(tl!("mods-random-x"), Some(tl!("mods-random-x-sub")), Mods::RANDOM_X);
            item(tl!("mods-fx-tv"), Some(tl!("mods-fx-tv-sub")), Mods::FX_TV);
            item(tl!("mods-fx-scanline"), Some(tl!("mods-fx-scanline-sub")), Mods::FX_SCANLINE);
            item(tl!("mods-fx-glitch"), Some(tl!("mods-fx-glitch-sub")), Mods::FX_GLITCH);

            (width, h + 0.2)
        });
    }

    fn save_edit(&mut self) {
        let Some(edit) = &self.info_edit else { unreachable!() };
        let info = edit.info.clone();


        {
            let mut texts = vec![
                info.name.as_str(),
                info.level.as_str(),
                info.charter.as_str(),
                info.composer.as_str(),
                info.illustrator.as_str(),
                info.intro.as_str(),
            ];
            if let Some(tip) = &info.tip {
                texts.push(tip.as_str());
            }
            texts.extend(info.tags.iter().map(String::as_str));
            if let Err(err) = crate::censor::check_texts(texts) {
                show_message(err.to_string()).error();
                return;
            }
        }
        let path = self.local_path.clone().unwrap();
        let edit = edit.clone();
        let is_owner = self.is_owner();
        let def_illu = self.illu.texture.1.clone();
        self.save_task = Some(Task::new(async move {
            let dir = prpr::dir::Dir::new(format!("{}/{path}", dir::charts()?))?;
            let patches = edit.to_patches().await.with_context(|| tl!("edit-load-file-failed"))?;
            if !is_owner && patches.contains_key(&info.chart) {
                bail!(tl!("edit-downloaded"));
            }
            for (name, bytes) in patches.into_iter() {
                dir.create(name)?.write_all(&bytes)?;
            }
            let _ = std::fs::remove_file(thumbnail_path(&path)?);
            load_local_tuple(&path, def_illu, info).await
        }));
    }

    fn try_save_with_autocomplete(&mut self) {
        let intro = &self.info_edit.as_ref().unwrap().info.intro;
        let unresolved = find_unresolved_mentions(intro);
        if unresolved.is_empty() {
            self.save_edit();
        } else {
            let mentions_list = unresolved
                .iter()
                .map(|(start, end, _)| &intro[*start..*end])
                .collect::<Vec<_>>()
                .join(", ");
            let content = tl!("collab-autocomplete-content", "mentions" => mentions_list);
            Dialog::plain(tl!("collab-autocomplete-title"), content)
                .buttons(vec![ttl!("cancel").into_owned(), ttl!("confirm").into_owned()])
                .listener(|_dialog, pos| {
                    if pos == 1 {
                        CONFIRM_AUTOCOMPLETE.store(true, Ordering::SeqCst);
                    } else if pos == 0 {
                        SKIP_AUTOCOMPLETE.store(true, Ordering::SeqCst);
                    }
                    false
                })
                .show();
        }
    }

    fn start_autocomplete(&mut self) {
        let intro = self.info_edit.as_ref().unwrap().info.intro.clone();
        self.autocomplete_task = Some(Task::new(async move {
            let unresolved = find_unresolved_mentions(&intro);


            let mut resolved: Vec<(usize, usize, String)> = Vec::new();
            for (start, end, name) in unresolved {
                let name_owned = name.clone();
                let (users, _) = Client::query::<User>().search(name_owned).send().await?;
                let matched = users.into_iter().find(|u| u.name == name);
                let Some(user) = matched else {
                    bail!(tl!("collab-autocomplete-failed", "name" => name));
                };


                let suffix = &intro[start + 1 + name.len()..end];
                let new_text = format!("@{}#{}{}", name, user.id, suffix);
                resolved.push((start, end, new_text));
            }

            let mut result = intro.into_bytes();
            for (start, end, new_text) in resolved.into_iter().rev() {
                result.splice(start..end, new_text.into_bytes());
            }
            Ok(String::from_utf8(result).unwrap())
        }));
    }

    fn update_chart_info(&self) -> Result<()> {
        Self::global_update_chart_info(self.local_path.as_ref().unwrap(), self.info.clone())
    }

    fn global_update_chart_info(local_path: &str, info: BriefChartInfo) -> Result<()> {
        let _ = std::fs::remove_file(thumbnail_path(local_path)?);
        get_data_mut().charts[get_data().find_chart_by_path(local_path).unwrap()].info = info;
        NEED_UPDATE.store(true, Ordering::Relaxed);
        save_data()?;
        Ok(())
    }

    fn load_tuple(&mut self, (local_path, info, preview, illu): LocalTuple) -> Result<()> {
        self.local_path = Some(local_path);
        if let Some(preview) = &mut self.preview {
            preview.pause()?;
        }
        self.preview = Some(create_music(preview)?);
        self.info = info.into();
        self.illu = illu;
        self.update_chart_info()?;

        Ok(())
    }

    fn to_bare_chart_ref(&self) -> ChartRef {
        ChartRef::new_bare(self.info.id, self.local_path.as_deref())
    }

    fn toggle_in(&mut self, uuid: Uuid) {
        let data = get_data();
        let col = data.collection_info(&uuid).as_ref().clone();
        let mut chart_ref = self.to_bare_chart_ref();
        if self.info.id.is_some() {
            let Some(entity) = self.entity.clone() else {
                show_message(tl!("still-loading")).error();
                return;
            };
            chart_ref.info = Some(Box::new(ChartRefChartInfo::from_chart(&entity)));
        }
        let add = col.charts.iter().all(|it| it != &chart_ref);
        match col.update(uuid, &[chart_ref], add) {
            CollectionUpdate::Unchanged => {}
            CollectionUpdate::Updated { sync_task, add } => {
                if let Some(task) = sync_task {
                    self.toggle_fav_task = Some(task);
                } else {
                    self.is_fav = None;
                    FAV_UPDATED.store(true, Ordering::SeqCst);
                    if add {
                        show_message(tl!("fav-added")).duration(1.5).ok();
                    }
                }
            }
        }
    }

    fn get_fav_menu_options(&mut self) -> Vec<String> {
        let data = get_data();
        let mut options = Vec::new();
        self.fav_menu_options.clear();
        let chart_ref = self.to_bare_chart_ref();
        for uuid in data.collection_uuids() {
            let col = data.collection_info(uuid);
            if !col.is_owned() {
                continue;
            }
            self.fav_menu_options.push(*uuid);
            let contains = col.charts.iter().any(|it| it == &chart_ref);
            options.push(format!("{} {}", if contains { '\u{2713}' } else { ' ' }, col.name));
        }
        options
    }
}

impl Scene for SongScene {
    fn on_result(&mut self, tm: &mut TimeManager, res: Box<dyn Any>) -> Result<()> {
        let res = match res.downcast::<SimpleRecord>() {
            Err(res) => res,
            Ok(rec) => {
                self.fade_start = tm.now() as f32 + fade_in_time().unwrap_or_default();
                if self.my_rate_score == Some(0) && thread_rng().gen_ratio(2, 5) {
                    self.rate_dialog.enter(tm.real_time() as _);
                }
                // 提前取出成就所需数据，避免 rec 被移动
                let (ach_score, ach_accuracy, ach_full_combo) = (
                    rec.score.max(0) as u64,
                    rec.accuracy,
                    rec.full_combo,
                );
                if let Some(record) = &mut self.record {
                    record.update(&rec);
                } else {
                    self.record = Some(*rec);
                }
                self.load_ldb();
                // 记录成就进度（游玩次数、分数、准确率、难度、全连）
                // 反作弊：联网谱面传入服务端权威元数据，本地被篡改的难度/标签不参与判定
                let authority = self.entity.as_ref().map(|e| (e.difficulty, e.level.clone()));
                if let Err(err) = crate::achievement::record_play(
                    ach_score,
                    &self.info.level,
                    self.info.difficulty,
                    ach_accuracy,
                    ach_full_combo,
                    authority,
                ) {
                    warn!(?err, "记录成就进度失败");
                }
                return Ok(());
            }
        };
        let res = match res.downcast::<anyhow::Error>() {
            Ok(error) => {
                show_error(error.context(tl!("load-chart-failed")));
                return Ok(());
            }
            Err(res) => res,
        };
        let _res = match res.downcast::<Option<f32>>() {
            Ok(offset) => {
                if let Some(offset) = *offset {
                    let dir = format!("{}/{}", dir::charts()?, self.local_path.as_ref().unwrap().replace(':', "_"));
                    let path = std::path::Path::new(&dir);
                    if !path.exists() {
                        std::fs::create_dir_all(path)?;
                    }
                    let dir = prpr::dir::Dir::new(dir)?;
                    match self.chart_type {
                        ChartType::Integrated => {
                            dir.create("offset")?.write_all(&offset.to_be_bytes())?;
                            if let Ok(Some(info)) = ASSET_CHART_INFO.lock().as_deref_mut() {
                                info.offset = offset;
                            }
                        }
                        _ => {
                            let mut info: ChartInfo = serde_yaml::from_reader(&dir.open("info.yml")?)?;
                            info.offset = offset;
                            dir.create("info.yml")?.write_all(serde_yaml::to_string(&info)?.as_bytes())?;
                            let path = thumbnail_path(self.local_path.as_ref().unwrap())?;
                            if path.exists() {
                                std::fs::remove_file(path)?;
                            }
                        }
                    }
                    show_message(tl!("edit-saved")).ok();
                }
                return Ok(());
            }
            Err(res) => res,
        };
        Ok(())
    }

    fn pause(&mut self, _tm: &mut TimeManager) -> Result<()> {
        if let Some(preview) = &mut self.preview {
            preview.pause()?;
        }
        Ok(())
    }

    fn resume(&mut self, _tm: &mut TimeManager) -> Result<()> {
        if let Some(preview) = &mut self.preview {
            preview.play()?;
        }
        Ok(())
    }

    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.first_in {
            self.first_in = false;
            tm.seek_to(-fade_in_time().unwrap_or_default() as _);
            self.load_ldb();
        }
        if let Some(music) = &mut self.preview {
            music.seek_to(0.)?;
            music.play()?;
        }
        self.update_menu();
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        let t = tm.now() as f32;
        if self.scene_task.is_some()
            || self.save_task.is_some()
            || self.upload_task.is_some()
            || self.review_task.is_some()
            || self.edit_tags_task.is_some()
            || self.rate_task.is_some()
            || self.overwrite_task.is_some()
            || self.update_cksum_task.is_some()
            || self.toggle_fav_task.is_some()
            || self.export_task.is_some()
            || self.autocomplete_task.is_some()
        {
            return Ok(true);
        }
        if let Some(dl) = &mut self.downloading {
            if dl.touch(touch, t) {
                let atomicity = dl.atomicity.clone();
                let _guard = atomicity.lock().unwrap();
                self.downloading = None;
                return Ok(true);
            }
            return Ok(false);
        }
        let rt = tm.real_time() as f32;
        if self.tags.touch(touch, rt) {
            return Ok(true);
        }
        if self.rate_dialog.touch(touch, rt) {
            return Ok(true);
        }
        if self.menu.showing() {
            self.menu.touch(touch, t);
            return Ok(true);
        }
        if self.fav_menu.showing() {
            self.fav_menu.touch(touch, t);
            return Ok(true);
        }
        if self.side_enter_time.is_finite() {
            if self.side_enter_time > 0. && tm.real_time() as f32 > self.side_enter_time + edit_transit().unwrap_or_default() {
                if touch.position.x < 1. - self.side_content.width() && touch.phase == TouchPhase::Started && self.save_task.is_none() {
                    if matches!(self.side_content, SideContent::Mods) {
                        if let Some(index) = get_data().find_chart_by_path(self.local_path.as_deref().unwrap()) {
                            let chart = &mut get_data_mut().charts[index];
                            if chart.mods != self.mods {
                                chart.mods = self.mods;
                                save_data()?;
                            }
                        }
                    }
                    if matches!(self.side_content, SideContent::Edit) && self.info_edit.as_ref().is_some_and(|it| it.updated) {
                        confirm_dialog(tl!("warn"), tl!("cancel-not-saved"), self.confirm_cancel_edit.clone());
                    } else {
                        self.hide_side(rt);
                    }
                    return Ok(true);
                }
                match self.side_content {
                    SideContent::Edit => {
                        if self.edit_scroll.touch(touch, t) {
                            return Ok(true);
                        }
                    }
                    SideContent::Leaderboard => {
                        if self.ldb_type_btn.touch(touch, rt) {
                            self.ldb_std ^= true;
                            self.ldb_scroll.y_scroller.offset = 0.;
                            self.load_ldb();
                            return Ok(true);
                        }
                        if self.ldb_scroll.touch(touch, t) {
                            return Ok(true);
                        }
                        if let Some((_, ldb)) = &mut self.ldb {
                            for item in ldb {
                                if item.btn.touch(touch) {
                                    button_hit();
                                    self.sf
                                        .goto(t, ProfileScene::new(item.inner.player.id, self.icons.user.clone(), self.rank_icons.clone()));
                                    return Ok(true);
                                }
                            }
                        }
                    }
                    SideContent::Info => {
                        if self.info_scroll.touch(touch, t) {
                            return Ok(true);
                        }
                        if self.uploader_btn.touch(touch) {
                            button_hit();
                            self.sf.goto(
                                t,
                                ProfileScene::new(self.info.uploader.as_ref().unwrap().id, self.icons.user.clone(), self.rank_icons.clone()),
                            );
                            return Ok(true);
                        }
                        if self.open_web_btn.touch(touch, rt) {
                            open_url(&format!("https://phira.moe/chart/{}", self.info.id.unwrap()))?;
                            return Ok(true);
                        }
                        if self.level_author_btn.touch(touch, rt) {
                            if let Some(author) = &self.level_author {
                                open_url(&author.link)?;
                                return Ok(true);
                            }
                        }
                    }
                    SideContent::Mods => {
                        if self.mod_scroll.touch(touch, t) {
                            return Ok(true);
                        }
                        let rt = tm.real_time() as _;
                        for (btn, clicked) in &mut self.mod_btns {
                            if btn.touch(touch, rt) {
                                *clicked = true;
                                return Ok(true);
                            }
                        }
                    }
                }
            }
            return Ok(false);
        }
        if self.back_btn.touch(touch) {
            back_sound();
            self.next_scene = Some(NextScene::PopWithResult(Box::new(false)));
            return Ok(true);
        }
        if self.scene_task.is_none() && self.next_scene.is_none() && self.play_btn.touch(touch, t) {
            play_sound();
            if self.local_path.is_some() {
                *prpr::scene::LAUNCH_FLASH.lock().unwrap() = Some(t as f64);
                self.launch(GameMode::Normal, false)?;
            } else {
                self.start_download()?;
            }
            return Ok(true);
        }
        if !self.menu_options.is_empty() && self.menu_btn.touch(touch) {
            button_hit();
            self.need_show_menu = true;
            return Ok(true);
        }
        if self.fav_btn.touch(touch) {
            self.fav_long_touch.reset();
            button_hit();
            let data = get_data();
            if let Some(uuid) = data.collection_uuids().iter().find(|uuid| data.collection_info(uuid).is_default) {
                self.toggle_in(*uuid);
            }
            return Ok(true);
        }
        if self.fav_btn.long_touch(touch, t, &mut self.fav_long_touch) {
            button_hit();
            let options = self.get_fav_menu_options();
            self.fav_menu.set_options(options);
            self.need_show_fav_menu = true;
            return Ok(true);
        }
        if let Some(path) = &self.local_path {
            if self.edit_btn.touch(touch) {
                button_hit();
                let mut info: ChartInfo = serde_yaml::from_str(&std::fs::read_to_string(format!("{}/{path}/info.yml", dir::charts()?))?)?;
                info.id = self.info.id;
                UPLOAD_NOT_SAVED.store(false, Ordering::SeqCst);
                self.info_edit = Some(ChartInfoEdit::new(info));
                self.side_content = SideContent::Edit;
                self.side_enter_time = tm.real_time() as _;
                return Ok(true);
            }
            if self.mod_btn.touch(touch) {
                button_hit();
                self.side_content = SideContent::Mods;
                self.side_enter_time = tm.real_time() as _;
                return Ok(true);
            }
        }
        if self.info.id.is_some() && self.chart_type != ChartType::XCSim && self.ldb_btn.touch(touch) {
            button_hit();
            self.side_content = SideContent::Leaderboard;
            self.side_enter_time = tm.real_time() as _;
        }
        if self.info_btn.touch(touch) {
            button_hit();
            if let Some(uploader) = &self.info.uploader {
                UserManager::request(uploader.id);
            }
            self.collaborators = parse_collaborators(&self.info.intro);
            for id in self.collaborators.keys() {
                UserManager::request(*id);
            }
            self.side_content = SideContent::Info;
            self.side_enter_time = tm.real_time() as _;
            return Ok(true);
        }

        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = tm.now() as f32;
        self.menu.update(t);
        self.fav_menu.update(t);
        if self.fav_btn.update_long_touch(t, &mut self.fav_long_touch) {
            button_hit();
            let options = self.get_fav_menu_options();
            self.fav_menu.set_options(options);
            self.need_show_fav_menu = true;
        }
        self.illu.settle(t);
        let rt = tm.real_time() as f32;
        self.tags.update(rt);
        self.rate_dialog.update(rt);
        if self.tags.confirmed.take() == Some(true) {
            let mut tags = self.tags.tags.tags().to_vec();
            tags.push(self.tags.division.to_owned());
            if self.side_enter_time.is_finite() && matches!(self.side_content, SideContent::Edit) {
                let edit = self.info_edit.as_mut().unwrap();
                edit.info.tags = tags;
                edit.updated = true;
            } else {
                let id = self.info.id.unwrap();
                self.entity.as_mut().unwrap().tags = tags.clone();
                self.edit_tags_task = Some(Task::new(async move {
                    recv_raw(Client::post(
                        format!("/chart/{id}/edit-tags"),
                        &json!({
                            "tags": tags,
                        }),
                    ))
                    .await?;
                    Ok(())
                }));
            }
        }
        if self.rate_dialog.confirmed.take() == Some(true) {
            if let Some(id) = self.info.id {
                let score = self.rate_dialog.rate.score;
                self.rate_task = Some(Task::new(async move {
                    recv_raw(Client::post(
                        format!("/chart/{id}/rate"),
                        &json!({
                            "score": score,
                        }),
                    ))
                    .await?;
                    Ok(())
                }));
            }
        }
        if self.side_enter_time < 0. && -tm.real_time() as f32 + edit_transit().unwrap_or_default() < self.side_enter_time {
            self.side_enter_time = f32::INFINITY;
        }
        if CONFIRM_AUTOCOMPLETE.fetch_and(false, Ordering::SeqCst) {
            self.start_autocomplete();
        }
        if SKIP_AUTOCOMPLETE.fetch_and(false, Ordering::SeqCst) {
            self.save_edit();
        }
        if let Some(task) = &mut self.autocomplete_task {
            if let Some(res) = task.take() {
                self.autocomplete_task = None;
                match res {
                    Err(err) => {
                        show_error(err);
                    }
                    Ok(new_intro) => {
                        if let Some(edit) = self.info_edit.as_mut() {
                            edit.info.intro = new_intro;
                            edit.updated = true;
                        }
                        show_message(tl!("collab-autocomplete-done")).duration(1.).ok();
                        self.save_edit();
                    }
                }
            }
        }
        if let Some(task) = &mut self.load_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err.context(tl!("load-charts-failed")));
                    }
                    Ok(chart) => {
                        if let Some(chart) = chart {
                            self.entity = Some(chart.as_ref().clone());
                            if self
                                .info
                                .updated
                                .map_or(chart.updated != chart.created, |local_updated| local_updated != chart.updated)
                                && self.local_path.is_some()
                            {
                                let chart_updated = self
                                    .info
                                    .chart_updated
                                    .map_or(chart.chart_updated != chart.created, |local_updated| local_updated != chart.chart_updated);
                                confirm_dialog(
                                    tl!("need-update"),
                                    if chart_updated {
                                        tl!("need-update-content")
                                    } else {
                                        tl!("need-update-info-only-content")
                                    },
                                    Arc::clone(&self.should_update),
                                );
                            }
                        } else if let Some(local) = &self.local_path {
                            let conf = format!("{}/{}/info.yml", dir::charts()?, local);
                            let mut info: ChartInfo = serde_yaml::from_reader(File::open(&conf)?)?;
                            info.id = None;
                            info.uploader = None;
                            info.created = None;
                            info.updated = None;
                            info.chart_updated = None;
                            serde_yaml::to_writer(File::create(conf)?, &info)?;
                            self.info = info.into();
                            self.update_chart_info()?;
                        }
                        self.update_menu();
                    }
                }
                self.load_task = None;
            }
        }
        if let Some(task) = &mut self.preview_task {
            if let Some(result) = task.take() {
                match result {
                    Err(err) => {
                        show_error(err.context(tl!("load-preview-failed")));
                    }
                    Ok(clip) => {
                        self.preview = Some(create_music(clip)?);
                    }
                }
                self.preview_task = None;
            }
        }
        if let Some(dl) = &mut self.downloading {
            if let Some(tuple) = dl.check()? {
                self.local_path = dl.local_path.take();
                self.downloading = None;
                if let Some(tuple) = tuple {
                    self.load_tuple(tuple)?;
                }
                self.update_menu();
            }
        }
        if let Some(task) = &mut self.scene_task {
            if let Some(res) = poll_future(task.as_mut()) {
                match res {
                    Err(err) => {
                        error!(?err, "failed to play");
                        *self.background.lock().unwrap() = None;
                        self.tr_start = f32::NAN;
                        let error = format!("{err:?}");
                        Dialog::plain(tl!("failed-to-play"), error)
                            .buttons(vec![tl!("play-cancel").into_owned(), tl!("play-switch-to-offline").into_owned()])
                            .listener(move |_dialog, pos| {
                                if pos == 1 {
                                    get_data_mut().config.offline_mode = true;
                                    let _ = save_data();
                                    show_message(tl!("switched-to-offline")).ok();
                                }
                                false
                            })
                            .show();
                    }
                    Ok(scene) => self.next_scene = Some(scene),
                }
                self.scene_task = None;
            }
        }
        if let Some(task) = &mut self.fetch_best_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        warn!(?err, "failed to fetch best record");
                    }
                    Ok(rec) => {
                        self.update_record(rec)?;
                    }
                }
                self.fetch_best_task = None;
            }
        }
        if self.menu.changed() {
            let option = self.menu_options[self.menu.selected()];
            match option {
                "delete" => {
                    confirm_delete(self.should_delete.clone());
                }
                "rate" => {
                    self.rate_dialog.enter(tm.real_time() as _);
                }
                "exercise" => {
                    *prpr::scene::LAUNCH_FLASH.lock().unwrap() = Some(t as f64);
                    self.launch(GameMode::Exercise, false)?;
                }
                "offset" => {
                    *prpr::scene::LAUNCH_FLASH.lock().unwrap() = Some(t as f64);
                    self.launch(GameMode::TweakOffset, false)?;
                }
                "unlock" => {
                    *prpr::scene::LAUNCH_FLASH.lock().unwrap() = Some(t as f64);
                    self.launch(GameMode::Normal, true)?;
                }
                "review-approve" => {
                    confirm_dialog(tl!("warn"), tl!("review-approve-confirm"), Arc::clone(&self.should_review_approve));
                }
                "review-deny" => {
                    request_input("deny-reason", InputBox::new().mode(InputMode::Multiline));
                }
                "review-del" => {
                    confirm_delete(self.chart_should_delete.clone());
                }
                "review-edit-tags" => {
                    let Some(entity) = self.entity.as_ref() else {
                        show_message(tl!("review-not-loaded")).warn();
                        return Ok(());
                    };
                    self.tags.set(entity.tags.clone());
                    self.tags.enter(tm.real_time() as _);
                }
                "stabilize" => {
                    confirm_dialog(tl!("stabilize"), tl!("stabilize-warn"), Arc::clone(&self.should_stabilize));
                }
                "stabilize-approve" => {
                    confirm_dialog(tl!("warn"), tl!("stabilize-approve-confirm"), Arc::clone(&self.should_stabilize_approve));
                }
                "stabilize-approve-ranked" => {
                    confirm_dialog(tl!("warn"), tl!("stabilize-approve-confirm"), Arc::clone(&self.should_stabilize_approve_ranked));
                }
                "stabilize-comment" => {
                    request_input("stabilize-comment", InputBox::new().mode(InputMode::Multiline));
                }
                "stabilize-deny" => {
                    request_input("stabilize-deny-reason", InputBox::new().mode(InputMode::Multiline));
                }
                "export" => {
                    request_export(format!("{}.pez", sanitize(&self.info.name)));
                }
                _ => {}
            }
        }
        if let Some(config) = take_export() {
            fn export_inner(path: String, output: File) -> Result<()> {
                let charts = dir::charts()?;
                compress_folder(Path::new(&format!("{charts}/{path}")), &mut BufWriter::new(output))?;
                Ok(())
            }

            match config {
                Err(err) => show_error(err.into()),
                Ok(config) => {
                    let path = self.local_path.clone().unwrap();
                    let (tx, rx) = mpsc::sync_channel(1);
                    std::thread::spawn(move || {
                        let result = export_inner(path, config.file);
                        if result.is_err() {
                            if let Err(err) = (config.deleter)() {
                                warn!("failed to delete export file: {:?}", err);
                            }
                        }
                        let _ = tx.send(result);
                    });
                    self.export_task = Some(rx);
                }
            }
        }
        if let Some(rx) = &mut self.export_task {
            match rx.try_recv() {
                Ok(Err(err)) => {
                    show_error(err);
                    self.export_task = None;
                }
                Ok(Ok(())) => {
                    resolve_export();
                    self.export_task = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    show_error(Error::msg("Export thread panicked"));
                    self.export_task = None;
                }
            }
        }
        if self.should_delete.fetch_and(false, Ordering::Relaxed) {
            self.next_scene = Some(NextScene::PopWithResult(Box::new(true)));
        }
        if self.fav_menu.changed() {
            let selected = self.fav_menu.selected();
            self.fav_menu.set_selected(usize::MAX);
            self.toggle_in(self.fav_menu_options[selected]);
            let _ = save_data();
            let options = self.get_fav_menu_options();
            self.fav_menu.set_options(options);
        }
        if self.chart_should_delete.fetch_and(false, Ordering::Relaxed) {
            let id = self.info.id.unwrap();
            self.review_task = Some(Task::new(async move {
                recv_raw(Client::delete(format!("/chart/{id}"))).await?;
                Ok(tl!("review-deleted").into_owned())
            }));
        }
        if self.should_review_approve.fetch_and(false, Ordering::Relaxed) {
            let id = self.info.id.unwrap();
            self.review_task = Some(Task::new(async move {
                #[derive(Deserialize)]
                struct Resp {
                    passed: bool,
                }
                let resp: Resp = recv_raw(Client::post(
                    format!("/chart/{id}/review"),
                    &json!({
                        "approve": true
                    }),
                ))
                .await?
                .json()
                .await?;
                Ok((if resp.passed { tl!("review-passed") } else { tl!("review-approved") }).into_owned())
            }));
        }
        if self.should_stabilize_approve.fetch_and(false, Ordering::Relaxed) {
            let id = self.info.id.unwrap();
            self.review_task = Some(Task::new(async move {
                let resp: StableR = recv_raw(Client::post(
                    format!("/chart/{id}/stabilize"),
                    &json!({
                        "kind": 0,
                    }),
                ))
                .await?
                .json()
                .await?;
                Ok((if resp.status == 0 {
                    tl!("stabilize-approved")
                } else {
                    tl!("stabilize-approved-passed")
                })
                .into())
            }));
        }
        if self.should_stabilize_approve_ranked.fetch_and(false, Ordering::Relaxed) {
            let id = self.info.id.unwrap();
            self.review_task = Some(Task::new(async move {
                let resp: StableR = recv_raw(Client::post(
                    format!("/chart/{id}/stabilize"),
                    &json!({
                        "kind": 1,
                    }),
                ))
                .await?
                .json()
                .await?;
                Ok((if resp.status == 0 {
                    tl!("stabilize-approved")
                } else {
                    tl!("stabilize-approved-passed")
                })
                .into())
            }));
        }
        if self.should_stabilize.fetch_and(false, Ordering::Relaxed) {
            let id = self.info.id.unwrap();
            self.stabilize_task = Some(Task::new(async move {
                recv_raw(Client::post(format!("/chart/{id}/req-stabilize"), &())).await?;
                Ok(())
            }));
        }
        if let Some(task) = &mut self.save_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err.context(tl!("edit-save-failed")));
                    }
                    Ok(tuple) => {
                        self.info_edit.as_mut().unwrap().updated = false;
                        self.load_tuple(tuple)?;
                        show_message(tl!("edit-saved")).duration(1.).ok();
                    }
                }
                self.save_task = None;
            }
        }
        if let Some(task) = &mut self.upload_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err.context(tl!("upload-failed")));
                    }
                    Ok(info) => {
                        show_message(tl!("upload-success")).ok();
                        self.info = info;
                        self.update_chart_info()?;
                        self.side_enter_time = -tm.real_time() as _;
                    }
                }
                self.upload_task = None;
            }
        }
        match self.side_content {
            SideContent::Edit => {
                self.edit_scroll.update(t);
            }
            SideContent::Leaderboard => {
                if self.ldb_scroll.y_scroller.pulled {
                    self.ldb_scroll.y_scroller.offset = 0.;
                    self.load_ldb();
                }
                self.ldb_scroll.update(t);
            }
            SideContent::Info => {
                self.info_scroll.update(t);
            }
            SideContent::Mods => {
                self.mod_scroll.update(t);
            }
        }
        if CONFIRM_UPLOAD.fetch_and(false, Ordering::Relaxed) {
            let local_path = self.local_path.clone().unwrap();
            let id = self.info.id;
            self.update_cksum_task = Some(Task::new(async move {
                if let Some(id) = id {
                    use hex::ToHex;
                    let mut fs = fs_from_path(&local_path)?;
                    let info = prpr::fs::load_info(fs.as_mut()).await?;
                    let chart = fs.load_file(&info.chart).await?;
                    let cksum: String = Sha256::digest(&chart).encode_hex();
                    #[derive(Deserialize)]
                    struct VerifyR {
                        ok: bool,
                    }
                    let resp: VerifyR = recv_raw(Client::get(format!("/chart/{id}/verify-cksum?checksum={cksum}")))
                        .await?
                        .json()
                        .await?;
                    Ok(resp.ok)
                } else {
                    Ok(true)
                }
            }));
        }
        if let Some(task) = &mut self.update_cksum_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err.context(tl!("upload-failed")));
                    }
                    Ok(ok) => {
                        if ok {
                            CONFIRM_CKSUM.store(true, Ordering::Relaxed);
                        } else {
                            Dialog::simple(tl!("upload-confirm-clear-ldb"))
                                .buttons(vec![ttl!("cancel").into_owned(), ttl!("confirm").into_owned()])
                                .listener(move |_dialog, pos| {
                                    if pos == 1 {
                                        CONFIRM_CKSUM.store(true, Ordering::Relaxed);
                                    }
                                    false
                                })
                                .show();
                        }
                    }
                }
                self.update_cksum_task = None;
            }
        }
        if CONFIRM_CKSUM.fetch_and(false, Ordering::Relaxed) {
            let path = self.local_path.clone().unwrap();
            let info = self.info.clone();
            self.upload_task = Some(Task::new(async move {
                let root = format!("{}/{path}", dir::charts()?);
                let root = Path::new(&root);
                let mut chart_bytes = Vec::new();
                compress_folder(root, &mut Cursor::new(&mut chart_bytes))?;
                let file = Client::upload_file("chart.zip", chart_bytes)
                    .await
                    .with_context(|| tl!("upload-chart-failed"))?;
                if let Some(id) = info.id {
                    #[derive(Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct Resp {
                        updated: DateTime<Utc>,
                        chart_updated: DateTime<Utc>,
                    }
                    let resp: Resp = recv_raw(Client::request(Method::PATCH, format!("/chart/{id}")).json(&json!({
                        "file": file,
                        "created": info.created.unwrap(),
                    })))
                    .await?
                    .json()
                    .await?;
                    let conf = root.join("info.yml");
                    let mut info: ChartInfo = serde_yaml::from_reader(File::open(&conf)?)?;
                    info.updated = Some(resp.updated);
                    info.chart_updated = Some(resp.chart_updated);
                    serde_yaml::to_writer(File::create(conf)?, &info)?;
                    Ok(info.into())
                } else {
                    #[derive(Deserialize)]
                    struct Resp {
                        id: i32,
                        created: DateTime<Utc>,
                    }
                    let resp: Resp = recv_raw(Client::post(
                        "/chart/upload",
                        &json!({
                            "file": file,
                        }),
                    ))
                    .await?
                    .json()
                    .await?;
                    let conf = root.join("info.yml");
                    let mut info: ChartInfo = serde_yaml::from_reader(File::open(&conf)?)?;
                    info.id = Some(resp.id);
                    info.created = Some(resp.created);
                    info.updated = Some(resp.created);
                    info.chart_updated = Some(resp.created);
                    info.uploader = Some(get_data().me.as_ref().unwrap().id);
                    serde_yaml::to_writer(File::create(conf)?, &info)?;
                    Ok(info.into())
                }
            }));
        }
        if let Some(task) = &mut self.ldb_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err.context(tl!("ldb-load-failed")));
                    }
                    Ok(items) => {
                        let rank = get_data()
                            .me
                            .as_ref()
                            .and_then(|me| items.iter().find(|it| it.inner.player.id == me.id).map(|it| it.rank));
                        for item in &items {
                            UserManager::request(item.inner.player.id);
                        }
                        self.ldb = Some((rank, items));
                        self.ldb_fader.sub(tm.real_time() as _);
                    }
                }
                self.ldb_task = None;
            }
        }
        if let Some((id, text)) = take_input() {
            match id.as_str() {
                "deny-reason" => {
                    let id = self.info.id.unwrap();
                    self.review_task = Some(Task::new(async move {
                        recv_raw(Client::post(
                            format!("/chart/{id}/review"),
                            &json!({
                                "approve": false,
                                "reason": text,
                            }),
                        ))
                        .await?;
                        Ok(tl!("review-denied").into_owned())
                    }));
                }
                "stabilize-comment" => {
                    let id = self.info.id.unwrap();
                    self.review_task = Some(Task::new(async move {
                        recv_raw(Client::post(
                            format!("/chart/{id}/stabilize-comment"),
                            &json!({
                                "comment": text,
                            }),
                        ))
                        .await?;
                        Ok(tl!("stabilize-commented").into())
                    }));
                }
                "stabilize-deny-reason" => {
                    let id = self.info.id.unwrap();
                    self.review_task = Some(Task::new(async move {
                        let resp: StableR = recv_raw(Client::post(
                            format!("/chart/{id}/stabilize"),
                            &json!({
                                "kind": -1,
                                "reason": text,
                            }),
                        ))
                        .await?
                        .json()
                        .await?;
                        Ok((if resp.status == 0 {
                            tl!("stabilize-denied")
                        } else {
                            tl!("stabilize-denied-passed")
                        })
                        .into())
                    }));
                }
                _ => return_input(id, text),
            }
        }
        if let Some((id, file)) = take_file() {
            if id == "overwrite" {
                self.overwrite_from = Some(file);
                CONFIRM_OVERWRITE.store(false, Ordering::SeqCst);
                Dialog::simple(tl!("edit-overwrite-confirm"))
                    .buttons(vec![ttl!("cancel").into_owned(), ttl!("confirm").into_owned()])
                    .listener(move |_dialog, pos| {
                        if pos == 1 {
                            CONFIRM_OVERWRITE.store(true, Ordering::SeqCst);
                        }
                        false
                    })
                    .show();
            } else {
                return_file(id, file);
            }
        }
        if CONFIRM_OVERWRITE.fetch_and(false, Ordering::Relaxed) {
            let path = self.overwrite_from.take().unwrap();
            let local_path = self.local_path.clone().unwrap();
            let def_illu = self.illu.texture.1.clone();
            let chart_id = self.info.id.unwrap();
            let owner = self.info.uploader.as_ref().unwrap().id;
            self.overwrite_task = Some(Task::new(async move {
                let (dir, id) = gen_custom_dir()?;
                let to_path = format!("{}/{}/", dir::charts()?, local_path);
                let file = File::open(path).context("cannot open file")?;
                if let Err(err) = import_chart_to(&dir, format!("custom/{id}"), file).await {
                    std::fs::remove_dir_all(dir)?;
                    return Err(err);
                }
                let mut fs = prpr::fs::fs_from_file(&dir)?;
                let mut info = prpr::fs::load_info(fs.as_mut()).await?;
                drop(fs);
                info.id = Some(chart_id);
                info.uploader = Some(owner);
                serde_yaml::to_writer(File::create(dir.join("info.yml"))?, &info)?;

                std::fs::remove_dir_all(&to_path)?;
                std::fs::rename(&dir, &to_path)?;

                load_local_tuple(&local_path, def_illu, info).await
            }));
        }
        if let Some(task) = &mut self.overwrite_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err.context(tl!("edit-overwrite-failed")));
                    }
                    Ok(tuple) => {
                        self.load_tuple(tuple)?;
                        show_message(tl!("edit-overwrite-success")).ok();
                    }
                }
                self.overwrite_task = None;
            }
        }
        if let Some(task) = &mut self.review_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err.context(tl!("review-action-failed")));
                    }
                    Ok(msg) => {
                        show_message(msg).ok();
                    }
                }
                self.review_task = None;
            }
        }
        if let Some(task) = &mut self.stabilize_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err.context(tl!("stabilize-failed")));
                    }
                    Ok(_) => {
                        show_message(tl!("stabilize-requested")).ok();
                    }
                }
                self.review_task = None;
            }
        }
        if let Some(task) = &mut self.edit_tags_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err.context(tl!("review-edit-tags-failed")));
                    }
                    Ok(_) => {
                        show_message(tl!("review-edit-tags-done")).ok();
                    }
                }
                self.edit_tags_task = None;
            }
        }
        if let Some(task) = &mut self.rate_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err.context(tl!("rate-failed")));
                    }
                    Ok(_) => {
                        show_message(tl!("rate-done")).ok();
                    }
                }
                self.rate_dialog.dismiss(rt);
                self.rate_task = None;
            }
        }
        if self.should_update.fetch_and(false, Ordering::Relaxed) {
            self.start_download()?;
        }
        if let Some(task) = &mut self.my_rating_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        warn!(?err, "failed to fetch my rating status");
                    }
                    Ok(score) => {
                        self.rate_dialog.rate.score = score;
                        self.my_rate_score = Some(score);
                    }
                }
                self.my_rating_task = None;
            }
        }
        if let Some(task) = &mut self.scene_task {
            if let Some(res) = poll_future(task.as_mut()) {
                self.next_scene = Some(res?);
                self.scene_task = None;
            }
        }
        if let Some(task) = &mut self.toggle_fav_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => {
                        show_error(err);
                    }
                    Ok((col, added)) => {
                        let data = get_data();
                        if let Some(uuid) = data.collection_uuids().iter().find(|it| data.collection_info(it).id == Some(col.id)) {
                            let uuid = *uuid;
                            let local = data.collection_info(&uuid);
                            data.set_collection_info(&uuid, local.merge(&col))?;
                        }
                        if added {
                            show_message(tl!("fav-added")).duration(1.5).ok();
                        }
                        FAV_UPDATED.store(true, Ordering::SeqCst);
                        self.is_fav = None;
                    }
                }
                self.toggle_fav_task = None;
            }
        }
        if self.confirm_cancel_edit.swap(false, Ordering::Relaxed) {
            self.hide_side(rt);
        }
        if self.tr_start.is_nan() && self.background.lock().unwrap().is_some() && !get_data().prefer_reduced_motion {
            self.tr_start = rt;
        }

        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        // 背景使用原始比例，不随 UI 比例缩放
        set_camera(&ui.bg_camera());
        let t = tm.now() as f32;
        ui.fill_rect(ui.screen_rect(), (*self.illu.texture.1, ui.screen_rect()));
        ui.fill_rect(ui.screen_rect(), semi_black(0.55));

        // UI 使用带比例的 camera
        set_camera(&ui.camera());

        let r = ui.back_rect();
        self.back_btn.set(ui, r);
        ui.fill_rect(r, (*self.icons.back, r, ScaleType::Fit));

        let alpha = fade_in_time().map_or(1., |tt| ((t - self.fade_start) / tt).clamp(-1., 0.) + 1.);
        ui.alpha::<Result<()>>(alpha, |ui| {
            let title_size = 1.2;
            let subtitle_size = 0.5;

            let title_r = ui
                .text(&self.info.name)
                .max_width(0.57 - r.right())
                .size(title_size)
                .pos(r.right() + 0.02, r.y)
                .draw();

            ui.text(&self.info.composer)
                .size(subtitle_size)
                .pos(r.x + 0.1, r.bottom() + 0.06)
                .color(semi_white(0.8))
                .draw();

            let level_text = format!("{} Lv.{}", self.info.level, self.info.difficulty);
            let level_size = 0.45;
            let level_r = ui
                .text(level_text)
                .size(level_size)
                .pos(r.right() + 0.02, title_r.bottom() + 0.02)
                .color(semi_white(0.7))
                .draw();

            let s = 0.25;
            let rank_r = Rect::new(-0.94, ui.top - s - 0.06, s, s);
            let icon = self.record.as_ref().map_or(0, |it| icon_index(it.score as _, it.full_combo));

            ui.fill_rect(rank_r, (*self.rank_icons[icon], rank_r, ScaleType::Fit));

            let score = self.record.as_ref().map(|it| it.score).unwrap_or_default();
            let accuracy = self.record.as_ref().map(|it| it.accuracy).unwrap_or_default();

            let score_r = ui
                .text(format!("{score:07}"))
                .pos(rank_r.right() + 0.02, rank_r.center().y)
                .anchor(0., 1.)
                .size(1.2)
                .draw();

            ui.text(format!("{:.2}%", accuracy * 100.))
                .pos(score_r.x, score_r.bottom() + 0.01)
                .anchor(0., 0.)
                .size(0.7)
                .color(semi_white(0.7))
                .draw();

            if self.info.id.is_some() && self.chart_type != ChartType::XCSim {
                let h = 0.09;
                let mut ldb_r = Rect::new(score_r.x, score_r.y - h, h, h);
                let ldb_bg = ldb_r.feather(-0.01);
                ui.fill_path(&ldb_bg.rounded(0.015), semi_black(0.3));

                ui.fill_rect(ldb_r, (*self.icons.ldb, ldb_r, ScaleType::Fit));
                if let Some((rank, _)) = &self.ldb {
                    ui.text(if let Some(rank) = rank {
                        format!("#{rank}")
                    } else {
                        tl!("ldb-no-rank").into_owned()
                    })
                    .pos(ldb_r.right() + 0.01, ldb_r.center().y)
                    .anchor(0., 0.5)
                    .no_baseline()
                    .size(0.7)
                    .draw();
                } else {
                    ui.loading(
                        ldb_r.right() + 0.04,
                        ldb_r.center().y,
                        t,
                        WHITE,
                        LoadingParams {
                            radius: 0.027,
                            width: 0.007,
                            ..Default::default()
                        },
                    );
                }
                ldb_r.w += 0.13;
                self.ldb_btn.set(ui, ldb_r);
            }

            // 开始按钮：白色平行四边形，压在右上角、右边裁平 ——
            // 左边留斜边（上边右移），右边的两个角去掉（右边缘是贴着屏边的直边）。
            let w = 0.42;
            let h = 0.26;
            let lean = h * 0.22;
            let play_r = Rect::new(1. - w, ui.top - h, w, h);
            self.play_btn.inner.set(ui, play_r);
            // 按下反馈：按住期间整块压暗（不缩放 —— 缩放会让图标跟着一起动，看着晃）。
            // 注意 DRectButton::progress 是「1 = 没按、0 = 按住并已稳定」，所以取 1 - progress
            // 才是按下程度：按住不放会一路降到 0.84 并停在那儿，松手再回到纯白。
            let pressed = 1. - self.play_btn.progress(t);
            let shade = 1. - 0.16 * pressed;
            let mut builder = lyon::path::Path::builder();
            builder.begin(lyon::math::point(play_r.x + lean, play_r.y));
            builder.line_to(lyon::math::point(play_r.right(), play_r.y));
            builder.line_to(lyon::math::point(play_r.right(), play_r.bottom()));
            builder.line_to(lyon::math::point(play_r.x, play_r.bottom()));
            builder.close();
            ui.fill_path(&builder.build(), Color::new(shade, shade, shade, 1.));
            // 图标：先按贴图比例算出居中的方框再画。`ScaleType::Fit` 是「拉满整个矩形」，
            // 直接把一个宽扁的矩形交给它，▶ 会被拉长；所以要自己算等比尺寸。
            // 白底上用深色图标，否则白图压白底看不见。
            let tex = if self.local_path.is_some() { *self.icons.play } else { *self.icons.download };
            let ratio = tex.width() / tex.height();
            let side = (play_r.h * PLAY_ICON_RATIO).min(play_r.w * 0.28);
            let (iw, ih) = if ratio > 1. { (side, side / ratio) } else { (side * ratio, side) };
            // 平行四边形重心在矩形中心右移 lean/2：图标跟着往右挪一点才居中
            let icon_r = Rect::new(
                play_r.center().x + lean * 0.5 - iw * 0.5,
                play_r.center().y - ih * 0.5,
                iw,
                ih,
            );
            ui.fill_rect(icon_r, (tex, icon_r, ScaleType::Fit, Color::new(0.06, 0.06, 0.08, 1.)));

            ui.scope(|ui| {
                ui.dx(1. - 0.03);
                ui.dy(-ui.top + 0.03);
                let s = 0.085;
                let r = Rect::new(-s, 0., s, s);
                let cc = semi_white(0.4);

                let draw_btn = |ui: &mut Ui, r: Rect, icon: &SafeTexture, enabled: bool| {
                    let bg = r.feather(-0.01);
                    ui.fill_path(&bg.rounded(0.015), if enabled { semi_black(0.3) } else { semi_black(0.15) });
                    ui.fill_rect(r, (**icon, r, ScaleType::Fit, if enabled { WHITE } else { cc }));
                };

                draw_btn(ui, r, &self.icons.menu, !self.menu_options.is_empty());
                self.menu_btn.set(ui, r);
                if self.need_show_menu {
                    self.need_show_menu = false;
                    self.menu.set_bottom(true);
                    self.menu.set_selected(usize::MAX);
                    let d = 0.28;
                    let h = self.menu_options.len().min(5) as f32 * 0.1;
                    self.menu.show(ui, t, Rect::new(r.x - d, r.bottom() + 0.02, r.w + d, h));
                }
                ui.dx(-r.w - 0.025);
                draw_btn(ui, r, &self.icons.info, true);
                self.info_btn.set(ui, r);
                ui.dx(-r.w - 0.025);

                if self.local_path.as_ref().is_none_or(|it| !it.starts_with(':') && !it.starts_with("builtin:")) {

                    let is_fav = if let Some(fav) = self.is_fav {
                        fav
                    } else {
                        let chart_ref = self.to_bare_chart_ref();
                        let fav = get_data().collections().any(|col| col.charts.iter().any(|it| it == &chart_ref));
                        self.is_fav = Some(fav);
                        fav
                    };
                    let fav_icon = if is_fav { &self.icons.star } else { &self.icons.star_outline };
                    draw_btn(ui, r, fav_icon, true);
                    self.fav_btn.set(ui, r);
                    if self.need_show_fav_menu {
                        self.need_show_fav_menu = false;
                        self.fav_menu.set_bottom(true);
                        self.fav_menu.set_selected(usize::MAX);
                        let d = 0.28;
                        let h = self.fav_menu_options.len().min(5) as f32 * 0.1;
                        self.fav_menu.show(ui, t, Rect::new(r.x - d, r.bottom() + 0.02, r.w + d, h));
                    }
                    ui.dx(-r.w - 0.025);

                    draw_btn(ui, r, &self.icons.edit, self.local_path.is_some());
                    self.edit_btn.set(ui, r);
                    ui.dx(-r.w - 0.025);
                }
                draw_btn(ui, r, &self.icons.r#mod, self.local_path.is_some());
                self.mod_btn.set(ui, r);
            });

            if let Some(dl) = &mut self.downloading {
                dl.render(ui, t);
            }

            let rt = tm.real_time() as f32;
            if self.side_enter_time.is_finite() {
                let p = edit_transit().map_or(1., |t| ((rt - self.side_enter_time.abs()) / t).min(1.));
                let p = 1. - (1. - p).powi(3);
                let p = if self.side_enter_time < 0. { 1. - p } else { p };
                ui.fill_rect(ui.screen_rect(), semi_black(p * 0.6));
                let w = self.side_content.width();
                let lf = f32::tween(&1.04, &(1. - w), p);
                ui.scope(|ui| {
                    ui.dx(lf);
                    ui.dy(-ui.top);
                    let r = Rect::new(-0.2, 0., 0.2 + w, ui.top * 2.);
                    ui.fill_rect(r, (Color::default(), (r.x, r.y), Color::new(0., 0., 0., p * 0.7), (r.right(), r.y)));

                    match self.side_content {
                        SideContent::Edit => self.side_chart_info(ui, rt),
                        SideContent::Leaderboard => {
                            self.side_ldb(ui, rt);
                            Ok(())
                        }
                        SideContent::Info => {
                            self.side_info(ui, rt);
                            Ok(())
                        }
                        SideContent::Mods => {
                            self.side_mods(ui, rt);
                            Ok(())
                        }
                    }
                })?;
            }

            Ok(())
        })?;

        self.menu.render(ui, t, 1.);
        self.fav_menu.render(ui, t, 1.);

        if self.save_task.is_some() {
            ui.full_loading(tl!("edit-saving"), t);
        }
        if self.upload_task.is_some() {
            ui.full_loading(tl!("uploading"), t);
        }
        if self.review_task.is_some() {
            ui.full_loading(tl!("review-doing"), t);
        }
        if self.export_task.is_some() {
            ui.full_loading(tl!("exporting"), t);
        }
        if self.edit_tags_task.is_some()
            || self.rate_task.is_some()
            || self.overwrite_task.is_some()
            || self.update_cksum_task.is_some()
            || self.toggle_fav_task.is_some()
            || self.autocomplete_task.is_some()
        {
            ui.full_loading_simple(t);
        }
        let rt = tm.real_time() as f32;
        self.tags.render(ui, rt);
        self.rate_dialog.render(ui, rt);

        if !self.tr_start.is_nan() {
            let p = ((rt - self.tr_start - 0.2) / 0.4).clamp(0., 1.);
            if p >= 1. {
                self.tr_start = f32::NAN;
            }
            let p = 1. - (1. - p).powi(3);
            let mut r = ui.screen_rect();
            r.y += r.h * (1. - p);
            rect_shadow(r, 0.01, 0.5);
            ui.fill_rect(r, (**self.background.lock().unwrap().as_ref().unwrap(), r));
            ui.fill_rect(r, semi_black(0.3));
        }

        self.sf.render(ui, t);

        // 「开始」一按下就白闪，一直白到加载页滑进来为止（那之后由加载页接着白并淡掉）。
        // 加载页要先把谱面准备好才出现，这段时间人已经在等了，先白着才不显得卡。
        if let Some(t0) = *prpr::scene::LAUNCH_FLASH.lock().unwrap() {
            let e = t as f64 - t0;
            if e >= LAUNCH_FLASH_MAX as f64 {
                *prpr::scene::LAUNCH_FLASH.lock().unwrap() = None;
            } else if !get_data().prefer_reduced_motion {
                ui.fill_rect(ui.screen_rect(), WHITE);
            }
        }

        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        if !self.tr_start.is_nan() {
            return NextScene::None;
        }
        if let Some(scene) = self.next_scene.take().or_else(|| self.sf.next_scene(tm.now() as _)) {
            *self.background.lock().unwrap() = None;
            if let Some(music) = &mut self.preview {
                let _ = music.pause();
            }
            scene
        } else {
            NextScene::None
        }
    }
}

pub fn compress_folder<W: Write + Seek>(src: &Path, dst: &mut W) -> Result<()> {
    let mut zip = ZipWriter::new(dst);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o755);
    for entry in WalkDir::new(src) {
        let entry = entry?;
        let path = entry.path();
        let name = path.strip_prefix(src)?;
        let mod_time = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| DateTime::<Utc>::from(t).naive_utc())
            .unwrap_or_else(|| Utc::now().naive_utc());
        if path.is_file() {
            zip.start_file_from_path(name, options.last_modified_time(mod_time.try_into().unwrap_or_default()))?;
            let mut f = File::open(path)?;
            std::io::copy(&mut f, &mut zip)?;
        } else if !name.as_os_str().is_empty() {
            zip.add_directory_from_path(name, options.last_modified_time(mod_time.try_into().unwrap_or_default()))?;
        }
    }
    zip.finish()?;
    Ok(())
}