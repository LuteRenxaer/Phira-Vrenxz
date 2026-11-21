
use bitflags::bitflags;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

pub static TIPS: Lazy<Vec<String>> = Lazy::new(|| {
    include_str!("tips.txt")
        .split('\n')
        .map(str::to_owned)
        .collect()
});

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DynamicBackgroundMode {
    #[default]
    Off,
    StaticBrightness,
    DynamicBrightness,
}

impl DynamicBackgroundMode {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::StaticBrightness => 1,
            Self::DynamicBrightness => 2,
        }
    }
}

const DEFAULT_METRICS_PARALLEL_MIN: usize = 4096;

bitflags! {
    #[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq, Debug)]
    #[serde(transparent)]
    pub struct Mods: i32 {
        const AUTOPLAY = 0x0001;
        const FLIP_X = 0x0002;
        const FADE_OUT = 0x0004;
        const FADE_IN = 0x0008;
        const NIGHTCORE = 0x0010;
        const RAINBOW = 0x0020;
        const NO_SHADER = 0x0040;
        const INSTANT_DEATH_AP = 0x0080;
        const INSTANT_DEATH_FC = 0x0100;

        const GHOST = 0x0200;

        const FLIP_Y = 0x0400;

        const RANDOM_X = 0x1000;

        const FX_TV = 0x2000;
        const FX_SCANLINE = 0x4000;
        const FX_GLITCH = 0x8000;

        const UNRATED = Self::AUTOPLAY.bits() | Self::NO_SHADER.bits();
    }
}

impl Mods {
    pub fn toggle_mod(&mut self, flag: Mods) {
        if self.contains(flag) {
            self.remove(flag);
        } else {
            for &conflict in Mods::conflicts(flag) {
                self.remove(conflict);
            }
            self.insert(flag);
        }
    }
    fn conflicts(flag: Mods) -> &'static [Mods] {
        match flag {
            Mods::FADE_IN => &[Mods::FADE_OUT],
            Mods::FADE_OUT => &[Mods::FADE_IN],
            Mods::INSTANT_DEATH_AP => &[Mods::INSTANT_DEATH_FC],
            Mods::INSTANT_DEATH_FC => &[Mods::INSTANT_DEATH_AP],
            Mods::FX_TV => &[Mods::FX_SCANLINE, Mods::FX_GLITCH],
            Mods::FX_SCANLINE => &[Mods::FX_TV, Mods::FX_GLITCH],
            Mods::FX_GLITCH => &[Mods::FX_TV, Mods::FX_SCANLINE],
            _ => &[],
        }
    }
}

fn default_custom_crash_code() -> u32 {
    888
}

fn default_custom_crash_reason() -> String {
    String::new()
}

fn default_custom_crash_title() -> String {
    String::new()
}

/// 性能档位默认值：完全优化（与引擎现状一致）
fn default_performance() -> u8 {
    Config::PERF_FULL
}

fn default_noise_color() -> String {
    "#FF0000".to_string()
}

fn default_noise_demo() -> u8 {
    1
}

fn default_noise_opacity() -> f32 {
    1.0
}

fn default_true() -> bool {
    true
}

fn default_perf_lowres() -> u32 {
    100
}

fn default_perf_fx_density() -> u32 {
    20
}

fn default_judge_perfect_ms() -> f32 {
    80.
}

fn default_judge_good_ms() -> f32 {
    160.
}

fn default_judge_bad_ms() -> f32 {
    220.
}

fn default_retry_rewind() -> f32 {
    3.
}

fn default_hp_custom_max() -> f32 {
    100.
}

fn default_exercise_step() -> f32 {
    0.05
}

fn default_great_ms() -> f32 {
    70.
}

fn default_ok_ms() -> f32 {
    130.
}

