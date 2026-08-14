prpr_l10n::tl_file!("settings");

use super::{NextPage, OffsetPage, Page, SharedState};
use crate::{
    dir, get_data, get_data_mut,
    popup::ChooseButton,
    save_data,
    scene::BGM_VOLUME_UPDATED,
    sync_data,
    tabs::{Tabs, TitleFn},
};
use anyhow::Result;
use bytesize::ByteSize;
use inputbox::InputBox;
use macroquad::prelude::*;
use once_cell::sync::Lazy;
use prpr::{
    ext::{open_url, poll_future, semi_black, semi_white, LocalTask, RectExt, SafeTexture},
    scene::{request_file, request_input, return_file, return_input, show_error, show_message, take_file, take_input, CrashCode, CrashScene, NextScene},
    task::Task,
    ui::{DRectButton, Scroll, Slider, Ui, PREFER_REDUCED_MOTION},
};
use prpr_l10n::{LanguageIdentifier, LANG_IDENTS, LANG_NAMES};
use reqwest::Url;
use serde::Deserialize;
use std::{borrow::Cow, fs, io, net::ToSocketAddrs, path::PathBuf, sync::atomic::Ordering};

const ITEM_HEIGHT: f32 = 0.15;
const INTERACT_WIDTH: f32 = 0.28;
const STATUS_PAGE: &str = "https://status.phira.cn";

struct NameList(String);
impl<'de> Deserialize<'de> for NameList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = Vec::<String>::deserialize(deserializer)?;
        Ok(Self(s.join(", ")))
    }
}

#[derive(Deserialize)]
struct LocalizationListRaw {
    #[serde(rename = "en-US")]
    en_us: NameList,
    #[serde(rename = "fr-FR")]
    fr_fr: NameList,
    #[serde(rename = "de-DE")]
    de_de: NameList,
    #[serde(rename = "id-ID")]
    id_id: NameList,
    #[serde(rename = "ja-JP")]
    ja_jp: NameList,
    #[serde(rename = "ko-KR")]
    ko_kr: NameList,
    #[serde(rename = "pl-PL")]
    pl_pl: NameList,
    #[serde(rename = "pt-BR")]
    pt_br: NameList,
    #[serde(rename = "ru-RU")]
    ru_ru: NameList,
    #[serde(rename = "th-TH")]
    th_th: NameList,
    #[serde(rename = "zh-TW")]
    zh_tw: NameList,
    #[serde(rename = "tr-TR")]
    tr_tr: NameList,
    #[serde(rename = "vi-VN")]
    vi_vn: NameList,
}

struct LocalizationList(String);
impl<'de> Deserialize<'de> for LocalizationList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = LocalizationListRaw::deserialize(deserializer)?;
        Ok(Self(format!(
            "\
English (en-US)\n{}\n
French (fr-FR)\n{}\n
German (de-DE)\n{}\n
Indonesian (id-ID)\n{}\n
Japanese (ja-JP)\n{}\n
Korean (ko-KR)\n{}\n
Polish (pl-PL)\n{}\n
Portuguese (pt-BR)\n{}\n
Russian (ru-RU)\n{}\n
Thai (th-TH)\n{}\n
Traditional Chinese (zh-TW)\n{}\n
Turkish (tr-TR)\n{}\n
Vietnamese (vi-VN)\n{}",
            raw.en_us.0,
            raw.fr_fr.0,
            raw.de_de.0,
            raw.id_id.0,
            raw.ja_jp.0,
            raw.ko_kr.0,
            raw.pl_pl.0,
            raw.pt_br.0,
            raw.ru_ru.0,
            raw.th_th.0,
            raw.zh_tw.0,
            raw.tr_tr.0,
            raw.vi_vn.0
        )))
    }
}

#[derive(Deserialize)]
struct StaffList {
    development: NameList,
    operations: NameList,
    documentation: NameList,
    art: NameList,
    music: NameList,
    audio: NameList,
    community: NameList,
    localization: LocalizationList,
}

static STAFF_LIST: Lazy<StaffList> = Lazy::new(|| {
    let data = include_str!("../../staff.yml");
    serde_yaml::from_str(data).unwrap()
});

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingListType {
    General,
    Audio,
    Chart,
    Debug,
    About,
}

pub struct SettingsPage {
    list_general: GeneralList,
    list_audio: AudioList,
    list_chart: ChartList,
    list_debug: DebugList,

    tabs: Tabs<SettingListType>,

    scroll: Scroll,
    save_time: f32,

    icon: SafeTexture,
}

impl SettingsPage {
    const SAVE_TIME: f32 = 0.5;

    pub fn new(icon: SafeTexture, icon_lang: SafeTexture) -> Self {
        Self {
            list_general: GeneralList::new(icon_lang),
            list_audio: AudioList::new(),
            list_chart: ChartList::new(),
            list_debug: DebugList::new(),

            tabs: Tabs::new([
                (SettingListType::General, || tl!("general")),
                (SettingListType::Audio, || tl!("audio")),
                (SettingListType::Chart, || tl!("chart")),
                (SettingListType::Debug, || tl!("debug")),
                (SettingListType::About, || tl!("about")),
            ] as [(SettingListType, TitleFn); 5]),

            scroll: Scroll::new(),
            save_time: f32::INFINITY,

            icon,
        }
    }
}

