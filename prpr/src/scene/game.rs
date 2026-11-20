#![allow(unused)]

prpr_l10n::tl_file!("game");

#[path = "life_bar.rs"]
mod life_bar;

use super::{
    challenge::{ChallengeEndData, ChallengeRoundRecord, HpConfig, HpLoseCondition, rank_index, grade}, draw_background,
    ending::RecordUpdateState,
    loading::{BasicPlayer, FinishedStats, LoadingScene, SaveFn, UpdateFn, UploadFn},
    request_input, return_input, show_message, take_input, ChallengeConfig, EndingScene, NextScene, Scene,
};
use crate::{
    bin::BinaryReader,
    config::{Config, HpMode, JudgePreset, Mods},
    core::{copy_fbo, internal_id, BadNote, Chart, ChartExtra, Effect, Matrix, Point, Resource, UIElement, Vector, PGR_FONT},
    ext::{format_number, get_viewport, parse_time, poll_future, screen_aspect, semi_black, semi_white, draw_parallelogram, draw_parallelogram_ex, draw_text_aligned, LocalTask, RectExt, SafeTexture, ScaleType, PARALLELOGRAM_SLOPE},
    fs::{fs_from_file, load_info, FileSystem},
    info::{ChartFormat, ChartInfo},
    judge::{icon_index, Judge},
    parse::{parse_extra, parse_pec, parse_phigros, parse_rpe},
    task::Task,
    time::TimeManager,
    ui::{back_sound, suspend_sound, RectButton, TextPainter, Ui},
};
use anyhow::{bail, Context, Result};
use concat_string::concat_string;
use inputbox::InputBox;
use lyon::path::Path;
use macroquad::{prelude::*, window::InternalGlContext};
use sasa::{AudioClip, Music, MusicParams, Sfx};
use serde::{Deserialize, Serialize};

/// 临时歌词链路运行时取证日志（定位歌词不显示用，定位完成后移除）
#[allow(unused)]
fn lyric_trace(msg: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(r"F:\RustProject\phirLie\lyric_trace.log")
    {
        let _ = writeln!(
            f,
            "{} {}",
            chrono::Utc::now().format("%H:%M:%S%.3f"),
            msg
        );
    }
}

/// [LIFE 样式] LIFE 文本：黑色填充 + 红色描边（8 方向偏移副本 + 4x 超采样平滑）；
/// 扣血红闪时随 life_flash 线性渐变纯红（k=1 全红 → k=0 黑），描边保持红色
fn draw_life_text(ui: &mut Ui, s: &str, x: f32, y: f32, size: f32, ax: f32, ay: f32, k: f32) {
    // 平滑描边：4x 超采样（字形以 4 倍字号光栅化，再乘 scale(1/4) 缩回显示，
    // 消除位图字体低分辨率光栅化的像素颗粒/锯齿）
    // + 8 方向副本（水平/垂直偏移 3px，对角偏移 3/√2 px，保证斜向描边连续无断裂像素点）。
    // 关键：DrawText 的 pos/锚点平移不受 scale 矩阵缩放，故 pos 必须用显示空间坐标，
    // 锚点通过 measure 出的显示边界手动换算（不能用内置 anchor，否则按 4 倍边界偏移）。
    let vp = get_viewport();
    let px_per_unit = vp.2 as f32 * 0.5;
    const SS: f32 = 4.0; // 超采样倍数
    const DIAG: f32 = 0.7071067811865476; // 1/√2，对角偏移系数
    let d = 3.0 / px_per_unit; // 显示空间描边偏移（约 3px，平移不受 scale 缩放）
    let dc = d * DIAG; // 对角描边偏移（总距离同为 d，描边宽度均匀）
    let edge = Color::new(0.9, 0.05, 0.05, 1.0); // 红色描边
    let fill = Color::new(k, 0.0, 0.0, 1.0); // 主体：正常黑（k=0）→ 扣血红闪渐变红（k=1）
    let ss = Matrix::new_scaling(1.0 / SS); // 超采样缩回矩阵（只作用于字形顶点）
    let sz = size * SS; // 超采样光栅化字号
    // 量出超采样字号的显示边界（pos=0、anchor=0 处测量），手动换算锚点
    let m = ui.text(s).size(sz).anchor(0.0, 0.0).measure_using(&PGR_FONT);
    let (bx, by) = (x - m.w / SS * ax, y - m.h / SS * ay); // 显示空间位置（手动锚点）
    // 8 方向描边副本（先画，主体后画盖住内侧）
    ui.text(s).pos(bx - d, by).anchor(0.0, 0.0).size(sz).scale(ss).color(edge).draw_using(&PGR_FONT);
    ui.text(s).pos(bx + d, by).anchor(0.0, 0.0).size(sz).scale(ss).color(edge).draw_using(&PGR_FONT);
    ui.text(s).pos(bx, by - d).anchor(0.0, 0.0).size(sz).scale(ss).color(edge).draw_using(&PGR_FONT);
    ui.text(s).pos(bx, by + d).anchor(0.0, 0.0).size(sz).scale(ss).color(edge).draw_using(&PGR_FONT);
    ui.text(s).pos(bx - dc, by - dc).anchor(0.0, 0.0).size(sz).scale(ss).color(edge).draw_using(&PGR_FONT);
    ui.text(s).pos(bx + dc, by - dc).anchor(0.0, 0.0).size(sz).scale(ss).color(edge).draw_using(&PGR_FONT);
    ui.text(s).pos(bx - dc, by + dc).anchor(0.0, 0.0).size(sz).scale(ss).color(edge).draw_using(&PGR_FONT);
    ui.text(s).pos(bx + dc, by + dc).anchor(0.0, 0.0).size(sz).scale(ss).color(edge).draw_using(&PGR_FONT);
    // 主体填充
    ui.text(s).pos(bx, by).anchor(0.0, 0.0).size(sz).scale(ss).color(fill).draw_using(&PGR_FONT);
}
use std::{
    any::Any,
    cell::RefCell,
    fs::File,
    io::{Cursor, ErrorKind},
    ops::{Deref, DerefMut, Range},
    path::PathBuf,
    process::{Command, Stdio},
    rc::Rc,
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    time::Duration,
};
use tracing::{debug, warn};

const PAUSE_CLICK_INTERVAL: f32 = 0.7;

/// [谱面揭晓] 全景缩小视角系数：Reveal 模式下视野扩大 2.5 倍（zoom out），
/// 使屏幕外尚未进入游玩区域的判定线与 Note 进入可视范围。仅该模式生效。
const REVEAL_ZOOM_OUT: f32 = 2.5;

/// 鏆傚仠鍚庣户缁椂鐨勫€掑甫鏃堕暱锛氬悓鏃跺喅瀹氬睆骞曚笂銆? / 2 / 1銆嶅€掕鏃剁殑闀垮害銆?
const PAUSE_REWIND_DURATION: f64 = 3.0;
/// 鍊掑甫鏈熼棿鐨勫彉鏆椾笉閫忔槑搴︼紙瀵归綈 phire 鐨?PAUSE_BACKGROUND_ALPHA锛夈€?
const PAUSE_BACKGROUND_ALPHA: f32 = 0.6;

#[rustfmt::skip]
#[cfg(closed)]
mod inner;
#[cfg(closed)]
use inner::*;

const WAIT_TIME: f64 = 0.5;
const AFTER_TIME: f64 = 0.7;

/// 鍐呭祵璺冲叧鎻愮ず闈㈡澘鍥剧墖銆?
const MESSAGE_PANEL_PNG: &[u8] = include_bytes!("../../../assets/icons/message_panel.png");

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimpleRecord {
    pub score: i32,
    pub accuracy: f32,
    pub full_combo: bool,
}


/// 澶氫汉姝ｅ紡娓哥帺鏃剁殑鏆傚仠/缁х画骞挎挱閽╁瓙锛氱敱澶氫汉闈㈡澘鍦ㄥ惎鍔ㄥ灞€鍓嶇疆鍏ワ紝
/// GameScene 鏋勯€犳椂鍙栬蛋銆備粎鍦ㄧ湡姝ｈ繘鍏?绂诲紑鏆傚仠鏃惰皟鐢紙鏆傚仠浼?true銆佺户缁紶 false锛夛紝
/// 鍗曟満娓哥帺涓?None锛屼笉鍙楀奖鍝嶃€?
pub static PAUSE_NOTIFY: Mutex<Option<Arc<dyn Fn(bool) + Send + Sync>>> = Mutex::new(None);

/// 鎸変笅銆屽紑濮嬨€嶇殑鏃跺埢锛堝叡浜?TimeManager 鐨勭鏁帮級銆?
///
/// 鐧介棯浠庤繖涓€鍒昏捣锛氶€夋洸椤靛厛鐧界潃锛堝姞杞介〉杩樻病璧锋潵锛夛紝鍔犺浇椤佃捣鏉ュ悗鎺ョ潃鐧?鈥斺€?
/// 鐧藉睆鏄畠鐨?*搴曟澘**锛孶I 鐢诲湪鐧戒箣涓婏紱绛夎繖椤垫粦鍏ュ畬鎴愬啀鐢卞姞杞介〉鎶婂畠娣℃帀銆?
/// `None` = 娌℃湁姝ｅ湪杩涜鐨勭櫧闂€傛斁鍏ㄥ眬鏄洜涓烘寜涓嬫椂鏄€夋洸椤点€佹粦鍏ユ椂宸茬粡鏄姞杞介〉浜嗐€?
pub static LAUNCH_FLASH: Mutex<Option<f64>> = Mutex::new(None);

impl SimpleRecord {
    pub fn update(&mut self, other: &SimpleRecord) -> bool {
        let mut changed = false;
        if other.score > self.score {
            self.score = other.score;
            changed = true;
        }
        if other.accuracy > self.accuracy {
            self.accuracy = other.accuracy;
            changed = true;
        }
        if other.full_combo & !self.full_combo {
            self.full_combo = other.full_combo;
            changed = true;
        }
        changed
    }
}

fn fmt_time(t: f32) -> String {
    let f = t < 0.;
    let t = t.abs();
    let secs = t % 60.;
    let mut t = (t / 60.) as u64;
    let mins = t % 60;
    t /= 60;
    let hrs = t % 100;
    format!("{}{hrs:02}:{mins:02}:{secs:05.2}", if f { "-" } else { "" })
}

/// 鏆傚仠鎸夐挳鐨勫叆鍦?閫€鍦鸿繘搴︼紙娓哥帺涓ǔ瀹氫负 1锛夈€?
/// 鎶芥垚鑷敱鍑芥暟浠ヤ究鍦?`ui()` / `overlay_ui()` 閲岄兘鑳界畻锛堝悗鑰呭凡缁忓€熺敤浜?`self.res`锛夈€?
fn pause_button_progress(state: &State, now: f64, track_length: f64) -> f32 {
    (match state {
        State::Starting => {
            if now <= GameScene::BEFORE_TIME {
                1. - (1. - now / GameScene::BEFORE_TIME).powi(3)
            } else {
                1.
            }
        }
        State::BeforeMusic | State::Playing => 1.,
        State::Ending => {
            let t = now - track_length - WAIT_TIME;
            1. - (t / (AFTER_TIME + 0.3)).min(1.).powi(2)
        }
    }) as f32
}

/// 鏆傚仠鎸夐挳锛堟父鐜╃晫闈㈠乏涓婅鐨勫皬鏂瑰潡锛変腑蹇冧綅缃€?
fn pause_button_center(aspect_ratio: f32, p: f32) -> Point {
    let pause_w = 0.015;
    let pause_h = pause_w * 3.2;
    let eps = 2e-2 / aspect_ratio;
    let top = -1. / aspect_ratio;
    Point::new(pause_w * 4.0 - 1., top + eps * 3.5 - (1. - p) * 0.4 + pause_h / 2.)
}