fn default_meh_ms() -> f32 {
    160.
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(rename = "adjust_time_new")]
    pub adjust_time: bool,
    pub aggressive: bool,
    pub ap_fc_indicator: bool,
    /// 准度指示器（误差刻度条）显示开关
    #[serde(default = "default_true")]
    pub indicator_enabled: bool,
    /// 准度指示器延迟模式：false=冰与火（沿用判定上限 ±220ms），true=phigros（±160ms 边界，±80 Perfect / ±160 Good，超界贴边显示）
    #[serde(default)]
    pub indicator_delay_phigros: bool,
    /// 指示器显示颜色模式：false=冰与火（内置固定色），true=phira（Perfect/Good 取皮肤 yml colorPerfect/colorGood，bad/miss 固定 0xebff7474）
    #[serde(default)]
    pub indicator_color_phira: bool,
    pub full_screen_judge: bool,
    /// 普通游玩自定义判定窗口（毫秒）；0/0/0 表示使用内置默认。
    #[serde(default = "default_judge_perfect_ms")]
    pub judge_perfect_ms: f32,
    #[serde(default = "default_judge_good_ms")]
    pub judge_good_ms: f32,
    #[serde(default = "default_judge_bad_ms")]
    pub judge_bad_ms: f32,
    /// 晚按补偿（毫秒），从 Late 误差中扣除。
    #[serde(default)]
    pub judge_late_leniency_ms: f32,
    /// Perfect+ 判定阈值（毫秒），0 关闭。
    #[serde(default)]
    pub judge_perfect_plus_ms: f32,
    /// 细致判定：Good 再分为 Great/Ok/Meh。
    #[serde(default)]
    pub judge_detailed: bool,
    #[serde(default = "default_great_ms")]
    pub judge_great_ms: f32,
    #[serde(default = "default_ok_ms")]
    pub judge_ok_ms: f32,
    #[serde(default = "default_meh_ms")]
    pub judge_meh_ms: f32,
    #[serde(default)]
    pub judge_preset: JudgePreset,
    pub combo_text_debug: bool,
    /// Global Arcaea-style judgement mode (disables score upload)
    pub arcaea_judgement: bool,
    /// Global FNF-style judgement mode (disables score upload)
    pub fnf_judgement: bool,
    pub custom_combo_text: String,
    pub custom_watermark: String,
    pub show_score: bool,
    pub show_score_initialized: bool,
    pub show_combo: bool,
    pub custom_accent: String,
    pub score_offset_x: f32,
    pub score_offset_y: f32,
    /// 游玩界面实时准确率偏移（相对分数下方默认位）
    pub play_acc_offset_x: f32,
    pub play_acc_offset_y: f32,
    /// 结算统计面板（两格统计卡）整体偏移
    pub result_offset_x: f32,
    pub result_offset_y: f32,
    pub combo_offset_x: f32,
    pub combo_offset_y: f32,
    pub home_play_offset_x: f32,
    pub home_play_offset_y: f32,
    pub home_menu_offset_x: f32,
    pub home_menu_offset_y: f32,
    pub old_home: bool,
    /// 玩家选的内置主页背景文件名（`assets/backgrounds/` 下的文件名，如 `bg1.png`）。
    ///
    /// - `None`：没选过 —— 沿用 [`Config::old_home`] 决定的那张内置默认
    ///   （`background.jpg` / `background_old.jpg`），保证默认观感不变；
    /// - `Some(name)`：玩家在设置页显式选过这一张。
    ///
    /// 优先级（从高到低）：自定义背景图（Phira-Vrenxz 的 `custom_background_path`）
    /// > 本项 > `old_home`。
    ///
    /// 老 data.json 里没有这个字段：容器级 `#[serde(default)]` 会把它填成 `None`，
    /// 既不报错也不会动到别的配置。
    #[serde(default)]
    pub home_background: Option<String>,
    pub aspect_ratio: Option<f32>,
    pub audio_buffer_size: Option<u32>,
    pub chart_debug: bool,
    pub roman_numerals: bool,
    pub chinese_numerals: bool,
    pub autoplay_display_text: String,
    /// Autoplay 模式显示连击数 + COMBO 文本（false 时显示 Autoplay 文本）
    #[serde(default)]
    pub autoplay_combo_style: bool,
    pub disable_effect: bool,
    pub double_click_to_pause: bool,
    #[serde(default)]
    pub dynamic_background: DynamicBackgroundMode,
    pub double_hint: bool,
    pub fullscreen_mode: bool,
    pub fxaa: bool,
    pub interactive: bool,
    pub mods: Mods,
    pub mp_address: String,
    pub mp_enabled: bool,
    pub note_scale: f32,
    pub offline_mode: bool,
    pub offset: f32,
    pub particle: bool,
    pub player_name: String,
    pub player_rks: f32,
    pub preferred_sample_rate: Option<u32>,
    pub res_pack_path: Option<String>,
    pub sample_count: u32,
    pub show_acc: bool,
    pub show_avg_fps: bool,
    /// 游玩中实时显示 FPS
    #[serde(default)]
    pub show_fps: bool,
    /// FPS 文字位置：0 左上 / 1 右上 / 2 左下 / 3 右下。
    #[serde(default)]
    pub fps_position: u8,
    pub speed: f32,
    /// 失败后自动重试次数（0 关闭）。
    #[serde(default)]
    pub auto_retry: u8,
    /// 自动重试时从失败前多少秒重开。
    #[serde(default = "default_retry_rewind")]
    pub retry_rewind_sec: f32,
    /// 自动保存本地回放触点。
    #[serde(default = "default_true")]
    pub record_replay: bool,
    /// 帧率上限；0 = 不限制。
    #[serde(default)]
    pub fps_limit: u32,
    /// 练习模式每轮自动提速。
    #[serde(default)]
    pub exercise_auto_speed: bool,
    #[serde(default = "default_exercise_step")]
    pub exercise_speed_step: f32,
    pub touch_debug: bool,
    #[serde(default)]
    pub note_debug: bool,
    /// 上传成绩开关（默认关闭；开启后成绩将上传至服务器，被封后果自负）
    #[serde(default)]
    pub upload_score_enabled: bool,
    /// 成绩上传目标服务器。
    pub use_keyboard: bool,
    pub volume_bgm: f32,
    pub volume_music: f32,
    pub volume_sfx: f32,
    /// 控制台开关（Windows 调试控制台，settings 里可切换）
    pub console_enabled: bool,
    /// 垂直同步开关（settings 里可切换；关闭后帧率不再被刷新率限制）
    pub vsync: bool,

    /// 性能优化档位：0=无 1=少量 2=中等 3=完全 4=完全积极 5=自定义
    #[serde(default = "default_performance")]
    pub performance: u8,
    // —— 自定义档（performance == 5 时生效）——
    /// 负载统计并行
    #[serde(default = "default_true")]
    pub perf_custom_metrics: bool,
    /// 屏幕外剔除
    #[serde(default = "default_true")]
    pub perf_custom_cull: bool,
    /// 粒子削减（保留 hit_fx，连续 note 减半）
    #[serde(default)]
    pub perf_custom_particles: bool,
    /// 强制关闭垂直同步
    #[serde(default)]
    pub perf_custom_vsync_off: bool,
    /// 自定义低清 Note 阈值（可见数）
    #[serde(default = "default_perf_lowres")]
    pub perf_custom_lowres: u32,
    /// 自定义打击特效密度阈值
    #[serde(default = "default_perf_fx_density")]
    pub perf_custom_fx_density: u32,

    #[serde(default = "default_custom_crash_code")]
    pub custom_crash_code: u32,
    #[serde(default = "default_custom_crash_reason")]
    pub custom_crash_reason: String,
    #[serde(default = "default_custom_crash_title")]
    pub custom_crash_title: String,

    /// 主界面是否显示"章节"按钮
    #[serde(default = "default_true")]
    pub home_chapter_button_show: bool,

    /// Flat block ranges; skips materials, hover and music filtering, not judgement
    #[serde(default)]
    pub block_area_simple: bool,

    // ===== 视觉标签页：噪域设置 =====
    /// 噪域颜色（`#RRGGBB` 十六进制）
    #[serde(default = "default_noise_color")]
    pub noise_color: String,
    /// 噪域演示模式：1=仿 phi，2=纯色块
    #[serde(default = "default_noise_demo")]
    pub noise_demo: u8,
    /// 噪域 glitch 效果开关
    #[serde(default)]
    pub noise_glitch: bool,
    /// 噪域透明度（0.0~1.0）
    #[serde(default = "default_noise_opacity")]
    pub noise_opacity: f32,

    // ===== 视觉标签页（GUI 开关）=====
    /// 版本水印开关（默认打开）
    #[serde(default = "default_true")]
    pub show_watermark: bool,
    /// 难度与谱面名称显示开关（默认打开）
    #[serde(default = "default_true")]
    pub show_chart_info: bool,

    // ===== 谱面标签页：噪域整体开关 =====
    /// 谱面噪域（blockArea）功能开关（默认打开；关闭后不渲染噪域也不拦截触摸）
    #[serde(default = "default_true")]
    pub noise_enabled: bool,

    // ===== HP 血量模式（普通游玩为 mod）=====
    /// 普通游玩 HP mod：Off 关闭；1/20/50 非 Perfect 扣 1；100/200 Bad/Miss 扣 1
    #[serde(default)]
    pub hp: HpMode,
    #[serde(default = "default_hp_custom_max")]
    pub hp_custom_max: f32,
    #[serde(default)]
    pub hp_custom_bad_only: bool,

    autoplay: Option<bool>,
}