impl Page for SettingsPage {
    fn label(&self) -> Cow<'static, str> {
        tl!("label")
    }

    fn exit(&mut self) -> Result<()> {
        BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
        if self.save_time.is_finite() {
            save_data()?;
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        if match self.tabs.selected() {
            SettingListType::General => self.list_general.top_touch(touch, t),
            SettingListType::Audio => self.list_audio.top_touch(touch, t),
            SettingListType::Chart => self.list_chart.top_touch(touch, t),
            SettingListType::Debug => self.list_debug.top_touch(touch, t),
            SettingListType::About => false,
        } {
            return Ok(true);
        }

        if self.tabs.touch(touch, s.rt) {
            return Ok(true);
        }

        if self.scroll.touch(touch, t) {
            return Ok(true);
        }
        if let Some(p) = match self.tabs.selected() {
            SettingListType::General => self.list_general.touch(touch, t)?,
            SettingListType::Audio => self.list_audio.touch(touch, t)?,
            SettingListType::Chart => self.list_chart.touch(touch, t)?,
            SettingListType::Debug => self.list_debug.touch(touch, t)?,
            SettingListType::About => None,
        } {
            if p {
                self.save_time = t;
            }
            self.scroll.y_scroller.halt();
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let changed = match self.tabs.selected() {
            SettingListType::General => self.list_general.update(t)?,
            SettingListType::Audio => self.list_audio.update(t)?,
            SettingListType::Chart => self.list_chart.update(t)?,
            SettingListType::Debug => self.list_debug.update(t)?,
            SettingListType::About => false,
        };
        self.scroll.update(t);
        if changed {
            self.save_time = t;
        }
        if t > self.save_time + Self::SAVE_TIME {
            save_data()?;
            self.save_time = f32::INFINITY;
        }
        Ok(())
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let rt = s.rt;

        s.fader.render(ui, s.t, |ui| {
            let r = ui.content_rect();
            self.tabs.render(ui, rt, r, |ui, item| {
                let r = r.feather(-0.01);
                self.scroll.size((r.w, r.h));
                ui.scope(|ui| {
                    ui.dx(r.x);
                    ui.dy(r.y);
                    self.scroll.render(ui, |ui| match item {
                        SettingListType::General => self.list_general.render(ui, r, t),
                        SettingListType::Audio => self.list_audio.render(ui, r, t),
                        SettingListType::Chart => self.list_chart.render(ui, r, t),
                        SettingListType::Debug => self.list_debug.render(ui, r, t),
                        SettingListType::About => render_about(ui, r, &self.icon),
                    });
                });

                Ok(())
            })
        })?;

        Ok(())
    }

    fn next_page(&mut self) -> NextPage {
        if matches!(self.tabs.selected(), SettingListType::Audio) {
            return self.list_audio.next_page().unwrap_or_default();
        }
        NextPage::None
    }

    fn next_scene(&mut self, _s: &mut SharedState) -> NextScene {
        let data = get_data();
        let config = &data.config;
        if self.list_debug.take_crash_request() {
            NextScene::Overlay(Box::new(CrashScene::new(
                CrashCode::ManualCrash,
                "".to_string(),
            )))
        } else if self.list_debug.take_custom_crash_request() {
            let code = config.custom_crash_code;
            let reason = config.custom_crash_reason.clone();
            let title = config.custom_crash_title.clone();
            NextScene::Overlay(Box::new(CrashScene::new(
                CrashCode::Custom { code, reason },
                title,
            )))
        } else {
            NextScene::None
        }
    }
}

fn render_about(ui: &mut Ui, mut r: Rect, icon: &SafeTexture) -> (f32, f32) {
    r.x = 0.;
    r.y = 0.;
    let ow = r.w;

    let r = r.feather(-0.02);
    let ct = r.center();

    let icon_size = 0.12;
    let ir = Rect::new(
        ct.x - icon_size,
        r.y + 0.05,
        icon_size * 2.,
        icon_size * 2.
    );
    ui.fill_path(&ir.rounded(0.02), (**icon, ir));

    let (first, text) = (
        "phirLte",
        "版本号：V1.2.55\n二次修改内容：\n1. 更改ui\n2. 添加中文数字\n3. 添加新设置功能\n4. AutoPlay文本可以自定义\n\n原版开发：PhirLie\n运营维护：Lute_RencaiDY\n测试人员：\n洋洋酱～\n几何邻屿",
    );

    let tr = ui.text(first)
        .pos(ct.x, ir.bottom() + 0.04)
        .anchor(0.5, 0.)
        .size(0.7)
        .draw();

    let max_text_width = r.w * 0.9;
    ui.text(text)
        .pos(ct.x, tr.bottom() + 0.06)
        .size(0.5)
        .multiline()
        .max_width(max_text_width)
        .h_center()
        .anchor(0.5, 0.)
        .draw();

    (ow, r.bottom())
}


fn render_title<'a>(ui: &mut Ui, title: impl Into<Cow<'a, str>>, subtitle: Option<Cow<'a, str>>) -> f32 {
    const TITLE_SIZE: f32 = 0.55;
    const SUBTITLE_SIZE: f32 = 0.3;
    const LEFT: f32 = 0.06;
    const PAD: f32 = 0.01;

    let title = title.into();
    if let Some(subtitle) = subtitle {
        let r1 = ui.text(Cow::clone(&title)).size(TITLE_SIZE).measure();
        let r2 = ui
            .text(Cow::clone(&subtitle))
            .size(SUBTITLE_SIZE)
            .max_width(1.5)
            .no_baseline()
            .measure();
        let h = r1.h + PAD + r2.h;

        ui.text(subtitle)
            .pos(LEFT, (ITEM_HEIGHT + h) / 2.)
            .anchor(0., 1.)
            .size(SUBTITLE_SIZE)
            .max_width(1.5)
            .color(semi_white(0.5))
            .draw();

        ui.text(title)
            .pos(LEFT, (ITEM_HEIGHT - h) / 2.)
            .no_baseline()
            .size(TITLE_SIZE)
            .draw()
            .right()
    } else {
        ui.text(title)
            .pos(LEFT, ITEM_HEIGHT / 2.)
            .anchor(0., 0.5)
            .no_baseline()
            .size(TITLE_SIZE)
            .draw()
            .right()
    }
}


fn render_switch(ui: &mut Ui, r: Rect, t: f32, btn: &mut DRectButton, on: bool) {
    btn.build(ui, t, r, |_, _| {});

    let scale = 0.8;
    let orig_w = r.w;
    let orig_h = r.h;
    let new_w = orig_w * scale;
    let new_h = orig_h * scale;
    let new_x = r.x + (orig_w - new_w) / 2.;
    let new_y = r.y + (orig_h - new_h) / 2.;
    let bg = Rect::new(new_x, new_y, new_w, new_h);

    let bg_color = if on {
        Color::from_hex_rgb(0x4CAF50)
    } else {
        Color::from_hex_rgb(0x9E9E9E)
    };
    ui.fill_path(&bg.rounded(bg.h / 2.), bg_color);

    let knob_radius = bg.h * 0.38;
    let knob_x = if on {
        bg.right() - knob_radius - bg.h * 0.12
    } else {
        bg.x + knob_radius + bg.h * 0.12
    };
    let knob = Rect::new(
        knob_x - knob_radius,
        bg.y + bg.h / 2. - knob_radius,
        knob_radius * 2.,
        knob_radius * 2.,
    );
    ui.fill_path(&knob.rounded(knob_radius), WHITE);
}

