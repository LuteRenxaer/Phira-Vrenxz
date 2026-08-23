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
    pub character_name: String,
    pub character_model_path: Option<String>,
    pub character_default_expr: u32,
    pub character_pet_expr: u32,
    pub character_voice_dir: Option<String>,
    pub character_model_offset_x: f32,
    pub character_model_offset_y: f32,
    pub character_model_scale: f32,
    pub character_pet_offset_x: f32,
    pub character_pet_offset_y: f32,
    pub character_pet_width: f32,
    pub character_pet_height: f32,
    pub show_character: bool,
    pub score_offset_x: f32,
    pub score_offset_y: f32,
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
    pub speed: f32,
    pub touch_debug: bool,
    pub use_keyboard: bool,
    pub volume_bgm: f32,
    pub volume_music: f32,
    pub volume_sfx: f32,


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
            custom_watermark: "phirLie".to_string(),
            show_score: true,
            show_score_initialized: false,
            show_combo: true,
            custom_accent: "#4C84FF".to_string(),
            character_name: "星野(临战)".to_string(),
            character_model_path: None,
            character_default_expr: 0,
            character_pet_expr: 24,
            character_voice_dir: None,
            character_model_offset_x: 0.,
            character_model_offset_y: 0.,
            character_model_scale: 0.00065,
            character_pet_offset_x: 0.,
            character_pet_offset_y: -0.650,
            character_pet_width: 0.3,
            character_pet_height: 0.3,
            show_character: true,
            score_offset_x: 0.0,
            score_offset_y: 0.0,
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