/// 普通游玩 HP 血量档位
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JudgePreset {
    #[default]
    Default,
    Phigros,
    Phira,
    Custom,
}

impl JudgePreset {
    pub fn windows_ms(self) -> [f32; 3] {
        match self {
            Self::Default | Self::Phigros => [80., 160., 220.],
            Self::Phira => [120., 200., 250.],
            Self::Custom => [80., 160., 220.],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HpMode {
    #[default]
    Off,
    Hp1,
    Hp20,
    Hp50,
    Hp100,
    Hp200,
    Custom,
}

impl HpMode {
    /// 普通游玩 HP 档位 → 引擎 HP 配置：
    /// 1/20/50 非 Perfect 扣 1；100/200 Bad/Miss 扣 1；Off 为 None
    /// 普通游玩自定义判定窗口；无效值回退内置默认。
    pub fn hp_config(&self) -> Option<crate::scene::HpConfig> {
        use crate::scene::{HpConfig, HpLoseCondition};
        match self {
            HpMode::Off => None,
            HpMode::Hp1 => Some(HpConfig { max_hp: 1., lose_condition: HpLoseCondition::NonPerfect }),
            HpMode::Hp20 => Some(HpConfig { max_hp: 20., lose_condition: HpLoseCondition::NonPerfect }),
            HpMode::Hp50 => Some(HpConfig { max_hp: 50., lose_condition: HpLoseCondition::NonPerfect }),
            HpMode::Hp100 => Some(HpConfig { max_hp: 100., lose_condition: HpLoseCondition::BadOrMiss }),
            HpMode::Hp200 => Some(HpConfig { max_hp: 200., lose_condition: HpLoseCondition::BadOrMiss }),
            HpMode::Custom => Some(HpConfig { max_hp: 100., lose_condition: HpLoseCondition::BadOrMiss }),
        }
    }
}


impl Default for Config {
    fn default() -> Self {
        Self {
            adjust_time: false,
            aggressive: true,
            ap_fc_indicator: true,
            indicator_enabled: default_true(),
            indicator_delay_phigros: false,
            indicator_color_phira: false,
            full_screen_judge: false,
            judge_perfect_ms: default_judge_perfect_ms(),
            judge_good_ms: default_judge_good_ms(),
            judge_bad_ms: default_judge_bad_ms(),
            judge_late_leniency_ms: 0.,
            judge_perfect_plus_ms: 0.,
            judge_detailed: false,
            judge_great_ms: default_great_ms(),
            judge_ok_ms: default_ok_ms(),
            judge_meh_ms: default_meh_ms(),
            judge_preset: JudgePreset::Default,
            combo_text_debug: false,
            arcaea_judgement: false,
            fnf_judgement: false,
            custom_combo_text: "COMBO".to_string(),
            custom_watermark: "Phira-Vrenxz".to_string(),
            show_score: true,
            show_score_initialized: false,
            show_combo: true,
            custom_accent: "#4C84FF".to_string(),
            score_offset_x: 0.0,
            score_offset_y: 0.0,
            play_acc_offset_x: 0.0,
            play_acc_offset_y: 0.0,
            result_offset_x: 0.0,
            result_offset_y: 0.0,
            combo_offset_x: 0.0,
            combo_offset_y: 0.0,
            home_play_offset_x: 0.0,
            home_play_offset_y: 0.0,
            home_menu_offset_x: 0.0,
            home_menu_offset_y: 0.0,
            old_home: false,
            home_background: None,
            aspect_ratio: None,
            audio_buffer_size: None,
            chart_debug: false,
            roman_numerals: false,
            chinese_numerals: false,
            autoplay_display_text: "Autoplay".to_string(),
            autoplay_combo_style: false,
            disable_effect: false,
            double_click_to_pause: true,
            dynamic_background: DynamicBackgroundMode::Off,
            double_hint: true,
            fxaa: false,
            interactive: true,
            mods: Mods::default(),
            mp_address: "mp2.phira.cn:12345".to_owned(),
            mp_enabled: false,
            note_scale: 1.0,
            offline_mode: false,
            fullscreen_mode: false,
            offset: 0.,
            particle: true,
            player_name: "Mivik".to_string(),
            player_rks: 15.,
            preferred_sample_rate: None,
            res_pack_path: None,
            sample_count: 1,
            show_acc: false,
            show_avg_fps: false,
            show_fps: false,
            fps_position: 0,
            speed: 1.,
            auto_retry: 0,
            retry_rewind_sec: 3.,
            record_replay: true,
            fps_limit: 0,
            exercise_auto_speed: false,
            exercise_speed_step: default_exercise_step(),
            touch_debug: false,
            note_debug: false,
            upload_score_enabled: false,
            use_keyboard: false,
            volume_music: 1.,
            volume_sfx: 1.,
            volume_bgm: 1.,
            custom_crash_code: default_custom_crash_code(),
            custom_crash_reason: default_custom_crash_reason(),
            custom_crash_title: default_custom_crash_title(),
            home_chapter_button_show: default_true(),
            block_area_simple: false,
            noise_color: default_noise_color(),
            noise_demo: default_noise_demo(),
            noise_glitch: false,
            noise_opacity: default_noise_opacity(),
            show_watermark: default_true(),
            show_chart_info: default_true(),
            noise_enabled: default_true(),
            hp: HpMode::Off,
            hp_custom_max: default_hp_custom_max(),
            hp_custom_bad_only: false,
            autoplay: None,
            console_enabled: false,
            vsync: true,
            performance: default_performance(),
            perf_custom_metrics: default_true(),
            perf_custom_cull: default_true(),
            perf_custom_particles: false,
            perf_custom_vsync_off: false,
            perf_custom_lowres: default_perf_lowres(),
            perf_custom_fx_density: default_perf_fx_density(),
        }
    }
}

impl Config {
    pub fn hp_config(&self) -> Option<crate::scene::HpConfig> {
        use crate::scene::{HpConfig, HpLoseCondition};
        if self.hp == HpMode::Custom {
            return Some(HpConfig {
                max_hp: self.hp_custom_max.clamp(1., 999.),
                lose_condition: if self.hp_custom_bad_only {
                    HpLoseCondition::BadOrMiss
                } else {
                    HpLoseCondition::NonPerfect
                },
            });
        }
        self.hp.hp_config()
    }

    /// 普通游玩自定义判定窗口；无效值回退内置默认。
    pub fn custom_judge_limits(&self) -> Option<(f64, f64, f64)> {        if self.judge_preset != JudgePreset::Custom {
            return None;
        }
        let p = self.judge_perfect_ms / 1000.;
        let g = self.judge_good_ms / 1000.;
        let b = self.judge_bad_ms / 1000.;
        (p.is_finite() && g.is_finite() && b.is_finite() && p > 0. && p <= g && g <= b)
            .then_some((p as f64, g as f64, b as f64))
    }

    pub fn init(&mut self) {
        if let Some(flag) = self.autoplay {
            self.mods.set(Mods::AUTOPLAY, flag);
        }
        #[cfg(target_env = "ohos")]
        {

            self.sample_count = 1;
        }
    }

    #[inline]
    pub fn has_mod(&self, m: Mods) -> bool {
        self.mods.contains(m)
    }

    #[inline]
    pub fn autoplay(&self) -> bool {
        self.has_mod(Mods::AUTOPLAY)
    }

    #[inline]
    pub fn flip_x(&self) -> bool {
        self.has_mod(Mods::FLIP_X)
    }

    #[inline]
    pub fn flip_y(&self) -> bool {
        self.has_mod(Mods::FLIP_Y)
    }

    /// 解析噪域颜色（`#RRGGBB` → (r,g,b) 0..1）。解析失败回退红色。
    pub fn noise_color_rgb(&self) -> (f32, f32, f32) {
        let s = self.noise_color.trim().trim_start_matches('#');
        if s.len() == 6 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&s[0..2], 16),
                u8::from_str_radix(&s[2..4], 16),
                u8::from_str_radix(&s[4..6], 16),
            ) {
                return (r as f32 / 255., g as f32 / 255., b as f32 / 255.);
            }
        }
        (1., 0., 0.)
    }