#[inline]
fn right_rect(w: f32) -> Rect {
    let rh = ITEM_HEIGHT * 2. / 3.;
    Rect::new(w - INTERACT_WIDTH - 0.04, (ITEM_HEIGHT - rh) / 2., INTERACT_WIDTH, rh)
}


fn render_section_title<'a>(ui: &mut Ui, title: impl Into<Cow<'a, str>>) -> f32 {
    const SIZE: f32 = 0.45;
    const LEFT: f32 = 0.04;
    let r = ui.text(title.into())
        .pos(LEFT, 0.)
        .size(SIZE)
        .color(semi_white(0.7))
        .draw();
    ui.dy(r.h + 0.01);
    r.h + 0.01
}

struct GeneralList {
    icon_lang: SafeTexture,

    lang_btn: ChooseButton,

    #[cfg(all(any(target_os = "windows", target_os = "linux"), not(target_env = "ohos")))]
    fullscreen_btn: DRectButton,

    cache_btn: DRectButton,
    offline_btn: DRectButton,
    server_status_btn: DRectButton,
    mp_btn: DRectButton,
    mp_addr_btn: DRectButton,
    #[cfg(not(target_env = "ohos"))]
    lowq_btn: DRectButton,
    prefer_reduced_motion_btn: DRectButton,
    insecure_btn: DRectButton,
    enable_anys_btn: DRectButton,
    anys_gateway_btn: DRectButton,
    fxaa_btn: DRectButton,
    reset_settings_btn: DRectButton,
    roman_numerals_btn: DRectButton,
    chinese_numerals_btn: DRectButton,
    custom_bg_btn: DRectButton,
    reset_bg_btn: DRectButton,

    cache_size: Option<u64>,
    cache_task: Option<Task<Result<u64>>>,
}

impl GeneralList {
    pub fn new(icon_lang: SafeTexture) -> Self {
        let mut this = Self {
            icon_lang,

            lang_btn: ChooseButton::new()
                .with_options(LANG_NAMES.iter().map(|s| s.to_string()).collect())
                .with_selected(
                    get_data()
                        .language
                        .as_ref()
                        .and_then(|it| it.parse::<LanguageIdentifier>().ok())
                        .and_then(|ident| LANG_IDENTS.iter().position(|it| *it == ident))
                        .unwrap_or_default(),
                ),

            #[cfg(all(any(target_os = "windows", target_os = "linux"), not(target_env = "ohos")))]
            fullscreen_btn: DRectButton::new(),

            cache_btn: DRectButton::new(),
            offline_btn: DRectButton::new(),
            server_status_btn: DRectButton::new(),
            mp_btn: DRectButton::new(),
            mp_addr_btn: DRectButton::new(),
            #[cfg(not(target_env = "ohos"))]
            lowq_btn: DRectButton::new(),
            prefer_reduced_motion_btn: DRectButton::new(),
            insecure_btn: DRectButton::new(),
            enable_anys_btn: DRectButton::new(),
            anys_gateway_btn: DRectButton::new(),
            fxaa_btn: DRectButton::new(),
            reset_settings_btn: DRectButton::new(),
            roman_numerals_btn: DRectButton::new(),
            chinese_numerals_btn: DRectButton::new(),
            custom_bg_btn: DRectButton::new(),
            reset_bg_btn: DRectButton::new(),

            cache_size: None,
            cache_task: None,
        };
        let data = get_data_mut();
        if !data.config.particle {
            data.config.particle = true;
        }
        let _ = this.update_cache_size();
        this
    }

    pub fn top_touch(&mut self, touch: &Touch, t: f32) -> bool {
        if self.lang_btn.top_touch(touch, t) {
            return true;
        }
        false
    }

    fn dir_size(path: impl Into<PathBuf>) -> io::Result<u64> {
        fn inner(mut dir: fs::ReadDir) -> io::Result<u64> {
            dir.try_fold(0, |acc, file| {
                let file = file?;
                let size = match file.metadata()? {
                    data if data.is_dir() => inner(fs::read_dir(file.path())?)?,
                    data => data.len(),
                };
                Ok(acc + size)
            })
        }

        inner(fs::read_dir(path.into())?)
    }

