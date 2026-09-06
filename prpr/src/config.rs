//! Configuration module of the playing environment.
//! e.g. player name, volume, speed, autoplay, etc.

use bitflags::bitflags;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

pub static TIPS: Lazy<Vec<String>> = Lazy::new(|| {
    include_str!("tips.txt")
        .split('\n')
        .map(str::to_owned)
        .collect()
});

/// 负载统计并行所需的最小“活跃 Note”数（与引擎默认一致）
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

        // ==== 花样 mod ====
        /// 幽灵：Note（含 Hold）半透明
        const GHOST = 0x0200;
        /// 垂直反转：谱面沿 Y 轴（上下）镜像显示与判定
        const FLIP_Y = 0x0400;
        /// 横摆：每个 Note 在横向上按确定性的伪随机位置散开（仅视觉）
        const RANDOM_X = 0x1000;
        /// 屏幕特效：老电视 / 扫描线 / 故障（互斥，见 [`Mods::conflicts`]）
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

fn default_true() -> bool {
    true
}

fn default_perf_lowres() -> u32 {
    100
}

fn default_perf_fx_density() -> u32 {
    20
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(rename = "adjust_time_new")]
    pub adjust_time: bool,
    pub aggressive: bool,
    pub ap_fc_indicator: bool,
    pub full_screen_judge: bool,
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
    pub aspect_ratio: Option<f32>,
    pub audio_buffer_size: Option<u32>,
    pub chart_debug: bool,
    pub roman_numerals: bool,
    pub chinese_numerals: bool,
    pub autoplay_display_text: String,
    pub disable_effect: bool,
    pub double_click_to_pause: bool,
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
    pub speed: f32,
    pub touch_debug: bool,
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


    autoplay: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            adjust_time: false,
            aggressive: true,
            ap_fc_indicator: true,
            full_screen_judge: false,
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
            aspect_ratio: None,
            audio_buffer_size: None,
            chart_debug: false,
            roman_numerals: false,
            chinese_numerals: false,
            autoplay_display_text: "Autoplay".to_string(),
            disable_effect: false,
            double_click_to_pause: true,
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
            speed: 1.,
            touch_debug: false,
            use_keyboard: false,
            volume_music: 1.,
            volume_sfx: 1.,
            volume_bgm: 1.,
            custom_crash_code: default_custom_crash_code(),
            custom_crash_reason: default_custom_crash_reason(),
            custom_crash_title: default_custom_crash_title(),
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
        match self.perf_profile() {
            Self::PERF_ULTRA => 12,
            Self::PERF_FULL => 500,
            Self::PERF_CUSTOM => self.perf_custom_fx_density as usize,
            _ => 20,
        }
    }

    /// 粒子削减：保留 hit_fx 主粒子、连续发射每两次减一次（完全积极 / 自定义）
    pub fn eff_fx_reduce(&self) -> bool {
        match self.perf_profile() {
            Self::PERF_ULTRA => true,
            _ => self.perf_custom_particles,
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