    /// 噪域是否以纯色块演示模式渲染（2=纯色块；兼容旧的 block_area_simple）
    pub fn noise_flat_mode(&self) -> bool {
        self.block_area_simple || self.noise_demo == 2
    }

    // ==== 性能优化档位 ====
    /// 档位：0=无优化，1=少量，2=中等，3=完全，4=完全积极，5=自定义
    pub const PERF_OFF: u8 = 0;
    pub const PERF_LIGHT: u8 = 1;
    pub const PERF_MEDIUM: u8 = 2;
    pub const PERF_FULL: u8 = 3;
    pub const PERF_ULTRA: u8 = 4;
    pub const PERF_CUSTOM: u8 = 5;

    #[inline]
    pub fn perf_profile(&self) -> u8 {
        self.performance.min(Self::PERF_CUSTOM)
    }

    #[inline]
    pub fn perf_is_custom(&self) -> bool {
        self.perf_profile() == Self::PERF_CUSTOM
    }

    /// 负载统计（每帧 Note 统计）是否启用并行
    pub fn metrics_parallel(&self) -> bool {
        match self.perf_profile() {
            Self::PERF_OFF | Self::PERF_LIGHT => false,
            Self::PERF_MEDIUM | Self::PERF_FULL | Self::PERF_ULTRA => true,
            _ => self.perf_custom_metrics,
        }
    }