    fn update_cache_size(&mut self) -> Result<()> {
        self.cache_size = None;

        let cache_dir = dir::cache()?;
        self.cache_task = Some(Task::new(async { Ok(Self::dir_size(cache_dir)?) }));
        Ok(())
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.lang_btn.touch(touch, t) {
            return Ok(Some(false));
        }

        #[cfg(all(any(target_os = "windows", target_os = "linux"), not(target_env = "ohos")))]
        if self.fullscreen_btn.touch(touch, t) {
            config.fullscreen_mode ^= true;

            macroquad::window::set_fullscreen(config.fullscreen_mode);

            return Ok(Some(true));
        }

        if self.cache_btn.touch(touch, t) {
            fs::remove_dir_all(dir::cache()?)?;
            self.update_cache_size()?;
            show_message(tl!("item-cache-cleared")).ok();
            return Ok(Some(false));
        }
        if self.offline_btn.touch(touch, t) {
            config.offline_mode ^= true;
            return Ok(Some(true));
        }
        if self.server_status_btn.touch(touch, t) {
            let _ = open_url(STATUS_PAGE);
            return Ok(Some(true));
        }
        if self.mp_btn.touch(touch, t) {
            config.mp_enabled ^= true;
            return Ok(Some(true));
        }
        if self.mp_addr_btn.touch(touch, t) {
            request_input("mp_addr", InputBox::new().default_text(&config.mp_address));
            return Ok(Some(true));
        }
        #[cfg(not(target_env = "ohos"))]
        if self.lowq_btn.touch(touch, t) {
            config.sample_count = if config.sample_count == 1 { 2 } else { 1 };
            return Ok(Some(true));
        }
        if self.prefer_reduced_motion_btn.touch(touch, t) {
            data.prefer_reduced_motion ^= true;
            PREFER_REDUCED_MOTION.store(data.prefer_reduced_motion, Ordering::Relaxed);
            return Ok(Some(true));
        }
        if self.insecure_btn.touch(touch, t) {
            data.accept_invalid_cert ^= true;
            return Ok(Some(true));
        }
        if self.enable_anys_btn.touch(touch, t) {
            data.enable_anys ^= true;
            return Ok(Some(true));
        }
        if self.anys_gateway_btn.touch(touch, t) {
            request_input("anys_gateway", InputBox::new().default_text(&data.anys_gateway));
            return Ok(Some(true));
        }
        if self.fxaa_btn.touch(touch, t) {
            config.fxaa ^= true;
            return Ok(Some(true));
        }

        if self.roman_numerals_btn.touch(touch, t) {
            config.roman_numerals ^= true;
            if config.roman_numerals {
                config.chinese_numerals = false;
            }
            return Ok(Some(true));
        }
        if self.chinese_numerals_btn.touch(touch, t) {
            config.chinese_numerals ^= true;
            if config.chinese_numerals {
                config.roman_numerals = false;
            }
            return Ok(Some(true));
        }
        if self.custom_bg_btn.touch(touch, t) {
            request_file("custom_bg");
            return Ok(Some(false));
        }
        if self.reset_bg_btn.touch(touch, t) {
            let data = get_data_mut();
            data.custom_background_path = None;
            show_message(tl!("item-custom-bg-reset")).ok();
            return Ok(Some(true));
        }
        if self.reset_settings_btn.touch(touch, t) {
            let data = get_data_mut();
            data.config = Default::default();
            data.custom_bgm_path = None;
            data.config.particle = true;
            BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
            show_message(tl!("item-reset-settings-done")).ok();
            return Ok(Some(true));
        }
        Ok(None)
    }

    pub fn update(&mut self, t: f32) -> Result<bool> {
        self.lang_btn.update(t);
        let data = get_data_mut();
        if self.lang_btn.changed() {
            data.language = Some(LANG_IDENTS[self.lang_btn.selected()].to_string());
            sync_data();
            return Ok(true);
        }
        if let Some((id, text)) = take_input() {
            if id == "mp_addr" {
                if let Err(err) = text.to_socket_addrs() {
                    show_error(anyhow::Error::new(err).context(tl!("item-mp-addr-invalid")));
                    return Ok(false);
                } else {
                    data.config.mp_address = text;
                    return Ok(true);
                }
            } else if id == "anys_gateway" {
                if let Err(err) = Url::parse(&text) {
                    show_error(anyhow::Error::new(err).context(tl!("item-anys-gateway-invalid")));
                    return Ok(false);
                } else {
                    data.anys_gateway = text.trim_end_matches('/').to_string();
                    return Ok(true);
                }
            } else {
                return_input(id, text);
            }
        }
        if let Some((id, file)) = take_file() {
            if id == "custom_bg" {
                let data = get_data_mut();
                data.custom_background_path = Some(file);
                show_message(tl!("item-custom-bg-set")).ok();
                return Ok(true);
            } else {
                return_file(id, file);
            }
        }
        if let Some(task) = &mut self.cache_task {
            if let Some(size) = task.take() {
                self.cache_size = size.ok();
                self.cache_task = None;
            }
        }
        Ok(false)
    }