/// 閫氱煡澶氫汉闈㈡澘鈥滅帺瀹惰繘鍏?绂诲紑鏆傚仠鈥濄€?
/// 鍙湁澶氫汉姝ｅ紡娓哥帺浼氬湪寮€璧涘墠缃叆 [`PAUSE_NOTIFY`]锛屽崟鏈烘父鐜╀负 None銆?
fn notify_paused(hook: Option<&Arc<dyn Fn(bool) + Send + Sync>>, paused: bool) {
    if let Some(f) = hook {
        f(paused);
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    fn on_game_start();
}

#[derive(PartialEq, Eq)]
pub enum GameMode {
    Normal,
    TweakOffset,
    Exercise,
    NoRetry,
    View,
    /// [Reveal] Play the chart in forced autoplay with a fixed watermark, no record, exit on finish.
    Reveal,
    /// [课题模式] 课题模式：连续游玩三张谱，无连击加成计分，判定窗口课题设定
    Challenge,
    /// [Replay] 回放：自动播放谱面并叠加录制的触点。
    Replay,
    /// 教程/演示：不计成绩、不继承全局 HP。
    Tutorial,
    /// 多人联机：由房间状态驱动，不继承单机全局 HP。
    Lan,
    /// [Re:课题] Re:课题：界面内切换，选谱不限数量不重复，自定义判定窗口
    ReChallenge,
}

#[derive(Clone)]
enum State {
    Starting,
    BeforeMusic,
    Playing,
    Ending,
}

struct LyricDisplay {
    line: crate::parse::lyrics::LyricLine,
    y_offset_start: f32,
    y_offset_target: f32,
    move_start_time: f64,
    appear_start: f64,
    disappear_start: f64,
}

pub struct GameScene {
    should_exit: bool,
    next_scene: Option<NextScene>,

    pub mode: GameMode,
    /// [课题模式] 课题会话配置（None 表示普通游玩）
    pub challenge: Option<ChallengeConfig>,
    /// 当前局是否自然完成（课题结算置位；中途退出仍为 false → 课题作废）
    challenge_finished: bool,
    /// [课题模式] 自然完成后直接启动的下一局加载任务（不进课题页，加载完成经 next_scene 切换）
    challenge_next: LocalTask<Result<NextScene>>,
    /// [HP 血量模式] 当前 HP / 上限 / 扣血条件
    hp: f32,
    hp_max: f32,
    hp_lose: HpLoseCondition,
    hp_running: bool,
    /// [第九章 LIFE 样式] 平滑显示的生命值比例（参考 shrinkSmoothTime=0.16 平滑收缩）
    displayed_hp: f32,
    /// [第九章 LIFE 样式] Mathf.SmoothDamp 速度（照抄 SecretChallengeLifeBar.cs lifeVelocity）
    life_velocity: f32,
    /// [第九章 LIFE 样式] 扣血红闪计时（0.15s 衰减）
    life_flash: f32,
    /// [第九章 LIFE 样式] 真实 shader 材质（惰性加载；None=加载失败）
    life_bar_mat: Option<life_bar::LifeBarMaterial>,
    /// HP 扣血判重：上一帧判定统计
    last_counts: [u32; 4],
    /// [HP 血量模式] 失败演出音效（HP 耗尽时播放 TrackCrash）
    crash_sfx: Option<Sfx>,
    /// [HP 血量模式] 失败演出计时：HP 耗尽后红屏→黑屏→结算；None 表示未触发
    fail_timer: Option<f32>,
    retries_left: u8,
    pub challenge_custom_limits: Option<(f64, f64, f64)>,
    /// 璋遍潰棰勮绛夆€滃彈鎺ф挱鏀锯€濇爣璁帮細涓?true 鏃舵挱鏀剧粨鏉熶笉杩涘叆 EndingScene 缁撶畻椤点€佷笉鍐欒褰曪紝鐩存帴閫€鍑鸿繑鍥炪€?
    preview_mode: bool,
    replay_view: Option<crate::replay::Replay>,
    replay_hit_gen: u64,
    replay_paused: bool,
    replay_controls_hidden: bool,
    replay_speed_idx: usize,
    replay_show_analysis: bool,
    replay_buttons: [crate::ui::DRectButton; 10],
    /// 棰勮琚墦鏂殑澶栭儴淇″彿锛堝澶氫汉妯″紡鎴夸富鐐逛簡寮€濮嬶級锛氱疆浣嶅悗 update() 妫€娴嬪埌浼氱珛鍗崇粨鏉熼瑙堝苟寮瑰洖銆?
    /// 浠呴瑙堢瓑鍙楁帶鎾斁浼氫紶鍏ワ紝鏅€氭父鐜╀负 None锛屼笉鍙楀奖鍝嶃€?
    interrupt: Option<Arc<AtomicBool>>,

    /// 澶氫汉姝ｅ紡娓哥帺锛氭殏鍋?缁х画鏃朵笂鎶ユ湇鍔″櫒鐨勯挬瀛愶紙瑙?[`PAUSE_NOTIFY`]锛?
    pause_notify: Option<Arc<dyn Fn(bool) + Send + Sync>>,
    /// HUD 编辑预览：纯示例数据停在游玩页，不加载谱面、不播放音乐
    hud_preview: bool,
    pub res: Resource,
    pub dynamic_bg: Option<crate::core::DynamicBackground>,
    pub chart: Chart,
    pub judge: Judge,
    pub gl: InternalGlContext<'static>,
    player: Option<BasicPlayer>,
    chart_bytes: Vec<u8>,
    chart_format: ChartFormat,
    info_offset: f32,
    effects: Vec<Effect>,

    first_in: bool,
    exercise_range: Range<f64>,
    exercise_press: Option<(i8, u64)>,
    exercise_btns: (RectButton, RectButton),

    pub music: Music,
    /// [噪域] 官方 JudgeControl 手势过渡：手指落入生效噪域时对 BGM 施加低通滤波（音量减小感）
    block_audio: crate::core::BlockAudio,

    state: State,
    pub last_update_time: f64,
    pause_rewind: Option<f64>,
    pause_first_time: f32,
    pause_alpha: f32,
    pause_blur_tex: Option<Texture2D>,
    pause_blur_rt: Option<RenderTarget>,
    pause_need_blur: bool,

    pub bad_notes: Vec<BadNote>,

    upload_fn: Option<UploadFn>,
    update_fn: Option<UpdateFn>,
    save_fn: Option<SaveFn>,

    best_record: Option<SimpleRecord>,

    pub touch_points: Vec<(f32, f32)>,
    fps_frame_count: u32,
    fps_total_time: f64,
    fps_last_frame_time: f64,
    /// 瀹炴椂甯х巼锛堟寚鏁板钩婊戯級锛岃缃噷寮€鍚€滄樉绀哄钩鍧囧抚鐜団€濆悗鍦ㄦ父鐜╃晫闈㈠乏涓婅鏄剧ず
    fps_smooth: f32,

    dead: bool,
    skip_done: bool,
    track_skipped: bool,
    skip_transition_progress: f32,
    skip_wait_timer: f32,
    skip_fade_out_progress: f32,
    skip_instant: bool,
    skip_blur_material: Option<Material>,
    skip_panel_texture: Option<Texture2D>,
    skip_panel_load_failed: bool,
    // 手动轨道跳过（Alt+S / 四指四角）
    track_skip_active: bool,
    track_skip_retracting: bool,
    track_skip_from_corners: bool,
    track_skip_progress: f32,
    track_skip_hold: f32,
    track_skip_countdown: f32,
    track_skip_white: f32,
    track_skip_alt_release: bool,
    track_skip_gp_release: bool,
    track_skip_corner_ids: [Option<u64>; 4],
    // lyrics
    lyrics: Vec<crate::parse::lyrics::LyricLine>,
    active_lyrics: Vec<LyricDisplay>,
    lyric_index: usize,
    text_height: f32,
}

macro_rules! reset {
    ($self:ident, $res:expr, $tm:ident) => {{
        $self.bad_notes.clear();
        $self.judge.reset();
        $self.chart.reset();
        $self.block_audio.reset();
        $res.judge_line_color = $res.res_pack.info.color_perfect();
        $self.music.pause()?;
        $self.music.seek_to(0.)?;
        $tm.speed = $res.config.speed as _;
        $tm.reset();
        $self.last_update_time = $tm.now();
        $self.state = State::Starting;
        $self.fps_frame_count = 0;
        $self.fps_total_time = 0.0;
        $self.fps_last_frame_time = $tm.real_time();
        $self.fps_smooth = 0.0;
        $self.dead = false;
        $self.hp = $self.hp_max;
        $self.displayed_hp = 1.0;
        $self.life_velocity = 0.0;
        $self.life_flash = 0.0;
        $self.fail_timer = None;
        $self.last_counts = [0; 4];
        if let Some(cfg) = &$self.challenge {
            if let Ok(mut session) = cfg.session.lock() {
                session.hp_left = None;
            }
        }
        $self.skip_done = false;
        $self.track_skipped = false;
        $self.skip_transition_progress = 0.0;
        $self.skip_wait_timer = 0.0;
        $self.skip_fade_out_progress = 0.0;
        $self.skip_instant = false;
        $self.track_skip_active = false;
        $self.track_skip_retracting = false;
        $self.track_skip_from_corners = false;
        $self.track_skip_progress = 0.0;
        $self.track_skip_hold = 0.0;
        $self.track_skip_countdown = 3.0;
        $self.track_skip_white = 0.0;
        $self.track_skip_alt_release = false;
        $self.track_skip_gp_release = false;
        $self.track_skip_corner_ids = [None; 4];
    }};
}

impl GameScene {
    pub const BEFORE_TIME: f64 = 0.7;
    pub const FADEOUT_TIME: f64 = WAIT_TIME + AFTER_TIME + 0.3;
    pub const FAIL_RED_TIME: f32 = 0.1;
    pub const FAIL_BLACK_TIME: f32 = 1.0;

    pub async fn load_chart_for_analysis(
        fs: &mut dyn FileSystem,
        info: &ChartInfo,
    ) -> Result<(Chart, Vec<u8>, ChartFormat)> {
        let bytes = Self::load_chart_bytes(fs, info).await?;
        let format = Self::infer_chart_format(info, &bytes);
        let chart = match format {
            ChartFormat::Pec => crate::parse::parse_pec(&String::from_utf8_lossy(&bytes), ChartExtra::default())?,
            ChartFormat::Pgr => crate::parse::parse_phigros(&String::from_utf8_lossy(&bytes), ChartExtra::default())?,
            ChartFormat::Rpe => crate::parse::parse_rpe(
                &String::from_utf8_lossy(&bytes),
                fs,
                ChartExtra::default(),
                info.use_rpe_170_speed.unwrap_or_default(),
            ).await?,
            ChartFormat::Pbc => anyhow::bail!("PBC analysis is not supported"),
        };
        Ok((chart, bytes, format))
    }

    pub async fn load_chart_bytes(fs: &mut dyn FileSystem, info: &ChartInfo) -> Result<Vec<u8>> {
        if let Ok(bytes) = fs.load_file(&info.chart).await {
            return Ok(bytes);
        }
        if let Some(name) = info.chart.strip_suffix(".pec") {
            if let Ok(bytes) = fs.load_file(&concat_string!(name, ".json")).await {
                return Ok(bytes);
            }
        }
        bail!("Cannot find chart file")
    }

    pub fn infer_chart_format(info: &ChartInfo, bytes: &[u8]) -> ChartFormat {
        info.format.clone().unwrap_or_else(|| {
            if let Ok(text) = String::from_utf8(bytes.to_vec()) {
                if text.starts_with('{') {
                    if text.contains("\"META\"") {
                        ChartFormat::Rpe
                    } else {
                        ChartFormat::Pgr
                    }
                } else {
                    ChartFormat::Pec
                }
            } else {
                ChartFormat::Pbc
            }
        })
    }

    pub async fn load_chart(fs: &mut dyn FileSystem, info: &ChartInfo) -> Result<(Chart, Vec<u8>, ChartFormat)> {
        let extra = fs.load_file("extra.json").await.ok().map(String::from_utf8).transpose()?;
        let mut extra = if let Some(extra) = extra {
            parse_extra(&extra, fs).await.context("Failed to parse extra")?
        } else {
            ChartExtra::default()
        };

        for name in &["lyrics.ttml", "lyric.ttml", "lyrics.xml", "lyric.xml", "lyrics.lrc", "lyric.lrc", "ttml", "lrc"] {
            if let Ok(bytes) = fs.load_file(name).await {
                let text = String::from_utf8_lossy(&bytes).to_string();
                match crate::parse::lyrics::parse(&text) {
                    Ok(lyrics) => {
                        lyric_trace(&format!("load_chart: {} -> {} lines", name, lyrics.len()));
                        extra.lyrics = lyrics;
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("failed to parse lyrics file {}: {}", name, e);
                    }
                }
            }
        }

        let bytes = Self::load_chart_bytes(fs, info).await.context("Failed to load chart")?;
        let format = Self::infer_chart_format(info, &bytes);
        let mut chart = match format {
            ChartFormat::Rpe => parse_rpe(&String::from_utf8_lossy(&bytes), fs, extra, info.use_rpe_170_speed.unwrap_or_default()).await,
            ChartFormat::Pgr => parse_phigros(&String::from_utf8_lossy(&bytes), extra),
            ChartFormat::Pec => parse_pec(&String::from_utf8_lossy(&bytes), extra),
            ChartFormat::Pbc => {
                let mut r = BinaryReader::new(Cursor::new(&bytes));
                r.read()
            }
        }?;
        chart.load_textures(fs).await?;
        chart.settings.hold_partial_cover = info.hold_partial_cover;
        Ok((chart, bytes, format))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        mode: GameMode,
        info: ChartInfo,
        config: Config,
        fs: Box<dyn FileSystem>,
        player: Option<BasicPlayer>,
        background: SafeTexture,
        illustration: SafeTexture,
        upload_fn: Option<UploadFn>,
        update_fn: Option<UpdateFn>,
        save_fn: Option<SaveFn>,
    ) -> Result<Self> {
        Self::new_preview(mode, info, config, fs, player, background, illustration, upload_fn, update_fn, save_fn, false, None, false, None).await
    }

    /// 璋遍潰棰勮绛夆€滃彈鎺ф挱鏀锯€濇瀯閫狅細`preview_mode` 涓?true 鏃惰嚜鐒舵挱瀹屼笉缁撶畻锛堣
    /// [`GameScene::finish_and_show_result`]锛夛紱`interrupt` 鏄閮ㄧ嚎绋嬪啓鍏ョ殑鎵撴柇淇″彿锛?
    /// 缃綅鍚庨瑙堢珛鍗崇粨鏉熷苟寮瑰洖锛堢敤浜庡浜烘ā寮忔埧涓诲紑濮嬫父鎴忕瓑鍦烘櫙锛夈€傛櫘閫氭父鐜╀繚鎸?
    /// `preview_mode = false, interrupt = None`锛岃涓轰笌 [`GameScene::new`] 瀹屽叏涓€鑷淬€?
    #[allow(clippy::too_many_arguments)]
    pub async fn new_preview(
        mode: GameMode,
        info: ChartInfo,
        mut config: Config,
        mut fs: Box<dyn FileSystem>,
        player: Option<BasicPlayer>,
        background: SafeTexture,
        illustration: SafeTexture,
        upload_fn: Option<UploadFn>,
        update_fn: Option<UpdateFn>,
        save_fn: Option<SaveFn>,
        preview_mode: bool,
        interrupt: Option<Arc<AtomicBool>>,
        hud_preview: bool,
        challenge: Option<ChallengeConfig>,
    ) -> Result<Self> {
        match mode {
            GameMode::TweakOffset => {
                config.mods.insert(Mods::AUTOPLAY);
            }
            GameMode::Exercise => {
                config.mods.remove(Mods::AUTOPLAY);
            }
            GameMode::Reveal => {
                config.mods.insert(Mods::AUTOPLAY);
            }
            _ => {}
        }

        let (mut chart, chart_bytes, chart_format) = if hud_preview {
            // HUD 编辑预览：不加载谱面，用空 Chart 占位
            (
                Chart::new(0.0, vec![], Default::default(), Default::default(), Default::default(), Default::default(), vec![]),
                Vec::new(),
                ChartFormat::Pec,
            )
        } else {
            Self::load_chart(fs.deref_mut(), &info).await?
        };
        chart.arcaea_judgement = info.arcaea_judgement || config.arcaea_judgement;
        chart.fnf_judgement = info.fnf_judgement || config.fnf_judgement;
        if config.mods.contains(Mods::NO_SHADER) {
            chart.extra.effects.clear();
            chart.extra.global_effects.clear();
        }
        let mut effects = std::mem::take(&mut chart.extra.global_effects);
        chart.extra.effects.append(&mut effects);
        if config.fxaa {
            chart
                .extra
                .effects
                .push(Effect::new(0.0..f64::INFINITY, include_str!("fxaa.glsl"), Vec::new(), false).unwrap());
        }

        if config.has_mod(Mods::NIGHTCORE) {
            config.speed *= 1.5;
        }
        let replay_view = if matches!(mode, GameMode::Replay) {
            crate::replay::take_pending()
        } else {
            None
        };
        if replay_view.is_some() {
            config.mods.remove(Mods::AUTOPLAY);
            config.speed = replay_view.as_ref().map_or(config.speed, |r| r.speed);
        }

        // 鑺辨牱 mod锛氬睆骞曠壒鏁堜緷璧栧叏灞忓悗澶勭悊绠＄嚎锛坈hart_target锛夛紝鍕鹃€夊悗寮哄埗寮€鍚绠＄嚎
        if config.has_mod(Mods::RAINBOW) || config.has_mod(Mods::FX_TV) || config.has_mod(Mods::FX_SCANLINE) || config.has_mod(Mods::FX_GLITCH) {
            config.disable_effect = false;
        }
        if config.has_mod(Mods::RAINBOW) {
            chart
                .extra
                .effects
                .push(Effect::new(0.0..f64::INFINITY, include_str!("rainbow.glsl"), Vec::new(), false).unwrap());
        }

        // 鑺辨牱 mod锛氬睆骞曠壒鏁堬紙鑰佺數瑙?/ 鎵弿绾?/ 鏁呴殰锛孎X_* 浜掓枼锛涙晠闅滅敤鍙傛暟鏇存槑鏄剧殑涓撶敤鐗堟湰锛?
        for (flag, src) in [
            (Mods::FX_TV, include_str!("../core/shaders/rpe/old_tv_pr.glsl")),
            (Mods::FX_SCANLINE, include_str!("../core/shaders/rpe/scanline_pr.glsl")),
        ] {
            if config.has_mod(flag) {
                chart
                    .extra
                    .effects
                    .push(Effect::new(0.0..f64::INFINITY, src, Vec::new(), false).unwrap());
            }
        }


        let info_offset = info.offset;
        // [HP 血量模式] 失败演出音效：从全局 assets 读取 TrackCrash 字节
        // （谱面 fs 根目录是谱面文件夹，不含 assets/sfx；改用 assets 根 fs）
        let crash_bytes = if let Ok(mut asset_fs) = crate::fs::fs_from_assets("") {
            asset_fs.load_file("sfx/TrackCrash.ogg").await.ok()
        } else {
            None
        };
        let mut res = Resource::new(
            config,
            info,
            fs,
            player.as_ref().and_then(|it| it.avatar.clone()),
            background,
            illustration,
            chart.extra.effects.is_empty() && effects.is_empty(),
            hud_preview,
        )
        .await
        .context("Failed to load resources")?;


        chart.hitsounds.drain().for_each(|(name, clip)| {
            if let Ok(clip) = res.create_sfx(clip) {
                res.extra_sfxs.insert(name, clip);
            }
        });

        // [HP 血量模式] 失败演出音效：HP 耗尽时播放 TrackCrash
        let crash_sfx = crash_bytes.and_then(|bytes| {
            AudioClip::new(bytes).ok().and_then(|clip| res.create_sfx(clip).ok())
        });

        let exercise_range = (chart.offset + info_offset + res.config.offset) as f64..res.track_length;

        let mut judge = Judge::new(&chart);
        judge.set_late_leniency(res.config.judge_late_leniency_ms as f64 / 1000.);
        judge.set_perfect_plus(res.config.judge_perfect_plus_ms as f64 / 1000.);
        judge.set_detailed(
            res.config.judge_detailed,
            res.config.judge_great_ms as f64 / 1000.,
            res.config.judge_ok_ms as f64 / 1000.,
            res.config.judge_meh_ms as f64 / 1000.,
        );

        // [课题模式] 自定义判定窗口 + 课题计分（无连击加成，单局满分 1000000）
        let challenge_limits = challenge.as_ref().and_then(|c| c.custom_limits);
        let limits = challenge_limits.or_else(|| {
            if res.config.arcaea_judgement || res.config.fnf_judgement {
                return None;
            }
            if res.config.judge_preset == JudgePreset::Custom {
                res.config.custom_judge_limits()
            } else {
                let [p, g, b] = res.config.judge_preset.windows_ms();
                Some((p as f64 / 1000., g as f64 / 1000., b as f64 / 1000.))
            }
        });
        judge.set_challenge(limits, challenge.is_some());

        // [HP 血量模式] 课题里 HP 由课题页显式决定（开/关），关掉就是真的关，
        // 不能再用普通游玩的 config.hp 兜底把它顶回来；只有非课题局才走全局 mod。
        let hp_cfg = match challenge.as_ref() {
            Some(c) => c.hp,
            None => (matches!(mode, GameMode::Normal) && !preview_mode).then(|| res.config.hp_config()).flatten(),
        };
        let (hp, hp_max, hp_lose, hp_running) = match hp_cfg {
            Some(c) => (c.max_hp, c.max_hp, c.lose_condition, true),
            None => (0., 0., HpLoseCondition::BadOrMiss, false),
        };

        let lyrics = std::mem::take(&mut chart.extra.lyrics);
        let music = Self::new_music(&mut res)?;
        // 澶氫汉姝ｅ紡娓哥帺鐨勬殏鍋滀笂鎶ラ挬瀛愶細鐢卞浜洪潰鏉垮湪瀵瑰眬鍚姩鍓嶇疆鍏ワ紙鍗曟満涓?None锛?
        let pause_notify = PAUSE_NOTIFY.lock().unwrap().take();
        let scene = Self {
            should_exit: false,
            next_scene: None,

            mode,
            challenge,
            challenge_finished: false,
            challenge_next: None,
            challenge_custom_limits: challenge_limits,
            hp,
            hp_max,
            hp_lose,
            hp_running,
            displayed_hp: 1.0,
            life_velocity: 0.0,
            life_flash: 0.0,
            life_bar_mat: None,
            last_counts: [0; 4],
            crash_sfx,
            fail_timer: None,
            retries_left: res.config.auto_retry,
            preview_mode,
            replay_view,
            replay_hit_gen: 0,
            replay_paused: false,
            replay_controls_hidden: false,
            replay_buttons: std::array::from_fn(|_| crate::ui::DRectButton::new()),
            replay_speed_idx: 1,
            replay_show_analysis: false,
            interrupt,

            pause_notify,
            hud_preview,
            res,
            dynamic_bg: None,
            chart,
            judge,
            gl: unsafe { get_internal_gl() },
            player,
            chart_bytes,
            chart_format,
            effects,
            info_offset,

            first_in: false,
            exercise_range,
            exercise_press: None,
            exercise_btns: (RectButton::new(), RectButton::new()),

            music,
            block_audio: Default::default(),

            state: State::Starting,
            last_update_time: 0.,
            pause_rewind: None,
            pause_first_time: f32::NEG_INFINITY,
            pause_alpha: 0.,
            pause_blur_tex: None,
            pause_blur_rt: None,
            pause_need_blur: false,

            bad_notes: Vec::new(),

            upload_fn,
            update_fn,
            save_fn,

            best_record: None,

            touch_points: Vec::new(),

            fps_frame_count: 0,
            fps_total_time: 0.0,
            fps_last_frame_time: 0.0,
            fps_smooth: 0.0,

            dead: false,
            skip_done: false,
            track_skipped: false,
            skip_transition_progress: 0.0,
            skip_wait_timer: 0.0,
            skip_fade_out_progress: 0.0,
            skip_instant: false,
            skip_blur_material: None,
            skip_panel_texture: None,
            skip_panel_load_failed: false,
            track_skip_active: false,
            track_skip_retracting: false,
            track_skip_from_corners: false,
            track_skip_progress: 0.0,
            track_skip_hold: 0.0,
            track_skip_countdown: 3.0,
            track_skip_white: 0.0,
            track_skip_alt_release: false,
            track_skip_gp_release: false,
            track_skip_corner_ids: [None; 4],
            lyrics,
            active_lyrics: Vec::new(),
            lyric_index: 0,
            text_height: 0.0,
        };
        if scene.mode == GameMode::Normal && !preview_mode && scene.res.config.record_replay {
            crate::replay::begin(&scene.res.info.name, &scene.res.info.chart, scene.res.config.speed);
        }
        Ok(scene)
    }

    fn new_music(res: &mut Resource) -> Result<Music> {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            res.audio.create_music(
                res.music.clone(),
                MusicParams {
                    amplifier: res.config.volume_music as _,
                    playback_rate: res.config.speed as _,
                    ..Default::default()
                },
            )
        })) {
            Ok(Ok(music)) => Ok(music),
            Ok(Err(e)) => Err(e).context("Failed to create music player"),
            Err(_) => anyhow::bail!("Music player panicked"),
        }
    }

    pub fn update_lyrics(&mut self, time: f64) {
        const FADE_IN_TIME: f64 = 0.5;
        const FADE_OUT_TIME: f64 = 0.3; // 与 render_lyrics 保持一致

        while self.lyric_index < self.lyrics.len() && self.lyrics[self.lyric_index].start_time <= time + FADE_IN_TIME {
            let line = &self.lyrics[self.lyric_index];
            let mut y_offset_start = 0.0;
            let mut y_offset_target = 0.0;
            let mut move_start_time = line.start_time;

            if !matches!(line.role, crate::parse::lyrics::LyricRole::Background) {
                // 上移缓动改为在"下一句歌词第一个字出现前 0.05 秒"触发
                let next_first_word_time = line.words.first().map(|w| w.start_time).unwrap_or(line.start_time);
                let next_move_start_time = next_first_word_time - 0.05;
                // 与 render_lyrics 中的 MOVE_TIME 保持一致：上移动画时长
                const MOVE_TIME: f64 = 1.0;
                // text_height 理论上在 render_lyrics 首帧测量后非零；若遇到首帧/seek
                // 直达歌曲中段导致 update 先于测量执行，用兜底高度避免位移被乘成 0
                let text_height = if self.text_height > 0.0 { self.text_height } else { 40.0 };
                // 新的主轨歌词出来时，把旧的主轨歌词往上挤一个位置（歌词滚动）。
                // 挤压条件不能依赖 end_time >= start_time：LRC 行间常有 0.3~0.4s 间隙，
                // 旧句行末时间戳会早于新句开始时间（parse_lrc 对 end < next_start 不改动），
                // 导致旧句从不被挤压、直接淡出消失，视觉上就是"歌词没有上移"。
                // 改为：只要旧主轨句还在显示中（未完全淡出）就让位。
                // 副轨（Duet）保持原语义：仅时间重叠时被挤压，和声并行时不被顶走。
                for active in &mut self.active_lyrics {
                    let should_move = match line.role {
                        crate::parse::lyrics::LyricRole::Main => {
                            matches!(active.line.role, crate::parse::lyrics::LyricRole::Main)
                                && active.disappear_start + FADE_OUT_TIME > time
                        }
                        crate::parse::lyrics::LyricRole::Duet => {
                            !matches!(active.line.role, crate::parse::lyrics::LyricRole::Background)
                                && active.line.end_time >= line.start_time
                        }
                        crate::parse::lyrics::LyricRole::Background => false,
                    };
                    if should_move {
                        active.y_offset_start = active.y_offset_target; // 从当前目标位置开始
                        active.y_offset_target -= text_height * 1.5;
                        // 上一句至少保持显示 1.2 秒，再与"下一句首字前 0.05 秒"取较晚者作为上移触发时间
                        active.move_start_time = (active.line.start_time + 1.2).max(next_move_start_time);
                        // 关键：被挤压的旧句不能按原 disappear_start（行末时间戳=下一句开始时间）
                        // 立即淡出，否则上移缓动全程被 FADE_OUT 掩盖，视觉上表现为"歌词直接消失、
                        // 上移滚动失效"。将淡出起点延后到上移动画完成后再停留 0.5 秒。
                        active.disappear_start =
                            (active.move_start_time + MOVE_TIME + 0.5).max(active.disappear_start);
                    }
                }
            } else {
                if let Some(last_main) = self.active_lyrics.iter().rev().find(|l| !matches!(l.line.role, crate::parse::lyrics::LyricRole::Background)) {
                    y_offset_start = last_main.y_offset_target;
                    y_offset_target = last_main.y_offset_target;
                    move_start_time = last_main.move_start_time;
                }
            }

            let disappear_start = line.words.last().map(|w| w.end_time).unwrap_or(line.end_time);

            {
                let joined: String = line.words.iter().map(|w| w.text.clone()).collect();
                lyric_trace(&format!(
                    "push idx={} t={:.3} start={:.3} end={:.3} role={:?} text={:?}",
                    self.lyric_index, time, line.start_time, line.end_time, line.role, joined
                ));
            }

            self.active_lyrics.push(LyricDisplay {
                line: line.clone(),
                y_offset_start,
                y_offset_target,
                move_start_time,
                appear_start: line.start_time,
                disappear_start,
            });
            self.lyric_index += 1;
        }

        // 不要自动移除歌词，让它们一直在上面
        // self.active_lyrics.retain(|lyric| lyric.disappear_start + FADE_OUT_TIME > time);
    }

    pub fn render_lyrics(&mut self, ui: &mut Ui, time: f64) {
        const FADE_IN_TIME: f64 = 0.5;
        const FADE_OUT_TIME: f64 = 0.3;

        if self.active_lyrics.is_empty() {
            static FRAME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let frame = FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if frame % 120 == 0 {
                lyric_trace(&format!(
                    "render-empty t={:.3} idx={} lyrics={} active=0",
                    time, self.lyric_index, self.lyrics.len()
                ));
            }
            return;
        }

        if self.text_height == 0.0 {
            self.text_height = ui.text("测试").size(0.8).measure().h;
        }

        let res = &mut self.res;
        let top = -1. / res.aspect_ratio;
        let lyrics_center_y = top + (1. / res.aspect_ratio * 2.) * (1. / 3.);

        // 过滤掉背景歌词，只处理主轨和和声
        let main_lyrics: Vec<_> = self.active_lyrics.iter()
            .filter(|l| !matches!(l.line.role, crate::parse::lyrics::LyricRole::Background))
            .collect();

        // 背景歌词单独渲染
        for lyric in &self.active_lyrics {
            if !matches!(lyric.line.role, crate::parse::lyrics::LyricRole::Background) {
                continue;
            }

            let appear_progress = ((time - lyric.appear_start) / FADE_IN_TIME).clamp(0.0, 1.0);
            let disappear_progress = ((time - lyric.disappear_start) / FADE_OUT_TIME).clamp(0.0, 1.0);
            let alpha = (1.0 - disappear_progress) * appear_progress;
            if alpha <= 0.0 {
                continue;
            }

            let size = 0.5;
            let x_pos = -1.0 + 0.1;
            let anchor_x = 0.0;
            let base_y = lyrics_center_y + self.text_height * 1.8;

            let mut total_width = 0.0;
            for word in &lyric.line.words {
                total_width += ui.text(&word.text).size(size).measure().w;
            }

            let mut current_x = 0.0;
            for word in &lyric.line.words {
                let word_width = ui.text(&word.text).size(size).measure().w;

                if time < word.start_time {
                    current_x += word_width;
                    continue;
                }

                let word_appear_progress = ((time - word.start_time) / FADE_IN_TIME).clamp(0.0, 1.0);
                let word_alpha = alpha * 0.6 * word_appear_progress;
                let color = Color::new(1., 1., 1., word_alpha as f32);

                let draw_x = x_pos as f32 + current_x;
                ui.text(&word.text)
                    .pos(draw_x, base_y)
                    .anchor(0.0, 0.5)
                    .size(size)
                    .color(color)
                    .draw();

                current_x += word_width;
            }
        }

        // 处理主轨歌词：最新的在中间，更早的在下面，整体上移
        for (idx, lyric) in main_lyrics.iter().enumerate() {
            let appear_progress = ((time - lyric.appear_start) / FADE_IN_TIME).clamp(0.0, 1.0);
            let disappear_progress = ((time - lyric.disappear_start) / FADE_OUT_TIME).clamp(0.0, 1.0);

            let alpha = (1.0 - disappear_progress) * appear_progress;
            if alpha <= 0.0 {
                static FRAME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                let frame = FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if frame % 120 == 0 {
                    let joined: String = lyric.line.words.iter().map(|w| w.text.clone()).collect();
                    lyric_trace(&format!(
                        "render-skip-main t={:.3} appear={:.3} disappear={:.3} start={:.3} end={:.3} alpha={:.3} text={:?}",
                        time, lyric.appear_start, lyric.disappear_start, lyric.line.start_time, lyric.line.end_time, alpha, joined
                    ));
                }
                continue;
            }

            let role = &lyric.line.role;
            let (x_pos, anchor_x, size) = match role {
                crate::parse::lyrics::LyricRole::Main => (-1.0 + 0.1, 0.0, 0.8),
                crate::parse::lyrics::LyricRole::Duet => (1.0 - 0.1, 1.0, 0.8),
                _ => (-1.0 + 0.1, 0.0, 0.8),
            };

            // 用 Out Quart 缓动计算当前 y_offset
            const MOVE_TIME: f64 = 1.0;
            let move_progress = ((time - lyric.move_start_time) / MOVE_TIME).clamp(0.0, 1.0);
            // Out Quart: 1 - (1 - t)^4
            let eased_progress = 1.0 - (1.0 - move_progress).powi(4);
            let current_y_offset = lyric.y_offset_start + (lyric.y_offset_target - lyric.y_offset_start) * eased_progress as f32;

            let base_y = lyrics_center_y + current_y_offset;
            let alpha_value = alpha;

            let mut total_width = 0.0;
            for word in &lyric.line.words {
                total_width += ui.text(&word.text).size(size).measure().w;
            }

            let mut current_x = 0.0;
            for word in &lyric.line.words {
                let word_width = ui.text(&word.text).size(size).measure().w;

                if time < word.start_time {
                    current_x += word_width;
                    continue;
                }

                let word_appear_progress = ((time - word.start_time) / FADE_IN_TIME).clamp(0.0, 1.0);
                // 整句统一浮起：以行起始时间为基准同步触发 float 动画，
                // 避免紧贴词（如 les/sons、I've）按各自 start_time 逐词上浮造成垂直错位
                // （"lessons" 被显示成 "les ons"、与 I've 错位）
                let line_appear_progress = ((time - lyric.line.start_time) / FADE_IN_TIME).clamp(0.0, 1.0);
                let line_ease_progress = (line_appear_progress * std::f64::consts::PI / 2.0).sin() as f32;
                let word_float_offset = line_ease_progress * self.text_height * 0.3;

                let word_alpha = alpha_value * word_appear_progress;
                let color = Color::new(1., 1., 1., word_alpha as f32);

                let draw_x = if anchor_x == 0.0 {
                    x_pos as f32 + current_x
                } else {
                    // 副轨右对齐：锚点为当前词右端，须补偿词宽 word_width，
                    // 否则每个词右端偏左 w_i，宽窄词交替时出现重叠/空隙且整行未对齐右边界
                    x_pos as f32 - total_width + current_x + word_width
                };

                let draw_y = base_y - word_float_offset;

                static FRAME: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                let frame = FRAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if frame % 120 == 0 {
                    lyric_trace(&format!(
                        "render-draw t={:.3} x={:.3} y={:.3} alpha={:.3} text={:?}",
                        time, draw_x, draw_y, word_alpha, word.text
                    ));
                }

                ui.text(&word.text)
                    .pos(draw_x, draw_y)
                    .anchor(if anchor_x == 0.0 { 0.0 } else { 1.0 }, 0.5)
                    .size(size)
                    .color(color)
                    .draw();

                current_x += word_width;
            }
        }
    }


    fn touch_scale(&self) -> f32 {
        (screen_width() / screen_height()) / self.res.aspect_ratio
    }

    /// 手动轨道跳过：长按 Alt+S，或四指同时按住屏幕四角 3 秒。
    fn update_track_skip(&mut self, tm: &TimeManager) -> Result<()> {
        if self.skip_done {
            self.track_skip_active = false;
            self.track_skip_retracting = false;
            self.track_skip_from_corners = false;
            self.track_skip_corner_ids = [None; 4];
            return Ok(());
        }
        if !matches!(self.state, State::Playing)
            || tm.paused()
            || self.pause_rewind.is_some()
            || self.mode == GameMode::View
        {
            self.track_skip_active = false;
            self.track_skip_retracting = false;
            self.track_skip_from_corners = false;
            self.track_skip_corner_ids = [None; 4];
            self.track_skip_hold = 0.;
            self.track_skip_progress = 0.;
            self.track_skip_countdown = 3.;
            self.track_skip_white = 0.;
            return Ok(());
        }

        if self.track_skip_alt_release && !is_key_down(KeyCode::LeftAlt) {
            self.track_skip_alt_release = false;
        }
        let alt_s_down = !self.track_skip_alt_release && is_key_down(KeyCode::LeftAlt) && is_key_down(KeyCode::S);
        let gp_nav = crate::gamepad::nav_input_static();
        if self.track_skip_gp_release && !gp_nav.skip_combo {
            self.track_skip_gp_release = false;
        }
        let gp_skip_down = !self.track_skip_gp_release && gp_nav.skip_combo;

        let half_h = 1. / self.res.aspect_ratio;
        let margin = 0.2;
        let mut corners = [false; 4];
        let mut ids = [None; 4];
        for touch in Judge::get_touches() {
            if touch.phase == TouchPhase::Ended {
                continue;
            }
            let x = touch.position.x;
            let y = touch.position.y;
            let corner = if x < -1. + margin && y < -half_h + margin {
                0
            } else if x > 1. - margin && y < -half_h + margin {
                1
            } else if x < -1. + margin && y > half_h - margin {
                2
            } else if x > 1. - margin && y > half_h - margin {
                3
            } else {
                continue;
            };
            corners[corner] = true;
            if ids[corner].is_none() {
                ids[corner] = Some(touch.id);
            }
        }
        let all_four = corners.iter().all(|it| *it);
        let corners_alive = self.track_skip_from_corners
            && self.track_skip_corner_ids.iter().all(|stored| {
                stored.map_or(false, |id| {
                    Judge::get_touches()
                        .iter()
                        .any(|touch| touch.id == id && touch.phase != TouchPhase::Ended)
                })
            });
        let active = alt_s_down || gp_skip_down || corners_alive || (all_four && !self.track_skip_active);
        let dt = get_frame_time().min(0.1);

        if active {
            self.track_skip_retracting = false;
            self.track_skip_from_corners = !alt_s_down && !gp_skip_down;
            if !self.track_skip_active {
                self.track_skip_active = true;
                self.track_skip_progress = 0.;
                self.track_skip_hold = 0.;
                if all_four {
                    self.track_skip_corner_ids = ids;
                }
            }
            self.track_skip_hold += dt;
            self.track_skip_progress = (self.track_skip_progress + dt / 0.3).min(1.);
            self.track_skip_countdown = (3. - self.track_skip_hold).max(0.);
            self.track_skip_white = (self.track_skip_hold / 3.).min(1.);
            if self.track_skip_countdown <= 0. {
                self.skip_done = true;
                self.track_skipped = true;
                self.track_skip_active = false;
                self.track_skip_retracting = false;
                self.track_skip_from_corners = false;
                self.track_skip_corner_ids = [None; 4];
                self.skip_transition_progress = 0.;
                self.skip_wait_timer = 0.;
                self.skip_fade_out_progress = 0.;
                self.skip_instant = false;
                if gp_skip_down {
                    self.track_skip_gp_release = true;
                }
                if !self.music.paused() {
                    self.music.pause()?;
                }
            }
        } else if self.track_skip_active {
            self.track_skip_retracting = true;
            self.track_skip_progress = (self.track_skip_progress - dt / 0.3).max(0.);
            if self.track_skip_progress <= 0. {
                self.track_skip_active = false;
                self.track_skip_retracting = false;
                self.track_skip_from_corners = false;
                self.track_skip_corner_ids = [None; 4];
                self.track_skip_hold = 0.;
                self.track_skip_countdown = 3.;
                self.track_skip_white = 0.;
            }
        }
        Ok(())
    }

    /// 娓哥帺鐣岄潰宸︿笂瑙掋€屾殏鍋滄寜閽€嶇殑鍏ュ満/閫€鍦鸿繘搴︼紙娓哥帺涓ǔ瀹氫负 1锛夈€?
    fn pause_button_progress(&self, tm: &TimeManager) -> f32 {
        if self.hud_preview {
            return 1.;
        }
        pause_button_progress(&self.state, tm.now(), self.res.track_length)
    }

    /// 鏆傚仠鎸夐挳锛堝乏涓婅鐨勫皬鏂瑰潡锛変腑蹇冧綅缃紝`p` 鐢?[`Self::pause_button_progress`] 缁欏嚭銆?
    fn pause_button_center(&self, p: f32) -> Point {
        pause_button_center(self.res.aspect_ratio, p)
    }

    fn ui(&mut self, ui: &mut Ui, tm: &mut TimeManager) -> Result<()> {
        let time = tm.now();
        let p = self.pause_button_progress(tm);
        let pause_center = self.pause_button_center(p);
        let res = &mut self.res;
        let eps = 2e-2 / res.aspect_ratio;
        let top = -1. / res.aspect_ratio;
        let pause_w = 0.015;
        let pause_h = pause_w * 3.2;
        let pause_btn_pressed = res.config.interactive
            && !self.skip_done
            && self.pause_rewind.is_none()
            && Judge::get_touches().iter().any(|touch| {
                touch.phase == TouchPhase::Started && {
                    let p = touch.position;
                    let p = Point::new(p.x, p.y);
                    (pause_center - p).norm() < 0.05
                }
            });
        if pause_btn_pressed {
            if !tm.paused() {
                let t = tm.now() as f32;
                if t - self.pause_first_time > PAUSE_CLICK_INTERVAL && res.config.double_click_to_pause {
                    self.pause_first_time = t;
                } else {
                    self.pause_first_time = f32::NEG_INFINITY;
                    if !self.music.paused() {
                        self.music.pause()?;
                    }
                    tm.pause();
                    suspend_sound();
                    self.pause_need_blur = true;
                    notify_paused(self.pause_notify.as_ref(), true);
                    #[cfg(target_env = "ohos")]
                    miniquad::native::set_interceptor_state(false);
                }
            }
        }
        ui.alpha(res.alpha, |ui| {
            ui.text("MAGIC BUGFIX TEXT").color(Color::new(0., 0., 0., 0.)).draw();
            if tm.now() as f32 - self.pause_first_time <= PAUSE_CLICK_INTERVAL {
                ui.fill_circle(pause_center.x, pause_center.y, 0.05, Color::new(1., 1., 1., 0.5));
            }

            // 鐗堟湰姘村嵃锛堢敱 GUI 寮€鍏尘带鍒讹級
            if res.config.show_watermark {
                let wm_label = format!("Phira-Vrenxz v{}", "1.3.3PRO");
                ui.text(&wm_label)
                    .pos(pause_center.x + 0.08, pause_center.y)
                    .anchor(0., 0.5)
                    .size(0.4)
                    .color(semi_white(0.6))
                    .draw();
            }

            // 瀹炴椂甯х巼锛氭樉绀哄湪鐗堟湰姘村嵃姝ｄ笅鏂癸紙鎸囨暟骞虫粦锛岀ǔ瀹氭樉绀猴級
            if res.config.show_fps && self.fps_smooth > 0.0 {
                let wm_label = format!("Phira-Vrenxz v{}", "1.3.3PRO");
                let wm_h = ui.text(&wm_label).size(0.4).measure().h;
                ui.text(format!("FPS {:.0}", self.fps_smooth))
                    .pos(pause_center.x + 0.08, pause_center.y + wm_h / 2. + 0.02)
                    .anchor(0., 0.)
                    .size(0.36)
                    .color(semi_white(0.85))
                    .draw();
            }

            let margin = 0.03;

            let legacy_aui = !res.info.use_attach_ui_fix.unwrap_or_default();
            let unit_h = if legacy_aui { ui.text("0").measure_using(&PGR_FONT).h } else { 0. };


            let h = 0.07;
            let score_top = top + eps * 2.2 - (1. - p) * 0.4 + res.config.score_offset_y;
            let score_right = 1. - margin + res.config.score_offset_x;
            // HUD 自定义：分数 + 准确率整块（右上角，可移动/缩放）。
            let score_slot = crate::hud::slot_or(ui, "game", "score", crate::hud::Cap(true, true, true), Rect::new(score_right - 0.75, score_top, 0.75, h + 0.3));
            let score_right = score_slot.right();
            let score_top = score_slot.y;
            let score = if res.config.roman_numerals {
                format_number(self.judge.score() as u32, true, false)
            } else if res.config.chinese_numerals {
                format_number(self.judge.score() as u32, false, true)
            } else {
                format!("{:07}", self.judge.score())
            };
            let base_score_size = 0.8;
            let score_size = if res.config.roman_numerals || res.config.chinese_numerals {
                let ref_width = ui.text("0000000").size(base_score_size).measure_using(&PGR_FONT).w;
                let actual_width = ui.text(&score).size(base_score_size).measure_using(&PGR_FONT).w;
                if actual_width > ref_width {
                    base_score_size * ref_width / actual_width
                } else {
                    base_score_size
                }
            } else {
                base_score_size
            };
            let scale_point = legacy_aui.then(|| {
                let ct = ui.text(&score).size(score_size).measure_using(&PGR_FONT).center();
                (score_right - ct.x, score_top + ct.y)
            });
            self.chart
                .with_element(ui, res, UIElement::Score, scale_point, (score_right, score_top), |ui, c| {
                    if res.config.show_score {
                        ui.text(&score)
                            .pos(score_right, score_top)
                            .anchor(1., 0.)
                            .size(score_size)
                            .color(c)
                            .draw_using(&PGR_FONT);
                    }
                    if res.config.show_acc {
                        ui.text(format!("{:05.2}%", self.judge.real_time_accuracy() * 100.))
                            .pos(score_right + (res.config.play_acc_offset_x - res.config.score_offset_x), score_top + h + res.config.play_acc_offset_y)
                            .anchor(1., 0.)
                            .size(0.4)
                            .color(Color { a: c.a * 0.7, ..c })
                            .draw_using(&PGR_FONT);
                    }
                });

            // HUD 自定义：暂停按钮（左上角，可移动/缩放；默认矩形以原 pause 中心为矩形中心，避免整体偏移）。
            let pause_slot = crate::hud::slot_or(ui, "game", "pause", crate::hud::Cap(true, true, true), Rect::new(pause_center.x - pause_w * 1.5, pause_center.y - pause_h / 2., pause_w * 3., pause_h));
            let pause_center_ed = pause_slot.center();
            self.chart.with_element(
                ui,
                res,
                UIElement::Pause,
                legacy_aui.then(|| (pause_center_ed.x, pause_center_ed.y)),
                (pause_center_ed.x - pause_w * 1.5, pause_center_ed.y - pause_h / 2.),
                |ui, c| {
                    let mut r = Rect::new(pause_center_ed.x - pause_w * 1.5, pause_center_ed.y - pause_h / 2., pause_w, pause_h);
                    ui.fill_rect(r, c);
                    r.x += pause_w * 2.;
                    ui.fill_rect(r, c);
                },
            );

            let is_autoplay = res.config.autoplay();

            if (self.judge.combo() >= 3 || is_autoplay) && res.config.show_combo && !self.hp_running {
                if legacy_aui {
                    let combo_top = top + eps * 2. - (1. - p) * 0.4 + res.config.combo_offset_y;
                    let btm = self
                        .chart
                        .with_element(ui, res, UIElement::ComboNumber, None, (res.config.combo_offset_x, combo_top + unit_h / 2.), |ui, c| {
                            if is_autoplay {
                                let text = &res.config.autoplay_display_text;
                                let size = 0.45;
                                let color = Color::new(1.0, 0.8, 0.2, c.a);
                                ui.text(text)
                                    .pos(0., combo_top)
                                    .anchor(0.5, 0.)
                                    .size(size)
                                    .color(color)
                                    .draw_using(&PGR_FONT)
                                    .bottom()
                            } else {
                                ui.text(format_number(self.judge.combo(), res.config.roman_numerals, res.config.chinese_numerals))
                                    .pos(0., combo_top)
                                    .anchor(0.5, 0.)
                                    .color(c)
                                    .draw_using(&PGR_FONT)
                                    .bottom()
                            }
                        });
                    let combo_top = btm + 0.01;

                    let combo_label = res.config.custom_combo_text();
                    let combo_display = if combo_label.is_empty() { "COMBO" } else { combo_label };

                    self.chart
                        .with_element(ui, res, UIElement::Combo, None, (res.config.combo_offset_x, combo_top + unit_h * 0.2), |ui, c| {
                            ui.text(if is_autoplay { "" } else { combo_display })
                                .pos(res.config.combo_offset_x, combo_top)
                                .anchor(0.5, 0.)
                                .size(0.4)
                                .color(c)
                                .draw_using(&PGR_FONT);
                        });
                } else {
                    if is_autoplay {
                        let text = &res.config.autoplay_display_text;
                        let size = 0.6;
                        let color = Color::new(1.0, 0.8, 0.2, res.alpha);
                        let text_r = ui.text(text).size(size).measure();
                        let y_pos = top + eps * 2. - (1. - p) * 0.4 + text_r.center().y + res.config.combo_offset_y;
                        self.chart.with_element(ui, res, UIElement::ComboNumber, None, (res.config.combo_offset_x, y_pos), |ui, c| {
                            ui.text(text)
                                .pos(res.config.combo_offset_x, y_pos)
                                .anchor(0.5, 0.5)
                                .size(size)
                                .color(color)
                                .draw_using(&PGR_FONT);
                        });
                    } else {
                        let combo = format_number(self.judge.combo(), res.config.roman_numerals, res.config.chinese_numerals);
                        let ct = ui.text(&combo).size(1.0).measure().center();
                        let combo_y = top + eps * 2. - (1. - p) * 0.4 + ct.y + res.config.combo_offset_y;
                        let btm = self.chart.with_element(ui, res, UIElement::ComboNumber, None, (res.config.combo_offset_x, combo_y), |ui, c| {
                            ui.text(&combo)
                                .pos(res.config.combo_offset_x, combo_y)
                                .anchor(0.5, 0.5)
                                .size(1.0)
                                .color(c)
                                .draw_using(&PGR_FONT)
                                .bottom()
                        });
                        let ct = ui.text("COMBO").size(0.4).measure().center();
                        let combo_top = btm + 0.01 + ct.y;

                        let combo_label = res.config.custom_combo_text();
                        let combo_display = if combo_label.is_empty() { "COMBO" } else { combo_label };

                        self.chart.with_element(ui, res, UIElement::Combo, None, (res.config.combo_offset_x, combo_top), |ui, c| {
                            ui.text(combo_display)
                                .pos(res.config.combo_offset_x, combo_top)
                                .anchor(0.5, 0.5)
                                .size(0.4)
                                .color(c)
                                .draw_using(&PGR_FONT);
                        });
                    }
                }
            }
            // [LIFE 顶替] Phigros SecretChallengeLife 样式：LIFE 数字在上、"LIFE" 标签在下（原 combo 区域，屏幕中上）
            // 仅 HP mod（血条 mod）开启时显示；关闭后血条与 LIFE 一并隐藏
            if self.hp_running {
                let life_num = self.hp.ceil().max(0.0) as i32;
                let life_x = 0.0;
                let life_y = top + eps * 2. - (1. - p) * 0.4 + res.config.combo_offset_y;
                // 数字底在 life_y（anchor y=0），LIFE 标签紧贴数字底部（原版数字/标签贴近）
                let num_size = 1.0;
                let num_h = ui.text(&life_num.to_string()).size(num_size).measure_using(&PGR_FONT).h;
                // 扣血红闪：life_flash 从 0.15 衰减到 0，k 从 1.0 渐变到 0.0（完全红 → 黑）
                let k = (self.life_flash / 0.15).min(1.0);
                draw_life_text(ui, &life_num.to_string(), life_x, life_y, num_size, 0.5, 0.0, k);
                // LIFE 标签与 combo 同字号（0.4），紧贴数字底部
                draw_life_text(ui, "LIFE", life_x, life_y + num_h + 0.02, 0.4, 0.5, 0.0, k);
            }
            ui.text("").draw_using(&PGR_FONT);
            let lf = -1. + margin;
            let bt = -top - eps * 2.8 + (1. - p) * 0.4;
            // HUD 自定义：曲名（左下，可移动/缩放；默认矩形基于原锚点）。受谱面信息开关控制。
            if res.config.show_chart_info {
                let name_slot = crate::hud::slot_or(ui, "game", "name", crate::hud::Cap(true, true, true), Rect::new(lf, bt - 0.55, 0.9, 0.55));
                let lf = name_slot.x;
                let bt = name_slot.bottom();
                let scale_point = legacy_aui.then(|| {
                    let ct = ui.text(&res.info.name).size(0.5).measure().center();
                    (lf + ct.x, bt - ct.y)
                });
                self.chart.with_element(ui, res, UIElement::Name, scale_point, (lf, bt), |ui, c| {
                    ui.text(&res.info.name)
                        .pos(lf, bt)
                        .anchor(0., 1.)
                        .size(0.5)
                        .color(c)
                        .max_width(0.8)
                        .draw();
                });

                // HUD 自定义：等级（右下，可移动/缩放；默认矩形基于原锚点）。
                let level_slot = crate::hud::slot_or(ui, "game", "level", crate::hud::Cap(true, true, true), Rect::new(-lf - 0.9, bt - 0.55, 0.9, 0.55));
                let lvl_x = level_slot.right();
                let lvl_bt = level_slot.bottom();
                let scale_point = legacy_aui.then(|| {
                    let ct = ui.text(&res.info.level).size(0.5).measure().center();
                    (lvl_x - ct.x, lvl_bt - ct.y)
                });
                self.chart.with_element(ui, res, UIElement::Level, scale_point, (lvl_x, lvl_bt), |ui, c| {
                    ui.text(&res.info.level).pos(lvl_x, lvl_bt).anchor(1., 1.).size(0.5).color(c).draw();
                });
            }

            let hw = 0.003;
            let height = eps * 1.0;
            let dest = (2. * res.time / res.track_length).clamp(0., 2.) as f32;
            // HUD 自定义：底部进度条（整条，可移动/缩放；默认矩形基于原位置）。
            let bar_slot = crate::hud::slot_or(ui, "game", "bar", crate::hud::Cap(true, true, true), Rect::new(-1., top, 2., height));
            let bar_x = bar_slot.x;
            let bar_y = bar_slot.y;
            let bar_w = bar_slot.w;
            self.chart
                .with_element(ui, res, UIElement::Bar, Some((bar_x, bar_y + bar_slot.h / 2.)), (bar_x, bar_y + bar_slot.h / 2.), |ui, color| {
                    let fill = (dest / 2.) * bar_w;
                    ui.fill_rect(Rect::new(bar_x, bar_y, fill, bar_slot.h), semi_white(0.6));
                    ui.fill_rect(Rect::new(bar_x + fill - hw, bar_y, hw * 2., bar_slot.h), WHITE);
                });
        });
        Ok(())
    }

    fn overlay_ui(&mut self, ui: &mut Ui, tm: &mut TimeManager) -> Result<()> {
        let res_off_x = self.res.config.result_offset_x;
        let res_off_y = self.res.config.result_offset_y;
        let c = semi_white(self.res.alpha);
        let res = &mut self.res;
        if self.pause_alpha > 0.001 {
            let h = 1. / res.aspect_ratio;
            // 楂樻柉妯＄硦鑳屾櫙锛氭殏鍋滄椂鎶撳彇鐢婚潰鍍忕礌锛岀敤 fastblur 鍋?CPU 绔珮鏂ā绯?
            #[cfg(target_os = "android")]
            {
                // Android 的 glReadPixels 抓帧会得到白屏，不做模糊抓帧；
                // 清掉可能残留的模糊纹理/离屏目标，避免复用到旧的白色画面。
                self.pause_blur_tex = None;
                self.pause_blur_rt = None;
                self.pause_need_blur = false;
            }
            if self.pause_need_blur && !cfg!(target_os = "android") {
                let vp = ui.viewport;
                let bw = (vp.2 as u32 / 8).max(1);
                let bh = (vp.3 as u32 / 8).max(1);
                // 鍒涘缓/澶嶇敤浣庡垎杈ㄧ巼 RenderTarget
                let need_new = self.pause_blur_rt.as_ref().map_or(true, |t| {
                    t.texture.width() as u32 != bw || t.texture.height() as u32 != bh
                });
                if need_new {
                    let gl = unsafe { get_internal_gl() };
                    let texture = miniquad::Texture::new_render_texture(
                        gl.quad_context,
                        miniquad::TextureParams {
                            width: bw,
                            height: bh,
                            format: miniquad::TextureFormat::RGBA8,
                            ..Default::default()
                        },
                    );
                    let render_pass = miniquad::RenderPass::new(gl.quad_context, texture, None);
                    self.pause_blur_rt = Some(RenderTarget {
                        texture: Texture2D::from_miniquad_texture(texture),
                        render_pass,
                    });
                }
                if let Some(rt) = &self.pause_blur_rt {
                    unsafe {
                        use miniquad::gl::*;
                        let mut current_fbo = 0;
                        glGetIntegerv(GL_FRAMEBUFFER_BINDING, &mut current_fbo);
                        // 浠庡綋鍓?framebuffer 缂╂斁鍒颁綆鍒嗚鲸鐜囩洰鏍?
                        glBindFramebuffer(GL_READ_FRAMEBUFFER, current_fbo as u32);
                        glBindFramebuffer(GL_DRAW_FRAMEBUFFER, internal_id(*rt));
                        glBlitFramebuffer(
                            0, 0, vp.2, vp.3,
                            0, 0, bw as i32, bh as i32,
                            GL_COLOR_BUFFER_BIT, GL_LINEAR,
                        );
                        glBindFramebuffer(GL_FRAMEBUFFER, current_fbo as u32);
                        // 璇诲彇浣庡垎杈ㄧ巼鍍忕礌
                        let mut pixels = vec![0u8; (bw * bh * 3) as usize];
                        glBindFramebuffer(GL_FRAMEBUFFER, internal_id(*rt));
                        // 鍏抽敭锛氳缃瀵归綈涓?瀛楄妭锛屽惁鍒?glReadPixels 榛樿4瀛楄妭瀵归綈
                        // 褰?bw*3 涓嶆槸4鐨勫€嶆暟鏃朵細瓒婄晫鍐欏叆锛屽鑷村爢鎹熷潖 (0xc0000374)
                        const GL_PACK_ALIGNMENT: u32 = 0x0D05;
                        let mut old_pack_alignment = 4;
                        glGetIntegerv(GL_PACK_ALIGNMENT, &mut old_pack_alignment);
                        glPixelStorei(GL_PACK_ALIGNMENT, 1);
                        glReadPixels(0, 0, bw as i32, bh as i32, GL_RGB, GL_UNSIGNED_BYTE, pixels.as_mut_ptr() as *mut _);
                        glPixelStorei(GL_PACK_ALIGNMENT, old_pack_alignment);
                        glBindFramebuffer(GL_FRAMEBUFFER, current_fbo as u32);
                        // 涓婁笅缈昏浆
                        let mut flipped = vec![0u8; (bw * bh * 3) as usize];
                        let row_size = (bw * 3) as usize;
                        for y in 0..bh as usize {
                            let src = (bh as usize - 1 - y) * row_size;
                            let dst = y * row_size;
                            flipped[dst..dst + row_size].copy_from_slice(&pixels[src..src + row_size]);
                        }
                        // 楂樻柉妯＄硦锛堜綆鍒嗚鲸鐜囷紝鍗婂緞灏忎竴鐐规晥鏋滀竴鏍凤級
                        let mut pixel_data: Vec<[u8; 3]> = flipped.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
                        fastblur::gaussian_blur(&mut pixel_data, bw as _, bh as _, 8.0);
                        // 杞崲涓?RGBA 鍒涘缓绾圭悊
                        let mut rgba = Vec::with_capacity((bw * bh * 4) as usize);
                        for p in &pixel_data {
                            rgba.push(p[0]);
                            rgba.push(p[1]);
                            rgba.push(p[2]);
                            rgba.push(255);
                        }
                        self.pause_blur_tex = Some(Texture2D::from_rgba8(bw as u16, bh as u16, &rgba));
                    }
                }
                self.pause_need_blur = false;
            }
            if let Some(tex) = self.pause_blur_tex {
                let bg_rect = Rect::new(-1., -h, 2., h * 2.);
                ui.fill_rect(bg_rect, (tex, bg_rect, ScaleType::Fit, Color::new(1., 1., 1., 0.7 * self.pause_alpha)));
            } else {
                #[cfg(target_os = "android")]
                {
                    // Android 拿不到可用的帧截图（glReadPixels 出来是白的），不能靠上一帧缓冲当背景。
                    // 这里自己铺谱面背景图，再压一层浓黑，保证暂停层绝不会是纯白。
                    let bg_rect = Rect::new(-1., -h, 2., h * 2.);
                    ui.fill_rect(
                        bg_rect,
                        (*res.background, bg_rect, ScaleType::Fit, Color::new(1., 1., 1., self.pause_alpha)),
                    );
                    draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., 0.8 * self.pause_alpha));
                }
                #[cfg(not(target_os = "android"))]
                {
                    draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., 0.5 * self.pause_alpha));
                }
            }

            let old_alpha = ui.alpha;
            ui.alpha *= self.pause_alpha;



            // ===== 缁熻闈㈡澘锛堟潵鑷?ending.rs 鏍峰紡锛?====
            let result = self.judge.result();
            let max_combo = result.max_combo;
            let counts = result.counts;
            // 瀹炴椂鍑嗙‘鐜? (P + G*0.65) / (P+G+B+M)锛屽垎姣嶄负0鏃?00%
            let judged = counts[0] + counts[1] + counts[2] + counts[3];
            let accuracy = if judged == 0 {
                1.0
            } else {
                (counts[0] as f64 + counts[1] as f64 * 0.65) / judged as f64
            };
            let early = result.early;
            let late = result.late;

            let panel_w = 0.9;
            let panel_h = 0.13;
            // 缁撶畻缁熻闈㈡澘鏁翠綋鍙皟鍋忕Щ
            let panel_x = -panel_w / 2. + res_off_x;
            let panel_gap = 0.04;

            // 闈㈡澘1锛歁ax Combo / Accuracy
            let s1_y = 0.16 + res_off_y;
            let s1 = Rect::new(panel_x, s1_y, panel_w, panel_h);
            ui.fill_path(&s1.rounded(0.01), Color::new(0., 0., 0., 0.4));
            {
                let dy = 0.02;
                ui.text(tl!("pause-max-combo"))
                    .pos(s1.x + 0.04, s1.bottom() - dy)
                    .anchor(0., 1.)
                    .size(0.28)
                    .color(semi_white(0.8))
                    .draw();
                ui.text(&max_combo.to_string())
                    .pos(s1.x + 0.04, s1.y + 0.02)
                    .anchor(0., 0.)
                    .size(0.55)
                    .color(WHITE)
                    .draw();
                ui.text(tl!("pause-accuracy"))
                    .pos(s1.right() - 0.04, s1.bottom() - dy)
                    .anchor(1., 1.)
                    .size(0.28)
                    .color(semi_white(0.8))
                    .draw();
                ui.text(&format!("{:.2}%", accuracy * 100.))
                    .pos(s1.right() - 0.04, s1.y + 0.02)
                    .anchor(1., 0.)
                    .size(0.55)
                    .color(WHITE)
                    .draw();
            }

            // 闈㈡澘2锛歅erfect/Good/Bad/Miss + Early/Late
            let s2_y = s1_y + panel_h + panel_gap;
            let s2 = Rect::new(panel_x, s2_y, panel_w, panel_h);
            ui.fill_path(&s2.rounded(0.01), Color::new(0., 0., 0., 0.4));
            {
                let dy = 0.02;
                let dy2 = 0.012;
                let big = 0.45;
                let sm = 0.22;
                let draw_count = |ui: &mut Ui, x: f32, name: &str, count: u32| {
                    ui.text(name)
                        .pos(x, s2.bottom() - dy)
                        .anchor(0.5, 1.)
                        .size(sm)
                        .color(semi_white(0.8))
                        .draw();
                    ui.text(&count.to_string())
                        .pos(x, s2.y + dy2)
                        .anchor(0.5, 0.)
                        .size(big)
                        .color(WHITE)
                        .draw();
                };
                draw_count(ui, s2.x + s2.w * 0.12, &tl!("judge-perfect").into_owned(), counts[0]);
                draw_count(ui, s2.x + s2.w * 0.30, &tl!("judge-good").into_owned(), counts[1]);
                draw_count(ui, s2.x + s2.w * 0.44, &tl!("judge-bad").into_owned(), counts[2]);
                draw_count(ui, s2.x + s2.w * 0.58, &tl!("judge-miss").into_owned(), counts[3]);
                // Early / Late
                let l = s2.x + s2.w * 0.72;
                let rt = s2.x + s2.w * 0.94;
                let cy = s2.center().y;
                ui.text(tl!("pause-early"))
                    .pos(l, cy - dy2 / 2.)
                    .anchor(0., 1.)
                    .size(0.24)
                    .color(semi_white(0.8))
                    .draw();
                ui.text(&early.to_string())
                    .pos(rt, cy - dy2 / 2.)
                    .anchor(1., 1.)
                    .size(0.24)
                    .color(WHITE)
                    .draw();
                ui.text(tl!("pause-late"))
                    .pos(l, cy + dy2 / 2.)
                    .anchor(0., 0.)
                    .size(0.24)
                    .color(semi_white(0.8))
                    .draw();
                ui.text(&late.to_string())
                    .pos(rt, cy + dy2 / 2.)
                    .anchor(1., 0.)
                    .size(0.24)
                    .color(WHITE)
                    .draw();
            }

            // 课题模式与 NoRetry 一致：不可重试
            let no_retry = matches!(
                self.mode,
                GameMode::NoRetry | GameMode::Challenge | GameMode::ReChallenge
            );
            let alpha = res.alpha;
            let btn_s = 0.06f32;
            // 暂停界面显示课题模式标识
            if let Some(cfg) = &self.challenge {
                let label = if cfg.re { "Re" } else { "课题" };
                let top = if matches!(self.mode, GameMode::Exercise | GameMode::TweakOffset) {
                    -0.3f32
                } else {
                    0f32
                };
                ui.text(&format!("{}-{}/{}", label, cfg.index + 1, cfg.total))
                    .pos(0.0, top + 0.185)
                    .anchor(0.5, 0.5)
                    .size(0.34)
                    .color(Color::new(0.86, 0.62, 1.0, 1.0 * alpha))
                    .draw();
            }
            // 鎸夐挳闂磋窛瀵归綈 phire锛坵 = 0.05锛夛細涓夊浘鏍囦腑蹇冮棿璺?= 2s + w = 0.17
            let btn_gap = 0.05f32;
            // 缁冧範 / 璋冩暣鍋忕Щ妯″紡涓嬫暣鎺掓寜閽笅绉伙紝閬垮紑閫熷害婊戞潯涓庣粌涔犺繘搴︽潯锛堝榻?phire 鐨?o = -0.3锛?
            let btn_y = if matches!(self.mode, GameMode::Exercise | GameMode::TweakOffset) { -0.3f32 } else { 0f32 };
            let btn_icons_arr = [*res.icon_back, *res.icon_retry, *res.icon_resume];
            let mut btn_disabled = [false, no_retry, self.dead];
            let mut btn_rects = [Rect::default(); 3];
            let spacing = btn_s * 2. + btn_gap;
            for i in 0..3 {
                let x = (i as f32 - 1.) * spacing;
                let r = Rect::new(x - btn_s, btn_y - btn_s, btn_s * 2., btn_s * 2.);
                btn_rects[i] = r;
                let color = if btn_disabled[i] { semi_white(alpha * 0.3) } else { semi_white(alpha) };
                let icon_r = r.feather(0.012);
                // 鎵嬪姩璁＄畻淇濇寔绾圭悊姣斾緥鐨勭粯鍒跺尯鍩燂紝纭繚涓嶆媺浼?
                let tex = btn_icons_arr[i];
                let tex_ratio = tex.width() / tex.height();
                let draw_r = if tex_ratio > 1. {
                    let h = icon_r.w / tex_ratio;
                    Rect::new(icon_r.x, icon_r.y + (icon_r.h - h) / 2., icon_r.w, h)
                } else {
                    let w = icon_r.h * tex_ratio;
                    Rect::new(icon_r.x + (icon_r.w - w) / 2., icon_r.y, w, icon_r.h)
                };
                ui.fill_rect(draw_r, (tex, draw_r, ScaleType::Fit, color));
            }

            if res.config.interactive {
                let mut clicked = None;
                for touch in Judge::get_touches() {
                    if touch.phase != TouchPhase::Started {
                        continue;
                    }
                    let p = touch.position;
                    let p = Point::new(p.x, p.y);
                    for i in 0..3 {
                        let br = btn_rects[i];
                        if p.x >= br.x && p.x <= br.right() && p.y >= br.y && p.y <= br.bottom() {
                            clicked = Some(i as i32 - 1);
                            break;
                        }
                    }
                }
                if no_retry && clicked == Some(0) || self.dead && clicked == Some(1) {
                    clicked = None;
                }
                let mut pos = self.music.position();
                if self.mode == GameMode::Exercise {
                    pos = tm.now();
                }
                if clicked.is_some_and(|it| it != -1) && (tm.speed - res.config.speed as f64).abs() > 0.01 {
                    debug!("recreating music");
                    self.music = res.audio.create_music(
                        res.music.clone(),
                        MusicParams {
                            amplifier: res.config.volume_music as _,
                            playback_rate: res.config.speed as _,
                            ..Default::default()
                        },
                    )?;
                }
                match clicked {
                    Some(-1) => {
                        back_sound();
                        self.should_exit = true;
                        #[cfg(target_env = "ohos")]
                        miniquad::native::set_interceptor_state(false);
                    }
                    Some(0) => {
                        reset!(self, res, tm);
                        if self.mode == GameMode::Exercise {
                            self.judge.advance_to(&mut self.chart, self.exercise_range.start);
                        }
                        #[cfg(target_env = "ohos")]
                        miniquad::native::set_interceptor_state(true);
                    }
                    Some(1) => {
                        {
                            if self.mode == GameMode::Exercise && (tm.now() > self.exercise_range.end || tm.now() < self.exercise_range.start) {
                                tm.seek_to(self.exercise_range.start);
                                self.music.seek_to(self.exercise_range.start)?;
                                pos = self.exercise_range.start;
                            }
                            self.music.play()?;
                            // 缁х画鏃跺€掑甫 1 绉掞紙瀵归綈 phire锛夛紝缁欏洖涓€鐐圭紦鍐?
                            res.time -= PAUSE_REWIND_DURATION;
                            let dst = pos - PAUSE_REWIND_DURATION;
                            if dst < 0. {
                                self.music.pause()?;
                                self.state = State::BeforeMusic;
                            } else {
                                self.music.seek_to(dst)?;
                            }
                            let now = tm.now();
                            tm.speed = res.config.speed as _;
                            tm.resume();
                            tm.seek_to(now - PAUSE_REWIND_DURATION);
                            self.pause_rewind = Some(tm.now());
                            // 澶氫汉姝ｅ紡娓哥帺锛氬箍鎾€滃凡缁х画鈥?
                            notify_paused(self.pause_notify.as_ref(), false);
                            #[cfg(target_env = "ohos")]
                            miniquad::native::set_interceptor_state(true);
                        }
                    }
                    _ => {}
                }
            }
            if self.mode == GameMode::Exercise {
                let asp = self.touch_scale();
                for touch in ui.ensure_touches() {
                    touch.position *= asp;
                }
                // 灏嗙粌涔犳ā寮忔帶鍒跺厓绱犳斁鍦ㄦ挱鏀炬寜閽笅鏂癸紝閬垮厤涓庣粺璁￠潰鏉垮拰璋遍潰閲嶅彔
                ui.dy(-0.22);
                ui.scope(|ui| {
                    ui.dx(0.3);
                    ui.dy(-0.28);
                    ui.slider(tl!("speed"), 0.5..2.0, 0.05, &mut self.res.config.speed, Some(0.5));
                });
                ui.dy(0.06);
                let hw = 0.7;
                let h = 0.06;
                let eh = 0.12;
                let rad = 0.03;
                let sp = self.offset().min(0.) as f64;
                ui.fill_rect(Rect::new(-hw, -h, hw * 2., h * 2.), GRAY);
                let st = -hw + ((self.exercise_range.start - sp) / (self.res.track_length - sp)) as f32 * hw * 2.;
                let en = -hw + ((self.exercise_range.end - sp) / (self.res.track_length - sp)) as f32 * hw * 2.;
                let t = tm.now();
                let cur = -hw + ((t - sp) / (self.res.track_length - sp)) as f32 * hw * 2.;
                ui.fill_rect(Rect::new(st, -h, en - st, h * 2.), WHITE);
                ui.fill_rect(Rect::new(st, -eh, 0., eh + h).feather(0.005), BLUE);
                ui.fill_circle(st, -eh, rad, BLUE);
                if self.exercise_press.is_none() {
                    let r = ui.rect_to_global(Rect::new(st, -eh, 0., 0.).feather(rad));
                    self.exercise_press = Judge::get_touches()
                        .iter()
                        .find(|it| it.phase == TouchPhase::Started && r.contains(it.position))
                        .map(|it| (-1, it.id));
                }
                ui.fill_rect(Rect::new(en, -h, 0., eh + h).feather(0.005), RED);
                ui.fill_circle(en, eh, rad, RED);
                if self.exercise_press.is_none() {
                    let r = ui.rect_to_global(Rect::new(en, eh, 0., 0.).feather(rad));
                    self.exercise_press = Judge::get_touches()
                        .iter()
                        .find(|it| it.phase == TouchPhase::Started && r.contains(it.position))
                        .map(|it| (1, it.id));
                }
                ui.fill_rect(Rect::new(cur, -h, 0., h * 2.).feather(0.005), GREEN);
                ui.fill_circle(cur, 0., rad, GREEN);
                if self.exercise_press.is_none() {
                    let r = ui.rect_to_global(Rect::new(cur, 0., 0., 0.).feather(rad));
                    self.exercise_press = Judge::get_touches()
                        .iter()
                        .find(|it| it.phase == TouchPhase::Started && r.contains(it.position))
                        .map(|it| (0, it.id));
                }
                ui.text(fmt_time(t as f32)).pos(0., 0.14).anchor(0.5, 0.).size(0.8).color(Color::new(1., 1., 1., 0.5)).draw();
                if let Some((ctrl, id)) = &self.exercise_press {
                    if let Some(touch) = Judge::get_touches().iter().rfind(|it| it.id == *id) {
                        let x = touch.position.x;
                        let p = (x + hw) as f64 / (hw * 2.) as f64 * (self.res.track_length - sp) + sp;
                        let p = if self.res.track_length - sp <= 3. || *ctrl == 0 {
                            p.clamp(sp, self.res.track_length)
                        } else {
                            p.clamp(
                                if *ctrl == -1 { sp } else { self.exercise_range.start + 3. },
                                if *ctrl == -1 {
                                    self.exercise_range.end - 3.
                                } else {
                                    self.res.track_length
                                },
                            )
                        };
                        if *ctrl == 0 {
                            tm.seek_to(p);
                            self.music.seek_to(p)?;
                            self.bad_notes.clear();
                            self.judge.reset();
                            self.chart.reset();
                            self.res.judge_line_color = self.res.res_pack.info.color_perfect();
                        } else {
                            *(if *ctrl == -1 {
                                &mut self.exercise_range.start
                            } else {
                                &mut self.exercise_range.end
                            }) = p;
                        }
                        if matches!(touch.phase, TouchPhase::Cancelled | TouchPhase::Ended) {
                            self.exercise_press = None;
                        }
                    }
                }
                ui.dy(-0.14);
                let r = ui.text(tl!("to")).size(0.8).anchor(0.5, 0.).draw();
                let mut tx = ui
                    .text(fmt_time(self.exercise_range.start as f32))
                    .pos(r.x - 0.02, 0.)
                    .anchor(1., 0.)
                    .size(0.8)
                    .color(BLACK);
                let re = tx.measure();
                self.exercise_btns.0.set(tx.ui, re);
                tx.ui
                    .fill_rect(re.feather(0.01), Color::new(1., 1., 1., if self.exercise_btns.0.touching() { 0.5 } else { 1. }));
                tx.draw();

                let mut tx = ui
                    .text(fmt_time(self.exercise_range.end as f32))
                    .pos(r.right() + 0.02, 0.)
                    .size(0.8)
                    .color(BLACK);
                let re = tx.measure();
                self.exercise_btns.1.set(tx.ui, re);
                tx.ui
                    .fill_rect(re.feather(0.01), Color::new(1., 1., 1., if self.exercise_btns.1.touching() { 0.5 } else { 1. }));
                tx.draw();
                for touch in ui.ensure_touches() {
                    touch.position /= asp;
                }
            }
            ui.alpha = old_alpha;
        }
        if let Some(time) = self.pause_rewind {
            // 鍊掑甫鏈熼棿鐨勫€掕鏃舵贰鍑猴細3 绉掑唴浠?0.6 涓嶉€忔槑搴︾嚎鎬у綊闆讹紝姝ｄ腑鏄剧ず鍓╀綑绉掓暟锛? / 2 / 1锛?
            let dt = tm.now() - time;
            let t = PAUSE_REWIND_DURATION - dt;
            if t <= 0. {
                self.pause_rewind = None;
            } else {
                let a = (t / PAUSE_REWIND_DURATION).clamp(0., 1.) * PAUSE_BACKGROUND_ALPHA as f64;
                let h = 1. / self.res.aspect_ratio;
                draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., a as f32));
                ui.text((t.ceil() as i32).to_string()).anchor(0.5, 0.5).size(1.).color(c).draw();
            }
        }
        // [谱面揭晓] Reveal 下 UI 触摸反馈须与谱面同倍缩小：触摸坐标是窗口 NDC，
        // 谱面经相机 + model_stack 双重缩放 1/REVEAL_ZOOM_OUT，UI 反馈宽度要等于
        // 谱面宽度，故坐标与半径一并乘缩放系数（Normal 等模式系数为 1 保持原样）。
        let touch_ui_scale = if self.mode == GameMode::Reveal { 1. / REVEAL_ZOOM_OUT } else { 1. };
        if self.res.config.touch_debug {
            for touch in Judge::get_touches() {
                ui.fill_circle(
                    touch.position.x * touch_ui_scale,
                    touch.position.y * touch_ui_scale,
                    0.04 * touch_ui_scale,
                    Color { a: 0.4, ..RED },
                );
            }
        }
        for pos in &self.touch_points {
            ui.fill_circle(
                pos.0 * touch_ui_scale,
                pos.1 * touch_ui_scale,
                0.04 * touch_ui_scale,
                Color { a: 0.4, ..BLUE },
            );
        }
        Ok(())
    }

    fn interactive(res: &Resource, state: &State) -> bool {
        res.config.interactive && matches!(state, State::Playing)
    }

    fn offset(&self) -> f32 {
        self.chart.offset + self.res.config.offset + self.info_offset
    }

    fn tweak_offset(&mut self, ui: &mut Ui, ita: bool) {
        ui.scope(|ui| {
            let width = 0.55;
            let height = 0.4;
            ui.dx(1. - width - 0.02);
            ui.dy(ui.top - height - 0.02);
            ui.fill_rect(Rect::new(0., 0., width, height), GRAY);
            ui.dy(0.02);
            ui.text(tl!("adjust-offset")).pos(width / 2., 0.).anchor(0.5, 0.).size(0.7).draw();
            ui.dy(0.16);
            let r = ui
                .text(format!("{}ms", (self.info_offset * 1000.).round() as i32))
                .pos(width / 2., 0.)
                .anchor(0.5, 0.)
                .size(0.6)
                .no_baseline()
                .draw();
            let d = 0.14;
            if ui.button("lg_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.026), "-") && ita {
                self.info_offset -= 0.05;
            }
            if ui.button("lg_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.026), "+") && ita {
                self.info_offset += 0.05;
            }
            let d = 0.08;
            if ui.button("sm_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.022), "-") && ita {
                self.info_offset -= 0.005;
            }
            if ui.button("sm_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.022), "+") && ita {
                self.info_offset += 0.005;
            }
            let d = 0.03;
            if ui.button("ti_sub", Rect::new(d, r.center().y, 0., 0.).feather(0.017), "-") && ita {
                self.info_offset -= 0.001;
            }
            if ui.button("ti_add", Rect::new(width - d, r.center().y, 0., 0.).feather(0.017), "+") && ita {
                self.info_offset += 0.001;
            }
            ui.dy(0.14);
            let pad = 0.02;
            let spacing = 0.01;
            let mut r = Rect::new(pad, 0., (width - pad * 2. - spacing * 2.) / 3., 0.06);
            if ui.button("cancel", r, tl!("offset-cancel")) {
                self.next_scene = Some(NextScene::PopWithResult(Box::new(None::<f32>)));
            }
            r.x += r.w + spacing;
            if ui.button("reset", r, tl!("offset-reset")) {
                self.info_offset = 0.;
            }
            r.x += r.w + spacing;
            if ui.button("save", r, tl!("offset-save")) {
                self.next_scene = Some(NextScene::PopWithResult(Box::new(Some(self.info_offset))));
            }
        });
    }

    pub fn get_avg_fps(&self) -> Option<f32> {
        if self.fps_frame_count > 0 && self.fps_total_time > 0.0 {
            Some(self.fps_frame_count as f32 / self.fps_total_time as f32)
        } else {
            None
        }
    }


    fn finish_and_show_result(&mut self) -> Result<()> {
        // 璋遍潰棰勮锛氳嚜鐒舵挱瀹屼篃涓嶈繘 EndingScene 缁撶畻椤点€佷笉鍐欒褰曪紝鐩存帴缁撴潫杩斿洖
        // [Reveal] Play finish exits directly: no EndingScene, no record, just pop back.
        if self.preview_mode || matches!(self.mode, GameMode::Reveal | GameMode::Replay) {
            self.should_exit = true;
            return Ok(());
        }
        let replay_result = {
            let r = self.judge.result();
            Some(crate::replay::ReplayResult {
                score: r.score,
                accuracy: r.accuracy,
                max_combo: r.max_combo,
                counts: r.counts,
                full_combo: r.max_combo == r.num_of_notes && r.num_of_notes > 0,
                early: r.early,
                late: r.late,
            })
        };
        if let Some(path) = crate::replay::finish(replay_result) {
            info!("replay saved: {}", path.display());
        }
        let mut record_data = None;
        if !self.track_skipped {
            if let Some(upload_fn) = &self.upload_fn {
                if !self.res.config.offline_mode
                    && !self.res.config.mods.intersects(Mods::UNRATED)
                    && !self.res.config.use_keyboard
                    && self.res.config.speed >= 1.0 - 1e-3
                {
                    if let Some(player) = &self.player {
                        if let Some(chart) = &self.res.info.id {
                            record_data = Some(encode_record(self, player.id, *chart));
                        }
                    }
                }
            }
        }
        let result = self.judge.result();
        let record = if self.track_skipped
            || self.dead
            || self.res.config.mods.intersects(Mods::UNRATED)
            || self.res.config.speed < 1.0 - 1e-3
        {
            None
        } else {
            Some(SimpleRecord {
                score: result.score as _,
                accuracy: result.accuracy as _,
                full_combo: result.max_combo == result.num_of_notes,
            })
        };
        self.next_scene = match self.mode {
            GameMode::Normal | GameMode::NoRetry | GameMode::View | GameMode::Reveal => {
                let historic_best = self.player.as_ref().map_or(0, |it| it.historic_best);
                if let Some(new_rec) = &record {
                    // 鏈眬鑷劧瀹屾垚涓旀垚缁╂湁鏁堬細鎶婂畬鏁寸粨绠椾氦缁?SaveFn锛堝浜洪潰鏉挎嵁姝や笂鎶ュ畬鎴愯€岄潪鏀惧純锛?
                    if let Some(f) = &self.save_fn {
                        f(FinishedStats {
                            score: result.score,
                            accuracy: result.accuracy as f32,
                            full_combo: new_rec.full_combo,
                            max_combo: result.max_combo,
                            perfect: result.counts[0],
                            good: result.counts[1],
                            bad: result.counts[2],
                            miss: result.counts[3],
                        })?;
                    }
                    if let Some(best) = &mut self.best_record {
                        best.update(new_rec);
                    } else {
                        self.best_record = record.clone();
                    }
                    if let Some(best) = &self.best_record {
                        if let Some(player) = &mut self.player {
                            player.historic_best = player.historic_best.max(best.score as _);
                        }
                    }
                }
                Some(NextScene::Overlay(Box::new(EndingScene::new(
                    self.res.background.clone(),
                    self.res.illustration.clone(),
                    self.res.player.clone(),
                    self.res.icons.clone(),
                    self.res.arc_icon.clone(),
                    self.res.icon_retry.clone(),
                    self.res.icon_proceed.clone(),
                    self.res.mod_icons.clone(),
                    self.res.info.clone(),
                    self.judge.result(),
                    &self.res.config,
                    self.res.res_pack.ending.clone(),
                    self.upload_fn.as_ref().map(Arc::clone),
                    self.player.as_ref().map(|it| it.rks),
                    historic_best,
                    record_data,
                    self.best_record.clone(),
                    if self.res.config.show_avg_fps { self.get_avg_fps() } else { None },
                    None, // 普通游玩无课题结算数据
                )?)))
            }
            GameMode::Challenge | GameMode::ReChallenge => {
                // 课题局自然完成：写入成绩推进会话。
                // 还有下一局 → 直接启动下一局加载页（不进课题页，HP 继承上一局剩余值）；
                // 全部完成 → 进课题结算（原结算界面 + 所选谱面记录 + 总分色档评分）。
                if !self.challenge_finished {
                    self.finish_challenge_round();
                    if let Some(cfg) = self.challenge.clone() {
                        let (completed, total, aborted) = {
                            let s = cfg.session.lock().unwrap();
                            (s.completed_rounds, s.paths.len(), s.aborted)
                        };
                        if aborted || completed >= total {
                            let ending = self.build_challenge_ending(&cfg)?;
                            Some(NextScene::Replace(Box::new(ending)))
                        } else {
                            match self.build_challenge_next(&cfg, completed) {
                                Ok(task) => {
                                    self.challenge_next = task;
                                    None // 加载完成经 next_scene() 直接切到下一局
                                }
                                Err(err) => {
                                    eprintln!("[challenge] 课题下一局启动失败: {err:?}");
                                    self.should_exit = true;
                                    Some(NextScene::Pop)
                                }
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            GameMode::TweakOffset => Some(NextScene::PopWithResult(Box::new(None::<f32>))),
            GameMode::Replay | GameMode::Tutorial | GameMode::Lan => Some(NextScene::Pop),
            GameMode::Exercise => None,
        };
        Ok(())
    }

    /// [课题模式] 单局自然完成：写入成绩与推进已完局数到共享会话
    fn finish_challenge_round(&mut self) {
        if let Some(cfg) = &self.challenge {
            if !self.challenge_finished {
                let result = self.judge.result();
                if let Ok(mut s) = cfg.session.lock() {
                    if !s.aborted {
                        s.results.push((result.score, result.accuracy as f32));
                        // [课题] 记录本局判定计数与 FC 标记（结算列表展示 PhiraPro 卡片用）
                        s.round_counts.push(result.counts);
                        // [课题] 记录本局 Early/Late（Phigros 官方结算风格字段）
                        s.round_early_late.push((result.early, result.late));
                        // [课题] 记录本局最大连击（Phigros 官方课题结算 Max Combo 字段）
                        s.round_max_combo.push(result.max_combo);
                        s.round_fc.push(result.num_of_notes > 0 && result.max_combo == result.num_of_notes);
                        s.completed_rounds += 1;
                        // [课题] 记录本局真实曲绘（课题结算封面展示用，顺序同 paths/rounds）
                        s.round_illustrations.push(self.res.illustration.clone());
                        // 记录本局结束剩余 HP，供下一局继承（开 HP 且未耗尽时）
                        if self.hp_running {
                            s.hp_left = Some(self.hp.max(0.));
                        }
                    }
                }
                self.challenge_finished = true;
            }
        }
    }

    /// [课题模式] 全部完成后的课题结算：原结算界面 + 所选谱面记录 + 总分色档评分
    fn build_challenge_ending(&self, cfg: &ChallengeConfig) -> Result<EndingScene> {
        let (round_names, results, round_counts, round_early_late, round_max_combo, round_fc, round_illustrations) = {
            let s = cfg.session.lock().unwrap();
            (
                s.round_names.clone(),
                s.results.clone(),
                s.round_counts.clone(),
                s.round_early_late.clone(),
                s.round_max_combo.clone(),
                s.round_fc.clone(),
                s.round_illustrations.clone(),
            )
        };
        let total_score: u32 = results.iter().map(|(s, _)| *s).sum();
        let all_fc = round_fc.len() == 3 && round_fc.iter().all(|f| *f);
        let rank = rank_index(total_score, all_fc);
        let rank_name = grade(total_score);
        let rounds = round_names
            .iter()
            .zip(results.iter())
            .zip(round_counts.iter())
            .zip(round_early_late.iter())
            .zip(round_max_combo.iter())
            .zip(round_fc.iter())
            .map(|((((((n, d), (sc, ac)), counts), el), mc), fc)| ChallengeRoundRecord {
                name: n.clone(),
                difficulty: d.clone(),
                score: *sc,
                accuracy: *ac,
                max_combo: *mc,
                counts: *counts,
                early: el.0,
                late: el.1,
                full_combo: *fc,
            })
            .collect();
        EndingScene::new(
            self.res.background.clone(),
            self.res.illustration.clone(),
            self.res.player.clone(),
            self.res.icons.clone(),
            self.res.arc_icon.clone(),
            self.res.icon_retry.clone(),
            self.res.icon_proceed.clone(),
            self.res.mod_icons.clone(),
            self.res.info.clone(),
            self.judge.result(),
            &self.res.config,
            self.res.res_pack.ending.clone(),
            None, // 课题不上传
            self.player.as_ref().map(|it| it.rks),
            0, // historic_best 课题无意义
            None,
            None,
            if self.res.config.show_avg_fps {
                self.get_avg_fps()
            } else {
                None
            },
            Some(ChallengeEndData {
                re: cfg.re,
                rounds,
                total_score,
                rank_name,
                rank,
                all_fc,
                covers: round_illustrations,
            }),
        )
    }

    /// [课题模式] 构造下一局的加载任务（直接 Overlay LoadingScene，HP 继承上一局剩余值）
    fn build_challenge_next(&self, cfg: &ChallengeConfig, next_index: usize) -> Result<LocalTask<Result<NextScene>>> {
        let next_path = {
            let s = cfg.session.lock().unwrap();
            s.paths.get(next_index).cloned()
        };
        let Some(next_path) = next_path else {
            bail!("课题下一局路径缺失（index={next_index}）");
        };
        let re = cfg.re;
        let total = cfg.total;
        let custom_limits = cfg.custom_limits;
        let session = Arc::clone(&cfg.session);
        let hp = cfg.next_hp();
        let mut config = self.res.config.clone();
        config.hp = HpMode::Off;
        let player = self.player.clone();
        let challenge = ChallengeConfig {
            re,
            total,
            index: next_index,
            custom_limits,
            scoring: true,
            hp,
            session,
        };
        let task = async move {
            let mut next_fs = fs_from_file(std::path::Path::new(&next_path))?;
            let mut info = load_info(next_fs.as_mut()).await?;
            // 课题续局不上传、不评级
            info.id = None;
            let scene = LoadingScene::new_preview(
                GameMode::Challenge,
                info,
                config,
                next_fs,
                player,
                None,
                None,
                None,
                None,
                None,
                false,
                None,
                Some(challenge),
            )
            .await?;
            Ok::<_, anyhow::Error>(NextScene::Replace(Box::new(scene)))
        };
        Ok(Some(Box::pin(task)))
    }
}

/// 构造上传成绩数据（对应 Phira 服务端 /play/upload 的 Record 结构，字段与其一致）。
/// 原上游实现被 `#[cfg(closed)] mod inner` 隐藏且从未启用，本处按
/// Phira-Vrenxz/src/score_upload.rs 中 Record 的字段定义补全。
fn encode_record(game: &GameScene, player: i32, chart: i32) -> Vec<u8> {
    let result = game.judge.result();
    let best = game
        .player
        .as_ref()
        .map_or(false, |p| result.score as u32 > p.historic_best);
    let record = serde_json::json!({
        "id": null,
        "player": player,
        "chart": chart,
        "score": result.score as i64,
        "accuracy": result.accuracy,
        "perfect": result.counts[0] as i32,
        "good": result.counts[1] as i32,
        "bad": result.counts[2] as i32,
        "miss": result.counts[3] as i32,
        "speed": game.res.config.speed,
        "max_combo": result.max_combo as i32,
        "full_combo": result.max_combo == result.num_of_notes,
        "best": best,
        "mods": mods_to_strings(game.res.config.mods),
        "std": null,
        "std_score": null,
    });
    serde_json::to_vec(&record).unwrap_or_default()
}

/// 将 Mods 位标志转为服务端认可的字符串列表（snake_case 命名）。
fn mods_to_strings(mods: Mods) -> Vec<String> {
    const NAMES: &[(Mods, &str)] = &[
        (Mods::AUTOPLAY, "autoplay"),
        (Mods::FLIP_X, "flip_x"),
        (Mods::FADE_OUT, "fade_out"),
        (Mods::FADE_IN, "fade_in"),
        (Mods::NIGHTCORE, "nightcore"),
        (Mods::RAINBOW, "rainbow"),
        (Mods::NO_SHADER, "no_shader"),
        (Mods::INSTANT_DEATH_AP, "instant_death_ap"),
        (Mods::INSTANT_DEATH_FC, "instant_death_fc"),
        (Mods::GHOST, "ghost"),
        (Mods::FLIP_Y, "flip_y"),
        (Mods::RANDOM_X, "random_x"),
        (Mods::FX_TV, "fx_tv"),
        (Mods::FX_SCANLINE, "fx_scanline"),
        (Mods::FX_GLITCH, "fx_glitch"),
    ];
    NAMES
        .iter()
        .filter(|(flag, _)| mods.contains(*flag))
        .map(|(_, name)| name.to_string())
        .collect()
}

impl GameScene {
    fn replay_seek(&mut self, tm: &mut TimeManager, dst: f64) {
        let dst = dst.clamp(0., self.res.track_length.max(0.001));
        tm.seek_to(dst);
        let _ = self.music.seek_to(dst);
        self.res.time = dst;
        self.judge.reset();
        self.chart.reset();
        for line in &mut self.chart.lines {
            for note in &mut line.notes {
                if note.time < dst {
                    note.judge = crate::judge::JudgeStatus::Judged;
                }
            }
            line.cache.reset(&mut line.notes);
        }
        self.replay_paused = false;
        let _ = self.music.play();
    }

    fn replay_nav(&mut self, tm: &mut TimeManager, next: bool) {
        let Some(replay) = &self.replay_view else { return };
        let cur = self.res.time;
        let target = if next {
            replay.hits.iter().filter(|h| h.t as f64 > cur + 0.01 && h.kind >= 2).map(|h| h.t as f64).fold(f64::INFINITY, f64::min)
        } else {
            replay.hits.iter().filter(|h| (h.t as f64) < cur - 0.01 && h.kind >= 2).map(|h| h.t as f64).fold(f64::NEG_INFINITY, f64::max)
        };
        if target.is_finite() {
            self.replay_seek(tm, target);
        }
    }

    fn render_replay_controls(&mut self, ui: &mut Ui) {
        let top = ui.top;
        if self.replay_controls_hidden {
            let r = Rect::new(0.72, -top + 0.02, 0.22, 0.06);
            self.replay_buttons[9].inner.set(ui, r);
            ui.fill_rect(r, Color::new(0., 0., 0., 0.75));
            ui.text("显示控制").pos(r.center().x, r.center().y).anchor(0.5, 0.5).size(0.30).draw();
            return;
        }
        let bar = Rect::new(-1., top - 0.16, 2., 0.16);
        ui.fill_rect(bar, Color::new(0., 0., 0., 0.75));
        let labels = [
            "返回",
            if self.replay_paused { "继续" } else { "暂停" },
            "重播",
            "-5s",
            "+5s",
            match self.replay_speed_idx { 0 => "0.5x", 1 => "1x", 2 => "1.5x", _ => "2x" },
            "上个失误",
            "下个失误",
            "隐藏",
            "分析",
        ];
        for i in 0..10 {
            let r = Rect::new(-0.98 + i as f32 * 0.17, top - 0.115, 0.15, 0.07);
            self.replay_buttons[i].inner.set(ui, r);
            let active = i == 9 && self.replay_show_analysis;
            ui.fill_rect(r, if active { WHITE } else { Color::new(0.07, 0.08, 0.10, 0.9) });
            ui.text(labels[i])
                .pos(r.center().x, r.center().y)
                .anchor(0.5, 0.5)
                .size(0.24)
                .color(if active { BLACK } else { WHITE })
                .draw();
        }
        let progress = (self.res.time as f32 / self.res.track_length.max(0.001) as f32).clamp(0., 1.);
        let pb = Rect::new(-0.98, top - 0.032, 1.96, 0.012);
        ui.fill_rect(pb, Color::new(0.20, 0.22, 0.25, 1.));
        ui.fill_rect(Rect::new(pb.x, pb.y, pb.w * progress, pb.h), Color::new(0.9, 0.92, 0.95, 1.));
        if let Some(replay) = &self.replay_view {
            for h in &replay.hits {
                let x = pb.x + pb.w * (h.t / self.res.track_length.max(0.001) as f32).clamp(0., 1.);
                ui.fill_rect(Rect::new(x, pb.y - 0.004, 0.002, 0.020), Color::new(1., 0.3, 0.35, 0.9));
            }
        }
        ui.text(format!("{:.2} / {:.2}", self.res.time, self.res.track_length))
            .pos(0.95, top - 0.052)
            .anchor(1., 0.5)
            .size(0.26)
            .color(Color::new(1., 1., 1., 0.8))
            .draw();
        if self.replay_show_analysis {
            let panel = Rect::new(0.55, -top + 0.10, 0.42, 0.35);
            ui.fill_rect(panel, Color::new(0., 0., 0., 0.78));
            if let Some(replay) = &self.replay_view {
                let max = replay.hits.iter().map(|h| h.diff.abs()).fold(0.001, f32::max);
                for h in &replay.hits {
                    let x = panel.x + 0.02 + (h.t / self.res.track_length.max(0.001) as f32).clamp(0., 1.) * (panel.w - 0.04);
                    let y = panel.center().y - h.diff / max * (panel.h * 0.42);
                    let col = if h.diff.abs() < 0.08 { Color::new(0.4, 1., 0.5, 0.9) } else { Color::new(1., 0.4, 0.4, 0.9) };
                    ui.fill_rect(Rect::new(x, y, 0.003, 0.003), col);
                }
            }
        }
    }
}

impl Scene for GameScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        on_game_start();
        #[cfg(target_env = "ohos")]
        miniquad::native::set_interceptor_state(true);
        super::set_ime_enabled(false);
        crate::gamepad::set_gameplay_active(true);
        self.music = Self::new_music(&mut self.res)?;
        self.res.camera.render_target = target;
        tm.speed = self.res.config.speed as _;
        tm.adjust_time = self.res.config.adjust_time;
        reset!(self, self.res, tm);
        set_camera(&self.res.camera);
        self.first_in = true;
        Ok(())
    }

    fn pause(&mut self, tm: &mut TimeManager) -> Result<()> {
        if !tm.paused() {
            self.pause_rewind = None;
            self.music.pause()?;
            tm.pause();
            suspend_sound();
            // 澶氫汉姝ｅ紡娓哥帺锛氭妸鏆傚仠鐘舵€佸箍鎾粰鎴块棿锛堟埧闂撮噷鍏朵粬浜烘嵁姝ゆ樉绀恒€屾殏鍋滀腑銆嶏級
            notify_paused(self.pause_notify.as_ref(), true);
        }
        #[cfg(target_env = "ohos")]
        miniquad::native::set_interceptor_state(false);
        Ok(())
    }

    fn resume(&mut self, tm: &mut TimeManager) -> Result<()> {
        if !matches!(self.state, State::Playing) {
            tm.resume();
            // 澶氫汉姝ｅ紡娓哥帺锛氬箍鎾€滃凡缁х画鈥濓紙涓庝笂闈㈢殑鐘舵€佸垏鎹竴涓€瀵瑰簲锛?
            notify_paused(self.pause_notify.as_ref(), false);
        }
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        // HUD edit preview: stay on game page with dummy data, no music playback
        if self.hud_preview {
            if crate::hud::edit_active() {
                if let Some(p) = crate::hud::take_goto() {
                    if p != crate::hud::PageId::Game {
                        // Switch to another page: write back the request (main menu consumes after Pop), then exit preview
                        crate::hud::request_goto(p);
                        self.should_exit = true;
                    }
                }
            }
            return Ok(());
        }
        self.res.audio.recover_if_needed()?;
        // [HP 血量模式] 失败演出：冻结游戏，推进红屏→黑屏计时，结束后退出进结算
        if let Some(t) = self.fail_timer {
            let nt = t + get_frame_time();
            if nt >= Self::FAIL_RED_TIME + Self::FAIL_BLACK_TIME {
                self.fail_timer = None;
                // 自动重试：从失败前若干秒重开，不进入结算。
                if matches!(self.mode, GameMode::Normal)
                    && self.res.config.auto_retry > 0
                    && self.retries_left > 0
                {
                    self.retries_left -= 1;
                    let dst = (self.res.time - self.res.config.retry_rewind_sec as f64)
                        .max(self.offset() as f64);
                    reset!(self, self.res, tm);
                    tm.seek_to(dst);
                    self.music.seek_to(dst)?;
                    self.music.play()?;
                    self.res.time = dst;
                    self.state = State::Playing;
                    self.dead = false;
                    return Ok(());
                }
                match self.mode {
                    // 普通/NoRetry/View：HP 耗尽失败进结算页展示成绩（失败局不写记录）
                    GameMode::Normal | GameMode::NoRetry | GameMode::View | GameMode::Challenge | GameMode::ReChallenge => {
                        self.finish_and_show_result()?;
                    }
                    // 课题/Re课题：HP 耗尽整场作废，直接 Pop 回课题页
                    _ => {
                        self.should_exit = true;
                    }
                }
            } else {
                self.fail_timer = Some(nt);
            }
            return Ok(());
        }
        // 棰勮鏈熼棿琚閮ㄦ墦鏂紙濡傚浜烘埧闂存埧涓荤偣浜嗗紑濮嬶級锛氱珛鍗崇粨鏉熼瑙堬紝涓嶇粨绠椼€佺洿鎺ュ脊鍥?
        if self.preview_mode && self.interrupt.as_ref().is_some_and(|it| it.load(Ordering::Relaxed)) {
            self.should_exit = true;
            return Ok(());
        }
        // 鏇存柊鏆傚仠鐣岄潰娓愬彉锛堣烦杩囪氨闈㈠姩鐢绘湡闂村己鍒堕殣钘忔殏鍋滆彍鍗曪級
        let target = if self.skip_done { 0. } else if tm.paused() { 1. } else { 0. };
        self.pause_alpha += (target - self.pause_alpha) * 0.15;
        if matches!(self.state, State::Playing) {
            let music_pos = self.music.position();
            // 短谱面音频播完后 position 冻结在 track_length，若继续 update 会把游戏时间钳死，
            // 高帧率下 now 永远无法越过 track_length + WAIT_TIME + AFTER_TIME，导致无法结算。
            if music_pos < self.res.track_length - 1e-3 {
                tm.update(music_pos);
            }
        }
        if self.mode == GameMode::Exercise && tm.now() > self.exercise_range.end && !tm.paused() {
            if self.res.config.exercise_auto_speed {
                self.res.config.speed =
                    (self.res.config.speed + self.res.config.exercise_speed_step).min(2.0);
            }
            let state = self.state.clone();
            reset!(self, self.res, tm);
            self.state = state;
            tm.seek_to(self.exercise_range.start);
            tm.pause();
            self.music.pause()?;
            #[cfg(target_env = "ohos")]
            miniquad::native::set_interceptor_state(false);
        }
        let offset = self.offset();
        let time = tm.now();
        self.update_lyrics(time);
        let time = match self.state {
            State::Starting => {
                if time >= Self::BEFORE_TIME {
                    self.res.alpha = 1.;
                    self.state = State::BeforeMusic;
                    tm.reset();
                    tm.seek_to(if self.mode == GameMode::Exercise {
                        self.exercise_range.start
                    } else {
                        offset.min(0.) as f64
                    });
                    self.last_update_time = tm.real_time();
                    if self.first_in && self.mode == GameMode::Exercise {
                        tm.pause();
                        self.first_in = false;
                    }
                    tm.now()
                } else {
                    #[cfg(target_os = "windows")]
                    {
                        let emitter_config = self.res.emitter.emitter.config.clone();
                        let emitter_square_config = self.res.emitter.emitter_square.config.clone();
                        self.res.emitter.emitter.config.size = 0.0;
                        self.res.emitter.emitter_square.config.size = 0.0;
                        self.res.emitter.emitter.emit(vec2(0.0, 0.0), 1);
                        self.res.emitter.emitter_square.emit(vec2(0.0, 0.0), 1);
                        self.res.emitter.emitter.config = emitter_config;
                        self.res.emitter.emitter_square.config = emitter_square_config;
                    }
                    self.res.alpha = (1. - (1. - time / Self::BEFORE_TIME).powi(3)) as f32;
                    if self.mode == GameMode::Exercise {
                        self.exercise_range.start
                    } else {
                        offset as f64
                    }
                }
            }
            State::BeforeMusic => {
                if time >= 0.0 {
                    self.music.seek_to(time)?;
                    if !tm.paused() {
                        self.music.play()?;
                    }
                    self.state = State::Playing;
                }
                time
            }
            State::Playing => {
                if time > self.res.track_length + WAIT_TIME {
                    self.state = State::Ending;
                    #[cfg(target_env = "ohos")]
                    miniquad::native::set_interceptor_state(false);
                }
                time
            }
            State::Ending => {
                let t = time - self.res.track_length - WAIT_TIME;
                if t >= AFTER_TIME + 0.3 {
                    self.finish_and_show_result()?;
                }
                self.res.alpha = (1. - (t / AFTER_TIME).min(1.).powi(2)) as f32;
                self.res.track_length
            }
        };
        let time = (time - offset as f64).max(0.);
        self.res.time = time;
        if crate::replay::is_recording() {
            let frames: Vec<_> = Judge::get_touches()
                .into_iter()
                .map(|touch| {
                    let phase = match touch.phase {
                        TouchPhase::Started => 0,
                        TouchPhase::Moved => 1,
                        TouchPhase::Ended => 2,
                        TouchPhase::Cancelled => 3,
                        TouchPhase::Stationary => 4,
                    };
                    (touch.id, phase, touch.position.x, touch.position.y)
                })
                .collect();
            if !frames.is_empty() {
                crate::replay::capture(&frames, time as f32);
            }
        }
        // 鎬ц兘鑷€傚簲锛氭寜褰撳墠 Note 璐熻浇鍐冲畾鏄惁鍚敤浣庡垎杈ㄧ巼娓叉煋 / 鍏抽棴鎵撳嚮鐗规晥
        {
            let (visible_notes, upcoming_notes) = self.chart.load_metrics(&self.res);
            // 闃堝€肩敱鈥滄€ц兘浼樺寲妗ｄ綅鈥濆喅瀹氾紙鏃犱紭鍖栨。姘镐笉鍚敤浣庢竻娓叉煋锛?
            self.res.low_res_notes = visible_notes >= self.res.config.eff_lowres_threshold();
            self.res.suppress_hit_fx = upcoming_notes > self.res.config.eff_fx_density_threshold();
        }
        if !tm.paused() && self.pause_rewind.is_none() && self.mode != GameMode::View && !self.skip_done {
            self.gl.quad_gl.viewport(self.res.camera.viewport);
            if let Some(replay) = &self.replay_view {
                crate::judge::set_replay_touches(replay.touches_at(time as f32));
            }
            self.judge.update(&mut self.res, &mut self.chart, &mut self.bad_notes);
            self.gl.quad_gl.viewport(None);
            if crate::replay::is_recording() && self.judge.hit_generation() != self.replay_hit_gen {
                self.replay_hit_gen = self.judge.hit_generation();
                let hits: Vec<_> = self
                    .judge
                    .recent_hits()
                    .iter()
                    .map(|&(t, j, diff)| {
                        let kind = match j {
                            crate::judge::Judgement::Perfect => 0,
                            crate::judge::Judgement::Good => 1,
                            crate::judge::Judgement::Bad => 2,
                            crate::judge::Judgement::Miss => 3,
                        };
                        (t as f32, kind, diff as f32)
                    })
                    .collect();
                crate::replay::capture_hits(&hits);
            }
        } else {
            // Pause hides hover and suspends the filter, while release events
            // still end the infected finger's lifetime.
            self.chart.blocked_touches.clear();
        }
        // [噪域] 官方 JudgeControl 手势过渡：手指落入生效噪域 → BGM 低通滤波（音量减小感）+ SFX 反馈
        if tm.paused() || self.pause_rewind.is_some() {
            self.block_audio.suspend(&mut self.music);
        } else {
            self.block_audio.sync(
                &mut self.music,
                !self.res.config.block_area_simple && matches!(self.state, State::Playing) && !self.chart.blocked_touches.is_empty(),
            );
        }
        if let Some(update) = &mut self.update_fn {
            update(self.res.time, &mut self.res, &mut self.judge);
        }
        let counts = self.judge.counts();
        self.res.judge_line_color = if counts[2] + counts[3] == 0 && self.res.config.ap_fc_indicator {
            if counts[1] == 0 {
                self.res.res_pack.info.color_perfect()
            } else {
                self.res.res_pack.info.color_good()
            }
        } else {
            WHITE
        };
        if !self.dead
            && !self.skip_done
            && matches!(self.state, State::Playing)
            && (self.res.config.mods.contains(Mods::INSTANT_DEATH_AP) && counts[1] + counts[2] + counts[3] > 0
                || self.res.config.mods.contains(Mods::INSTANT_DEATH_FC) && counts[2] + counts[3] > 0)
        {
            self.dead = true;
            self.skip_done = true;
            self.track_skipped = true;
            self.skip_transition_progress = 0.0;
            self.skip_wait_timer = 0.0;
            self.skip_fade_out_progress = 0.0;
            self.skip_instant = true;
            if !self.music.paused() {
                self.music.pause()?;
            }
            #[cfg(target_env = "ohos")]
            miniquad::native::set_interceptor_state(false);
        }
        // [HP 血量模式] 扣血判定：按 HP 档位规则对新增的失误判定扣除 HP
        if !self.dead && !self.skip_done && matches!(self.state, State::Playing) && self.hp_running {
            let counts = self.judge.counts();
            let non_perfect = counts[1] + counts[2] + counts[3];
            let bad_miss = counts[2] + counts[3];
            let (old_non_perfect, old_bad_miss) = {
                let lc = self.last_counts;
                (lc[1] + lc[2] + lc[3], lc[2] + lc[3])
            };
            let lose: f32 = match self.hp_lose {
                HpLoseCondition::NonPerfect => (non_perfect as i64 - old_non_perfect as i64).max(0) as f32,
                HpLoseCondition::BadOrMiss => (bad_miss as i64 - old_bad_miss as i64).max(0) as f32,
            };
            self.last_counts = counts;
            if lose > 0. {
                self.hp = (self.hp - lose).max(0.0);
                // [第九章 LIFE 样式] 扣血红闪（参考 LifeHitFlashDuration=0.15s）
                self.life_flash = 0.15;
                if self.hp <= 0.0 {
                    // HP 耗尽：进入失败演出——播放 TrackCrash、停 BGM、冻结游戏，
                    // 红屏 0.1s → 黑屏 1s 后进结算（课题局作废）
                    self.dead = true;
                    if let Some(cfg) = &self.challenge {
                        if let Ok(mut s) = cfg.session.lock() {
                            s.aborted = true;
                        }
                    }
                    if let Some(sfx) = &mut self.crash_sfx {
                        crate::judge::play_sfx(sfx, &self.res.config);
                    }
                    let _ = self.music.pause();
                    self.fail_timer = Some(0.0);
                }
            }
        } else {
            self.last_counts = self.judge.counts();
        }
        // [第九章 LIFE 样式] displayedLife 平滑逼近（照抄 SecretChallengeLifeBar.cs：
        // Mathf.SmoothDamp(displayedLife, target, ref lifeVelocity, shrinkSmoothTime=0.16)）
        // + 红闪衰减（LifeHitFlashDuration=0.15）
        {
            let dt = get_frame_time().min(0.1);
            let target = if self.hp_running {
                (self.hp / self.hp_max.max(1.0)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            // Unity Mathf.SmoothDamp 精确算法
            let smooth_time = 0.16f32.max(0.0001);
            let omega = 2.0 / smooth_time;
            let x = omega * dt;
            let exp = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
            let change = self.displayed_hp - target;
            let temp = (self.life_velocity + omega * change) * dt;
            self.life_velocity = (self.life_velocity - omega * temp) * exp;
            self.displayed_hp = target + (change + temp) * exp;
            // 收敛钳制（Unity 亦在 target 差极小后直接归位）
            if (target - self.displayed_hp).abs() < 0.0001 {
                self.displayed_hp = target;
                self.life_velocity = 0.0;
            }
            self.displayed_hp = self.displayed_hp.clamp(0.0, 1.0);
            if self.life_flash > 0.0 {
                self.life_flash = (self.life_flash - dt).max(0.0);
            }
        }
        self.res.judge_line_color.a *= self.res.alpha;
        self.chart.update(&mut self.res);
        let res = &mut self.res;
        if res.config.interactive
            && (is_key_pressed(KeyCode::Space) || crate::gamepad::last_frame().menu_pressed)
        {
            if tm.paused() && !self.dead {
                if matches!(self.state, State::Playing) {
                    self.music.play()?;
                    tm.resume();
                }
            } else if matches!(self.state, State::Playing | State::BeforeMusic) && !self.dead {
                if !self.music.paused() {
                    self.music.pause()?;
                }
                tm.pause();
                suspend_sound();
                self.pause_need_blur = true;
            }
        }
        if Self::interactive(res, &self.state) && !self.skip_done {
            if is_key_pressed(KeyCode::Left) && res.config.use_keyboard {
                res.time -= 1.;
                let dst = (self.music.position() - 1.).max(0.);
                self.music.seek_to(dst)?;
                tm.seek_to(dst);
            }
            if is_key_pressed(KeyCode::Right) && res.config.use_keyboard {
                res.time += 5.;
                let dst = (self.music.position() + 5.).min(res.track_length);
                self.music.seek_to(dst)?;
                tm.seek_to(dst);
            }
            if is_key_pressed(KeyCode::Q) {
                self.should_exit = true;
            }
        }
        for e in &mut self.effects {
            e.update(&self.res);
        }
        if let Some((id, text)) = take_input() {
            let offset = self.offset().min(0.);
            match id.as_str() {
                "exercise_start" => {
                    if let Some(t) = parse_time(&text) {
                        if !(offset as f64..self.res.track_length.min(self.exercise_range.end - 3.).max(offset as f64)).contains(&t) {
                            show_message(tl!("ex-time-out-of-range")).error();
                        } else {
                            self.exercise_range.start = t;
                            show_message(tl!("ex-time-set")).ok();
                        }
                    } else {
                        show_message(tl!("ex-invalid-format")).error();
                    }
                }
                "exercise_end" => {
                    if let Some(t) = parse_time(&text) {
                        if !((self.exercise_range.start + 3.).max(offset as f64).min(self.res.track_length)..self.res.track_length).contains(&t) {
                            show_message(tl!("ex-time-out-of-range")).error();
                        } else {
                            self.exercise_range.end = t;
                            show_message(tl!("ex-time-set")).ok();
                        }
                    } else {
                        show_message(tl!("ex-invalid-format")).error();
                    }
                }
                _ => return_input(id, text),
            }
        }
        // 手动轨道跳过（Alt+S / 四指四角）
        self.update_track_skip(tm)?;
        // 璺冲叧鍔ㄧ敾鏇存柊
        if self.skip_done {
            let dt = get_frame_time();
            if self.skip_transition_progress < 1.0 {
                self.skip_transition_progress = (self.skip_transition_progress + dt / 0.5).min(1.0);
                if self.skip_transition_progress >= 1.0 {
                    crate::ui::message_sound();
                }
            } else if self.skip_fade_out_progress < 1.0 {
                self.skip_wait_timer += dt;
                if self.skip_wait_timer >= 3.0 {
                    self.skip_fade_out_progress = (self.skip_fade_out_progress + dt / 0.5).min(1.0);
                }
            } else {
                if !self.music.paused() {
                    self.music.pause()?;
                }
                tm.seek_to(self.res.track_length + WAIT_TIME + AFTER_TIME + 0.3);
                self.state = State::Ending;
                self.skip_done = false;
                self.skip_transition_progress = 0.0;
                self.skip_fade_out_progress = 0.0;
                self.skip_wait_timer = 0.0;
                self.skip_instant = false;
                return Ok(());
            }
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        // HUD 自定义：编辑模式下手势全部交给编辑器，不落游戏逻辑。
        if crate::hud::edit_active() {
            crate::hud::set_cur_page(crate::hud::PageId::Game);
            return Ok(crate::hud::editor_touch(touch));
        }
        if self.mode == GameMode::Replay {
            let top = screen_height() / screen_width();
            if self.replay_controls_hidden {
                if self.replay_buttons[9].touch(touch, tm.now() as f32) {
                    self.replay_controls_hidden = false;
                    return Ok(true);
                }
                return Ok(false);
            }
            let y = top - 0.10;
            for i in 0..10 {
                let r = Rect::new(-0.98 + i as f32 * 0.17, y, 0.15, 0.07);
                if self.replay_buttons[i].touch(touch, tm.now() as f32) {
                    match i {
                        0 => {
                            self.should_exit = true;
                            self.next_scene = Some(NextScene::Pop);
                        }
                        1 => {
                            self.replay_paused = !self.replay_paused;
                            if self.replay_paused {
                                tm.pause();
                                let _ = self.music.pause();
                            } else {
                                tm.resume();
                                let _ = self.music.play();
                            }
                        }
                        2 => self.replay_seek(tm, 0.),
                        3 => self.replay_seek(tm, self.res.time - 5.),
                        4 => self.replay_seek(tm, self.res.time + 5.),
                        5 => {
                            const SPEEDS: [f32; 4] = [0.5, 1., 1.5, 2.];
                            self.replay_speed_idx = (self.replay_speed_idx + 1) % SPEEDS.len();
                            tm.speed = SPEEDS[self.replay_speed_idx] as f64 * self.res.config.speed as f64;
                        }
                        6 => self.replay_nav(tm, false),
                        7 => self.replay_nav(tm, true),
                        8 => self.replay_controls_hidden = true,
                        _ => self.replay_show_analysis = !self.replay_show_analysis,
                    }
                    return Ok(true);
                }
            }
            // 底部进度条点击跳转
            let bar = Rect::new(-0.98, top - 0.025, 1.96, 0.018);
            if bar.contains(touch.position) {
                let p = ((touch.position.x - bar.x) / bar.w).clamp(0., 1.);
                self.replay_seek(tm, p as f64 * self.res.track_length.max(0.001));
                return Ok(true);
            }
            return Ok(false);
        }
        if self.skip_done {
            return Ok(false);
        }
        if self.mode == GameMode::Exercise && tm.paused() {
            let touch = Touch {
                position: touch.position * self.touch_scale(),
                ..touch.clone()
            };
            if self.exercise_btns.0.touch(&touch) {
                request_input("exercise_start", InputBox::new().default_text(fmt_time(self.exercise_range.start as f32)));
                return Ok(true);
            }
            if self.exercise_btns.1.touch(&touch) {
                request_input("exercise_end", InputBox::new().default_text(fmt_time(self.exercise_range.end as f32)));
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        if self.res.config.show_fps || self.res.config.show_avg_fps {
            let current_time = tm.real_time();
            if matches!(self.state, State::Playing) && !tm.paused() {
                let frame_delta = current_time - self.fps_last_frame_time;
                self.fps_total_time += frame_delta;
                self.fps_frame_count += 1;
                // 瀹炴椂甯х巼骞虫粦锛?/甯ч棿闅?鍋氭寚鏁板钩鍧?
                if frame_delta > 0.0 {
                    let inst = (1.0 / frame_delta) as f32;
                    self.fps_smooth = if self.fps_smooth <= 0.0 { inst } else { self.fps_smooth * 0.9 + inst * 0.1 };
                }
            }
            self.fps_last_frame_time = current_time;
        }

        // 娓哥帺涓疄鏃?FPS 瑕嗙洊灞傛爣璁帮紙鎻愬墠蹇収锛岄伩鍏嶄笌 res 鍊熺敤鍐茬獊锛?
        // FPS text is drawn by the global overlay.

        // 动态背景：先更新流体/频谱纹理，再让 Resource 使用最新帧。
        if let Some(dbg) = &mut self.dynamic_bg {
            dbg.update(
                tm.now() as f32,
                ui.viewport,
                Some(self.music.position() as f32),
            );
            self.res.background = dbg.texture();
        }

        let res = &mut self.res;
        let asp = ui.viewport.2 as f32 / ui.viewport.3 as f32;
        if res.update_size(ui.viewport) || self.mode == GameMode::View {
            set_camera(&res.camera);
        }

        let msaa = res.config.sample_count > 1;
        let started_touches: Vec<(f32, f32)> = Judge::get_touches()
            .iter()
            .filter(|it| it.phase == TouchPhase::Started)
            .map(|it| (it.position.x, it.position.y))
            .collect();
        eprintln!(
            "[DBG] R0 t={:.3} reveal={} p11={:.4} p22={:.4} models={} emit={} block={} vp={:.3}x{:.3} asp={:.4} playing={} paused={} touches={:?}",
            res.time,
            self.mode == GameMode::Reveal,
            unsafe { get_internal_gl() }.quad_gl.get_projection_matrix().x_axis.x,
            unsafe { get_internal_gl() }.quad_gl.get_projection_matrix().y_axis.y,
            res.model_stack.len(),
            res.emitter.is_empty(),
            self.chart.blocked_touches.len(),
            ui.viewport.2,
            ui.viewport.3,
            res.aspect_ratio,
            matches!(self.state, State::Playing),
            tm.paused(),
            started_touches
        );

        let chart_onto = res
            .chart_target
            .as_ref()
            .map(|it| if msaa { it.input() } else { it.output() })
            .or(res.camera.render_target);
        push_camera_state();
        set_camera(&Camera2D {
            // [谱面揭晓] Reveal 模式下缩小相机 zoom（视野扩大 2.5 倍），背景 / 判定线 / 特效等
            // 经相机投影的高层绘制随之缩小；Normal 等模式保持原样。
            zoom: vec2(
                if self.mode == GameMode::Reveal { 1. / REVEAL_ZOOM_OUT } else { 1. },
                -asp * if self.mode == GameMode::Reveal { 1. / REVEAL_ZOOM_OUT } else { 1. },
            ),
            viewport: if res.chart_target.is_some() { None } else { Some(ui.viewport) },
            render_target: chart_onto,
            ..Default::default()
        });
        clear_background(BLACK);
        draw_background(*res.background);
        pop_camera_state();

        let chart_target_vp = if res.chart_target.is_some() {
            let vp = res.camera.viewport.unwrap();
            Some((vp.0 - ui.viewport.0, vp.1 - ui.viewport.1, vp.2, vp.3))
        } else {
            res.camera.viewport
        };
        self.gl.quad_gl.render_pass(chart_onto.map(|it| it.render_pass));
        self.gl.quad_gl.viewport(chart_target_vp);

        let h = 1. / res.aspect_ratio;
        draw_rectangle(-1., -h, 2., h * 2., Color::new(0., 0., 0., res.alpha * res.info.background_dim));

        // [谱面揭晓] Reveal 模式对整个谱面模型栈缩放 1/2.5：Note 顶点经
        // res.model_stack（不含相机矩阵）生成，缩放后其谱面 NDC 坐标变小进入可视范围，
        // line.rs 的 note_above_screen / note.rs draw_tex_pts 两类屏外剔除随之自动放宽。
        // 判定逻辑（触摸经屏幕像素换算，不走渲染模型栈）完全不受影响。
        res.with_model(
            Matrix::identity().append_nonuniform_scaling(&Vector::new(
                if self.mode == GameMode::Reveal { 1. / REVEAL_ZOOM_OUT } else { 1. },
                if self.mode == GameMode::Reveal { 1. / REVEAL_ZOOM_OUT } else { 1. },
            )),
            |res| {
                let m = res.model_stack.last().unwrap();
                eprintln!(
                    "[DBG] R1 t={:.3} m11={:.4} m22={:.4} proj_p11={:.4} proj_p22={:.4} ct={}",
                    res.time,
                    m[(0, 0)],
                    m[(1, 1)],
                    unsafe { get_internal_gl() }.quad_gl.get_projection_matrix().x_axis.x,
                    unsafe { get_internal_gl() }.quad_gl.get_projection_matrix().y_axis.y,
                    res.chart_target.is_some()
                );
                self.chart.render(ui, res);

                // [噪域] 噪域区域渲染（Phigros 官方 shader 语义合成：ActiveBlock/ReadyBlock/
                // DisabledBlock/TouchEffect/GlowMask/EdgeMask/BlockCompose/SubtractBlockBlender/
                // BlockSprite），失效手指触摸标记一并由 render_block_overlay 绘制
                self.chart.render_block_overlay(res);
            },
        );

        self.gl.quad_gl.render_pass(
            res.chart_target
                .as_ref()
                .map(|it| it.output().render_pass)
                .or_else(|| res.camera.render_pass()),
        );

        self.bad_notes.retain(|dummy| dummy.render(res));
        let t = tm.real_time();
        let dt = (t - std::mem::replace(&mut self.last_update_time, t)) as f32;
        // 鍒ゅ畾鐗规晥锛堢矑瀛愶級鏄渶鍚冩€ц兘鐨勯儴鍒嗭細娌℃湁瀛樻椿绮掑瓙鏃剁洿鎺ヨ烦杩囦袱閬嶇矑瀛愭覆鏌撶绾?
        if res.config.eff_particles() && !res.emitter.is_empty() {
            if self.mode == GameMode::Reveal {
                // [谱面揭晓] Reveal 下粒子发射位置是未缩放的谱面世界坐标（判定阶段不经
                // 渲染期 with_model），绘制又走 get_projection_matrix()（仅相机投影、不经
                // model_stack），故须临时切换到与 chart 相同的缩小相机，粒子特效才能随
                // 判定线/note 一起缩进游玩区域。渲染目标必须保持当前 output pass：
                // 此前误用 chart_onto（chart_target 的 input 目标）会把粒子画进离屏
                // input，后续无展示路径导致特效消失（写法对齐 TweakOffset 的离屏绘制）。
                push_camera_state();
                set_camera(&Camera2D {
                    zoom: vec2(1. / REVEAL_ZOOM_OUT, -asp / REVEAL_ZOOM_OUT),
                    render_target: res
                        .chart_target
                        .as_ref()
                        .map(|it| it.output())
                        .or(res.camera.render_target),
                    ..Default::default()
                });
                eprintln!(
                    "[DBG] R2 particle p11={:.4} p22={:.4}",
                    unsafe { get_internal_gl() }.quad_gl.get_projection_matrix().x_axis.x,
                    unsafe { get_internal_gl() }.quad_gl.get_projection_matrix().y_axis.y
                );
                res.emitter.draw(dt);
                pop_camera_state();
            } else {
                res.emitter.draw(dt);
            }
        }

        let combo_debug = res.config.combo_text_debug();
        let combo = self.judge.combo();
        let alpha = res.alpha;
        let top = -1.0 / res.aspect_ratio;
        let eps = 2e-2 / res.aspect_ratio;
        let is_starting = matches!(self.state, State::Starting);
        let combo_top = top + eps * 2.0 - (1.0 - if is_starting { 1.0 } else { 0.0 }) * 0.4;
        let watermark_text = res.config.custom_watermark().to_string();

        // HUD 自定义：标记当前编辑页为 game，并清空本帧槽位登记（随后 ui() 内 slot_or 重新登记）。
        crate::hud::set_cur_page(crate::hud::PageId::Game);
        crate::hud::begin_frame();
        // [第九章 LIFE 样式] Phigros SecretChallengeLife 生命条：真实 GLSL shader 管线还原
        // （SecretChallengeLifeBar.mat 参数 + 双噪声纹理湍流流动合成/对比度/阈值 + 粉红描边 +
        // 深辉光曝光 + 距离场填充裁剪），逻辑照抄 SecretChallengeLifeBar.cs
        // （Mathf.SmoothDamp 平滑收缩 shrinkSmoothTime=0.16）；
        // [HP 门控] 血条与 LIFE 仅在 HP mod（血条 mod）开启时渲染；关闭血条后整块隐藏。
        // 此前曾为修复不可见而解除 hp_running 门控，真实根因是离屏 render_target 覆盖，
        // 现离屏修复保留、门控恢复（普通模式未开 HP 时不再显示满条血条）。
        if self.hp_running {
            // [层级修复] 血条须位于 combo 连击数图层之下、其他 UI 之前：
            // 必须显式渲染到 chart_target 离屏输出（render_target=chart_target.output()），
            // 随谱面贴回后显示。严禁用 set_camera(&ui.camera())：其 render_target 为
            // None，macroquad set_camera 内部会把渲染目标切回默认屏幕，血条画在屏幕
            // 后被后续 chart_target 全屏贴回覆盖导致不可见（历史根因）。
            push_camera_state();
            set_camera(&Camera2D {
                zoom: vec2(1., -asp),
                // [宽高比锁定修复] 血条/LIFE 必须渲染在谱面游玩区域内：
                // 此前 viewport: None 使 NDC 全幅铺满离屏 target（=窗口尺寸），
                // UI 被拉成窗口宽高比；改为锁定区域的 chart_target_vp 后，
                // NDC -1..1 映射到谱面 viewport 裁剪区域，血条与数字随之收进游玩区域。
                viewport: chart_target_vp,
                render_target: res
                    .chart_target
                    .as_ref()
                    .map(|it| it.output())
                    .or(res.camera.render_target),
                ..Default::default()
            });
            let right = 1.0;
            // 几何（修正，对照 SecretChallengeLifeMod.prefab）：
            // AnimatedLifeBar(RawImage) Anchor(0,0)-(1,1) 全屏 stretch、SizeDelta(0,0)，
            // shader 内部按距离场 discard 裁出中央弧形带；BackgroundMask 为 y+400 中央横带。
            // 此前"全屏宽贴顶横条 26%"是把 shader 当顶部条用，UV 映射错位，必须改为全屏 quad。
            let w = 2.0;
            let h = ui.top * 2.0;
            let x = -1.0;
            let y = -ui.top;
            let ratio = self.displayed_hp.clamp(0.0, 1.0);
            // 课题局进度（如 1/3）：原在顶边，现移至 Life 条下方避免被遮
            if let Some(cfg) = &self.challenge {
                let label = if cfg.re { "Re" } else { "课题" };
                ui.text(&format!("{}-{}/{}", label, cfg.index + 1, cfg.total))
                    .pos(right - 0.02, y + h + 0.04)
                    .anchor(1.0, 0.0)
                    .size(0.34)
                    .color(Color::new(0.86, 0.62, 1.0, 1.0))
                    .draw();
            }
            // 真实 shader 管线：惰性加载材质（失败时记录错误并画纯色底槽占位，不退回数学近似）
            if self.life_bar_mat.is_none() {
                match life_bar::LifeBarMaterial::load() {
                    Ok(m) => self.life_bar_mat = Some(m),
                    Err(e) => {
                        tracing::warn!("life bar shader failed: {e}");
                        if let Ok(exe) = std::env::current_exe() {
                            let _ = std::fs::write(exe.with_file_name("life_bar_shader_error.txt"), format!("{e}"));
                        }
                    }
                }
            }
            if let Some(lb) = &self.life_bar_mat {
                // 全屏 quad：对照 prefab AnimatedLifeBar 全屏 stretch，形状完全由 shader discard 决定
                let mat = &lb.material;
                // ApplyShaderParameters：每帧把归一化生命写入 _Life uniform
                let t = get_time() as f32;
                mat.set_uniform("_Life", ratio);
                // 动态湍流：_TurbulenceFlow 驱动双噪声 UV 流动（原版 cs 每帧随
                // NoiseSpeed 更新 TurbulenceFlow，方向斜漂、速度适中）
                mat.set_uniform("_TurbulenceFlow", vec4(t * 0.05, t * 0.02, t * 0.05, t * 0.02));
                gl_use_material(*mat);
                // [2026-10-31 半透明→v14 阴影渐变] 顶点 alpha 已由 shader 末尾的
                // 垂直渐变接管（顶部 1.0 → 底部 0.0），此处 draw_rectangle 顶点色
                // 仅作 RGB=白色透传，alpha 0.6 不再参与混合权重。
                draw_rectangle(x, y, w, h, Color::new(1.0, 1.0, 1.0, 0.6));
                gl_use_default_material();
                // [第九章 LIFE 样式] 血条上方中央：生命数字 + LIFE 标签（参考 Phigros 原版 UI，黑字红描边）
                let life_num = self.hp.ceil().max(0.0) as i32;
                // 扣血红闪：life_flash 从 0.15 衰减到 0，k 从 1.0 渐变到 0.0（完全红 → 黑）
                let k = (self.life_flash / 0.15).min(1.0);
                let num_size = 1.0;
                let num_h = ui.text(&life_num.to_string()).size(num_size).measure_using(&PGR_FONT).h;
                // 数字中心在血条上方，LIFE 标签紧贴数字底部（与 combo 同字号 0.4）
                draw_life_text(ui, &life_num.to_string(), 0.0, -top + 0.045, num_size, 0.5, 0.5, k);
                draw_life_text(ui, "LIFE", 0.0, -top + 0.045 + num_h * 0.5 + 0.02, 0.4, 0.5, 0.0, k);
            } else {
                // 材质加载失败：仅画简单底槽占位（不退回数学近似）
                draw_rectangle(x, y, w, h, Color::new(0.741, 0.0, 0.0, 1.0));
            }
            pop_camera_state();
            }
        self.ui(ui, tm)?;

        // 娓哥帺涓疄鏃跺抚鐜囷紙宸︿笂瑙掞級
        // FPS text is drawn by the global overlay.

        if combo_debug {
            let debug_info = format!(
                "Combo: {} | Pos: ({:.3}, {:.3}) | Size: 1.0 | Color: ({:.2}, {:.2}, {:.2}, {:.2})",
                combo,
                0.0,
                combo_top + 0.02,
                WHITE.r, WHITE.g, WHITE.b, alpha
            );

            ui.text(&debug_info)
                .pos(0.5, -0.40)
                .anchor(0.5, 0.5)
                .size(0.35)
                .color(semi_white(0.9 * alpha))
                .draw();

            let marker_size = 0.02;
            ui.fill_rect(
                Rect::new(-marker_size, combo_top - marker_size, marker_size * 2.0, marker_size * 2.0),
                Color::new(1.0, 0.0, 0.0, 0.8 * alpha),
            );
            ui.fill_rect(
                Rect::new(-0.05, combo_top - 0.001, 0.1, 0.002),
                Color::new(1.0, 0.0, 0.0, 0.5 * alpha),
            );
            ui.fill_rect(
                Rect::new(-0.001, combo_top - 0.05, 0.002, 0.1),
                Color::new(1.0, 0.0, 0.0, 0.5 * alpha),
            );
        }

        // ===== 准度指示器（仿《冰与火之舞》：底部中央误差刻度条） =====
        if matches!(self.state, State::Playing) && !tm.paused() && self.res.config.indicator_enabled {
            let now = self.res.time;
            let slot_w = 0.6;
            let slot_h = 0.014;
            let slot_y = -top - 0.16;
            let slot_x = -slot_w / 2.0;
            let half = slot_w / 2.0;
            let phigros = self.res.config.indicator_delay_phigros;
            let phira_color = self.res.config.indicator_color_phira;
            // 显示边界：phigros 模式固定 ±160ms（左右槽端），冰与火模式沿用判定上限（±LIMIT_BAD）
            let limit = if phigros {
                crate::judge::LIMIT_GOOD as f32 // 0.16s = ±160ms
            } else {
                crate::judge::LIMIT_BAD as f32 // 0.22s
            };

            // 槽底
            ui.fill_rect(
                Rect::new(slot_x, slot_y, slot_w, slot_h),
                Color::new(1., 1., 1., 0.22 * alpha),
            );
            // 中央 0ms 基准线
            ui.fill_rect(
                Rect::new(-0.002, slot_y - 0.045, 0.004, 0.09),
                Color::new(1., 1., 1., 0.55 * alpha),
            );
            // Perfect / Good 区间分界参考线
            let lp = (crate::judge::LIMIT_PERFECT as f32 / limit) * half;
            let lg = (crate::judge::LIMIT_GOOD as f32 / limit) * half;
            for x in [lp, -lp, lg, -lg] {
                ui.fill_rect(
                    Rect::new(x - 0.001, slot_y - 0.006, 0.002, slot_h + 0.012),
                    Color::new(1., 1., 1., 0.28 * alpha),
                );
            }
            // 最近判定刻度（带残影淡出）：误差负=early 偏左、正=late 偏右；超界在边界处显示
            for &(ht, judgement, diff) in self.judge.recent_hits().iter().rev().take(12) {
                let age = (now - ht) as f32;
                if age < 0.0 || age > 1.2 {
                    continue;
                }
                let fade = (1.0 - age / 1.2) * 0.9 * alpha;
                let frac = (diff as f32 / limit).clamp(-1.0, 1.0);
                let cx = slot_x + half + frac * half;
                let color = if phira_color {
                    let c = match judgement {
                        crate::judge::Judgement::Perfect => self.res.res_pack.info.color_perfect(),
                        crate::judge::Judgement::Good => self.res.res_pack.info.color_good(),
                        crate::judge::Judgement::Bad | crate::judge::Judgement::Miss => {
                            Color::from_hex_argb(0xebff7474)
                        }
                    };
                    Color::new(c.r, c.g, c.b, c.a * fade)
                } else {
                    match judgement {
                        crate::judge::Judgement::Perfect => Color::new(0.25, 0.95, 0.55, fade),
                        crate::judge::Judgement::Good => Color::new(0.98, 0.85, 0.30, fade),
                        crate::judge::Judgement::Bad => Color::new(0.95, 0.40, 0.35, fade),
                        crate::judge::Judgement::Miss => Color::new(0.65, 0.65, 0.65, fade),
                    }
                };
                ui.fill_rect(
                    Rect::new(cx - 0.006, slot_y - 0.032, 0.012, 0.064),
                    color,
                );
            }
        }

        self.overlay_ui(ui, tm)?;

        if !watermark_text.is_empty() {
            let top = ui.top;
            ui.text(&watermark_text)
                .pos(0., top - 0.010)
                .anchor(0.5, 1.)
                .size(0.28)
                .no_baseline()
                .color(Color::new(1., 1., 1., 0.3))
                .draw_using(&PGR_FONT);
        }

        if self.mode == GameMode::TweakOffset {
            push_camera_state();
            self.gl.quad_gl.viewport(None);
            set_camera(&Camera2D {
                zoom: vec2(1., -screen_aspect()),
                render_target: self.res.chart_target.as_ref().map(|it| it.output()).or(self.res.camera.render_target),
                ..Default::default()
            });
            self.tweak_offset(ui, Self::interactive(&self.res, &self.state));
            pop_camera_state();
        }

        if !self.res.no_effect && !self.effects.is_empty() {
            push_camera_state();
            set_camera(&Camera2D {
                zoom: vec2(1., asp),
                ..Default::default()
            });
            for e in &self.effects {
                e.render(&mut self.res);
            }
            pop_camera_state();
        }
        if msaa || !self.res.no_effect {
            if let Some(target) = &self.res.chart_target {
                self.gl.flush();
                push_camera_state();
                self.gl.quad_gl.viewport(None);
                set_camera(&Camera2D {
                    zoom: vec2(1., asp),
                    render_target: self.res.camera.render_target,
                    viewport: Some(ui.viewport),
                    ..Default::default()
                });
                if self.skip_done {
                    // 鎳掑姞杞芥ā绯婃潗璐?
                    if self.skip_blur_material.is_none() {
                        const BLUR_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying vec2 uv;
uniform mat4 Model;
uniform mat4 Projection;
uniform vec2 UVScale;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = (texcoord - vec2(0.5)) * UVScale + vec2(0.5);
}"#;
                        let blur_fragment = include_str!("../core/shaders/gaussian_blur.glsl");
                        self.skip_blur_material = Some(
                            load_material(
                                BLUR_VERTEX,
                                blur_fragment,
                                MaterialParams {
                                    uniforms: vec![
                                        ("screenSize".to_owned(), UniformType::Float2),
                                        ("blurSize".to_owned(), UniformType::Float1),
                                        ("UVScale".to_owned(), UniformType::Float2),
                                    ],
                                    textures: vec!["screenTexture".to_owned()],
                                    ..Default::default()
                                },
                            )
                            .ok(),
                        )
                        .flatten();
                    }
                    if let Some(mat) = &self.skip_blur_material {
                        let tex = target.output().texture;
                        let screen_dim = vec2(tex.width(), tex.height());
                        mat.set_uniform("screenSize", screen_dim);
                        // 妯＄硦寮哄害闅忓姩鐢昏繘搴﹀彉鍖栵紝鏈€澶?鍍忕礌
                        let blur_amount = self.skip_transition_progress.min(1.0) * 8.0;
                        mat.set_uniform("blurSize", blur_amount);
                        mat.set_texture("screenTexture", tex);
                        let vp = ui.viewport;
                        mat.set_uniform("UVScale", vec2(vp.2 as _, vp.3 as _) / screen_dim);
                        gl_use_material(*mat);
                        draw_rectangle(-1., -ui.top, 2., ui.top * 2., WHITE);
                        gl_use_default_material();
                    } else {
                        draw_texture_ex(
                            target.output().texture,
                            -1.,
                            -ui.top,
                            WHITE,
                            DrawTextureParams {
                                dest_size: Some(vec2(2., ui.top * 2.)),
                                ..Default::default()
                            },
                        );
                    }
                } else {
                    draw_texture_ex(
                        target.output().texture,
                        -1.,
                        -ui.top,
                        WHITE,
                        DrawTextureParams {
                            dest_size: Some(vec2(2., ui.top * 2.)),
                            ..Default::default()
                        },
                    );
                }
                pop_camera_state();
            }
        }

        // 璺冲叧鎻愮ず闈㈡澘鍦ㄦā绯婁箣鍚庢覆鏌擄紝姘村钩鎷変几灞曞紑锛圤ut Quart锛?
        if self.skip_done && self.skip_transition_progress >= 1.0 {
            // 纭繚娓叉煋鍒板睆骞曪紙鑰屼笉鏄灞?chart_target锛夛紝骞朵娇鐢ㄤ笌 ui.text 涓€鑷寸殑 UI 鍧愭爣绯荤粺
            self.gl.quad_gl.render_pass(self.res.camera.render_pass());
            self.gl.quad_gl.viewport(Some(ui.viewport));
            push_camera_state();
            set_camera(&ui.camera());
            // 鎳掑姞杞界汗鐞嗭紙澶辫触鍚庝笉鍐嶉噸璇曪級
            if self.skip_panel_texture.is_none() && !self.skip_panel_load_failed {
                match image::load_from_memory(MESSAGE_PANEL_PNG) {
                    Ok(img) => {
                        let rgba = img.to_rgba8();
                        let (w, h) = (rgba.width(), rgba.height());
                        let pixels = rgba.into_raw();
                        self.skip_panel_texture = Some(Texture2D::from_rgba8(w as u16, h as u16, &pixels));
                    }
                    Err(_) => {
                        self.skip_panel_load_failed = true;
                    }
                }
            }
            if let Some(tex) = self.skip_panel_texture {
                // 杩涘叆鎷変几锛歄ut Quart锛屽墠0.5绉?
                let in_t = (self.skip_wait_timer / 0.5).min(1.0);
                let in_eased = 1.0 - (1.0 - in_t).powi(4);
                // 閫€鍑烘媺浼革細Out Quart锛岀敤 skip_fade_out_progress
                let out_eased = 1.0 - (1.0 - self.skip_fade_out_progress).powi(4);
                // 鏈€缁堟媺浼歌繘搴?= 杩涘叆 - 閫€鍑?
                let stretch_eased = (in_eased - out_eased).clamp(0.0, 1.0);
                // 闈㈡澘鏈€澶у搴︿负灞忓箷鐨?0%锛岄珮搴︽寜鍥剧墖姣斾緥
                let max_w = 1.2;
                let panel_h = max_w * tex.height() as f32 / tex.width() as f32;
                let panel_w = max_w * stretch_eased;
                let panel_x = -panel_w / 2.0;
                let panel_y = -panel_h / 2.0;
                draw_texture_ex(
                    tex,
                    panel_x,
                    panel_y,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(panel_w, panel_h)),
                        ..Default::default()
                    },
                );
                // 面板上的“跳过谱面”文字
                if stretch_eased >= 0.8 {
                    let text_fade = ((stretch_eased - 0.8) / 0.2).clamp(0.0, 1.0);
                    ui.text("跳过谱面")
                        .pos(0., 0.)
                        .anchor(0.5, 0.5)
                        .size(0.5)
                        .color(Color::new(1.0, 1.0, 1.0, text_fade))
                        .draw();
                }
            }
            pop_camera_state();
        }

        // 歌词渲染：贴回分支后 GL 通道可能仍停留在离屏 chart_target，
        // 需显式切回屏幕 render pass 并恢复 UI 相机，否则歌词被画进离屏缓冲不可见
        self.gl.quad_gl.render_pass(self.res.camera.render_pass());
        self.gl.quad_gl.viewport(Some(ui.viewport));
        push_camera_state();
        set_camera(&ui.camera());
        self.render_lyrics(ui, tm.now());
        pop_camera_state();

        // [Reveal] Fixed watermark at the top center, always visible during reveal mode.
        if self.mode == GameMode::Reveal {
            self.gl.quad_gl.render_pass(self.res.camera.render_pass());
            self.gl.quad_gl.viewport(Some(ui.viewport));
            push_camera_state();
            set_camera(&ui.camera());
            let top = ui.top;
            ui.text("Phira-Vrenxz\u{8c31}\u{9762}\u{63ed}\u{6653}-\u{6240}\u{6709}\u{8005}Lute_Rencai")
                .pos(0.0, -top + 0.02)
                .anchor(0.5, 0.0)
                .size(0.38)
                .color(Color::new(1.0, 0.92, 0.45, 1.0))
                .draw_using(&PGR_FONT);
            pop_camera_state();
        }



        // 手动轨道跳过提示条（Alt+S / 四指四角）
        if self.track_skip_active && !self.skip_done {
            self.gl.quad_gl.render_pass(self.res.camera.render_pass());
            self.gl.quad_gl.viewport(Some(ui.viewport));
            push_camera_state();
            set_camera(&ui.camera());
            let eased = (self.track_skip_progress * std::f32::consts::PI / 2.).sin();
            let top = -ui.top;
            let bar_h = 0.075 * eased;
            ui.fill_rect(Rect::new(-1., top, 2., bar_h), Color::new(0., 0., 0., 0.88));
            ui.fill_rect(
                Rect::new(-1., top, self.track_skip_white * 2., bar_h),
                Color::new(1., 1., 1., 0.9),
            );
            let alpha = (self.track_skip_hold / 0.3).clamp(0., 1.);
            ui.text(format!(
                "将在 {} 秒后跳过谱面，如需取消请松开按键",
                self.track_skip_countdown.ceil() as i32
            ))
            .pos(0., top + bar_h * 0.5)
            .anchor(0.5, 0.5)
            .size(0.32)
            .color(Color::new(1., 1., 1., 0.85 * alpha))
            .draw();
            pop_camera_state();
        }

        // HUD 自定义：编辑模式下在最上层绘制编辑层（选框/手柄/工具条）。
        if crate::hud::edit_active() {
            self.gl.quad_gl.render_pass(self.res.camera.render_pass());
            self.gl.quad_gl.viewport(Some(ui.viewport));
            push_camera_state();
            set_camera(&ui.camera());
            crate::hud::editor_render(ui);
            pop_camera_state();
        }

        // [HP 血量模式] 失败演出遮罩：红屏渐显 0.3s → 黑屏渐显 1s（置于最上层）
        if let Some(t) = self.fail_timer {
            self.gl.quad_gl.render_pass(self.res.camera.render_pass());
            self.gl.quad_gl.viewport(Some(ui.viewport));
            push_camera_state();
            set_camera(&ui.camera());
            let red_a = (t / Self::FAIL_RED_TIME).clamp(0.0, 1.0);
            if red_a > 0.0 {
                draw_rectangle(-1.5, -1.5, 3.0, 3.0, Color::new(1.0, 0.0, 0.0, red_a));
            }
            let black_a = ((t - Self::FAIL_RED_TIME) / Self::FAIL_BLACK_TIME).clamp(0.0, 1.0);
            if black_a > 0.0 {
                draw_rectangle(-1.5, -1.5, 3.0, 3.0, Color::new(0.0, 0.0, 0.0, black_a));
            }
            pop_camera_state();
        }
        if let Some(replay) = &self.replay_view {
            push_camera_state();
            set_camera(&ui.camera());
            for t in replay.active_touches(self.res.time as f32, 0.08) {
                ui.fill_circle(t.x, -t.y, 0.035, Color::new(1., 0.85, 0.2, 0.45));
            }
            let (p, g, b, m, acc) = replay.stats();
            let (early, late) = replay.early_late();
            let stats_y = -ui.top + 0.06;
            ui.text(format!(
                "REPLAY  P {p}  G {g}  B {b}  M {m}  E {early}  L {late}  ACC {:.2}%  OFF {:+.1}ms",
                acc * 100.,
                replay.recommended_offset_ms()
            ))
                .pos(0., stats_y)
                .anchor(0.5, 0.)
                .size(0.45)
                .color(Color::new(1., 1., 1., 0.8))
                .draw();
            pop_camera_state();
        }
        if self.res.config.note_debug {
            push_camera_state();
            set_camera(&ui.camera());
            let now = self.res.time;
            let mut y = -ui.top + 0.10;
            let mut count = 0;
            'outer: for (line_id, line) in self.chart.lines.iter().enumerate() {
                for (note_id, note) in line.notes.iter().enumerate() {
                    if (note.time - now).abs() > 2.0 || note.fake {
                        continue;
                    }
                    ui.text(format!("L{line_id} N{note_id} t={:.3} {:?}", note.time, note.kind))
                        .pos(-0.97, y)
                        .anchor(0., 0.)
                        .size(0.26)
                        .color(Color::new(1., 0.9, 0.4, 0.8))
                        .draw();
                    y += 0.032;
                    count += 1;
                    if count >= 24 {
                        break 'outer;
                    }
                }
            }
            pop_camera_state();
        }
        if self.mode == GameMode::Replay {
            self.render_replay_controls(ui);
        }
        Ok(())
    }


    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        if self.should_exit || self.next_scene.is_some() {
            crate::gamepad::set_gameplay_active(false);
        }
        if self.should_exit {
            super::set_ime_enabled(true);
            if tm.paused() {
                tm.resume();
                // 澶氫汉姝ｅ紡娓哥帺锛氭殏鍋滀腑鐩存帴閫€鍑烘椂澶嶄綅鏈嶅姟鍣ㄤ晶鐘舵€侊紝
                // 鍚﹀垯鎴块棿閲屽叾浠栦汉浼氫竴鐩寸湅鍒般€屾殏鍋滀腑銆?
                notify_paused(self.pause_notify.as_ref(), false);
            }
            tm.speed = 1.0;
            tm.adjust_time = false;
            match self.mode {
                GameMode::Normal => {
                    if let Some(rec) = &self.best_record {
                        NextScene::PopWithResult(Box::new(rec.clone()))
                    } else {
                        NextScene::Pop
                    }
                }
                GameMode::Exercise | GameMode::NoRetry | GameMode::View | GameMode::Reveal | GameMode::Replay
                | GameMode::Tutorial | GameMode::Lan => NextScene::Pop,
                GameMode::TweakOffset => NextScene::PopWithResult(Box::new(None::<f32>)),
                GameMode::Challenge | GameMode::ReChallenge => {
                    // 非自然完成（返回键 / HP 耗尽等触发退出）→ 课题整场作废
                    if !self.challenge_finished {
                        if let Some(cfg) = &self.challenge {
                            if let Ok(mut s) = cfg.session.lock() {
                                s.aborted = true;
                            }
                        }
                    }
                    NextScene::Pop
                }
            }
        } else if let Some(task) = self.challenge_next.as_mut() {
            // [课题模式] 自然完成后直接切下一局加载页（不进课题页）；加载失败 → 课题作废回课题页
            if let Some(result) = poll_future(task.as_mut()) {
                self.challenge_next = None;
                match result {
                    Ok(ns) => ns,
                    Err(err) => {
                        eprintln!("[challenge] 课题下一局加载失败: {err:?}");
                        if let Some(cfg) = &self.challenge {
                            if let Ok(mut s) = cfg.session.lock() {
                                s.aborted = true;
                            }
                        }
                        NextScene::Pop
                    }
                }
            } else {
                NextScene::None
            }
        } else if let Some(next_scene) = self.next_scene.take() {
            if !matches!(next_scene, NextScene::None) && tm.paused() {
                tm.resume();
            }
            tm.speed = 1.0;
            tm.adjust_time = false;
            next_scene
        } else {
            NextScene::None
        }
    }
}