    /// 负载统计启用并行所需的最小“活跃 Note”数量（中等 = 现有一半力度，即阈值放大一倍）
    pub fn metrics_parallel_min(&self) -> usize {
        match self.perf_profile() {
            Self::PERF_MEDIUM => 2 * DEFAULT_METRICS_PARALLEL_MIN,
            Self::PERF_FULL | Self::PERF_ULTRA => DEFAULT_METRICS_PARALLEL_MIN,
            Self::PERF_CUSTOM if self.perf_custom_metrics => DEFAULT_METRICS_PARALLEL_MIN,
            _ => usize::MAX,
        }
    }

    /// 屏幕外剔除（aggressive）是否生效；非自定义档由档位接管
    pub fn eff_cull(&self) -> bool {
        match self.perf_profile() {
            Self::PERF_OFF => false,
            Self::PERF_LIGHT..=Self::PERF_ULTRA => true,
            _ => self.aggressive,
        }
    }

    /// 低分辨率 Note 渲染阈值（可见 Note 数 ≥ 该值）；无优化档永不启用
    pub fn eff_lowres_threshold(&self) -> usize {
        match self.perf_profile() {
            Self::PERF_OFF => usize::MAX,
            Self::PERF_LIGHT..=Self::PERF_FULL => 100,
            Self::PERF_ULTRA => 60,
            _ => self.perf_custom_lowres as usize,
        }
    }