    pub fn render(&mut self, ui: &mut Ui, r: Rect, t: f32) -> (f32, f32) {
        let w = r.w;
        let mut h = 0.;
        macro_rules! item {
            ($($b:tt)*) => {{
                $($b)*
                ui.dy(ITEM_HEIGHT + 0.02);
                h += ITEM_HEIGHT + 0.02;
            }}
        }
        let rr = right_rect(w);

        let data = get_data();
        let config = &data.config;


        h += render_section_title(ui, tl!("section-basic"));
        item! {
            let rt = render_title(ui, tl!("item-lang"), None);
            let w = 0.06;
            let r = Rect::new(rt + 0.01, (ITEM_HEIGHT - w) / 2., w, w);
            ui.fill_rect(r, (*self.icon_lang, r));
            self.lang_btn.render(ui, rr, t);
        }

        #[cfg(all(any(target_os = "windows", target_os = "linux"), not(target_env = "ohos")))]
        item! {
            render_title(ui, tl!("item-fullscreen"), None);
            render_switch(ui, rr, t, &mut self.fullscreen_btn, config.fullscreen_mode);
        }

        item! {
            render_title(ui, tl!("item-offline"), Some(tl!("item-offline-sub")));
            render_switch(ui, rr, t, &mut self.offline_btn, config.offline_mode);
        }


        ui.dy(0.04);
        h += 0.04;
        h += render_section_title(ui, tl!("section-network"));
        item! {
            render_title(ui, tl!("item-server-status"), Some(tl!("item-server-status-sub")));
            self.server_status_btn.render_text(ui, rr, t, tl!("check-status"), 0.5, true);
        }
        item! {
            render_title(ui, tl!("item-mp"), Some(tl!("item-mp-sub")));
            render_switch(ui, rr, t, &mut self.mp_btn, config.mp_enabled);
        }
        item! {
            render_title(ui, tl!("item-mp-addr"), Some(tl!("item-mp-addr-sub")));
            self.mp_addr_btn.render_text(ui, rr, t, &config.mp_address, 0.4, false);
        }
        item! {
            render_title(ui, tl!("item-insecure"), Some(tl!("item-insecure-sub")));
            render_switch(ui, rr, t, &mut self.insecure_btn, data.accept_invalid_cert);
        }
        item! {
            render_title(ui, tl!("item-enable-anys"), Some(tl!("item-enable-anys-sub")));
            render_switch(ui, rr, t, &mut self.enable_anys_btn, data.enable_anys);
        }
        item! {
            render_title(ui, tl!("item-anys-gateway"), Some(tl!("item-anys-gateway-sub")));
            self.anys_gateway_btn.render_text(ui, rr, t, &data.anys_gateway, 0.4, false);
        }


        ui.dy(0.04);
        h += 0.04;
        h += render_section_title(ui, tl!("section-display"));
        #[cfg(not(target_env = "ohos"))]
        item! {
            render_title(ui, tl!("item-lowq"), Some(tl!("item-lowq-sub")));
            render_switch(ui, rr, t, &mut self.lowq_btn, config.sample_count == 1);
        }
        item! {
            render_title(ui, tl!("item-prefer-reduced-motion"), Some(tl!("item-prefer-reduced-motion-sub")));
            render_switch(ui, rr, t, &mut self.prefer_reduced_motion_btn, data.prefer_reduced_motion);
        }
        item! {
            render_title(ui, tl!("item-fxaa"), Some(tl!("item-fxaa-sub")));
            render_switch(ui, rr, t, &mut self.fxaa_btn, config.fxaa);
        }
        item! {
            render_title(ui, tl!("item-roman-numerals"), Some(tl!("item-roman-numerals-sub")));
            render_switch(ui, rr, t, &mut self.roman_numerals_btn, config.roman_numerals);
        }
        item! {
            render_title(ui, tl!("item-chinese-numerals"), Some(tl!("item-chinese-numerals-sub")));
            render_switch(ui, rr, t, &mut self.chinese_numerals_btn, config.chinese_numerals);
        }
        item! {
            let bg_name = if let Some(path) = &data.custom_background_path {
                let path = std::path::Path::new(path);
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("custom_bg")
                    .to_string()
            } else {
                tl!("item-custom-bg-default").to_string()
            };
            render_title(ui, tl!("item-custom-bg"), Some(tl!("item-custom-bg-sub")));
            self.custom_bg_btn.render_text(ui, rr, t, bg_name, 0.4, false);
        }
        item! {
            render_title(ui, tl!("item-reset-bg"), None);
            self.reset_bg_btn.render_text(ui, rr, t, tl!("item-reset-bg-btn"), 0.5, true);
        }


        ui.dy(0.04);
        h += 0.04;
        h += render_section_title(ui, tl!("section-storage"));
        item! {
            let cache_size = if let Some(size) = self.cache_size {
                Cow::Owned(tl!("item-cache-size", "size" => ByteSize(size).to_string()))
            } else {
                tl!("item-cache-size-loading")
            };
            render_title(ui, tl!("item-clear-cache"), Some(cache_size));
            self.cache_btn.render_text(ui, rr, t, tl!("item-clear-cache-btn"), 0.5, true);
        }
        item! {
            render_title(ui, tl!("item-reset-settings"), Some(tl!("item-reset-settings-sub")));
            self.reset_settings_btn.render_text(ui, rr, t, tl!("item-reset-settings-btn"), 0.5, true);
        }
        self.lang_btn.render_top(ui, t, 1.);
        (w, h)
    }
}



struct AudioList {
    adjust_btn: DRectButton,
    music_slider: Slider,
    sfx_slider: Slider,
    bgm_slider: Slider,
    cali_btn: DRectButton,
    #[cfg(not(target_os = "android"))]
    preferred_sample_rate_btn: DRectButton,
    #[cfg(target_env = "ohos")]
    audio_buffer_size_btn: DRectButton,
    custom_bgm_btn: DRectButton,
    reset_bgm_btn: DRectButton,
    cali_task: LocalTask<Result<OffsetPage>>,
    next_page: Option<NextPage>,
}

impl AudioList {
    pub fn new() -> Self {
        Self {
            adjust_btn: DRectButton::new(),
            music_slider: Slider::new(0.0..2.0, 0.05),
            sfx_slider: Slider::new(0.0..2.0, 0.05),
            bgm_slider: Slider::new(0.0..2.0, 0.05),
            cali_btn: DRectButton::new(),
            #[cfg(not(target_os = "android"))]
            preferred_sample_rate_btn: DRectButton::new(),
            #[cfg(target_env = "ohos")]
            audio_buffer_size_btn: DRectButton::new(),
            custom_bgm_btn: DRectButton::new(),
            reset_bgm_btn: DRectButton::new(),

            cali_task: None,
            next_page: None,
        }
    }

    pub fn top_touch(&mut self, _touch: &Touch, _t: f32) -> bool {
        false
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.adjust_btn.touch(touch, t) {
            config.adjust_time ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.music_slider.touch(touch, t, &mut config.volume_music) {
            return Ok(wt);
        }
        if let wt @ Some(_) = self.sfx_slider.touch(touch, t, &mut config.volume_sfx) {
            return Ok(wt);
        }
        let old = config.volume_bgm;
        if let wt @ Some(_) = self.bgm_slider.touch(touch, t, &mut config.volume_bgm) {
            if (config.volume_bgm - old).abs() > 0.001 {
                BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
            }
            return Ok(wt);
        }
        if self.cali_btn.touch(touch, t) {
            self.cali_task = Some(Box::pin(OffsetPage::new()));
            return Ok(Some(false));
        }
        #[cfg(not(target_os = "android"))]
        if self.preferred_sample_rate_btn.touch(touch, t) {
            let options = [None, Some(44100), Some(48000), Some(88200), Some(96000), Some(192000)];
            let current = config.preferred_sample_rate;
            let selected = options.iter().position(|&r| r == current).unwrap_or(0);
            config.preferred_sample_rate = options[(selected + 1) % options.len()];
            return Ok(Some(true));
        }
        #[cfg(target_env = "ohos")]
        if self.audio_buffer_size_btn.touch(touch, t) {
            let options = [128u32, 256u32, 512u32];
            let current = config.audio_buffer_size.unwrap_or(256);
            let selected = options.iter().position(|&r| r == current).unwrap_or(1);
            config.audio_buffer_size = Some(options[(selected + 1) % options.len()]);
            return Ok(Some(true));
        }
        if self.custom_bgm_btn.touch(touch, t) {
            request_file("custom_bgm");
            return Ok(Some(false));
        }
        if self.reset_bgm_btn.touch(touch, t) {
            let data = get_data_mut();
            data.custom_bgm_path = None;
            BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
            show_message(tl!("item-custom-bgm-reset")).ok();
            return Ok(Some(true));
        }
        Ok(None)
    }

    pub fn update(&mut self, _t: f32) -> Result<bool> {
        if let Some(task) = &mut self.cali_task {
            if let Some(res) = poll_future(task.as_mut()) {
                match res {
                    Err(err) => show_error(err.context(tl!("load-cali-failed"))),
                    Ok(page) => {
                        self.next_page = Some(NextPage::Overlay(Box::new(page)));
                    }
                }
                self.cali_task = None;
            }
        }
        if let Some((id, file)) = take_file() {
            if id == "custom_bgm" {
                let data = get_data_mut();
                data.custom_bgm_path = Some(file);
                BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
                show_message(tl!("item-custom-bgm-set")).ok();
                return Ok(true);
            } else {
                return_file(id, file);
            }
        }
        Ok(false)
    }

    pub fn render(&mut self, ui: &mut Ui, r: Rect, t: f32) -> (f32, f32) {
        let w = r.w;
        let mut h = 0.;
        macro_rules! item {
            ($($b:tt)*) => {{
                $($b)*
                ui.dy(ITEM_HEIGHT + 0.02);
                h += ITEM_HEIGHT + 0.02;
            }}
        }
        let rr = right_rect(w);

        let data = get_data();
        let config = &data.config;
        item! {
            render_title(ui, tl!("item-adjust"), Some(tl!("item-adjust-sub")));
            render_switch(ui, rr, t, &mut self.adjust_btn, config.adjust_time);
        }
        item! {
            render_title(ui, tl!("item-music"), None);
            self.music_slider.render(ui, rr, t, config.volume_music, format!("{:.2}", config.volume_music));
        }
        item! {
            render_title(ui, tl!("item-sfx"), None);
            self.sfx_slider.render(ui, rr, t, config.volume_sfx, format!("{:.2}", config.volume_sfx));
        }
        item! {
            render_title(ui, tl!("item-bgm"), None);
            self.bgm_slider.render(ui, rr, t, config.volume_bgm, format!("{:.2}", config.volume_bgm));
        }
        item! {
            render_title(ui, tl!("item-cali"), None);
            self.cali_btn.render_text(ui, rr, t, format!("{:.0}ms", config.offset * 1000.), 0.5, true);
        }
        #[cfg(not(target_os = "android"))]
        item! {
            render_title(ui, tl!("item-preferred-sample-rate"), None);
            let text = if let Some(rate) = config.preferred_sample_rate {
                format!("{} Hz", rate)
            } else {
                tl!("preferred-sample-rate-default").to_string()
            };
            self.preferred_sample_rate_btn.render_text(ui, rr, t, text, 0.5, false);
        }
        #[cfg(target_env = "ohos")]
        item! {
            render_title(ui, tl!("item-audio-buffer-size"), None);
            let buf_size = config.audio_buffer_size.unwrap_or(256);
            self.audio_buffer_size_btn.render_text(ui, rr, t, format!("{}", buf_size), 0.5, false);
        }
        item! {
            let bgm_name = if let Some(path) = &data.custom_bgm_path {
                let path = std::path::Path::new(path);
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("custom_bgm")
                    .to_string()
            } else {
                tl!("item-custom-bgm-default").to_string()
            };
            render_title(ui, tl!("item-custom-bgm"), Some(tl!("item-custom-bgm-sub")));
            self.custom_bgm_btn.render_text(ui, rr, t, bgm_name, 0.4, false);
        }
        item! {
            render_title(ui, tl!("item-reset-bgm"), None);
            self.reset_bgm_btn.render_text(ui, rr, t, tl!("item-reset-bgm-btn"), 0.5, true);
        }
        (w, h)
    }

    pub fn next_page(&mut self) -> Option<NextPage> {
        self.next_page.take()
    }
}

// ========== ChartList ==========

struct ChartList {
    show_acc_btn: DRectButton,
    ap_fc_indicator_btn: DRectButton,
    show_avg_fps_btn: DRectButton,
    dc_pause_btn: DRectButton,
    dhint_btn: DRectButton,
    opt_btn: DRectButton,
    use_keyboard_btn: DRectButton,
    particle_btn: DRectButton,
    disable_effect_btn: DRectButton,
    interactive_btn: DRectButton,
    speed_slider: Slider,
    size_slider: Slider,
    full_screen_judge_btn: DRectButton,
    combo_text_debug_btn: DRectButton,
    watermark_btn: DRectButton,
    combo_text_btn: DRectButton,
}

impl ChartList {
    pub fn new() -> Self {
        Self {
            show_acc_btn: DRectButton::new(),
            ap_fc_indicator_btn: DRectButton::new(),
            show_avg_fps_btn: DRectButton::new(),
            dc_pause_btn: DRectButton::new(),
            dhint_btn: DRectButton::new(),
            opt_btn: DRectButton::new(),
            use_keyboard_btn: DRectButton::new(),
            particle_btn: DRectButton::new(),
            disable_effect_btn: DRectButton::new(),
            interactive_btn: DRectButton::new(),
            speed_slider: Slider::new(0.5..2., 0.05),
            size_slider: Slider::new(0.8..1.2, 0.005),
            full_screen_judge_btn: DRectButton::new(),
            combo_text_debug_btn: DRectButton::new(),
            watermark_btn: DRectButton::new(),
            combo_text_btn: DRectButton::new(),
        }
    }