    /// 打击特效密度阈值（即将击打数 > 该值时关闭打击特效）。
    ///
    /// - 完全优化（默认档）：阈值放宽到 500——普通谱粒子特效全开，
    ///   只有 SkyFire 这类超密压测谱（即将击打 note 常年远超 500）才自动抑制，保住帧率；
    /// - 无 / 少量 / 中等：保留旧的密度自适应（20）；
    /// - 完全积极：更早触发（12），并叠加粒子削减；
    /// - 自定义：使用自定义滑杆值。
    pub fn eff_fx_density_threshold(&self) -> usize {
        usize::MAX
    }

    /// 粒子削减：保留 hit_fx 主粒子、连续发射每两次减一次（完全积极 / 自定义）
    pub fn eff_fx_reduce(&self) -> bool {
        match self.perf_profile() {
            Self::PERF_ULTRA => true,
            Self::PERF_CUSTOM => self.perf_custom_particles,
            _ => false,
        }
    }

    /// 是否强制关闭垂直同步
    pub fn eff_vsync_off(&self) -> bool {
        match self.perf_profile() {
            Self::PERF_ULTRA => true,
            _ => self.perf_custom_vsync_off,
        }
    }

    /// 粒子总开关是否生效（档位接管：非自定义档粒子保持开启，削减按档位）；
    /// 自定义档使用 `particle` 手动开关。
    pub fn eff_particles(&self) -> bool {
        if self.perf_is_custom() {
            self.particle
        } else {
            true
        }
    }

    #[inline]
    pub fn full_screen_judge(&self) -> bool {
        self.full_screen_judge
    }

    #[inline]
    pub fn combo_text_debug(&self) -> bool {
        self.combo_text_debug
    }

    #[inline]
    pub fn custom_watermark(&self) -> &str {
        &self.custom_watermark
    }

    #[inline]
    pub fn custom_combo_text(&self) -> &str {
        &self.custom_combo_text
    }
}