    pub fn top_touch(&mut self, _touch: &Touch, _t: f32) -> bool {
        false
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.show_acc_btn.touch(touch, t) {
            config.show_acc ^= true;
            return Ok(Some(true));
        }
        if self.ap_fc_indicator_btn.touch(touch, t) {
            config.ap_fc_indicator ^= true;
            return Ok(Some(true));
        }
        if self.show_avg_fps_btn.touch(touch, t) {
            config.show_avg_fps ^= true;
            return Ok(Some(true));
        }
        if self.dc_pause_btn.touch(touch, t) {
            config.double_click_to_pause ^= true;
            return Ok(Some(true));
        }
        if self.dhint_btn.touch(touch, t) {
            config.double_hint ^= true;
            return Ok(Some(true));
        }
        if self.opt_btn.touch(touch, t) {
            config.aggressive ^= true;
            return Ok(Some(true));
        }
        if self.use_keyboard_btn.touch(touch, t) {
            config.use_keyboard ^= true;
            return Ok(Some(true));
        }
        if self.particle_btn.touch(touch, t) {
            config.particle ^= true;
            return Ok(Some(true));
        }
        if self.disable_effect_btn.touch(touch, t) {
            config.disable_effect ^= true;
            return Ok(Some(true));
        }
        if self.interactive_btn.touch(touch, t) {
            config.interactive ^= true;
            return Ok(Some(true));
        }
        if self.full_screen_judge_btn.touch(touch, t) {
            data.config.full_screen_judge ^= true;
            return Ok(Some(true));
        }
        if self.combo_text_debug_btn.touch(touch, t) {
            data.config.combo_text_debug ^= true;
            return Ok(Some(true));
        }
        if self.watermark_btn.touch(touch, t) {
            request_input("watermark", InputBox::new().default_text(&data.config.custom_watermark));
            return Ok(Some(true));
        }
        if self.combo_text_btn.touch(touch, t) {
            request_input("custom_combo_text", InputBox::new().default_text(&data.config.custom_combo_text));
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.speed_slider.touch(touch, t, &mut config.speed) {
            return Ok(wt);
        }
        if let wt @ Some(_) = self.size_slider.touch(touch, t, &mut config.note_scale) {
            return Ok(wt);
        }
        Ok(None)
    }

    pub fn update(&mut self, _t: f32) -> Result<bool> {
        if let Some((id, text)) = take_input() {
            if id == "watermark" {
                let data = get_data_mut();
                data.config.custom_watermark = text;
                return Ok(true);
            }
            if id == "custom_combo_text" {
                let data = get_data_mut();
                data.config.custom_combo_text = text;
                return Ok(true);
            }
            return_input(id, text);
        }
        Ok(false)
    }

    pub fn render(&mut self, ui: &mut Ui, r: Rect, t: f32) -> (f32, f32) {
        let w = r.w;
        let mut h = 0.;
        macro_rules! item {
            ($($b:tt)*) => {{
                $($b)*
                ui.dy(ITEM_HEIGHT + 0.02);
                h += ITEM_HEIGHT + 0.02;
            }}
        }
        let rr = right_rect(w);

        let data = get_data();
        let config = &data.config;
        item! {
            render_title(ui, tl!("item-show-acc"), None);
            render_switch(ui, rr, t, &mut self.show_acc_btn, config.show_acc);
        }
        item! {
            render_title(ui, tl!("item-ap-fc-indicator"), Some(tl!("item-ap-fc-indicator-sub")));
            render_switch(ui, rr, t, &mut self.ap_fc_indicator_btn, config.ap_fc_indicator);
        }
        item! {
            render_title(ui, tl!("item-show-avg-fps"), Some(tl!("item-show-avg-fps-sub")));
            render_switch(ui, rr, t, &mut self.show_avg_fps_btn, config.show_avg_fps);
        }
        item! {
            render_title(ui, tl!("item-dc-pause"), None);
            render_switch(ui, rr, t, &mut self.dc_pause_btn, config.double_click_to_pause);
        }
        item! {
            render_title(ui, tl!("item-dhint"), Some(tl!("item-dhint-sub")));
            render_switch(ui, rr, t, &mut self.dhint_btn, config.double_hint);
        }
        item! {
            render_title(ui, tl!("item-opt"), Some(tl!("item-opt-sub")));
            render_switch(ui, rr, t, &mut self.opt_btn, config.aggressive);
        }
        item! {
            render_title(ui, tl!("item-use-keyboard"), Some(tl!("item-use-keyboard-sub")));
            render_switch(ui, rr, t, &mut self.use_keyboard_btn, config.use_keyboard);
        }
        item! {
            render_title(ui, tl!("item-particle"), Some(tl!("item-particle-sub")));
            render_switch(ui, rr, t, &mut self.particle_btn, config.particle);
        }
        item! {
            render_title(ui, tl!("item-disable-effect"), Some(tl!("item-disable-effect-sub")));
            render_switch(ui, rr, t, &mut self.disable_effect_btn, config.disable_effect);
        }
        item! {
            render_title(ui, tl!("item-interactive"), Some(tl!("item-interactive-sub")));
            render_switch(ui, rr, t, &mut self.interactive_btn, config.interactive);
        }
        item! {
            render_title(ui, tl!("item-speed"), None);
            self.speed_slider.render(ui, rr, t, config.speed, format!("{:.2}", config.speed));
        }
        item! {
            render_title(ui, tl!("item-note-size"), None);
            self.size_slider.render(ui, rr, t, config.note_scale, format!("{:.3}", config.note_scale));
        }
        item! {
            render_title(ui, tl!("item-full-screen-judge"), Some(tl!("item-full-screen-judge-sub")));
            render_switch(ui, rr, t, &mut self.full_screen_judge_btn, config.full_screen_judge);
        }
        item! {
            render_title(ui, tl!("item-combo-text-debug"), Some(tl!("item-combo-text-debug-sub")));
            render_switch(ui, rr, t, &mut self.combo_text_debug_btn, config.combo_text_debug);
        }
        item! {
            let watermark_text = if config.custom_watermark.is_empty() {
                tl!("item-watermark-default").into_owned()
            } else {
                config.custom_watermark.clone()
            };
            render_title(ui, tl!("item-watermark"), Some(watermark_text.into()));
            self.watermark_btn.render_text(ui, rr, t, tl!("item-watermark-edit"), 0.5, true);
        }
        item! {
            let combo_text = if config.custom_combo_text.is_empty() {
                tl!("item-combo-text-default").into_owned()
            } else {
                config.custom_combo_text.clone()
            };
            render_title(ui, tl!("item-combo-text"), Some(combo_text.into()));
            self.combo_text_btn.render_text(ui, rr, t, tl!("item-combo-text-edit"), 0.5, true);
        }
        (w, h)
    }
}

// ================================================================
// 修改后的 DebugList —— 包含自定义崩溃功能（含标题）
// ================================================================

struct DebugList {
    chart_debug_btn: DRectButton,
    touch_debug_btn: DRectButton,
    crash_btn: DRectButton,

    // 自定义崩溃相关
    custom_title_btn: DRectButton,
    custom_code_btn: DRectButton,
    custom_reason_btn: DRectButton,
    custom_crash_btn: DRectButton,
    crash_requested: bool,
    custom_crash_requested: bool,
}

impl DebugList {
    pub fn new() -> Self {
        Self {
            chart_debug_btn: DRectButton::new(),
            touch_debug_btn: DRectButton::new(),
            crash_btn: DRectButton::new(),
            custom_title_btn: DRectButton::new(),
            custom_code_btn: DRectButton::new(),
            custom_reason_btn: DRectButton::new(),
            custom_crash_btn: DRectButton::new(),
            crash_requested: false,
            custom_crash_requested: false,
        }
    }

    pub fn top_touch(&mut self, _touch: &Touch, _t: f32) -> bool {
        false
    }

    pub fn take_crash_request(&mut self) -> bool {
        std::mem::take(&mut self.crash_requested)
    }

    pub fn take_custom_crash_request(&mut self) -> bool {
        std::mem::take(&mut self.custom_crash_requested)
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.chart_debug_btn.touch(touch, t) {
            config.chart_debug ^= true;
            return Ok(Some(true));
        }
        if self.touch_debug_btn.touch(touch, t) {
            config.touch_debug ^= true;
            return Ok(Some(true));
        }
        if self.crash_btn.touch(touch, t) {
            self.crash_requested = true;
            return Ok(Some(false));
        }
        // 自定义标题
        if self.custom_title_btn.touch(touch, t) {
            let current = config.custom_crash_title.clone();
            request_input("custom_crash_title", InputBox::new().default_text(&current));
            return Ok(Some(false));
        }
        // 自定义代码
        if self.custom_code_btn.touch(touch, t) {
            let current = config.custom_crash_code.to_string();
            request_input("custom_crash_code", InputBox::new().default_text(&current));
            return Ok(Some(false));
        }
        // 自定义原因
        if self.custom_reason_btn.touch(touch, t) {
            let current = config.custom_crash_reason.clone();
            request_input("custom_crash_reason", InputBox::new().default_text(&current));
            return Ok(Some(false));
        }
        // 触发自定义崩溃
        if self.custom_crash_btn.touch(touch, t) {
            self.custom_crash_requested = true;
            return Ok(Some(false));
        }
        Ok(None)
    }

    pub fn update(&mut self, _t: f32) -> Result<bool> {
        if let Some((id, text)) = take_input() {
            let data = get_data_mut();
            if id == "custom_crash_title" {
                data.config.custom_crash_title = text;
                return Ok(true);
            } else if id == "custom_crash_code" {
                if let Ok(code) = text.parse::<u32>() {
                    data.config.custom_crash_code = code;
                    return Ok(true);
                } else {
                    show_error(anyhow::anyhow!("无效的错误代码，请输入数字"));
                    return Ok(false);
                }
            } else if id == "custom_crash_reason" {
                data.config.custom_crash_reason = text;
                return Ok(true);
            } else {
                return_input(id, text);
            }
        }
        Ok(false)
    }

    pub fn render(&mut self, ui: &mut Ui, r: Rect, t: f32) -> (f32, f32) {
        let w = r.w;
        let mut h = 0.;
        macro_rules! item {
            ($($b:tt)*) => {{
                $($b)*
                ui.dy(ITEM_HEIGHT + 0.02);
                h += ITEM_HEIGHT + 0.02;
            }}
        }
        let rr = right_rect(w);

        let data = get_data();
        let config = &data.config;

        // 原有的调试开关
        item! {
            render_title(ui, tl!("item-chart-debug"), Some(tl!("item-chart-debug-sub")));
            render_switch(ui, rr, t, &mut self.chart_debug_btn, config.chart_debug);
        }
        item! {
            render_title(ui, tl!("item-touch-debug"), Some(tl!("item-touch-debug-sub")));
            render_switch(ui, rr, t, &mut self.touch_debug_btn, config.touch_debug);
        }
        item! {
            render_title(ui, "整活 - 崩溃按钮", Some("点击后触发崩溃页面(错误代码951)".into()));
            self.crash_btn.render_text(ui, rr, t, "崩溃", 0.5, true);
        }

        // ---- 自定义崩溃区域 ----
        ui.dy(0.04);
        h += 0.04;
        h += render_section_title(ui, "自定义崩溃");

        // 标题
        item! {
            let title_display = if config.custom_crash_title.is_empty() {
                "(默认)".to_string()
            } else {
                config.custom_crash_title.clone()
            };
            render_title(ui, "标题", Some(title_display.into()));
            self.custom_title_btn.render_text(ui, rr, t, "修改", 0.5, true);
        }

        // 代码
        item! {
            let code_display = format!("当前代码: {}", config.custom_crash_code);
            render_title(ui, "错误代码", Some(code_display.into()));
            self.custom_code_btn.render_text(ui, rr, t, "修改", 0.5, true);
        }

        // 原因
        item! {
            let reason_display = if config.custom_crash_reason.is_empty() {
                "(空)".to_string()
            } else {
                config.custom_crash_reason.clone()
            };
            let reason_text = format!("当前原因: {}", reason_display);
            render_title(ui, "原因", Some(reason_text.into()));
            self.custom_reason_btn.render_text(ui, rr, t, "修改", 0.5, true);
        }

        // 触发按钮
        item! {
            render_title(ui, "触发自定义崩溃", Some("使用上述设置的代码和原因触发崩溃".into()));
            self.custom_crash_btn.render_text(ui, rr, t, "触发", 0.5, true);
        }

        (w, h)
    }
}