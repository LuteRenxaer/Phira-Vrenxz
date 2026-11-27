prpr_l10n::tl_file!("settings");
use super::{NextPage, OffsetPage, Page, SharedState};
use crate::{
    dir, get_data, get_data_mut,
    popup::ChooseButton,
    save_data,
    scene::{home_background_index, BACKGROUND_UPDATED, BGM_VOLUME_UPDATED, BUILTIN_HOME_BACKGROUNDS, HOME_VIDEO_UPDATED, TEX_ICON_PROCEED},
    set_fullscreen_mode,
    sync_data,
    tabs::{Tabs, TitleFn},
};
use anyhow::Result;
use bytesize::ByteSize;
use inputbox::InputBox;
use macroquad::prelude::*;
use once_cell::sync::Lazy;
use prpr::{
    core::MAIN_FONT,
    ext::{draw_parallelogram, open_url, poll_future, semi_black, semi_white, LocalTask, RectExt, SafeTexture, PARALLELOGRAM_SLOPE},
    scene::{request_file, request_input, return_file, return_input, show_error, show_message, take_file, take_input, CrashCode, CrashScene, NextScene},
    task::Task,
    ui::{begin_hit_frame, Dialog, DRectButton, FontArc, Scroll, Slider, TextPainter, Ui, PREFER_REDUCED_MOTION},
};
use prpr_l10n::{set_prefered_locale, LanguageIdentifier, LANG_IDENTS, LANG_NAMES};
use reqwest::Url;
use serde::Deserialize;
use std::{borrow::Cow, cell::RefCell, fs, io, net::ToSocketAddrs, path::PathBuf, sync::atomic::Ordering};
const SIZE_ROW_H: f32 = 0.0905;
const SIZE_COL_GAP: f32 = 0.034;
/// 右栏控件是斜切的，右上角会比控件矩形多探出 skew，裁剪区要给它留余量
const SIZE_RIGHT_PAD: f32 = 0.022;
const SIZE_CTL_H: f32 = 0.046;
const SIZE_SW_W: f32 = 0.100;
const SIZE_CTL_MAX: f32 = 0.26;
/// 滑块比其它控件长一截，拖着更舒服
const SIZE_SLIDER_W: f32 = 0.22;
/// 内容区（屏幕比例）
const CONTENT_X0: f32 = 0.130;
const CONTENT_X1: f32 = 0.870;
const CONTENT_Y0: f32 = 0.176;
const CONTENT_Y1: f32 = 0.974;
/// 顶栏（屏幕比例）
const TAB_X0: f32 = 0.106;
const TAB_X1: f32 = 0.870;
const TAB_Y0: f32 = 0.030;
/// 顶栏高度（UI 坐标），与左上角返回按钮高度保持一致。
const TAB_BAR_H: f32 = 0.12;
/// 背板左右边（底部）：整体左移，让斜板和内容在窗口里居中
const PANEL_L: f32 = -0.010;
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
    Account,
    Audio,
    Chart,
    Visual,
    Custom,
    Debug,
    Replay,
    Plugin,
    About,
}
const SETTING_TABS: [(SettingListType, &str); 9] = [
    (SettingListType::General, "tab-general"),
    (SettingListType::Account, "tab-account"),
    (SettingListType::Audio, "tab-audio"),
    (SettingListType::Chart, "tab-chart"),
    (SettingListType::Visual, "tab-visual"),
    (SettingListType::Custom, "tab-custom"),
    (SettingListType::Debug, "tab-debug"),
    (SettingListType::Plugin, "tab-plugin"),
    (SettingListType::About, "tab-about"),
];
pub struct SettingsPage {
    list_general: GeneralList,
    list_audio: AudioList,
    list_chart: ChartList,
    list_visual: VisualList,
    list_custom: CustomList,
    list_debug: DebugList,
    list_replay: ReplayList,
    list_plugin: PluginList,
    tab: SettingListType,
    /// 当前页内容总高度（给滚动条用）
    content_h: f32,
    scroll: Scroll,
    save_time: f32,
    logout_btn: DRectButton,
    /// 账号页「切换账号」入口行
    switch_btn: DRectButton,
    /// 切换账号弹窗
    account_overlay: bool,
    account_rows: Vec<DRectButton>,
    account_del_btns: Vec<DRectButton>,
    account_add_btn: DRectButton,
    account_cancel_btn: DRectButton,
    switch_task: Option<Task<Result<()>>>,
    /// 内嵌的登录面板（「登录其他账号」用）
    login: Option<crate::login::Login>,
    login_open: bool,
    link_buttons: [DRectButton; 4],
    /// 切页动画起点（NEG_INFINITY = 未在动画中）
    tab_anim_start: f32,
    /// 切页方向：+1 向右滑入、-1 向左滑入
    tab_anim_dir: f32,
    icon: SafeTexture,
}
/// 屏幕横向比例 → UI 坐标
#[inline]
fn fx(v: f32) -> f32 {
    v * 2. - 1.
}
/// 屏幕纵向比例 → UI 坐标
#[inline]
fn fy(ui: &Ui, v: f32) -> f32 {
    (v * 2. - 1.) * ui.top
}
/// 顶栏下沿的屏幕纵向比例：高度固定为返回键的 UI 高度。
fn tab_y1(top: f32) -> f32 {
    TAB_Y0 + TAB_BAR_H / (2. * top)
}
/// 大背板保持 15° 斜切，同时让右上角停在屏幕内。
fn panel_skew(top: f32) -> f32 {
    PARALLELOGRAM_SLOPE * 2. * top
}
fn panel_right(top: f32) -> f32 {
    (0.98 - panel_skew(top) + 1.) / 2.
}
/// 选择栏比面板右侧再伸出一小截，并给最后一个标签留出角部空间。
fn tab_bar_right(top: f32) -> f32 {
    (panel_right(top) + 0.10)
        .max(TAB_X1 + 0.08)
        .min(0.96)
}
/// 斜切量：高度（屏幕高度比例）→ 水平位移（UI 坐标），约 10°
#[inline]
fn slant(ui: &Ui, h: f32) -> f32 {
    PARALLELOGRAM_SLOPE * h * 2. * ui.top
}
/// 用四边形填充（UI 坐标），用于各种斜切块
fn fill_quad(ui: &mut Ui, pts: [(f32, f32); 4], color: Color) {
    let mut b = lyon::path::Path::builder();
    b.begin(lyon::math::point(pts[0].0, pts[0].1));
    b.line_to(lyon::math::point(pts[1].0, pts[1].1));
    b.line_to(lyon::math::point(pts[2].0, pts[2].1));
    b.line_to(lyon::math::point(pts[3].0, pts[3].1));
    b.end(true);
    ui.fill_path(&b.build(), color);
}
/// 对勾：两段实心四边形，避免细描边在某些缩放下直接看不见
fn check_shape(ui: &mut Ui, cx: f32, cy: f32, d: f32, t: f32, color: Color) {
    let pts = [
        (cx - d * 0.95, cy + d * 0.02),
        (cx - d * 0.28, cy + d * 0.72),
        (cx + d * 1.00, cy - d * 0.80),
    ];
    let seg = |a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (nx, ny) = (-dy / len * t * 0.5, dx / len * t * 0.5);
        [
            (a.0 + nx, a.1 + ny),
            (b.0 + nx, b.1 + ny),
            (b.0 - nx, b.1 - ny),
            (a.0 - nx, a.1 - ny),
        ]
    };
    fill_quad(ui, seg(pts[0], pts[1]), color);
    fill_quad(ui, seg(pts[1], pts[2]), color);
}
/// 斜切矩形（上边缘右移 skew）
fn fill_slant(ui: &mut Ui, r: Rect, skew: f32, color: Color) {    fill_quad(
        ui,
        [
            (r.x + skew, r.y),
            (r.right() + skew, r.y),
            (r.right(), r.bottom()),
            (r.x, r.bottom()),
        ],
        color,
    );
}
/// 描边四边形
fn stroke_quad(ui: &mut Ui, pts: [(f32, f32); 4], width: f32, color: Color) {
    let mut b = lyon::path::Path::builder();
    b.begin(lyon::math::point(pts[0].0, pts[0].1));
    b.line_to(lyon::math::point(pts[1].0, pts[1].1));
    b.line_to(lyon::math::point(pts[2].0, pts[2].1));
    b.line_to(lyon::math::point(pts[3].0, pts[3].1));
    b.end(true);
    ui.stroke_path(&b.build(), width, color);
}
/// 斜切矩形描边
fn stroke_slant(ui: &mut Ui, r: Rect, skew: f32, width: f32, color: Color) {
    stroke_quad(
        ui,
        [
            (r.x + skew, r.y),
            (r.right() + skew, r.y),
            (r.right(), r.bottom()),
            (r.x, r.bottom()),
        ],
        width,
        color,
    );
}
fn chevron(ui: &mut Ui, r: Rect, color: Color) {
    use lyon::math::point;
    let cx = r.center().x + r.h * 0.06;
    let cy = r.center().y;
    let d = r.h * 0.20;
    let mut b = lyon::path::Path::builder();
    b.begin(point(cx - d * 0.7, cy - d));
    b.line_to(point(cx + d * 0.5, cy));
    b.line_to(point(cx - d * 0.7, cy + d));
    ui.stroke_path(&b.build(), (r.h * 0.075).max(0.0035), color);
}
/// 白色斜切箭头按钮
fn arrow_button(ui: &mut Ui, t: f32, btn: &mut DRectButton, r: Rect) {
    btn.build(ui, t, r, |ui, _| {
        fill_slant(ui, r, r.h * 0.30, WHITE);
        chevron(ui, r, Color::new(0.06, 0.06, 0.08, 1.));
    });
}
/// 只画箭头，不注册按钮（整行是按钮时用）
fn arrow_shape(ui: &mut Ui, r: Rect) {
    fill_slant(ui, r, r.h * 0.30, WHITE);
    chevron(ui, r, Color::new(0.06, 0.06, 0.08, 1.));
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum PaletteDrag {
    Wheel,
    Value,
}
fn palette_panel() -> Rect {
    Rect::new(-0.48, -0.44, 0.96, 0.88)
}
fn palette_wheel() -> Rect {
    Rect::new(-0.40, -0.33, 0.56, 0.56)
}
fn palette_value_bar() -> Rect {
    Rect::new(0.30, -0.33, 0.08, 0.56)
}
fn palette_done() -> Rect {
    Rect::new(-0.12, 0.33, 0.24, 0.08)
}
fn parse_hex_rgb(s: &str) -> Option<(f32, f32, f32)> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()? as f32 / 255.;
    let g = u8::from_str_radix(&s[2..4], 16).ok()? as f32 / 255.;
    let b = u8::from_str_radix(&s[4..6], 16).ok()? as f32 / 255.;
    Some((r, g, b))
}
fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let h = if d <= 1e-6 {
        0.
    } else if max == r {
        ((g - b) / d).rem_euclid(6.) / 6.
    } else if max == g {
        ((b - r) / d + 2.) / 6.
    } else {
        ((r - g) / d + 4.) / 6.
    };
    let s = if max <= 1e-6 { 0. } else { d / max };
    (h, s, max)
}
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Color {
    let h6 = h.rem_euclid(1.) * 6.;
    let i = h6.floor() as i32;
    let f = h6 - i as f32;
    let p = v * (1. - s);
    let q = v * (1. - f * s);
    let t = v * (1. - (1. - f) * s);
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::new(r, g, b, 1.)
}
fn color_hex(c: Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        (c.r.clamp(0., 1.) * 255.).round() as u8,
        (c.g.clamp(0., 1.) * 255.).round() as u8,
        (c.b.clamp(0., 1.) * 255.).round() as u8
    )
}
/// 圆形调色板：角度 = 色相，半径 = 饱和度，右侧竖条 = 明度。
struct ColorPalette {
    open: bool,
    hue: f32,
    sat: f32,
    val: f32,
    drag: Option<PaletteDrag>,
}
impl ColorPalette {
    fn new() -> Self {
        Self {
            open: false,
            hue: 0.,
            sat: 1.,
            val: 1.,
            drag: None,
        }
    }
    fn open(&mut self) {
        let current = get_data().config.noise_color.clone();
        let (r, g, b) = parse_hex_rgb(&current).unwrap_or((1., 0., 0.));
        let (h, s, v) = rgb_to_hsv(r, g, b);
        self.hue = h;
        self.sat = s;
        self.val = v;
        self.drag = None;
        self.open = true;
    }
    fn is_open(&self) -> bool {
        self.open
    }
    fn apply(&self) {
        let color = hsv_to_rgb(self.hue, self.sat, self.val);
        get_data_mut().config.noise_color = color_hex(color);
    }
    fn set_wheel(&mut self, pos: Vec2) {
        let r = palette_wheel();
        let cx = r.center().x;
        let cy = r.center().y;
        let dx = (pos.x - cx) / (r.w * 0.5);
        let dy = (pos.y - cy) / (r.h * 0.5);
        self.sat = (dx * dx + dy * dy).sqrt().clamp(0., 1.);
        if self.sat > 1e-4 {
            self.hue = (dy.atan2(dx) / std::f32::consts::TAU).rem_euclid(1.);
        }
        self.apply();
    }
    fn set_value(&mut self, y: f32) {
        let r = palette_value_bar();
        self.val = (1. - (y - r.y) / r.h).clamp(0., 1.);
        self.apply();
    }
    fn touch(&mut self, touch: &Touch) -> bool {
        if !self.open {
            return false;
        }
        let pos = touch.position;
        match touch.phase {
            TouchPhase::Started => {
                if palette_done().contains(pos) {
                    self.open = false;
                    self.drag = None;
                    return true;
                }
                if palette_wheel().contains(pos) {
                    self.drag = Some(PaletteDrag::Wheel);
                    self.set_wheel(pos);
                } else if palette_value_bar().contains(pos) {
                    self.drag = Some(PaletteDrag::Value);
                    self.set_value(pos.y);
                } else if !palette_panel().contains(pos) {
                    self.open = false;
                    self.drag = None;
                }
                true
            }
            TouchPhase::Moved | TouchPhase::Stationary => {
                if let Some(drag) = self.drag {
                    match drag {
                        PaletteDrag::Wheel => self.set_wheel(pos),
                        PaletteDrag::Value => self.set_value(pos.y),
                    }
                }
                true
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                self.drag = None;
                true
            }
        }
    }
    fn render(&mut self, ui: &mut Ui, _t: f32) {
        if !self.open {
            return;
        }
        let panel = palette_panel();
        let wheel = palette_wheel();
        let value_bar = palette_value_bar();
        let done = palette_done();
        let current = color_hex(hsv_to_rgb(self.hue, self.sat, self.val));
        ui.abs_scope(|ui| {
            ui.fill_rect(ui.screen_rect(), semi_black(0.62));
            ui.fill_path(&panel.rounded(0.02), Color::new(0.06, 0.07, 0.09, 0.98));
            ui.stroke_path(&panel.rounded(0.02), 0.0018, semi_white(0.22));
            let title = tl!("color-palette-title");
            ui.text(title.as_ref())
                .pos(panel.x + 0.03, panel.y + 0.045)
                .anchor(0., 0.5)
                .size(0.42)
                .color(WHITE)
                .draw();
            ui.text(&current)
                .pos(panel.right() - 0.03, panel.y + 0.045)
                .anchor(1., 0.5)
                .size(0.30)
                .color(semi_white(0.65))
                .draw();
            // 圆形色盘：shader 逐像素算 HSV
            let gr = ui.rect_to_global(wheel);
            let mat = &*WHEEL_MATERIAL;
            mat.set_uniform("uValue", self.val);
            gl_use_material(*mat);
            draw_rectangle(gr.x, gr.y, gr.w, gr.h, WHITE);
            gl_use_default_material();
            // 当前选点
            let angle = self.hue * std::f32::consts::TAU;
            let hx = wheel.center().x + angle.cos() * self.sat * wheel.w * 0.5;
            let hy = wheel.center().y + angle.sin() * self.sat * wheel.h * 0.5;
            ui.stroke_circle(hx, hy, 0.015, 0.006, WHITE);
            ui.stroke_circle(hx, hy, 0.021, 0.0025, semi_black(0.6));
            // 明度竖条
            let top = hsv_to_rgb(self.hue, self.sat, 1.);
            ui.fill_rect(
                value_bar,
                (top, (value_bar.x, value_bar.y), BLACK, (value_bar.x, value_bar.bottom())),
            );
            ui.stroke_path(&value_bar.rounded(0.01), 0.0015, semi_white(0.25));
            let vy = value_bar.y + (1. - self.val) * value_bar.h;
            ui.fill_rect(
                Rect::new(value_bar.x - 0.008, vy - 0.004, value_bar.w + 0.016, 0.008),
                WHITE,
            );
            // 完成按钮
            ui.fill_path(&done.rounded(0.012), WHITE);
            let text = tl!("color-palette-done");
            ui.text(text.as_ref())
                .pos(done.center().x, done.center().y)
                .anchor(0.5, 0.5)
                .size(0.34)
                .color(Color::new(0.05, 0.05, 0.07, 1.))
                .draw();
        });
    }
}
mod palette_shader {
    pub const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
varying lowp vec2 uv;
uniform mat4 Model;
uniform mat4 Projection;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = texcoord;
}"#;
    pub const FRAGMENT: &str = r#"#version 100
precision mediump float;
varying lowp vec2 uv;
uniform float uValue;
vec3 hsv2rgb(vec3 c) {
    vec3 p = abs(fract(c.xxx + vec3(1.0, 2.0 / 3.0, 1.0 / 3.0)) * 6.0 - 3.0);
    return c.z * mix(vec3(1.0), clamp(p - 1.0, 0.0, 1.0), c.y);
}
void main() {
    vec2 p = uv * 2.0 - 1.0;
    float r = length(p);
    if (r > 1.0) discard;
    float h = atan(p.y, p.x) / 6.28318530718;
    h = fract(h + 1.0);
    vec3 rgb = hsv2rgb(vec3(h, r, uValue));
    gl_FragColor = vec4(rgb, 1.0);
}"#;
}
static WHEEL_MATERIAL: Lazy<Material> = Lazy::new(|| {
    load_material(
        palette_shader::VERTEX,
        palette_shader::FRAGMENT,
        MaterialParams {
            uniforms: vec![("uValue".to_owned(), miniquad::UniformType::Float1)],
            ..Default::default()
        },
    )
    .unwrap()
});
/// 开关滑块动画状态：key = 控件矩形中心（量化），value = (目标值, 当前值)
thread_local! {
    static SWITCH_ANIM: std::cell::RefCell<std::collections::HashMap<(i32, i32), (f32, f32)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}
/// 开关滑块的缓动进度（0 = 关，1 = 开）。每帧调用一次，内部按帧间隔推进。
/// 供回放管理等子页面复用设置页的面板/内容区。
pub(crate) fn draw_settings_content_panel(ui: &mut Ui) -> Rect {
    let top = ui.top;
    let sk = panel_skew(top);
    let panel_r = panel_right(top);
    fill_quad(
        ui,
        [
            (fx(PANEL_L) + sk, -top),
            (fx(panel_r) + sk, -top),
            (fx(panel_r), top),
            (fx(PANEL_L), top),
        ],
        Color::new(0.026, 0.030, 0.038, 0.80),
    );
    ui.fill_rect(
        Rect::new(-1., -top, 2., (TAB_Y0 + TAB_BAR_H) * 2. * top),
        Color::new(0.012, 0.014, 0.018, 0.92),
    );
    Rect::new(
        fx(CONTENT_X0),
        fy(ui, CONTENT_Y0),
        fx((panel_right(top) - 0.02).min(CONTENT_X1)) - fx(CONTENT_X0),
        fy(ui, CONTENT_Y1) - fy(ui, CONTENT_Y0),
    )
}

pub(crate) fn draw_settings_action_button(ui: &mut Ui, r: Rect, label: &str, btn: &mut DRectButton) {
    btn.inner.set(ui, r);
    let skew = r.h * 0.34;
    fill_slant(ui, r, skew, WHITE);
    stroke_slant(ui, r, skew, 0.0012, semi_white(0.35));
    ui.text(label)
        .pos(r.center().x, r.center().y)
        .anchor(0.5, 0.5)
        .size(0.34)
        .color(BLACK)
        .draw();
}

pub(crate) fn draw_settings_subpage_header(ui: &mut Ui, title: &str) {
    let top = ui.top;
    let y0 = fy(ui, TAB_Y0);
    let y1 = fy(ui, tab_y1(top));
    let r = Rect::new(fx(TAB_X0), y0, 0.42, y1 - y0);
    let sk = slant(ui, tab_y1(top) - TAB_Y0);
    fill_quad(
        ui,
        [
            (r.x + sk, r.y),
            (r.right() + sk, r.y),
            (r.right(), r.bottom()),
            (r.x, r.bottom()),
        ],
        WHITE,
    );
    ui.text(title)
        .pos(r.x + r.w * 0.5, r.center().y)
        .anchor(0.5, 0.5)
        .size(0.46)
        .color(BLACK)
        .draw();
    draw_settings_back(ui, 0.);
}

fn switch_smooth(r: Rect, on: bool) -> f32 {
    let key = ((r.center().x * 512.) as i32, (r.center().y * 512.) as i32);
    let target = if on { 1.0 } else { 0.0 };
    let dt = get_frame_time().min(0.1);
    SWITCH_ANIM.with(|m| {
        let mut m = m.borrow_mut();
        if m.len() > 512 {
            m.clear();
        }
        let e = m.entry(key).or_insert((target, target));
        e.0 = target;
        // 指数逼近：约 0.16s 到位
        let k = 1. - (-dt * 22.).exp();
        e.1 += (e.0 - e.1) * k;
        if (e.0 - e.1).abs() < 0.002 {
            e.1 = e.0;
        }
        e.1.clamp(0., 1.)
    })
}
/// 开关：深色斜切槽 + 白色斜切滑块，开启时左侧空白处有白色对勾（无描边）
pub(crate) fn render_switch(ui: &mut Ui, r: Rect, t: f32, btn: &mut DRectButton, on: bool) {
    let p = switch_smooth(r, on);
    btn.build(ui, t, r, |ui, _| {
        // 参照样式：左深色块（矮一截，内含细对勾）+ 右白色斜块（满高），两块等宽
                let h = r.h * 0.82;
                let w = r.w * 0.95;
        let v = Rect::new(r.right() - w, r.center().y - h * 0.5, w, h);
        let skew = h * 0.30;
        let kw = w * 0.5;
        // 深色块占据滑块对面那一半：开启时在左，关闭时在右
        let dx = v.x + (w - kw) * (1. - p);
        // 深色块右侧多伸 0.004 压到白块下面，斜角处不会漏出背景缝
        let dark = Rect::new(dx - 0.002, v.center().y - h * 0.36, kw + 0.004, h * 0.72);
        fill_slant(ui, dark, skew, Color::new(0.165, 0.175, 0.200, 0.92));
        // 白色滑块：满高、等宽，随动画滑动
        let kx = v.x + (w - kw) * p;
        fill_slant(ui, Rect::new(kx, v.y, kw, h), skew, WHITE);
        // 开启态：深色块里的细对勾，随滑块到位淡入
        let a = ((p - 0.60) / 0.40).clamp(0., 1.);
        if a > 0.001 {
            let cx = dark.center().x + skew * 0.5;
            let cy = dark.center().y + dark.h * 0.02;
                        let d = dark.h * 0.27;
            check_shape(ui, cx, cy, d, (dark.h * 0.018).max(0.0014), semi_white(a));
        }
    });
}
/// 设置页左上角返回按钮：结算页重试键同款斜切按钮 + 镜像 proceed 图标。
pub(crate) fn settings_back_rect(ui: &Ui) -> Rect {
    settings_back_rect_top(ui.top)
}
/// 同上，但只按 top 算（触摸判定没有 Ui 时用）
fn settings_back_rect_top(top: f32) -> Rect {
    let h = TAB_BAR_H;
    let w = 0.23_f32;
    let y = (TAB_Y0 * 2. - 1.) * top;
    Rect::new(-1. - h * PARALLELOGRAM_SLOPE, y, w, h)
}
/// 左上角返回键：结算页重试按钮同款斜切样式 + 镜像 proceed 图标。
pub(crate) fn draw_settings_back(ui: &mut Ui, offset: f32) {
    let base = settings_back_rect(ui);
    let br = Rect::new(base.x + offset, base.y, base.w, base.h);
    draw_parallelogram(br, None, Color::new(0.012, 0.012, 0.016, 0.94), true);
    let stripe = 0.08;
    draw_parallelogram(
        Rect::new(br.x + br.w * (1. - stripe), br.y, br.w * stripe, br.h),
        None,
        WHITE,
        false,
    );
    let bc = br.center();
    let hs = br.h * 0.3;
    if let Some(tex) = TEX_ICON_PROCEED.with(|it| it.borrow().clone()) {
        draw_texture_ex(
            *tex,
            bc.x - hs,
            bc.y - hs,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(hs * 2., hs * 2.)),
                flip_x: true,
                ..Default::default()
            },
        );
    } else {
        ui.text("←")
            .pos(bc.x, bc.y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(br.h * 0.55)
            .color(WHITE)
            .draw();
    }
}
/// 顶部标签栏：黑色斜切长条 + 白色斜切高亮块
fn tab_bar(ui: &mut Ui, active: SettingListType) {
    let y1_frac = tab_y1(ui.top);
    let bar_x1 = tab_bar_right(ui.top);
    let (y0, y1) = (fy(ui, TAB_Y0), fy(ui, y1_frac));
    let sk = slant(ui, y1_frac - TAB_Y0);
    // 黑色长条：左边缘跟着斜切，右端留出可见的斜切角
    fill_quad(
        ui,
        [
            (fx(TAB_X0) + sk, y0),
            (fx(bar_x1) + sk, y0),
            (fx(bar_x1), y1),
            (fx(TAB_X0), y1),
        ],
        Color::new(0.012, 0.012, 0.016, 0.94),
    );
    let slot = (TAB_X1 - TAB_X0) / SETTING_TABS.len() as f32;
    let cy = (y0 + y1) * 0.5;
    for (i, (tab, name)) in SETTING_TABS.iter().enumerate() {
        let cx = TAB_X0 + slot * (i as f32 + 0.5);
        let half = slot * 0.46;
        if *tab == active {
            fill_quad(
                ui,
                [
                    (fx(cx - half) + sk, y0),
                    (fx(cx + half) + sk, y0),
                    (fx(cx + half), y1),
                    (fx(cx - half), y1),
                ],
                WHITE,
            );
        }
        ui.text(tl!(*name))
            .pos(fx(cx) + sk * 0.5, cy)
            .anchor(0.5, 0.5)
            .size(0.46)
            .color(if *tab == active {
                Color::new(0.05, 0.05, 0.07, 1.)
            } else {
                semi_white(0.82)
            })
            .draw();
    }
}
// ---------------- 两栏设置行 ----------------
enum Ctl<'a> {
    Switch { btn: &'a mut DRectButton, on: bool },
    SwitchDelete { switch: &'a mut DRectButton, delete: &'a mut DRectButton, on: bool },
    /// 深色值方块（点一下弹输入/下拉/提示）
    Value { btn: &'a mut DRectButton, text: Cow<'a, str> },
    Arrow { btn: &'a mut DRectButton },
    Choose { btn: &'a mut ChooseButton, w: f32 },
    Slider { slider: &'a mut Slider, value: f32, text: String },
    /// 纯展示行（右侧无控件）
    None,
}
struct Row<'a> {
    label: Cow<'a, str>,
    sub: Option<Cow<'a, str>>,
    ctl: Ctl<'a>,
    group: u8,
}
impl<'a> Row<'a> {
    fn with_group(mut self, group: u8) -> Self {
        self.group = group;
        self
    }
}
fn row_switch<'a>(label: impl Into<Cow<'a, str>>, sub: Option<Cow<'a, str>>, btn: &'a mut DRectButton, on: bool) -> Row<'a> {
    Row { label: label.into(), sub, ctl: Ctl::Switch { btn, on }, group: 0 }
}
fn row_switch_delete<'a>(
    label: impl Into<Cow<'a, str>>,
    sub: Option<Cow<'a, str>>,
    switch: &'a mut DRectButton,
    delete: &'a mut DRectButton,
    on: bool,
) -> Row<'a> {
    Row { label: label.into(), sub, ctl: Ctl::SwitchDelete { switch, delete, on }, group: 0 }
}
fn row_value<'a>(label: impl Into<Cow<'a, str>>, sub: Option<Cow<'a, str>>, btn: &'a mut DRectButton, text: impl Into<Cow<'a, str>>) -> Row<'a> {
    Row { label: label.into(), sub, ctl: Ctl::Value { btn, text: text.into() }, group: 0 }
}
fn row_arrow<'a>(label: impl Into<Cow<'a, str>>, sub: Option<Cow<'a, str>>, btn: &'a mut DRectButton) -> Row<'a> {
    Row { label: label.into(), sub, ctl: Ctl::Arrow { btn }, group: 0 }
}
fn row_choose<'a>(label: impl Into<Cow<'a, str>>, sub: Option<Cow<'a, str>>, btn: &'a mut ChooseButton, w: f32) -> Row<'a> {
    Row { label: label.into(), sub, ctl: Ctl::Choose { btn, w }, group: 0 }
}
fn row_slider<'a>(label: impl Into<Cow<'a, str>>, sub: Option<Cow<'a, str>>, slider: &'a mut Slider, value: f32, text: String) -> Row<'a> {
    Row { label: label.into(), sub, ctl: Ctl::Slider { slider, value, text }, group: 0 }
}
const CTL_TEXT_SIZE: f32 = 0.36;
fn ctl_width(ui: &mut Ui, ctl: &Ctl) -> f32 {
    match ctl {
        Ctl::Switch { .. } => SIZE_SW_W,
        Ctl::SwitchDelete { .. } => SIZE_SW_W + 0.085,
        Ctl::Value { text, .. } => {
            let w = ui.text(Cow::clone(text)).size(CTL_TEXT_SIZE).max_width(SIZE_CTL_MAX - 0.05).measure().w;
            (w + 0.05).clamp(0.09, SIZE_CTL_MAX)
        }
        Ctl::Arrow { .. } => 0.058,
        Ctl::Choose { w, .. } => *w,
        Ctl::Slider { .. } => SIZE_SLIDER_W,
        Ctl::None => 0.,
    }
}
fn draw_row(ui: &mut Ui, t: f32, r: Rect, row: &mut Row) {
    // 分隔线
    ui.fill_rect(Rect::new(r.x, r.bottom() - 0.0014, r.w, 0.0014), semi_white(0.085));
    let cw = ctl_width(ui, &row.ctl);
    let label_w = (r.w - cw - 0.03).max(0.06);
    let has_sub = row.sub.is_some();
    let ly = if has_sub { r.y + r.h * 0.36 } else { r.y + r.h * 0.5 };
    ui.text(Cow::clone(&row.label))
        .pos(r.x, ly)
        .anchor(0., 0.5)
        .size(0.48)
        .max_width(label_w)
        .color(WHITE)
        .draw();
    if let Some(sub) = &row.sub {
        ui.text(Cow::clone(sub))
            .pos(r.x, r.y + r.h * 0.71)
            .anchor(0., 0.5)
            .size(0.25)
            .max_width(label_w)
            .color(semi_white(0.52))
            .draw();
    }
    let cy = r.y + r.h * 0.45;
    let cr = Rect::new(r.right() - cw, cy - SIZE_CTL_H * 0.5, cw, SIZE_CTL_H);
    match &mut row.ctl {
        Ctl::Switch { btn, on } => render_switch(ui, cr, t, btn, *on),
        Ctl::SwitchDelete { switch, delete, on } => {
            render_switch(ui, Rect::new(cr.x, cr.y, SIZE_SW_W, cr.h), t, switch, *on);
            let dr = Rect::new(cr.right() - 0.07, cr.y + cr.h * 0.22, 0.07, cr.h * 0.56);
            delete.build(ui, t, dr, |ui, _| {
                ui.text(tl!("row-delete"))
                    .pos(dr.center().x, dr.center().y)
                    .anchor(0.5, 0.5)
                    .size(0.24)
                    .color(Color::new(1., 0.45, 0.45, 0.95))
                    .draw();
            });
        }
        Ctl::Value { btn, text } => {
            let text = Cow::clone(text);
            btn.build(ui, t, cr, |ui, _| {
                crate::popup::chip(ui, cr, &text, CTL_TEXT_SIZE);
            });
        }
        Ctl::Arrow { btn } => arrow_button(ui, t, btn, cr),
        Ctl::Choose { btn, .. } => btn.render_flat(ui, cr, t, CTL_TEXT_SIZE),
        Ctl::Slider { slider, value, text } => {
            let text = text.clone();
            let track = Rect::new(cr.x, cr.y + cr.h * 0.46, cr.w, cr.h * 0.34);
            ui.text(text)
                .pos(cr.right(), track.y - 0.002)
                .anchor(1., 1.)
                .size(0.26)
                .color(semi_white(0.75))
                .draw();
            slider.render_flat(ui, track, *value);
        }
        Ctl::None => {}
    }
}
/// 两栏网格 + 滚动 + 滚动条。行按列优先排布：前 `left` 行在左栏。
fn render_grid(
    ui: &mut Ui,
    t: f32,
    area: Rect,
    scroll: &mut Scroll,
    rows: &mut [Row],
    content_h: &mut f32,
    left: Option<usize>,
) {
    let n = rows.len();
    let half = left.unwrap_or((n + 1) / 2).min(n);
    let usable_w = area.w - SIZE_RIGHT_PAD;
    let col_w = (usable_w - SIZE_COL_GAP) * 0.5;
    let per_col = half.max(n - half) as f32;
    // 行高不再往下压：固定用 SIZE_ROW_H，行数多了就整体上下滚，别挤成一坨
    let row_h = SIZE_ROW_H;
    let total_h = per_col * row_h + SIZE_ROW_H * 0.3;
    *content_h = total_h;
    scroll.size((area.w, area.h));
    ui.scope(|ui| {
        ui.dx(area.x);
        ui.dy(area.y);
        scroll.render(ui, |ui| {
            let (left, right) = rows.split_at_mut(half);
            for (i, row) in left.iter_mut().enumerate() {
                draw_row(ui, t, Rect::new(0., i as f32 * row_h, col_w, row_h), row);
            }
            for (i, row) in right.iter_mut().enumerate() {
                draw_row(ui, t, Rect::new(col_w + SIZE_COL_GAP, i as f32 * row_h, col_w, row_h), row);
            }
            (area.w, total_h)
        });
    });
}
/// 右侧滚动条 + 底部渐隐
fn render_scroll_hint(ui: &mut Ui, area: Rect, scroll: &Scroll, content_h: f32) {
    if content_h <= area.h + 1e-3 {
        return;
    }
    let max = content_h - area.h;
    let p = (scroll.y_scroller.offset / max).clamp(0., 1.);
    let track_x = fx(0.887);
    let w = 0.0045;
    ui.fill_rect(Rect::new(track_x, area.y, w, area.h), semi_white(0.05));
    let th = (area.h * area.h / content_h).max(0.04);
    let ty = area.y + (area.h - th) * p;
    ui.fill_rect(Rect::new(track_x, ty, w, th), semi_white(0.38));
    // 底部渐隐，避免内容在背板边缘被硬切
    let steps = 8;
    let hh = area.h * 0.07 / steps as f32;
    for i in 0..steps {
        let a = (i as f32 / steps as f32) * 0.62;
        ui.fill_rect(
            Rect::new(area.x - 0.02, area.bottom() - hh * (i + 1) as f32, area.w + 0.04, hh + 0.001),
            Color::new(0.028, 0.032, 0.04, a),
        );
    }
}
/// 账号页
fn render_account(ui: &mut Ui, r: Rect, t: f32, icon: &SafeTexture, logout: &mut DRectButton, switch: &mut DRectButton) {
    let me = get_data().me.clone();
    let av = Rect::new(r.x + 0.01, r.y + 0.03, 0.21, 0.21);
    match &me {
        Some(me) => {
            ui.avatar(
                av.center().x,
                av.center().y,
                av.w / 2.,
                t,
                crate::client::UserManager::opt_avatar(me.id, icon),
            );
        }
        None => {
            ui.fill_circle(av.center().x, av.center().y, av.w / 2., semi_black(0.45));
            ui.text("?")
                .pos(av.center().x, av.center().y)
                .anchor(0.5, 0.5)
                .size(0.6)
                .color(semi_white(0.55))
                .draw();
        }
    }
    let (name, rks) = match &me {
        Some(me) => (me.name.clone(), format!("{:.2}", me.rks)),
        None => (tl!("not-logged-in").to_string(), "--".to_string()),
    };
    let nx = av.right() + 0.05;
    ui.text(&name)
        .pos(nx, av.y + 0.012)
        .anchor(0., 0.)
        .size(0.66)
        .max_width(r.w * 0.5)
        .color(WHITE)
        .draw();
    // 名字底下的白色斜条
    let bw = (name.chars().count() as f32 * 0.16).clamp(0.16, 0.44);
    fill_slant(ui, Rect::new(nx, av.y + 0.088, bw, 0.005), 0.0025, semi_white(0.85));
    ui.text(format!("RKS {}", rks))
        .pos(nx, av.y + 0.10)
        .anchor(0., 0.)
        .size(0.34)
        .color(semi_white(0.7))
        .draw();
    let info = match &me {
        Some(me) => tl!("profile-id", "id" => me.id.to_string()),
        None => tl!("profile-id-none").into_owned(),
    };
    ui.text(info)
        .pos(nx, av.y + 0.155)
        .anchor(0., 0.)
        .size(0.30)
        .color(semi_white(0.45))
        .draw();
    // 简介
    let bio = me.as_ref().and_then(|m| m.bio.clone()).unwrap_or_default();
    ui.text(if bio.is_empty() { tl!("custom-character-no-intro").into_owned() } else { bio })
        .pos(r.x + 0.01, av.bottom() + 0.06)
        .anchor(0., 0.)
        .size(0.34)
        .max_width(r.w * 0.8)
        .multiline()
        .color(semi_white(0.72))
        .draw();
    // 切换账号行（在登出行上方）
    let srow = Rect::new(r.x, r.bottom() - 0.235, r.w, 0.115);
    ui.fill_rect(Rect::new(srow.x, srow.bottom() - 0.0014, srow.w, 0.0014), semi_white(0.085));
    switch.build(ui, t, srow, |ui, _| {
        ui.text(tl!("sx-account-switch"))
            .pos(srow.x, srow.center().y)
            .anchor(0., 0.5)
            .size(0.54)
            .color(WHITE)
            .draw();
    });
    let cr2 = Rect::new(srow.right() - 0.058, srow.center().y - SIZE_CTL_H * 0.5, 0.058, SIZE_CTL_H);
    arrow_shape(ui, cr2);
    // 登出行
    let row = Rect::new(r.x, r.bottom() - 0.115, r.w, 0.115);
    ui.fill_rect(Rect::new(row.x, row.bottom() - 0.0014, row.w, 0.0014), semi_white(0.085));
    logout.build(ui, t, row, |ui, _| {
        ui.text(tl!("sx-logout"))
            .pos(row.x, row.center().y)
            .anchor(0., 0.5)
            .size(0.54)
            .color(if me.is_some() { WHITE } else { semi_white(0.35) })
            .draw();
    });
    let cr = Rect::new(row.right() - 0.058, row.center().y - SIZE_CTL_H * 0.5, 0.058, SIZE_CTL_H);
    arrow_shape(ui, cr);
}
/// 关于页
fn render_about_page(ui: &mut Ui, t: f32, r: Rect, links: &mut [DRectButton; 4], scroll: &mut Scroll, content_h: &mut f32) {
    let head_h = 0.30;
    crate::splash_ui::draw_logo(ui, r.x + r.w * 0.20, r.y + 0.10, 0.80, 1.0);
    ui.fill_rect(Rect::new(r.x + r.w * 0.40, r.y + 0.03, 0.0012, 0.115), semi_white(0.25));
    ui.text(format!("Version : {}", crate::APP_VERSION))
        .pos(r.x + r.w * 0.43, r.y + 0.06)
        .anchor(0., 0.5)
        .size(0.32)
        .color(semi_white(0.8))
        .draw();
    ui.text(format!("Device : {} {}", std::env::consts::OS, std::env::consts::ARCH))
        .pos(r.x + r.w * 0.43, r.y + 0.105)
        .anchor(0., 0.5)
        .size(0.30)
        .color(semi_white(0.55))
        .draw();
    let items = [tl!("sx-audio-troubleshooting"), tl!("sx-watch-tutorial"), tl!("sx-open-source-licenses"), tl!("sx-privacy-policy")];
    let mut rows: Vec<Row> = Vec::with_capacity(4);
    let mut it = links.iter_mut();
    for name in items {
        if let Some(btn) = it.next() {
            rows.push(row_arrow(name, None, btn));
        }
    }
    render_grid(ui, t, Rect::new(r.x, r.y + head_h, r.w, r.h - head_h - 0.10), scroll, &mut rows, content_h, None);
    ui.text(tl!("sx-about-footer", "version" => crate::APP_VERSION))
        .pos(r.x + r.w * 0.5, r.bottom() - 0.02)
        .anchor(0.5, 0.5)
        .size(0.26)
        .color(semi_white(0.4))
        .draw();
}
impl SettingsPage {
    const SAVE_TIME: f32 = 0.5;
    pub fn new(icon: SafeTexture, icon_lang: SafeTexture) -> Self {
        Self {
            list_general: GeneralList::new(icon_lang),
            list_audio: AudioList::new(),
            list_chart: ChartList::new(),
            list_visual: VisualList::new(),
            list_custom: CustomList::new(),
            list_debug: DebugList::new(),
            list_replay: ReplayList::new(),
            list_plugin: PluginList::new(),
            tab: SettingListType::General,
            content_h: 1.,
            scroll: Scroll::new(),
            save_time: f32::INFINITY,
            logout_btn: DRectButton::new(),
            switch_btn: DRectButton::new(),
            account_overlay: false,
            account_rows: Vec::new(),
            account_del_btns: Vec::new(),
            account_add_btn: DRectButton::new(),
            account_cancel_btn: DRectButton::new(),
            switch_task: None,
            login: None,
            login_open: false,
            link_buttons: [
                DRectButton::new(),
                DRectButton::new(),
                DRectButton::new(),
                DRectButton::new(),
            ],
            tab_anim_start: f32::NEG_INFINITY,
            tab_anim_dir: 1.,
            icon,
        }
    }
    /// Rebuild dropdown option text after switching language, keeping current selections.
    fn refresh_locale(&mut self) {
        let c = &mut self.list_chart;
        c.indicator_delay_mode_btn
            .set_options(vec![tl!("sx-adofai").to_string(), "phigros".to_string()]);
        c.indicator_color_mode_btn
            .set_options(vec![tl!("sx-adofai").to_string(), "phira".to_string()]);
        c.fps_pos_btn.set_options(vec![
            tl!("sx-pos-tl").to_string(),
            tl!("sx-pos-tr").to_string(),
            tl!("sx-pos-bl").to_string(),
            tl!("sx-pos-br").to_string(),
        ]);
        c.judge_preset_btn.set_options(vec![
            tl!("sx-builtin").to_string(),
            "Phigros".to_string(),
            "Phira".to_string(),
            tl!("sx-custom").to_string(),
        ]);
        c.perf_btn.set_options(vec![
            tl!("item-perf-off").to_string(),
            tl!("item-perf-light").to_string(),
            tl!("item-perf-medium").to_string(),
            tl!("item-perf-full").to_string(),
            tl!("item-perf-ultra").to_string(),
            tl!("item-perf-custom").to_string(),
        ]);
        self.list_visual.noise_demo_btn.set_options(vec![
            tl!("sx-noise-demo-phigros").to_string(),
            tl!("sx-noise-demo-solid").to_string(),
        ]);
        self.list_visual.dynamic_bg_btn.set_options(vec![
            tl!("sx-off").to_string(),
            tl!("sx-static-brightness").to_string(),
            tl!("sx-dynamic-brightness").to_string(),
        ]);
        self.list_custom.home_bg_btn.set_options(vec![
            tl!("item-home-bg-default").to_string(),
            tl!("item-home-bg-one").to_string(),
            tl!("item-home-bg-two").to_string(),
            tl!("item-home-bg-three").to_string(),
            tl!("item-home-bg-old").to_string(),
            tl!("item-home-bg-loginbg").to_string(),
        ]);
    }
    fn tab_at(&self, p: Vec2) -> Option<SettingListType> {
        let top = 1. / (macroquad::window::screen_width() / macroquad::window::screen_height());
        // 左上角返回键贴图占住的区域归返回键，不算标签
        if settings_back_rect_top(top).contains(p) {
            return None;
        }
        let y0 = (TAB_Y0 * 2. - 1.) * top;
        let y1_frac = tab_y1(top);
        let y1 = (y1_frac * 2. - 1.) * top;
        if p.y < y0 || p.y > y1 {
            return None;
        }
        // 顶栏整体斜切：越靠上越向右偏
        let sk = PARALLELOGRAM_SLOPE * (y1_frac - TAB_Y0) * 2. * top;
        let dy = (p.y - y0) / (y1 - y0); // 0 = 顶部, 1 = 底部
        let x0 = fx(TAB_X0) + sk * (1. - dy);
        let x1 = fx(tab_bar_right(top)) + sk * (1. - dy);
        if p.x < x0 || p.x > x1 {
            return None;
        }
        if p.x > fx(TAB_X1) + sk * (1. - dy) {
            return None;
        }
        let slot = (fx(TAB_X1) - fx(TAB_X0)) / SETTING_TABS.len() as f32;
        let i = ((p.x - x0) / slot) as usize;
        SETTING_TABS.get(i.min(SETTING_TABS.len() - 1)).map(|(t, _)| *t)
    }
}
impl SettingsPage {
    /// 切换账号弹窗：列出登录过的账号，点一行直接切换；底部可登录其他账号。
    fn render_account_overlay(&mut self, ui: &mut Ui, t: f32) {
        let accounts = get_data().saved_accounts.clone();
        let cur = get_data().me.as_ref().map(|it| it.id);
        ui.fill_rect(ui.screen_rect(), semi_black(0.62));
        let row_h = 0.105;
        let shown = accounts.len().clamp(1, 5) as f32;
        let h = 0.115 + row_h * shown + 0.105 + 0.09 + 0.05;
        let w = 0.86f32;
        let r = Rect::new(-w * 0.5, -h * 0.5, w, h);
        ui.fill_path(&r.rounded(0.02), Color::new(0.10, 0.12, 0.17, 0.97));
        ui.text(tl!("sx-account-switch"))
            .pos(r.x + 0.045, r.y + 0.042)
            .anchor(0., 0.)
            .size(0.62)
            .color(WHITE)
            .draw();
        if self.account_rows.len() != accounts.len() {
            self.account_rows = accounts.iter().map(|_| DRectButton::new()).collect();
            self.account_del_btns = accounts.iter().map(|_| DRectButton::new()).collect();
        }
        let mut y = r.y + 0.115;
        for (i, acc) in accounts.iter().take(5).enumerate() {
            let row = Rect::new(r.x + 0.03, y, r.w - 0.06, row_h);
            let is_cur = cur == Some(acc.id);
            let name = if acc.name.is_empty() { format!("#{}", acc.id) } else { acc.name.clone() };
            let initial = acc.name.chars().next().map(|c| c.to_string()).unwrap_or_else(|| "?".to_string());
            let info = format!("ID {}   RKS {:.2}", acc.id, acc.rks);
            let cur_tag = if is_cur { tl!("sx-account-current").into_owned() } else { String::new() };
            self.account_rows[i].build(ui, t, row, |ui, _| {
                let d = row.h * 0.62;
                let cr = Rect::new(row.x + 0.012, row.center().y - d * 0.5, d, d);
                ui.fill_path(&cr.rounded(d * 0.5), if is_cur { semi_white(0.9) } else { semi_white(0.18) });
                ui.text(&initial)
                    .pos(cr.center().x, cr.center().y)
                    .anchor(0.5, 0.5)
                    .size(d * 0.8)
                    .color(if is_cur { semi_black(0.9) } else { WHITE })
                    .draw();
                ui.text(&name)
                    .pos(row.x + 0.012 + 0.09, row.center().y - 0.014)
                    .anchor(0., 0.5)
                    .size(0.5)
                    .max_width(row.w - 0.24)
                    .color(WHITE)
                    .draw();
                ui.text(&info)
                    .pos(row.x + 0.012 + 0.09, row.center().y + 0.028)
                    .anchor(0., 0.5)
                    .size(0.3)
                    .color(semi_white(0.55))
                    .draw();
                if !cur_tag.is_empty() {
                    ui.text(&cur_tag)
                        .pos(row.right() - 0.02, row.center().y)
                        .anchor(1., 0.5)
                        .size(0.34)
                        .color(semi_white(0.6))
                        .draw();
                }
            });
            if !is_cur {
                let del = Rect::new(row.right() - 0.078, row.center().y - 0.027, 0.054, 0.054);
                self.account_del_btns[i].build(ui, t, del, |ui, _| {
                    ui.text("×")
                        .pos(del.center().x, del.center().y)
                        .anchor(0.5, 0.5)
                        .size(0.5)
                        .color(semi_white(0.45))
                        .draw();
                });
            }
            y += row_h;
        }
        let add = Rect::new(r.x + 0.03, y + 0.02, r.w - 0.06, 0.105);
        self.account_add_btn.build(ui, t, add, |ui, _| {
            ui.text(tl!("sx-account-add"))
                .pos(add.x, add.center().y)
                .anchor(0., 0.5)
                .size(0.5)
                .color(semi_white(0.9))
                .draw();
        });
        let cancel = Rect::new(r.x + 0.03, add.bottom() + 0.02, r.w - 0.06, 0.09);
        self.account_cancel_btn.build(ui, t, cancel, |ui, _| {
            ui.text(tl!("sx-cancel"))
                .pos(cancel.center().x, cancel.center().y)
                .anchor(0.5, 0.5)
                .size(0.46)
                .color(semi_white(0.7))
                .draw();
        });
    }
}
impl Page for SettingsPage {
    fn label(&self) -> Cow<'static, str> {
        "".into()
    }
    fn back_button_rect(&self, ui: &Ui) -> Option<Rect> {
        // 点击区域固定用最终落点：入场/退场动画期间也不会点不到。
        Some(settings_back_rect(ui))
    }
    fn exit(&mut self) -> Result<()> {
        BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
        if self.save_time.is_finite() {
            save_data()?;
        }
        Ok(())
    }
    fn on_back_pressed(&mut self, _s: &mut SharedState) -> bool {
        false
    }
    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        // 内嵌登录面板 / 切换账号弹窗都是模态层，先把输入全吃掉
        if self.login_open {
            if let Some(login) = &mut self.login {
                login.touch(touch, s.t);
            }
            return Ok(true);
        }
        if self.account_overlay {
            let tt = s.t;
            if self.account_cancel_btn.touch(touch, tt) {
                self.account_overlay = false;
                return Ok(true);
            }
            if self.account_add_btn.touch(touch, tt) {
                self.account_overlay = false;
                let login = self.login.get_or_insert_with(crate::login::Login::new_bare);
                login.enter(tt);
                self.login_open = true;
                return Ok(true);
            }
            let cur = get_data().me.as_ref().map(|it| it.id);
            let ids: Vec<i32> = get_data().saved_accounts.iter().map(|it| it.id).collect();
            for (i, id) in ids.iter().enumerate() {
                if cur == Some(*id) {
                    continue;
                }
                if i < self.account_del_btns.len() && self.account_del_btns[i].touch(touch, tt) {
                    crate::client::Client::forget_account(*id);
                    return Ok(true);
                }
                if i < self.account_rows.len() && self.account_rows[i].touch(touch, tt) {
                    let id = *id;
                    self.switch_task = Some(Task::new(async move { crate::client::Client::switch_to(id).await }));
                    self.account_overlay = false;
                    show_message(tl!("sx-account-switching")).ok();
                    return Ok(true);
                }
            }
            return Ok(true);
        }
        // 调色板是模态层：打开时优先吃掉所有输入，避免点到后面的标签/按钮。
        if self.tab == SettingListType::Visual && self.list_visual.palette.is_open() {
            self.list_visual.palette.touch(touch);
            return Ok(true);
        }
        let t = s.t;
        // 返回键由 MainScene 统一处理；这里不要把它判成标签/滚动而吃掉点击。
        {
            let top = screen_height() / screen_width();
            let back = settings_back_rect_top(top);
            if back.contains(touch.position) {
                return Ok(false);
            }
        }
        if touch.phase == TouchPhase::Started {
            if let Some(tab) = self.tab_at(touch.position) {
                if tab != self.tab {
                    // 按标签左右顺序决定切页滑入方向
                    let idx = |v: SettingListType| SETTING_TABS.iter().position(|(t, _)| *t == v);
                    self.tab_anim_dir = if idx(tab) >= idx(self.tab) { 1. } else { -1. };
                    self.tab_anim_start = t;
                    self.tab = tab;
                    self.scroll.y_scroller.halt();
                    self.scroll.y_scroller.reset();
                }
                return Ok(true);
            }
        }
        // 下拉弹窗画在最上层，触摸也优先给它
        let top_hit = match self.tab {
            SettingListType::General => self.list_general.top_touch(touch, t) || self.list_chart.top_touch(touch, t),
            SettingListType::Chart => self.list_chart.top_touch(touch, t),
            SettingListType::Visual => self.list_visual.top_touch(touch, t) || self.list_chart.top_touch(touch, t),
            SettingListType::Custom => self.list_custom.top_touch(touch, t) || self.list_chart.top_touch(touch, t),
            SettingListType::Debug => self.list_chart.top_touch(touch, t),
            _ => false,
        };
        if top_hit {
            return Ok(true);
        }
        match self.tab {
            SettingListType::Account => {
                if self.switch_btn.touch(touch, t) {
                    crate::client::Client::sync_current_account();
                    self.account_overlay = true;
                    return Ok(true);
                }
                if self.logout_btn.touch(touch, t) && get_data().me.is_some() {
                    let data = get_data_mut();
                    data.me = None;
                    data.tokens = None;
                    save_data()?;
                    show_message(tl!("sx-logged-out")).ok();
                    return Ok(true);
                }
                return Ok(false);
            }
            SettingListType::About => {
                for i in 0..4 {
                    if self.link_buttons[i].touch(touch, t) {
                        match i {
                            0 => {
                                show_message(tl!("sx-audio-tip")).ok();
                            }
                            1 => {
                                self.list_general.start_tutorial();
                            }
                            2 => {
                                let _ = open_url("https://github.com/LuteRenxaer/PhirLie");
                            }
                            _ => {
                                show_message(tl!("sx-privacy-note")).ok();
                            }
                        }
                        return Ok(true);
                    }
                }
                return Ok(false);
            }
            _ => {}
        }
        // 行控件只认内容区里的点击：滚动后看不见的行、以及别处点进来都不该触发
        if !self.scroll.contains(touch) {
            return Ok(false);
        }
        if self.scroll.touch(touch, t) {
            return Ok(true);
        }
        let res = match self.tab {
            SettingListType::General => self.list_general.touch(touch, t)?,
            SettingListType::Audio => self.list_audio.touch(touch, t)?,
            SettingListType::Chart => self.list_chart.touch(touch, t)?,
            SettingListType::Visual => self.list_visual.touch(touch, t)?,
            SettingListType::Custom => self.list_custom.touch(touch, t)?,
            SettingListType::Debug => self.list_debug.touch(touch, t)?,
            SettingListType::Replay => self.list_replay.touch(touch, t)?,
            SettingListType::Plugin => self.list_plugin.touch(touch, t)?,
            _ => None,
        };
        if let Some(p) = res {
            if p {
                self.save_time = t;
            }
            self.scroll.y_scroller.halt();
            return Ok(true);
        }
        // 谱面/判定那些行只在本页或与其合并显示的页里才画，别的页不能再吃点击
        let chart_rows_shown = matches!(
            self.tab,
            SettingListType::General | SettingListType::Visual | SettingListType::Custom | SettingListType::Debug
        );
        if chart_rows_shown {
            if let Some(p) = self.list_chart.touch(touch, t)? {
                if p {
                    self.save_time = t;
                }
                return Ok(true);
            }
        }
        Ok(false)
    }
    fn enter(&mut self, _s: &mut SharedState) -> Result<()> {
        Ok(())
    }
    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        // 插件导入：文件选择器回来后在设置页统一处理（导入 + 刷新列表 + 提示）
        if let Some((id, file)) = take_file() {
            if id == "_import_plugin" {
                match crate::plugin::import_plugin_archive(std::path::Path::new(&file)) {
                    Ok(p) => {
                        self.list_plugin.refresh_plugins();
                        show_message(tl!("sx-plugin-imported", "name" => p.info.name, "version" => p.info.version).to_string()).ok();
                    }
                    Err(err) => show_error(err.context(tl!("sx-plugin-import-failed"))),
                }
            } else {
                return_file(id, file);
            }
        }
        self.list_general.poll_tutorial()?;
        let mut changed = match self.tab {
            SettingListType::General => self.list_general.update(t)?,            SettingListType::Audio => self.list_audio.update(t)?,
            SettingListType::Chart => self.list_chart.update(t)?,
            SettingListType::Visual => self.list_visual.update(t)?,
            SettingListType::Custom => self.list_custom.update(t)?,
            SettingListType::Debug => self.list_debug.update(t)?,
            SettingListType::Replay => self.list_replay.update(t)?,
            SettingListType::Plugin => self.list_plugin.update(t)?,
            _ => false,
        };
        if self.tab != SettingListType::Chart && self.list_chart.update(t)? {
            changed = true;
        }
        if self.list_general.take_language_changed() {
            self.refresh_locale();
            changed = true;
        }
        self.scroll.update(t);
        if let Some(task) = &mut self.switch_task {
            if let Some(res) = task.take() {
                self.switch_task = None;
                match res {
                    Ok(()) => {
                        show_message(tl!("sx-account-switched")).ok();
                    }
                    Err(err) => show_error(err.context(tl!("sx-account-switch-failed"))),
                }
            }
        }
        if let Some(login) = &mut self.login {
            login.update(t)?;
            if self.login_open && !login.is_open() && !login.busy() {
                self.login_open = false;
                crate::client::Client::sync_current_account();
            }
        }
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
        // 每帧重置命中帧号：只有这一帧真正画过的控件才接收触摸
        begin_hit_frame();
        s.fader.render(ui, s.t, |ui| {
            // 背景由 MainScene 统一绘制（与主页同一套底板），这里只画设置页自己的内容。
            let top = ui.top;
            // 大背板：保持 15° 斜切，右上角留在屏幕内
            let sk = panel_skew(top);
            let panel_r = panel_right(top);
            fill_quad(
                ui,
                [
                    (fx(PANEL_L) + sk, -top),
                    (fx(panel_r) + sk, -top),
                    (fx(panel_r), top),
                    (fx(PANEL_L), top),
                ],
                Color::new(0.026, 0.030, 0.038, 0.74),
            );
            // 左上返回键在面板淡出/右移之外单独绘制（见 render 末尾）。
            tab_bar(ui, self.tab);
            let area = Rect::new(
                fx(CONTENT_X0),
                fy(ui, CONTENT_Y0),
                fx((panel_right(top) - 0.02).min(CONTENT_X1)) - fx(CONTENT_X0),
                fy(ui, CONTENT_Y1) - fy(ui, CONTENT_Y0),
            );
            // 切页动画：内容顺着切换方向滑入 + 淡入（0.22s，ease-out cubic）
            let ap = ((t - self.tab_anim_start) / 0.22).clamp(0., 1.);
            let ap = 1. - (1. - ap).powi(3);
            let slide = self.tab_anim_dir * (1. - ap) * 0.085;
            ui.scissor(area, |ui| {
            ui.scope(|ui| {
                ui.dx(slide);
                ui.alpha(0.15 + 0.85 * ap, |ui| {
                    match self.tab {
                        SettingListType::Account => {
                            render_account(ui, area, t, &self.icon, &mut self.logout_btn, &mut self.switch_btn);
                        }
                        SettingListType::About => {
                            render_about_page(ui, t, area, &mut self.link_buttons, &mut self.scroll, &mut self.content_h);
                        }
                        _ => {
                            let mut rows = match self.tab {
                                SettingListType::General => {
                                    let mut r = self.list_general.rows();
                                    r.extend(self.list_chart.general_rows());
                                    r
                                }
                                SettingListType::Audio => self.list_audio.rows(),
                                SettingListType::Chart => self.list_chart.rows(),
                                SettingListType::Visual => {
                                    let mut r = self.list_visual.rows();
                                    r.extend(self.list_chart.visual_rows());
                                    r
                                }
                                SettingListType::Custom => {
                                    let mut r = self.list_custom.rows();
                                    r.extend(self.list_chart.custom_rows());
                                    r
                                }
                                SettingListType::Debug => {
                                    let mut r = self.list_debug.rows();
                                    r.extend(self.list_chart.debug_rows());
                                    r
                                }
                                SettingListType::Plugin => self.list_plugin.rows(),
                                SettingListType::Replay => self.list_replay.rows(),
                                _ => Vec::new(),
                            };
                            let custom_left = matches!(self.tab, SettingListType::Custom).then_some(CustomList::LEFT_ROWS);
                            render_grid(ui, t, area, &mut self.scroll, &mut rows, &mut self.content_h, custom_left);
                            drop(rows);
                        }
                    }
                    if matches!(
                        self.tab,
                        SettingListType::General
                            | SettingListType::Audio
                            | SettingListType::Chart
                            | SettingListType::Visual
                            | SettingListType::Custom
                            | SettingListType::Debug
                            | SettingListType::Plugin
                    ) {
                        render_scroll_hint(ui, area, &self.scroll, self.content_h);
                    }
                });
            });
            });
            // 下拉弹窗必须最后画
            match self.tab {
                SettingListType::General => self.list_general.render_top(ui, t),
                SettingListType::Chart => self.list_chart.render_top(ui, t),
                SettingListType::Visual => self.list_visual.render_top(ui, t),
                SettingListType::Custom => self.list_custom.render_top(ui, t),
                _ => {}
            }
            if self.tab != SettingListType::Chart {
                self.list_chart.render_top(ui, t);
            }
            draw_settings_back(ui, 0.);
            Ok::<(), anyhow::Error>(())
        })?;
        // 模态层：切换账号弹窗 / 内嵌登录面板，画在最上层
        if self.account_overlay {
            self.render_account_overlay(ui, s.t);
        }
        if let Some(login) = &mut self.login {
            login.render(ui, s.t);
        }
        Ok(())
    }
    /// 教学谱面等 Overlay 场景失败退出时（NextScene::PopWithResult），
    /// 把错误弹出来，别再静默回退到设置页让人以为"点了没反应"。
    fn on_result(&mut self, result: Box<dyn std::any::Any>, _s: &mut SharedState) -> Result<()> {
        if let Some(err) = result.downcast_ref::<anyhow::Error>() {
            show_error(anyhow::anyhow!("{err:#}"));
        }
        Ok(())
    }
    fn next_page(&mut self) -> NextPage {
        if let Some(p) = self.list_general.take_data_request() {
            return p;
        }
        if self.tab == SettingListType::Audio {
            if let Some(p) = self.list_audio.next_page() {
                return p;
            }
        }
        if self.tab == SettingListType::Custom {
            if let Some(p) = self.list_custom.next_page() {
                return p;
            }
        }
        if self.tab == SettingListType::Debug {
            if let Some(p) = self.list_debug.next_page() {
                return p;
            }
        }
        if self.tab == SettingListType::Replay {
            if let Some(p) = self.list_replay.next_page() {
                return p;
            }
        }
        NextPage::None
    }
    fn next_scene(&mut self, _s: &mut SharedState) -> NextScene {
        if let Some(scene) = self.list_replay.take_scene() {
            return scene;
        }
        if let Some(scene) = self.list_general.take_lan_scene() {
            return scene;
        }
        if let Some(scene) = self.list_general.take_tutorial_scene() {
            return scene;
        }
        let data = get_data();
        let config = &data.config;
        if self.list_debug.take_crash_request() {
            NextScene::Overlay(Box::new(CrashScene::new(CrashCode::ManualCrash, "".to_string())))
        } else if self.list_custom.take_custom_crash_request() {
            let code = config.custom_crash_code;
            let reason = config.custom_crash_reason.clone();
            let title = config.custom_crash_title.clone();
            NextScene::Overlay(Box::new(CrashScene::new(CrashCode::Custom { code, reason }, title)))
        } else {
            NextScene::None
        }
    }
}
struct GeneralList {
    /// 语言图标（当前版式未画，保留给后续用）
    _icon_lang: SafeTexture,
    lang_btn: ChooseButton,
    #[cfg(all(any(target_os = "windows", target_os = "linux"), not(target_env = "ohos")))]
    fullscreen_btn: DRectButton,
    cache_btn: DRectButton,
    legacy_btn: DRectButton,
    offline_btn: DRectButton,
    upload_score_btn: DRectButton,
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
    vsync_btn: DRectButton,
    console_btn: DRectButton,
    reset_settings_btn: DRectButton,
    roman_numerals_btn: DRectButton,
    chinese_numerals_btn: DRectButton,
    lan_btn: DRectButton,
    /// 数据传输页入口（导入/导出 .pfd、局域网跨设备传输）
    data_btn: DRectButton,
    data_page: Option<NextPage>,
    cache_size: Option<u64>,
    cache_task: Option<Task<Result<u64>>>,
    tutorial_task: LocalTask<Result<NextScene>>,
    tutorial_scene: Option<NextScene>,
    lan_scene: Option<NextScene>,
    language_changed: bool,
}
impl GeneralList {
    pub fn new(icon_lang: SafeTexture) -> Self {
        let mut this = Self {
            _icon_lang: icon_lang,
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
            legacy_btn: DRectButton::new(),
            offline_btn: DRectButton::new(),
            upload_score_btn: DRectButton::new(),
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
            vsync_btn: DRectButton::new(),
            console_btn: DRectButton::new(),
            reset_settings_btn: DRectButton::new(),
            roman_numerals_btn: DRectButton::new(),
            chinese_numerals_btn: DRectButton::new(),
            lan_btn: DRectButton::new(),
            data_btn: DRectButton::new(),
            data_page: None,
            cache_size: None,
            cache_task: None,
            tutorial_task: None,
            tutorial_scene: None,
            lan_scene: None,
            language_changed: false,
        };
        let data = get_data_mut();
        if !data.config.particle {
            data.config.particle = true;
        }
        let _ = this.update_cache_size();
        this
    }
    pub fn take_language_changed(&mut self) -> bool {
        std::mem::take(&mut self.language_changed)
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
            set_fullscreen_mode(config.fullscreen_mode);
            return Ok(Some(true));
        }
        if self.cache_btn.touch(touch, t) {
            fs::remove_dir_all(dir::cache()?)?;
            self.update_cache_size()?;
            show_message(tl!("item-cache-cleared")).ok();
            return Ok(Some(false));
        }
        if self.legacy_btn.touch(touch, t) {
            crate::migrate::manual_sync();
            return Ok(Some(false));
        }
        if self.offline_btn.touch(touch, t) {
            config.offline_mode ^= true;
            return Ok(Some(true));
        }
        if self.upload_score_btn.touch(touch, t) {
            if config.upload_score_enabled {
                config.upload_score_enabled = false;
            } else {
                Dialog::plain(
                    tl!("sx-upload-title"),
                    format!("{}\n{}", tl!("sx-upload-body"), tl!("sx-upload-warning")),
                )
                    .buttons(vec![tl!("sx-cancel").to_string(), tl!("sx-confirm-enable").to_string()])
                    .listener(move |_dialog, pos| {
                        if pos == 1 {
                            get_data_mut().config.upload_score_enabled = true;
                            let _ = crate::save_data();
                        }
                        false
                    })
                    .show();
            }
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
            config.sample_count = if config.sample_count == 1 { 4 } else { 1 };
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
        if self.vsync_btn.touch(touch, t) {
            match config.perf_profile() {
                prpr::config::Config::PERF_ULTRA => {
                }
                prpr::config::Config::PERF_CUSTOM => {
                    config.perf_custom_vsync_off ^= true;
                    prpr::set_swap_interval(if config.perf_custom_vsync_off || !config.vsync { 0 } else { 1 });
                }
                _ => {
                    config.vsync ^= true;
                    prpr::set_swap_interval(if config.vsync { 1 } else { 0 });
                }
            }
            let _ = crate::save_data();
            return Ok(Some(true));
        }
        if self.console_btn.touch(touch, t) {
            config.console_enabled ^= true;
            crate::set_console_enabled(config.console_enabled);
            let _ = crate::save_data();
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
        if self.reset_settings_btn.touch(touch, t) {
            let data = get_data_mut();
            data.config = Default::default();
            data.custom_bgm_path = None;
            data.custom_startup_bgm_path = None;
            data.home_video_path = None;
            data.config.particle = true;
            BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
            HOME_VIDEO_UPDATED.store(true, Ordering::Relaxed);
            BACKGROUND_UPDATED.store(true, Ordering::Relaxed);
            show_message(tl!("item-reset-settings-done")).ok();
            return Ok(Some(true));
        }
        if self.lan_btn.touch(touch, t) {
            self.lan_scene = Some(NextScene::Overlay(Box::new(crate::scene::LanScene::new())));
            return Ok(Some(false));
        }
        if self.data_btn.touch(touch, t) {
            self.data_page = Some(NextPage::Overlay(Box::new(super::DataManagePage::new())));
            return Ok(Some(false));
        }
        Ok(None)
    }
    /// 取出“打开数据传输页”的请求（由 SettingsPage 转成 next_page）
    pub fn take_data_request(&mut self) -> Option<NextPage> {
        self.data_page.take()
    }
    /// 关于页「观看教程」调用；加载完成后由 SettingsPage::next_scene 进入。
    pub fn start_tutorial(&mut self) {
        if self.tutorial_task.is_some() {
            return;
        }
        self.tutorial_task = Some(Box::pin(async move {
            let mut fs = prpr::fs::fs_from_file(std::path::Path::new("assets/Tutorial"))?;
            let mut info = prpr::fs::load_info(fs.as_mut()).await?;
            info.tip = Some(tl!("tutorial").to_string());
            let config = get_data().config.clone();
            let scene = prpr::scene::LoadingScene::new(
                prpr::scene::GameMode::Tutorial,
                info,
                config,
                fs,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
            Ok(NextScene::Overlay(Box::new(scene)))
        }));
    }
    pub fn update(&mut self, t: f32) -> Result<bool> {
        self.lang_btn.update(t);
        let data = get_data_mut();
        if self.lang_btn.changed() {
            data.language = Some(LANG_IDENTS[self.lang_btn.selected()].to_string());
            let locale = data.language.as_ref().and_then(|it| it.parse().ok());
            set_prefered_locale(locale);
            self.language_changed = true;
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
        if let Some(task) = &mut self.cache_task {
            if let Some(size) = task.take() {
                self.cache_size = size.ok();
                self.cache_task = None;
            }
        }
        Ok(false)
    }
    /// 关于页可能不在「通用」标签，因此由 SettingsPage 每帧调用。
    pub fn poll_tutorial(&mut self) -> Result<()> {
        if let Some(task) = &mut self.tutorial_task {
            if let Some(result) = poll_future(task.as_mut()) {
                match result {
                    Ok(scene) => {
                        self.tutorial_scene = Some(scene);
                    }
                    Err(err) => {
                        show_error(err.context(tl!("tutorial-load-failed").to_string()));
                    }
                }
                self.tutorial_task = None;
            }
        }
        Ok(())
    }
    pub fn take_tutorial_scene(&mut self) -> Option<NextScene> {
        self.tutorial_scene.take()
    }
    pub fn take_lan_scene(&mut self) -> Option<NextScene> {
        self.lan_scene.take()
    }
    /// 通用页：两栏排布，顺序对齐参考图（左栏「语言/全屏/离线/服务器状态/多人…」，
    /// 右栏「Anys/网关/性能…」），本客户端多出来的项接在各自栏末。
    pub fn rows(&mut self) -> Vec<Row<'_>> {
        let data = get_data();
        let config = &data.config;
        let mut v = Vec::with_capacity(21);
        // ---- 左栏 ----
        v.push(row_choose(tl!("item-lang"), None, &mut self.lang_btn, 0.28));
        #[cfg(all(any(target_os = "windows", target_os = "linux"), not(target_env = "ohos")))]
        v.push(row_switch(tl!("item-fullscreen"), None, &mut self.fullscreen_btn, config.fullscreen_mode));
        v.push(row_switch(tl!("item-offline"), Some(tl!("item-offline-sub")), &mut self.offline_btn, config.offline_mode));
        v.push(row_arrow(tl!("item-server-status"), Some(tl!("item-server-status-sub")), &mut self.server_status_btn));
        v.push(row_switch(tl!("item-mp"), Some(tl!("item-mp-sub")), &mut self.mp_btn, config.mp_enabled));
        v.push(row_value(tl!("item-mp-addr"), Some(tl!("item-mp-addr-sub")), &mut self.mp_addr_btn, config.mp_address.clone()));
        v.push(row_switch(
            tl!("item-prefer-reduced-motion"),
            Some(tl!("item-prefer-reduced-motion-sub")),
            &mut self.prefer_reduced_motion_btn,
            data.prefer_reduced_motion,
        ));
        v.push(row_switch(
            tl!("sx-upload-score"),
            Some(tl!("sx-upload-score-sub").into()),
            &mut self.upload_score_btn,
            config.upload_score_enabled,
        ));
        v.push(row_switch(tl!("item-insecure"), Some(tl!("item-insecure-sub")), &mut self.insecure_btn, data.accept_invalid_cert));
        let cache_size = if let Some(size) = self.cache_size {
            Cow::Owned(tl!("item-cache-size", "size" => ByteSize(size).to_string()))
        } else {
            tl!("item-cache-size-loading")
        };
        v.push(row_value(tl!("item-clear-cache"), Some(cache_size), &mut self.cache_btn, tl!("item-clear-cache-btn")));
        v.push(row_arrow(tl!("migrate-settings-item"), Some(tl!("migrate-settings-item-sub")), &mut self.legacy_btn));
        // ---- 右栏 ----
        v.push(row_switch(tl!("item-enable-anys"), Some(tl!("item-enable-anys-sub")), &mut self.enable_anys_btn, data.enable_anys));
        v.push(row_value(tl!("item-anys-gateway"), Some(tl!("item-anys-gateway-sub")), &mut self.anys_gateway_btn, data.anys_gateway.clone()));
        #[cfg(not(target_env = "ohos"))]
        v.push(row_switch(tl!("item-lowq"), Some(tl!("item-lowq-sub")), &mut self.lowq_btn, config.sample_count == 1));
        v.push(row_switch(tl!("item-fxaa"), Some(tl!("item-fxaa-sub")), &mut self.fxaa_btn, config.fxaa));
        let vsync_on = match config.perf_profile() {
            prpr::config::Config::PERF_ULTRA => false,
            prpr::config::Config::PERF_CUSTOM => !config.perf_custom_vsync_off,
            _ => config.vsync,
        };
        v.push(row_switch(tl!("settings-vsync"), Some(tl!("settings-vsync-desc")), &mut self.vsync_btn, vsync_on));
        v.push(row_switch(tl!("settings-console-log"), Some(tl!("settings-console-log-desc")), &mut self.console_btn, config.console_enabled));
        v.push(row_switch(tl!("item-roman-numerals"), Some(tl!("item-roman-numerals-sub")), &mut self.roman_numerals_btn, config.roman_numerals));
        v.push(row_switch(tl!("item-chinese-numerals"), Some(tl!("item-chinese-numerals-sub")), &mut self.chinese_numerals_btn, config.chinese_numerals));
        v.push(row_arrow(tl!("item-reset-settings"), Some(tl!("item-reset-settings-sub")), &mut self.reset_settings_btn));
        v.push(row_value(
            tl!("sx-lan"),
            Some(tl!("sx-lan-sub").into()),
            &mut self.lan_btn,
            tl!("sx-enter").to_string(),
        ));
        v.push(row_value(
            tl!("sx-data-transfer"),
            Some(tl!("sx-data-transfer-sub").into()),
            &mut self.data_btn,
            tl!("sx-enter").to_string(),
        ));
        v
    }
    pub fn render_top(&mut self, ui: &mut Ui, t: f32) {
        self.lang_btn.render_top(ui, t, 1.);
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
        Ok(false)
    }
    pub fn rows(&mut self) -> Vec<Row<'_>> {
        let data = get_data();
        let config = &data.config;
        let mut v = Vec::with_capacity(7);
        v.push(row_switch(tl!("item-adjust"), Some(tl!("item-adjust-sub")), &mut self.adjust_btn, config.adjust_time));
        v.push(row_slider(
            tl!("item-music"),
            None,
            &mut self.music_slider,
            config.volume_music,
            format!("{:.2}", config.volume_music),
        ));
        v.push(row_slider(
            tl!("item-sfx"),
            None,
            &mut self.sfx_slider,
            config.volume_sfx,
            format!("{:.2}", config.volume_sfx),
        ));
        v.push(row_slider(
            tl!("item-bgm"),
            Some(tl!("item-bgm-sub")),
            &mut self.bgm_slider,
            config.volume_bgm,
            format!("{:.2}", config.volume_bgm),
        ));
        v.push(row_value(
            tl!("item-cali"),
            None,
            &mut self.cali_btn,
            format!("{:.0}ms", config.offset * 1000.),
        ));
        #[cfg(not(target_os = "android"))]
        {
            let text = if let Some(rate) = config.preferred_sample_rate {
                format!("{} Hz", rate)
            } else {
                tl!("preferred-sample-rate-default").to_string()
            };
            v.push(row_value(tl!("item-preferred-sample-rate"), None, &mut self.preferred_sample_rate_btn, text));
        }
        #[cfg(target_env = "ohos")]
        {
            let buf_size = config.audio_buffer_size.unwrap_or(256);
            v.push(row_value(tl!("item-audio-buffer-size"), None, &mut self.audio_buffer_size_btn, format!("{buf_size}")));
        }
        v
    }
    pub fn render_top(&mut self, _ui: &mut Ui, _t: f32) {}
    pub fn next_page(&mut self) -> Option<NextPage> {
        self.next_page.take()
    }
}
struct ChartList {
    show_acc_btn: DRectButton,
    ap_fc_indicator_btn: DRectButton,
    indicator_enabled_btn: DRectButton,
    indicator_delay_mode_btn: ChooseButton,
    indicator_color_mode_btn: ChooseButton,
    show_avg_fps_btn: DRectButton,
    show_fps_btn: DRectButton,
    fps_pos_btn: ChooseButton,
    fps_limit_slider: Slider,
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
    arcaea_judgement_btn: DRectButton,
    fnf_judgement_btn: DRectButton,
    noise_enabled_btn: DRectButton,
    judge_perfect_slider: Slider,
    judge_good_slider: Slider,
    judge_bad_slider: Slider,
    judge_late_slider: Slider,
    judge_pp_slider: Slider,
    judge_detailed_btn: DRectButton,
    judge_great_slider: Slider,
    judge_ok_slider: Slider,
    judge_meh_slider: Slider,
    judge_preset_btn: ChooseButton,
    auto_retry_slider: Slider,
    retry_rewind_slider: Slider,
    record_replay_btn: DRectButton,
    hp_custom_slider: Slider,
    hp_custom_bad_btn: DRectButton,
    exercise_auto_btn: DRectButton,
    exercise_step_slider: Slider,
    perf_btn: ChooseButton,
    perf_custom_metrics_btn: DRectButton,
    perf_custom_cull_btn: DRectButton,
    perf_custom_fx_btn: DRectButton,
    perf_custom_vsync_btn: DRectButton,
    perf_lowres_slider: Slider,
    perf_fx_slider: Slider,
}
impl ChartList {
    pub fn new() -> Self {
        Self {
            show_acc_btn: DRectButton::new(),
            ap_fc_indicator_btn: DRectButton::new(),
            indicator_enabled_btn: DRectButton::new(),
            indicator_delay_mode_btn: ChooseButton::new()
                .with_options(vec![
                    tl!("sx-adofai").to_string(),
                    "phigros".to_string(),
                ])
                .with_selected(get_data().config.indicator_delay_phigros as usize),
            indicator_color_mode_btn: ChooseButton::new()
                .with_options(vec![
                    tl!("sx-adofai").to_string(),
                    "phira".to_string(),
                ])
                .with_selected(get_data().config.indicator_color_phira as usize),
            show_avg_fps_btn: DRectButton::new(),
            show_fps_btn: DRectButton::new(),
            fps_pos_btn: ChooseButton::new()
                .with_options(vec![tl!("sx-pos-tl").into(), tl!("sx-pos-tr").into(), tl!("sx-pos-bl").into(), tl!("sx-pos-br").into()])
                .with_selected(get_data().config.fps_position.min(3) as usize),
            fps_limit_slider: Slider::new(0.0..480.0, 10.0),
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
            arcaea_judgement_btn: DRectButton::new(),
            fnf_judgement_btn: DRectButton::new(),
            noise_enabled_btn: DRectButton::new(),
            judge_perfect_slider: Slider::new(20.0..120.0, 1.0),
            judge_good_slider: Slider::new(20.0..220.0, 1.0),
            judge_bad_slider: Slider::new(20.0..300.0, 1.0),
            judge_late_slider: Slider::new(0.0..100.0, 1.0),
            judge_pp_slider: Slider::new(0.0..80.0, 1.0),
            judge_detailed_btn: DRectButton::new(),
            judge_great_slider: Slider::new(20.0..120.0, 1.0),
            judge_ok_slider: Slider::new(60.0..180.0, 1.0),
            judge_meh_slider: Slider::new(80.0..220.0, 1.0),
            judge_preset_btn: ChooseButton::new()
                .with_options(vec![
                    tl!("sx-builtin").to_string(),
                    "Phigros".to_string(),
                    "Phira".to_string(),
                    tl!("sx-custom").to_string(),
                ])
                .with_selected(match get_data().config.judge_preset {
                    prpr::config::JudgePreset::Default => 0,
                    prpr::config::JudgePreset::Phigros => 1,
                    prpr::config::JudgePreset::Phira => 2,
                    prpr::config::JudgePreset::Custom => 3,
                }),
            auto_retry_slider: Slider::new(0.0..10.0, 1.0),
            retry_rewind_slider: Slider::new(0.0..10.0, 0.5),
            record_replay_btn: DRectButton::new(),
            hp_custom_slider: Slider::new(1.0..500.0, 1.0),
            hp_custom_bad_btn: DRectButton::new(),
            exercise_auto_btn: DRectButton::new(),
            exercise_step_slider: Slider::new(0.01..0.20, 0.01),
            perf_btn: ChooseButton::new()
                .with_options(vec![
                    tl!("item-perf-off").into_owned(),
                    tl!("item-perf-light").into_owned(),
                    tl!("item-perf-medium").into_owned(),
                    tl!("item-perf-full").into_owned(),
                    tl!("item-perf-ultra").into_owned(),
                    tl!("item-perf-custom").into_owned(),
                ])
                .with_selected((get_data().config.performance.min(5)) as usize),
            perf_custom_metrics_btn: DRectButton::new(),
            perf_custom_cull_btn: DRectButton::new(),
            perf_custom_fx_btn: DRectButton::new(),
            perf_custom_vsync_btn: DRectButton::new(),
            perf_lowres_slider: Slider::new(20.0..500.0, 5.0),
            perf_fx_slider: Slider::new(5.0..100.0, 5.0),
        }
    }
    pub fn top_touch(&mut self, touch: &Touch, t: f32) -> bool {
        self.perf_btn.top_touch(touch, t)
            || self.indicator_delay_mode_btn.top_touch(touch, t)
            || self.indicator_color_mode_btn.top_touch(touch, t)
            || self.judge_preset_btn.top_touch(touch, t)
            || self.fps_pos_btn.top_touch(touch, t)
    }
    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.perf_btn.touch(touch, t) {
            return Ok(Some(false));
        }
        if self.indicator_delay_mode_btn.touch(touch, t) {
            return Ok(Some(false));
        }
        if self.indicator_color_mode_btn.touch(touch, t) {
            return Ok(Some(false));
        }
        if self.judge_preset_btn.touch(touch, t) {
            return Ok(Some(false));
        }
        if self.fps_pos_btn.touch(touch, t) {
            return Ok(Some(false));
        }
        {
            let mut v = config.fps_limit as f32;
            if let wt @ Some(_) = self.fps_limit_slider.touch(touch, t, &mut v) {
                config.fps_limit = v.round().clamp(0., 480.) as u32;
                return Ok(wt);
            }
        }
        if config.perf_is_custom() {
            if self.perf_custom_metrics_btn.touch(touch, t) {
                config.perf_custom_metrics ^= true;
                return Ok(Some(true));
            }
            if self.perf_custom_cull_btn.touch(touch, t) {
                config.perf_custom_cull ^= true;
                return Ok(Some(true));
            }
            if self.perf_custom_fx_btn.touch(touch, t) {
                config.perf_custom_particles ^= true;
                return Ok(Some(true));
            }
            if self.perf_custom_vsync_btn.touch(touch, t) {
                config.perf_custom_vsync_off ^= true;
                return Ok(Some(true));
            }
            {
                let mut v = config.perf_custom_lowres as f32;
                if self.perf_lowres_slider.touch(touch, t, &mut v).is_some() {
                    config.perf_custom_lowres = v.round() as u32;
                    return Ok(Some(true));
                }
            }
            {
                let mut v = config.perf_custom_fx_density as f32;
                if self.perf_fx_slider.touch(touch, t, &mut v).is_some() {
                    config.perf_custom_fx_density = v.round() as u32;
                    return Ok(Some(true));
                }
            }
        }
        if self.show_acc_btn.touch(touch, t) {
            config.show_acc ^= true;
            return Ok(Some(true));
        }
        if self.ap_fc_indicator_btn.touch(touch, t) {
            config.ap_fc_indicator ^= true;
            return Ok(Some(true));
        }
        if self.indicator_enabled_btn.touch(touch, t) {
            config.indicator_enabled ^= true;
            return Ok(Some(true));
        }
        if self.show_avg_fps_btn.touch(touch, t) {
            config.show_avg_fps ^= true;
            return Ok(Some(true));
        }
        if self.show_fps_btn.touch(touch, t) {
            config.show_fps ^= true;
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
        if config.perf_is_custom() && self.opt_btn.touch(touch, t) {
            config.aggressive ^= true;
            return Ok(Some(true));
        }
        if self.use_keyboard_btn.touch(touch, t) {
            config.use_keyboard ^= true;
            return Ok(Some(true));
        }
        if config.perf_is_custom() && self.particle_btn.touch(touch, t) {
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
        if self.arcaea_judgement_btn.touch(touch, t) {
            data.config.arcaea_judgement ^= true;
            if data.config.arcaea_judgement {
                data.config.fnf_judgement = false;
            }
            return Ok(Some(true));
        }
        if self.fnf_judgement_btn.touch(touch, t) {
            data.config.fnf_judgement ^= true;
            if data.config.fnf_judgement {
                data.config.arcaea_judgement = false;
            }
            return Ok(Some(true));
        }
        if self.noise_enabled_btn.touch(touch, t) {
            data.config.noise_enabled ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.judge_perfect_slider.touch(touch, t, &mut config.judge_perfect_ms) {
            config.judge_preset = prpr::config::JudgePreset::Custom;
            self.judge_preset_btn.set_selected(3);
            config.judge_perfect_ms = config.judge_perfect_ms.round().clamp(20., 120.);
            config.judge_good_ms = config.judge_good_ms.max(config.judge_perfect_ms);
            config.judge_bad_ms = config.judge_bad_ms.max(config.judge_good_ms);
            return Ok(wt);
        }
        if let wt @ Some(_) = self.judge_good_slider.touch(touch, t, &mut config.judge_good_ms) {
            config.judge_preset = prpr::config::JudgePreset::Custom;
            self.judge_preset_btn.set_selected(3);
            config.judge_good_ms = config.judge_good_ms.round().clamp(config.judge_perfect_ms, 220.);
            config.judge_bad_ms = config.judge_bad_ms.max(config.judge_good_ms);
            return Ok(wt);
        }
        if let wt @ Some(_) = self.judge_bad_slider.touch(touch, t, &mut config.judge_bad_ms) {
            config.judge_preset = prpr::config::JudgePreset::Custom;
            self.judge_preset_btn.set_selected(3);
            config.judge_bad_ms = config.judge_bad_ms.round().clamp(config.judge_good_ms.min(220.), 300.);
            return Ok(wt);
        }
        if let wt @ Some(_) = self.judge_late_slider.touch(touch, t, &mut config.judge_late_leniency_ms) {
            config.judge_late_leniency_ms = config.judge_late_leniency_ms.round().clamp(0., 100.);
            return Ok(wt);
        }
        if let wt @ Some(_) = self.judge_pp_slider.touch(touch, t, &mut config.judge_perfect_plus_ms) {
            config.judge_perfect_plus_ms = config.judge_perfect_plus_ms.round().clamp(0., 80.);
            return Ok(wt);
        }
        if self.judge_detailed_btn.touch(touch, t) {
            config.judge_detailed ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.judge_great_slider.touch(touch, t, &mut config.judge_great_ms) {
            config.judge_great_ms = config.judge_great_ms.round().clamp(20., 120.);
            config.judge_ok_ms = config.judge_ok_ms.max(config.judge_great_ms);
            config.judge_meh_ms = config.judge_meh_ms.max(config.judge_ok_ms);
            return Ok(wt);
        }
        if let wt @ Some(_) = self.judge_ok_slider.touch(touch, t, &mut config.judge_ok_ms) {
            config.judge_ok_ms = config.judge_ok_ms.round().clamp(config.judge_great_ms, 180.);
            config.judge_meh_ms = config.judge_meh_ms.max(config.judge_ok_ms);
            return Ok(wt);
        }
        if let wt @ Some(_) = self.judge_meh_slider.touch(touch, t, &mut config.judge_meh_ms) {
            config.judge_meh_ms = config.judge_meh_ms.round().clamp(config.judge_ok_ms, 220.);
            return Ok(wt);
        }
        {
            let mut v = config.auto_retry as f32;
            if let wt @ Some(_) = self.auto_retry_slider.touch(touch, t, &mut v) {
                config.auto_retry = v.round().clamp(0., 10.) as u8;
                return Ok(wt);
            }
        }
        if self.record_replay_btn.touch(touch, t) {
            config.record_replay ^= true;
            return Ok(Some(true));
        }
        if self.hp_custom_bad_btn.touch(touch, t) {
            config.hp_custom_bad_only ^= true;
            return Ok(Some(true));
        }
        if self.exercise_auto_btn.touch(touch, t) {
            config.exercise_auto_speed ^= true;
            return Ok(Some(true));
        }
        if let wt @ Some(_) = self.exercise_step_slider.touch(touch, t, &mut config.exercise_speed_step) {
            config.exercise_speed_step = (config.exercise_speed_step * 100.).round() / 100.;
            return Ok(wt);
        }
        if let wt @ Some(_) = self.hp_custom_slider.touch(touch, t, &mut config.hp_custom_max) {
            config.hp_custom_max = config.hp_custom_max.round().clamp(1., 500.);
            return Ok(wt);
        }
        if let wt @ Some(_) = self.retry_rewind_slider.touch(touch, t, &mut config.retry_rewind_sec) {
            config.retry_rewind_sec = (config.retry_rewind_sec * 2.).round() / 2.;
            return Ok(wt);
        }
        if let wt @ Some(_) = self.speed_slider.touch(touch, t, &mut config.speed) {
            return Ok(wt);
        }
        if let wt @ Some(_) = self.size_slider.touch(touch, t, &mut config.note_scale) {
            return Ok(wt);
        }
        Ok(None)
    }
    pub fn update(&mut self, t: f32) -> Result<bool> {
        self.perf_btn.update(t);
        if self.perf_btn.changed() {
            get_data_mut().config.performance = self.perf_btn.selected().min(5) as u8;
            return Ok(true);
        }
        self.indicator_delay_mode_btn.update(t);
        if self.indicator_delay_mode_btn.changed() {
            get_data_mut().config.indicator_delay_phigros = self.indicator_delay_mode_btn.selected() == 1;
            return Ok(true);
        }
        self.indicator_color_mode_btn.update(t);
        if self.indicator_color_mode_btn.changed() {
            get_data_mut().config.indicator_color_phira = self.indicator_color_mode_btn.selected() == 1;
            return Ok(true);
        }
        self.judge_preset_btn.update(t);
        if self.judge_preset_btn.changed() {            let data = get_data_mut();
            data.config.judge_preset = match self.judge_preset_btn.selected() {
                0 => prpr::config::JudgePreset::Default,
                1 => prpr::config::JudgePreset::Phigros,
                2 => prpr::config::JudgePreset::Phira,
                _ => prpr::config::JudgePreset::Custom,
            };
            let [p, g, b] = data.config.judge_preset.windows_ms();
            if data.config.judge_preset != prpr::config::JudgePreset::Custom {
                data.config.judge_perfect_ms = p;
                data.config.judge_good_ms = g;
                data.config.judge_bad_ms = b;
            }
            return Ok(true);
        }
        self.fps_pos_btn.update(t);
        if self.fps_pos_btn.changed() {
            get_data_mut().config.fps_position = self.fps_pos_btn.selected().min(3) as u8;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn all_rows(&mut self) -> Vec<Row<'_>> {
        let data = get_data();
        let config = &data.config;
        let mut v = Vec::with_capacity(32);
        v.push(row_choose(tl!("item-perf"), Some(tl!("item-perf-sub")), &mut self.perf_btn, 0.20));
        if config.perf_profile() == prpr::config::Config::PERF_CUSTOM {
            v.push(row_switch(
                tl!("item-perf-custom-metrics"),
                Some(tl!("item-perf-custom-metrics-sub")),
                &mut self.perf_custom_metrics_btn,
                config.perf_custom_metrics,
            ));
            v.push(row_switch(
                tl!("item-perf-custom-cull"),
                Some(tl!("item-perf-custom-cull-sub")),
                &mut self.perf_custom_cull_btn,
                config.perf_custom_cull,
            ));
            v.push(row_switch(
                tl!("item-perf-custom-fx"),
                Some(tl!("item-perf-custom-fx-sub")),
                &mut self.perf_custom_fx_btn,
                config.perf_custom_particles,
            ));
            v.push(row_switch(
                tl!("item-perf-custom-vsync"),
                Some(tl!("item-perf-custom-vsync-sub")),
                &mut self.perf_custom_vsync_btn,
                config.perf_custom_vsync_off,
            ));
            v.push(row_slider(
                tl!("item-perf-custom-lowres"),
                Some(tl!("item-perf-custom-lowres-sub")),
                &mut self.perf_lowres_slider,
                config.perf_custom_lowres as f32,
                format!("{}", config.perf_custom_lowres),
            ));
            v.push(row_slider(
                tl!("item-perf-custom-fxdensity"),
                Some(tl!("item-perf-custom-fxdensity-sub")),
                &mut self.perf_fx_slider,
                config.perf_custom_fx_density as f32,
                format!("{}", config.perf_custom_fx_density),
            ));
        }
        v.push(row_switch(tl!("item-show-acc"), None, &mut self.show_acc_btn, config.show_acc));
        v.push(row_switch(
            tl!("item-ap-fc-indicator"),
            Some(tl!("item-ap-fc-indicator-sub")),
            &mut self.ap_fc_indicator_btn,
            config.ap_fc_indicator,
        ).with_group(2));
        v.push(row_switch(
            tl!("item-indicator-enabled"),
            Some(tl!("item-indicator-enabled-sub")),
            &mut self.indicator_enabled_btn,
            config.indicator_enabled,
        ).with_group(2));
        v.push(row_choose(
            tl!("item-indicator-delay"),
            Some(tl!("item-indicator-delay-sub")),
            &mut self.indicator_delay_mode_btn,
            0.20,
        ).with_group(2));
        v.push(row_choose(
            tl!("item-indicator-color"),
            Some(tl!("item-indicator-color-sub")),
            &mut self.indicator_color_mode_btn,
            0.20,
        ).with_group(2));
        v.push(row_switch(tl!("item-show-fps"), Some(tl!("item-show-fps-sub")), &mut self.show_fps_btn, config.show_fps).with_group(1));
        v.push(row_choose(
            tl!("item-fps-pos"),
            Some(tl!("item-fps-pos-sub")),
            &mut self.fps_pos_btn,
            0.20,
        ).with_group(1));
        v.push(row_slider(
            tl!("item-fps-limit"),
            Some(tl!("item-fps-limit-sub")),
            &mut self.fps_limit_slider,
            config.fps_limit as f32,
            format!("{}", config.fps_limit),
        ).with_group(1));
        v.push(row_switch(
            tl!("item-show-avg-fps"),
            Some(tl!("item-show-avg-fps-sub")),
            &mut self.show_avg_fps_btn,
            config.show_avg_fps,
        ).with_group(1));
        v.push(row_switch(tl!("item-dc-pause"), None, &mut self.dc_pause_btn, config.double_click_to_pause).with_group(1));
        v.push(row_switch(tl!("item-dhint"), Some(tl!("item-dhint-sub")), &mut self.dhint_btn, config.double_hint));
        if config.perf_is_custom() {
            v.push(row_switch(tl!("item-opt"), Some(tl!("item-opt-sub")), &mut self.opt_btn, config.aggressive));
        }
        v.push(row_switch(
            tl!("item-use-keyboard"),
            Some(tl!("item-use-keyboard-sub")),
            &mut self.use_keyboard_btn,
            config.use_keyboard,
        ).with_group(1));
        if config.perf_is_custom() {
            v.push(row_switch(tl!("item-particle"), Some(tl!("item-particle-sub")), &mut self.particle_btn, config.particle));
        }
        v.push(row_switch(
            tl!("item-disable-effect"),
            Some(tl!("item-disable-effect-sub")),
            &mut self.disable_effect_btn,
            config.disable_effect,
        ));
        v.push(row_switch(
            tl!("item-interactive"),
            Some(tl!("item-interactive-sub")),
            &mut self.interactive_btn,
            config.interactive,
        ));
        v.push(row_slider(
            tl!("sx-hp-max"),
            Some(tl!("sx-hp-max-sub").into()),
            &mut self.hp_custom_slider,
            config.hp_custom_max,
            format!("{:.0}", config.hp_custom_max),
        ).with_group(3));
        v.push(row_switch(
            tl!("sx-hp-bad-only"),
            Some(tl!("sx-hp-bad-only-sub").into()),
            &mut self.hp_custom_bad_btn,
            config.hp_custom_bad_only,
        ).with_group(3));
        v.push(row_switch(
            tl!("sx-record-replay"),
            Some(tl!("sx-record-replay-sub").into()),
            &mut self.record_replay_btn,
            config.record_replay,
        ).with_group(4));
        v.push(row_switch(
            tl!("sx-exercise-auto"),
            Some(tl!("sx-exercise-auto-sub").into()),
            &mut self.exercise_auto_btn,
            config.exercise_auto_speed,
        ));
        v.push(row_slider(
            tl!("sx-exercise-step"),
            Some(tl!("sx-exercise-step-sub").into()),
            &mut self.exercise_step_slider,
            config.exercise_speed_step,
            format!("+{:.2}x", config.exercise_speed_step),
        ));
        v.push(row_slider(
            tl!("sx-auto-retry"),
            Some(tl!("sx-auto-retry-sub").into()),
            &mut self.auto_retry_slider,
            config.auto_retry as f32,
            tl!("sx-retry-count", "n" => config.auto_retry).to_string(),
        ));
        v.push(row_slider(
            tl!("sx-retry-rewind"),
            Some(tl!("sx-retry-rewind-sub").into()),
            &mut self.retry_rewind_slider,
            config.retry_rewind_sec,
            format!("{:.1} s", config.retry_rewind_sec),
        ));
        v.push(row_slider(tl!("item-speed"), None, &mut self.speed_slider, config.speed, format!("{:.2}", config.speed)));
        v.push(row_slider(
            tl!("item-note-size"),
            None,
            &mut self.size_slider,
            config.note_scale,
            format!("{:.3}", config.note_scale),
        ));
        v.push(row_switch(
            tl!("item-full-screen-judge"),
            Some(tl!("item-full-screen-judge-sub")),
            &mut self.full_screen_judge_btn,
            config.full_screen_judge,
        ));
        v.push(row_choose(
            tl!("sx-judge-preset"),
            Some(tl!("sx-judge-preset-sub").into()),
            &mut self.judge_preset_btn,
            0.20,
        ));
        v.push(row_slider(
            tl!("sx-judge-perfect"),
            Some(tl!("sx-judge-perfect-sub").into()),
            &mut self.judge_perfect_slider,
            config.judge_perfect_ms,
            format!("{:.0} ms", config.judge_perfect_ms),
        ));
        v.push(row_slider(
            tl!("sx-judge-good"),
            Some(tl!("sx-judge-good-sub").into()),
            &mut self.judge_good_slider,
            config.judge_good_ms,
            format!("{:.0} ms", config.judge_good_ms),
        ));
        v.push(row_slider(
            tl!("sx-judge-bad"),
            Some(tl!("sx-judge-bad-sub").into()),
            &mut self.judge_bad_slider,
            config.judge_bad_ms,
            format!("{:.0} ms", config.judge_bad_ms),
        ));
        v.push(row_slider(
            tl!("sx-judge-late"),
            Some(tl!("sx-judge-late-sub").into()),
            &mut self.judge_late_slider,
            config.judge_late_leniency_ms,
            format!("{:.0} ms", config.judge_late_leniency_ms),
        ));
        v.push(row_slider(
            tl!("sx-judge-pp"),
            Some(tl!("sx-judge-pp-sub").into()),
            &mut self.judge_pp_slider,
            config.judge_perfect_plus_ms,
            format!("{:.0} ms", config.judge_perfect_plus_ms),
        ));
        v.push(row_switch(
            tl!("sx-judge-detailed"),
            Some(tl!("sx-judge-detailed-sub").into()),
            &mut self.judge_detailed_btn,
            config.judge_detailed,
        ));
        v.push(row_slider(
            tl!("sx-judge-great"),
            None,
            &mut self.judge_great_slider,
            config.judge_great_ms,
            format!("{:.0} ms", config.judge_great_ms),
        ));
        v.push(row_slider(
            tl!("sx-judge-ok"),
            None,
            &mut self.judge_ok_slider,
            config.judge_ok_ms,
            format!("{:.0} ms", config.judge_ok_ms),
        ));
        v.push(row_slider(
            tl!("sx-judge-meh"),
            None,
            &mut self.judge_meh_slider,
            config.judge_meh_ms,
            format!("{:.0} ms", config.judge_meh_ms),
        ));
        v.push(row_switch(
            tl!("item-arcaca-judgement"),
            Some(tl!("item-arcaca-judgement-sub")),
            &mut self.arcaea_judgement_btn,
            config.arcaea_judgement,
        ));
        v.push(row_switch(
            tl!("item-fnf-judgement"),
            Some(tl!("item-fnf-judgement-sub")),
            &mut self.fnf_judgement_btn,
            config.fnf_judgement,
        ));
        v.push(row_switch(
            tl!("sx-noise-enabled"),
            Some(tl!("sx-noise-enabled-sub").into()),
            &mut self.noise_enabled_btn,
            config.noise_enabled,
        ).with_group(3));
        v
    }
    pub fn rows(&mut self) -> Vec<Row<'_>> {
        let mut v = self.all_rows();
        v.retain(|r| r.group == 0);
        v
    }

    pub fn general_rows(&mut self) -> Vec<Row<'_>> {
        let mut v = self.all_rows();
        v.retain(|r| r.group == 1);
        v
    }

    pub fn visual_rows(&mut self) -> Vec<Row<'_>> {
        let mut v = self.all_rows();
        v.retain(|r| r.group == 2);
        v
    }

    pub fn custom_rows(&mut self) -> Vec<Row<'_>> {
        let mut v = self.all_rows();
        v.retain(|r| r.group == 3);
        v
    }

    pub fn debug_rows(&mut self) -> Vec<Row<'_>> {
        let mut v = self.all_rows();
        v.retain(|r| r.group == 4);
        v
    }

    pub fn render_top(&mut self, ui: &mut Ui, t: f32) {
        self.perf_btn.render_top(ui, t, 1.);
        self.indicator_delay_mode_btn.render_top(ui, t, 1.);
        self.indicator_color_mode_btn.render_top(ui, t, 1.);
        self.judge_preset_btn.render_top(ui, t, 1.);
        self.fps_pos_btn.render_top(ui, t, 1.);
    }
}
struct VisualList {
    noise_color_btn: DRectButton,
    noise_demo_btn: ChooseButton,
    noise_glitch_btn: DRectButton,
    noise_opacity_slider: Slider,
    dynamic_bg_btn: ChooseButton,
    show_watermark_btn: DRectButton,
    show_score_btn: DRectButton,
    show_chart_info_btn: DRectButton,
    /// 噪域颜色调色板
    palette: ColorPalette,
}
impl VisualList {
    pub fn new() -> Self {
        Self {
            noise_color_btn: DRectButton::new(),
            noise_demo_btn: ChooseButton::new()
                .with_options(vec![
                    tl!("sx-noise-demo-phigros").to_string(),
                    tl!("sx-noise-demo-solid").to_string(),
                ])
                .with_selected((get_data().config.noise_demo.saturating_sub(1)) as usize),
            noise_glitch_btn: DRectButton::new(),
            noise_opacity_slider: Slider::new(0.0..3.0, 0.01),
            dynamic_bg_btn: ChooseButton::new()
                .with_options(vec![
                    tl!("sx-off").to_string(),
                    tl!("sx-static-brightness").to_string(),
                    tl!("sx-dynamic-brightness").to_string(),
                ])
                .with_selected(get_data().config.dynamic_background.as_u8() as usize),
            show_watermark_btn: DRectButton::new(),
            show_score_btn: DRectButton::new(),
            show_chart_info_btn: DRectButton::new(),
            palette: ColorPalette::new(),
        }
    }
    pub fn top_touch(&mut self, touch: &Touch, t: f32) -> bool {
        if self.palette.is_open() {
            return self.palette.touch(touch);
        }
        self.noise_demo_btn.top_touch(touch, t)
            || self.dynamic_bg_btn.top_touch(touch, t)
    }
    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        if self.noise_demo_btn.touch(touch, t) {
            return Ok(Some(false));
        }
        if self.dynamic_bg_btn.touch(touch, t) {
            return Ok(Some(false));
        }
        let data = get_data_mut();
        let config = &mut data.config;
        if self.noise_color_btn.touch(touch, t) {
            self.palette.open();
            return Ok(Some(true));
        }
        if self.noise_glitch_btn.touch(touch, t) {
            config.noise_glitch ^= true;
            return Ok(Some(true));
        }
        if self.show_watermark_btn.touch(touch, t) {
            config.show_watermark ^= true;
            return Ok(Some(true));
        }
        if self.show_score_btn.touch(touch, t) {
            config.show_score ^= true;
            return Ok(Some(true));
        }
        if self.show_chart_info_btn.touch(touch, t) {
            config.show_chart_info ^= true;
            return Ok(Some(true));
        }
        {
            let mut v = config.noise_opacity;
            if self.noise_opacity_slider.touch(touch, t, &mut v).is_some() {
                config.noise_opacity = v;
                return Ok(Some(true));
            }
        }
        Ok(None)
    }
    pub fn update(&mut self, t: f32) -> Result<bool> {
        self.noise_demo_btn.update(t);
        self.dynamic_bg_btn.update(t);
        if self.dynamic_bg_btn.changed() {
            get_data_mut().config.dynamic_background = match self.dynamic_bg_btn.selected() {
                0 => prpr::config::DynamicBackgroundMode::Off,
                1 => prpr::config::DynamicBackgroundMode::StaticBrightness,
                _ => prpr::config::DynamicBackgroundMode::DynamicBrightness,
            };
            return Ok(true);
        }
        if self.noise_demo_btn.changed() {
            get_data_mut().config.noise_demo = (self.noise_demo_btn.selected() + 1) as u8;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn rows(&mut self) -> Vec<Row<'_>> {
        let data = get_data();
        let config = &data.config;
        vec![
            row_value(
                tl!("sx-noise-color"),
                Some(tl!("sx-noise-color-sub").into()),
                &mut self.noise_color_btn,
                config.noise_color.trim().trim_start_matches('#').to_string(),
            ),
            row_choose(
                tl!("sx-noise-demo"),
                Some(tl!("sx-noise-demo-sub").into()),
                &mut self.noise_demo_btn,
                0.22,
            ),
            row_switch(
                tl!("sx-noise-glitch"),
                Some(tl!("sx-noise-glitch-sub").into()),
                &mut self.noise_glitch_btn,
                config.noise_glitch,
            ),
            row_slider(
                tl!("sx-noise-opacity"),
                None,
                &mut self.noise_opacity_slider,
                config.noise_opacity,
                format!("{:.0}%", config.noise_opacity * 100.0),
            ),
            row_choose(
                tl!("sx-dynamic-bg"),
                Some(tl!("sx-dynamic-bg-sub").into()),
                &mut self.dynamic_bg_btn,
                0.22,
            ),
            row_switch(
                tl!("sx-watermark"),
                Some(tl!("sx-watermark-sub").into()),
                &mut self.show_watermark_btn,
                config.show_watermark,
            ),
            row_switch(
                tl!("sx-chart-info"),
                Some(tl!("sx-chart-info-sub").into()),
                &mut self.show_chart_info_btn,
                config.show_chart_info,
            ),
        ]
    }
    pub fn render_top(&mut self, ui: &mut Ui, t: f32) {
        self.noise_demo_btn.render_top(ui, t, 1.);
        self.dynamic_bg_btn.render_top(ui, t, 1.);
        self.palette.render(ui, t);
    }
}
struct ReplayItem {
    path: PathBuf,
    replay: prpr::replay::Replay,
}

struct ReplayList {
    items: Vec<ReplayItem>,
    play_btns: Vec<DRectButton>,
    edit_btns: Vec<DRectButton>,
    del_btns: Vec<DRectButton>,
    play_task: LocalTask<Result<NextScene>>,
    ready: Option<NextScene>,
    next_page: Option<NextPage>,
}

impl ReplayList {
    fn new() -> Self {
        let mut this = Self {
            items: Vec::new(),
            play_btns: Vec::new(),
            edit_btns: Vec::new(),
            del_btns: Vec::new(),
            play_task: None,
            ready: None,
            next_page: None,
        };
        this.reload();
        this
    }

    fn reload(&mut self) {
        self.items.clear();
        if let Ok(files) = fs::read_dir("replays") {
            for file in files.flatten() {
                let path = file.path();
                if path.extension().is_some_and(|e| e == "json") {
                    if let Some(replay) = prpr::replay::load(&path) {
                        self.items.push(ReplayItem { path, replay });
                    }
                }
            }
        }
        self.items.sort_by_key(|i| std::cmp::Reverse(i.replay.recorded_at));
        self.play_btns = (0..self.items.len()).map(|_| DRectButton::new()).collect();
        self.edit_btns = (0..self.items.len()).map(|_| DRectButton::new()).collect();
        self.del_btns = (0..self.items.len()).map(|_| DRectButton::new()).collect();
    }

    fn play(&mut self, index: usize) {
        let Some(item) = self.items.get(index) else { return };
        let local_path = item.replay.local_path.clone().or_else(|| {
            get_data()
                .charts
                .iter()
                .find(|c| c.info.name == item.replay.name)
                .map(|c| c.local_path.clone())
        });
        let Some(local_path) = local_path else {
            show_error(anyhow::anyhow!(tl!("sx-replay-chart-missing")));
            return;
        };
        prpr::replay::set_pending(item.replay.clone());
        let root = match dir::charts() {
            Ok(v) => v,
            Err(e) => {
                show_error(e);
                return;
            }
        };
        let full = local_path.replace('\\', "/");
        self.play_task = Some(Box::pin(async move {
            let mut fs = crate::scene::fs_from_path(&full)?;
            let info = prpr::fs::load_info(fs.as_mut()).await?;
            let config = get_data().config.clone();
            let scene = prpr::scene::LoadingScene::new(
                prpr::scene::GameMode::Replay,
                info,
                config,
                fs,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
            Ok(NextScene::Overlay(Box::new(scene)))
        }));
    }

    fn top_touch(&mut self, _touch: &Touch, _t: f32) -> bool { false }

    fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        for i in 0..self.items.len() {
            if self.play_btns[i].touch(touch, t) {
                self.play(i);
                return Ok(Some(false));
            }
            if self.edit_btns[i].touch(touch, t) {
                self.next_page = Some(NextPage::Overlay(Box::new(
                    super::replay_manager::ReplayEditPage::new(
                        self.items[i].path.clone(),
                        self.items[i].replay.clone(),
                    ),
                )));
                return Ok(Some(false));
            }
            if self.del_btns[i].touch(touch, t) {
                let _ = fs::remove_file(&self.items[i].path);
                self.reload();
                show_message(tl!("sx-replay-deleted")).ok();
                return Ok(Some(true));
            }
        }
        Ok(None)
    }

    fn update(&mut self, _t: f32) -> Result<bool> {
        if let Some(task) = &mut self.play_task {
            if let Some(result) = poll_future(task.as_mut()) {
                self.play_task = None;
                match result {
                    Ok(scene) => self.ready = Some(scene),
                    Err(err) => show_error(err),
                }
            }
        }
        Ok(false)
    }

    fn take_scene(&mut self) -> Option<NextScene> {
        self.ready.take()
    }

    fn next_page(&mut self) -> Option<NextPage> {
        self.next_page.take()
    }

    fn rows(&mut self) -> Vec<Row<'_>> {
        let mut v = Vec::new();
        let items = &self.items;
        let plays = &mut self.play_btns;
        let edits = &mut self.edit_btns;
        let dels = &mut self.del_btns;
        for (((item, play), edit), del) in items
            .iter()
            .zip(plays.iter_mut())
            .zip(edits.iter_mut())
            .zip(dels.iter_mut())
        {
            let sub = format!(
                "{:.2}x  P/G/B/M {}/{}/{}/{}",
                item.replay.speed,
                item.replay.stats().0,
                item.replay.stats().1,
                item.replay.stats().2,
                item.replay.stats().3
            );
            v.push(row_value(item.replay.name.clone(), Some(sub.into()), play, tl!("sx-play")));
            v.push(row_value(tl!("sx-edit-name", "name" => item.replay.name.clone()).to_string(), None, edit, tl!("sx-edit")));
            v.push(row_value(tl!("sx-delete-name", "name" => item.replay.name.clone()).to_string(), None, del, tl!("row-delete")));
        }
        v
    }
}

struct DebugList {
    chart_debug_btn: DRectButton,
    touch_debug_btn: DRectButton,
    note_debug_btn: DRectButton,
    combo_text_debug_btn: DRectButton,
    crash_btn: DRectButton,
    replay_folder_btn: DRectButton,
    replay_clear_btn: DRectButton,
    replay_export_btn: DRectButton,
    replay_manager_btn: DRectButton,
    replay_next_page: Option<NextPage>,
    crash_requested: bool,
}
impl DebugList {
    pub fn new() -> Self {
        Self {
            chart_debug_btn: DRectButton::new(),
            touch_debug_btn: DRectButton::new(),
            note_debug_btn: DRectButton::new(),
            combo_text_debug_btn: DRectButton::new(),
            crash_btn: DRectButton::new(),
            replay_folder_btn: DRectButton::new(),
            replay_clear_btn: DRectButton::new(),
            replay_export_btn: DRectButton::new(),
            replay_manager_btn: DRectButton::new(),
            replay_next_page: None,
            crash_requested: false,
        }
    }
    pub fn top_touch(&mut self, _touch: &Touch, _t: f32) -> bool {
        false
    }
    pub fn take_crash_request(&mut self) -> bool {
        std::mem::take(&mut self.crash_requested)
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
        if self.note_debug_btn.touch(touch, t) {
            config.note_debug ^= true;
            return Ok(Some(true));
        }
        if self.combo_text_debug_btn.touch(touch, t) {
            config.combo_text_debug ^= true;
            return Ok(Some(true));
        }
        if self.crash_btn.touch(touch, t) {
            tracing::info!("CRASH BTN CLICKED: setting crash_requested = true");
            self.crash_requested = true;
            return Ok(Some(false));
        }
        if self.replay_folder_btn.touch(touch, t) {
            let _ = std::fs::create_dir_all("replays");
            let _ = std::process::Command::new("explorer").arg("replays").spawn();
            return Ok(Some(false));
        }
        if self.replay_clear_btn.touch(touch, t) {
            if let Ok(files) = std::fs::read_dir("replays") {
                for file in files.flatten() {
                    if file.path().extension().is_some_and(|e| e == "json") {
                        let _ = std::fs::remove_file(file.path());
                    }
                }
            }
            show_message(tl!("sx-replay-cleared")).ok();
            return Ok(Some(false));
        }
        if self.replay_export_btn.touch(touch, t) {
            let out = std::path::Path::new("export/replays");
            let _ = std::fs::create_dir_all(out);
            if let Ok(files) = std::fs::read_dir("replays") {
                for file in files.flatten() {
                    if file.path().extension().is_some_and(|e| e == "json") {
                        if let Some(name) = file.path().file_name() {
                            let _ = std::fs::copy(file.path(), out.join(name));
                        }
                    }
                }
            }
            show_message(tl!("sx-replay-exported")).ok();
            return Ok(Some(false));
        }
        if self.replay_manager_btn.touch(touch, t) {
            self.replay_next_page = Some(NextPage::Overlay(Box::new(
                super::replay_manager::ReplayManager::new(),
            )));
            return Ok(Some(false));
        }
        Ok(None)
    }
    pub fn update(&mut self, _t: f32) -> Result<bool> {
        Ok(false)
    }
    pub fn next_page(&mut self) -> Option<NextPage> {
        self.replay_next_page.take()
    }
    pub fn rows(&mut self) -> Vec<Row<'_>> {
        let data = get_data();
        let config = &data.config;
        vec![
            row_switch(tl!("item-chart-debug"), Some(tl!("item-chart-debug-sub")), &mut self.chart_debug_btn, config.chart_debug),
            row_switch(tl!("item-touch-debug"), Some(tl!("item-touch-debug-sub")), &mut self.touch_debug_btn, config.touch_debug),
            row_switch(tl!("sx-note-debug"), Some(tl!("sx-note-debug-sub").into()), &mut self.note_debug_btn, config.note_debug),
            row_switch(
                tl!("sx-combo-debug"),
                Some(tl!("sx-combo-debug-sub").into()),
                &mut self.combo_text_debug_btn,
                config.combo_text_debug,
            ),
            row_value(
                tl!("item-crash-btn"),
                Some(tl!("item-crash-btn-sub")),
                &mut self.crash_btn,
                tl!("item-crash"),
            ),
            row_value(tl!("sx-replay-folder"), Some(tl!("sx-replay-folder-sub").into()), &mut self.replay_folder_btn, tl!("settings-open")),
            row_value(tl!("sx-replay-clear"), Some(tl!("sx-replay-clear-sub").into()), &mut self.replay_clear_btn, tl!("sx-clear")),
            row_value(tl!("sx-replay-export"), Some(tl!("sx-replay-export-sub").into()), &mut self.replay_export_btn, tl!("sx-export")),
        ]
    }
}
/// 设置页「插件」标签：插件开关 / 导入 / 重扫全部在这一页里完成（不再跳独立页）
fn key_setting(key: &str) -> serde_json::Value {
    crate::plugins::installed().ok()
        .and_then(|l| l.into_iter().find(|p| p.manifest.id == "phira-vrenxz.keydisplay"))
        .and_then(|p| p.settings.get(key).cloned())
        .unwrap_or(serde_json::Value::Null)
}
fn set_key_setting(key: &str, value: serde_json::Value) -> anyhow::Result<()> {
    let p = crate::plugins::installed()?.into_iter().find(|p| p.manifest.id == "phira-vrenxz.keydisplay")
        .ok_or_else(|| anyhow::anyhow!(tl!("sx-keydisplay-missing")))?;
    let mut s = p.settings.clone();
    if let Some(obj) = s.as_object_mut() { obj.insert(key.to_string(), value); }
    crate::plugins::change("phira-vrenxz.keydisplay", None, Some(s), false)
}
struct PluginList {
    plugins: Vec<crate::plugin::Plugin>,
    plugin_btns: Vec<DRectButton>,
    delete_btns: Vec<DRectButton>,
    import_btn: DRectButton,
    refresh_btn: DRectButton,
    setting_btns: Vec<DRectButton>,
    setting_items: Vec<(String, String, serde_json::Value)>,
}
impl PluginList {
    pub fn new() -> Self {
        let mut this = Self {
            plugins: Vec::new(),
            plugin_btns: Vec::new(),
            delete_btns: Vec::new(),
            import_btn: DRectButton::new(),
            refresh_btn: DRectButton::new(),
            setting_btns: Vec::new(),
            setting_items: Vec::new(),
        };
        this.refresh_plugins();
        this
    }
    /// 重新扫描 plugins 目录（导入 / 手动增删后调用）
    pub fn refresh_plugins(&mut self) {
        self.plugins = crate::plugin::load_plugins();
        self.plugin_btns = (0..self.plugins.len()).map(|_| DRectButton::new()).collect();
        self.delete_btns = (0..self.plugins.len()).map(|_| DRectButton::new()).collect();
        self.setting_items.clear();
        if let Ok(list) = crate::plugins::installed() {
            for p in list.into_iter().filter(|p| p.enabled) {
                if let Some(obj) = p.settings.as_object() {
                    for (k, v) in obj {
                        self.setting_items.push((p.manifest.id.clone(), k.clone(), v.clone()));
                    }
                }
            }
        }
        self.setting_btns = (0..self.setting_items.len()).map(|_| DRectButton::new()).collect();
    }

    pub fn update(&mut self, _t: f32) -> Result<bool> {
        Ok(false)
    }
    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        if self.import_btn.touch(touch, t) {
            request_file("_import_plugin");
            return Ok(Some(false));
        }
        for i in 0..self.setting_btns.len() {
            if self.setting_btns[i].touch(touch, t) {
                if let Some((id, key, val)) = self.setting_items.get(i).cloned() {
                    let next = match &val {
                        serde_json::Value::Bool(b) => serde_json::json!(!b),
                        serde_json::Value::Number(n) => serde_json::json!(((n.as_f64().unwrap_or(0.) + 0.1) % 1.0).max(0.05)),
                        serde_json::Value::String(s) if s.starts_with('#') => {
                            const COLORS: [&str; 5] = ["#4C84FF", "#FF4C6A", "#4CFF9E", "#FFC24C", "#C14CFF"];
                            let idx = COLORS.iter().position(|c| *c == s).unwrap_or(0);
                            serde_json::json!(COLORS[(idx + 1) % COLORS.len()])
                        }
                        _ => val.clone(),
                    };
                    if let Ok(mut list) = crate::plugins::installed() {
                        if let Some(p) = list.iter_mut().find(|p| p.manifest.id == id) {
                            if let Some(obj) = p.settings.as_object_mut() { obj.insert(key.clone(), next.clone()); }
                            let settings = p.settings.clone();
                            let _ = crate::plugins::change(&id, None, Some(settings), false);
                            self.setting_items[i].2 = next;
                        }
                    }
                }
                return Ok(Some(true));
            }
        }
        if self.refresh_btn.touch(touch, t) {
            self.refresh_plugins();
            let n = self.plugins.len();
            show_message(tl!("sx-plugins-rescanned", "n" => n).to_string()).ok();
            return Ok(Some(false));
        }
        for i in 0..self.delete_btns.len() {
            if self.delete_btns[i].touch(touch, t) {
                if let Some(p) = self.plugins.get(i) {
                    let name = p.info.name.clone();
                    match crate::plugin::delete_plugin(p) {
                        Ok(()) => { show_message(tl!("sx-plugin-deleted", "name" => name).to_string()).ok(); self.refresh_plugins(); }
                        Err(err) => { show_error(err); }
                    }
                }
                return Ok(Some(true));
            }
        }
        for i in 0..self.plugin_btns.len() {
            if self.plugin_btns[i].touch(touch, t) {
                if let Some(p) = self.plugins.get(i) {
                    let info = p.info.clone();
                    let target = !p.enabled();
                    let _ = crate::plugin::set_plugin_enabled(&info, target);
                    self.refresh_plugins();
                }
                return Ok(Some(true));
            }
        }
        Ok(None)
    }
    pub fn rows(&mut self) -> Vec<Row<'_>> {
        let mut v = Vec::new();
        v.push(row_value(
            tl!("sx-plugin-import"),
            Some(tl!("sx-plugin-import-sub").into()),
            &mut self.import_btn,
            tl!("sx-select-file"),
        ));
        v.push(row_arrow(
            tl!("sx-plugin-rescan"),
            Some(tl!("sx-plugin-rescan-sub").into()),
            &mut self.refresh_btn,
        ));
        {
            let plugins = &self.plugins;
            let btns = &mut self.plugin_btns;
            let del_btns = &mut self.delete_btns;
            for ((p, b), del) in plugins.iter().zip(btns.iter_mut()).zip(del_btns.iter_mut()) {
                let mut sub = format!("v{} by {}", p.info.version, p.info.author);
                if !p.entry_ok {
                    sub.push_str(&tl!("sx-plugin-entry-error"));
                } else if !p.info.description.is_empty() {
                    sub.push_str(" · ");
                    sub.push_str(&p.info.description);
                }
                let name = p.info.name.clone();
                let enabled = p.enabled();
                v.push(row_switch_delete(name.clone(), Some(sub.into()), b, del, enabled));
            }
            let items = &self.setting_items;
            let sbtns = &mut self.setting_btns;
            for ((id, key, val), b) in items.iter().zip(sbtns.iter_mut()) {
                let pname = plugins.iter().find(|p| p.pro_id.as_deref() == Some(id)).map(|p| p.info.name.as_str()).unwrap_or(id);
                let shown = match val {
                    serde_json::Value::Bool(v) => if *v { tl!("sx-toggle-on").to_string() } else { tl!("sx-toggle-off").to_string() },
                    serde_json::Value::Number(n) => format!("{:.0}%", n.as_f64().unwrap_or(0.) * 100.),
                    serde_json::Value::String(s) => s.clone(),
                    _ => val.to_string(),
                };
                v.push(row_value(format!("{pname} · {key}"), None, b, shown));
            }
            if plugins.is_empty() {
                v.push(Row {
                    label: tl!("sx-no-plugins").into(),
                    sub: Some(tl!("sx-no-plugins-sub").into()),
                    ctl: Ctl::None,
                    group: 0,
                });
            }
        }
        v
    }
}
struct CustomList {
    custom_bg_btn: DRectButton,
    reset_bg_btn: DRectButton,
    custom_video_btn: DRectButton,
    reset_video_btn: DRectButton,
    custom_bgm_btn: DRectButton,
    reset_bgm_btn: DRectButton,
    watermark_btn: DRectButton,
    combo_text_btn: DRectButton,
    autoplay_text_btn: DRectButton,
    custom_crash_title_btn: DRectButton,
    custom_crash_code_btn: DRectButton,
    custom_crash_reason_btn: DRectButton,
    custom_crash_btn: DRectButton,
    custom_crash_requested: bool,
    show_score_btn: DRectButton,
    show_combo_btn: DRectButton,
    show_acc_btn: DRectButton,
    /// 进入 HUD 自定义编辑模式（从 General 移入）。
    hud_btn: DRectButton,
    home_bg_btn: ChooseButton,
    font_btn: ChooseButton,
    import_font_btn: DRectButton,
    font_task: LocalTask<Result<()>>,
    font_options_count: usize,
}
impl CustomList {
    /// 左栏必须正好放下「内置主页背景 … 重置背景」这些行，剩余行才从右栏开始。
    const LEFT_ROWS: usize = 10;
    pub fn new() -> Self {
        Self {
            custom_bg_btn: DRectButton::new(),
            reset_bg_btn: DRectButton::new(),
            custom_video_btn: DRectButton::new(),
            reset_video_btn: DRectButton::new(),
            custom_bgm_btn: DRectButton::new(),
            reset_bgm_btn: DRectButton::new(),
            watermark_btn: DRectButton::new(),
            combo_text_btn: DRectButton::new(),
            autoplay_text_btn: DRectButton::new(),
            custom_crash_title_btn: DRectButton::new(),
            custom_crash_code_btn: DRectButton::new(),
            custom_crash_reason_btn: DRectButton::new(),
            custom_crash_btn: DRectButton::new(),
            custom_crash_requested: false,
            show_score_btn: DRectButton::new(),
            show_combo_btn: DRectButton::new(),
            show_acc_btn: DRectButton::new(),
            hud_btn: DRectButton::new(),
            home_bg_btn: ChooseButton::new()
                .with_options(vec![
                    tl!("item-home-bg-default").to_string(),
                    tl!("item-home-bg-one").to_string(),
                    tl!("item-home-bg-two").to_string(),
                    tl!("item-home-bg-three").to_string(),
                    tl!("item-home-bg-old").to_string(),
                    tl!("item-home-bg-loginbg").to_string(),
                ])
                .with_height(0.66)
                // 最后一项显示名很长，弹窗比按钮宽一些，避免被截断。
                .with_width(0.85)
                .with_selected(home_background_index()),
            font_btn: ChooseButton::new()
                .with_options({
                    let mut opts = vec![tl!("font-default").to_string()];
                    opts.extend(get_data().fonts.iter().map(|f| f.name.clone()));
                    opts
                })
                .with_height(0.54)
                .with_selected(get_data().font_id.min(get_data().fonts.len())),
            import_font_btn: DRectButton::new(),
            font_task: None,
            font_options_count: get_data().fonts.len() + 1,
        }
    }
    pub fn take_custom_crash_request(&mut self) -> bool {
        std::mem::take(&mut self.custom_crash_requested)
    }
    /// 自定义页：界面外观 / 自定义文本 / 背景 / 音乐 / 自定义崩溃 / 字体
    pub fn rows(&mut self) -> Vec<Row<'_>> {
        let data = get_data();
        let config = &data.config;
        let file_name = |p: &Option<String>, def: &str| -> String {
            p.as_ref()
                .and_then(|p| std::path::Path::new(p).file_name().and_then(|n| n.to_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| def.to_string())
        };
        let bg_name = file_name(&data.custom_background_path, &tl!("custom-default-bg"));
        let video_name = file_name(&data.home_video_path, &tl!("custom-not-selected"));
        // 主页和开屏共用同一首自定义音乐；旧数据只填了开屏路径时也能显示出来。
        let bgm_path = data.custom_bgm_path.as_ref().or(data.custom_startup_bgm_path.as_ref());
        let bgm_name = file_name(&bgm_path.cloned(), &tl!("custom-default-music"));
        let watermark = if config.custom_watermark.is_empty() {
            "phirLie".to_string()
        } else {
            config.custom_watermark.clone()
        };
        let combo_text = if config.custom_combo_text.is_empty() {
            tl!("item-combo-text-default").into_owned()
        } else {
            config.custom_combo_text.clone()
        };
        let autoplay_text = if config.autoplay_display_text.is_empty() {
            tl!("item-autoplay-text-default").into_owned()
        } else {
            config.autoplay_display_text.clone()
        };
        let crash_title = if config.custom_crash_title.is_empty() {
            tl!("custom-not-set").to_string()
        } else {
            config.custom_crash_title.clone()
        };
        let crash_code = tl!("custom-current-code", "code" => config.custom_crash_code.to_string()).to_string();
        let crash_reason = if config.custom_crash_reason.is_empty() {
            tl!("custom-not-set").to_string()
        } else {
            config.custom_crash_reason.clone()
        };
        vec![
            // ---- 左栏 ----
            row_choose(tl!("item-home-bg"), None, &mut self.home_bg_btn, 0.22),
            row_switch(tl!("custom-show-score"), None, &mut self.show_score_btn, config.show_score),
            row_switch(tl!("custom-show-combo"), None, &mut self.show_combo_btn, config.show_combo),
            row_switch(tl!("custom-show-acc"), None, &mut self.show_acc_btn, config.show_acc),
            row_value(tl!("item-hud"), Some(tl!("item-hud-sub")), &mut self.hud_btn, tl!("item-hud-open")),
            row_value(tl!("custom-watermark"), Some(watermark.into()), &mut self.watermark_btn, tl!("custom-edit")),
            row_value(tl!("custom-combo-text"), Some(combo_text.into()), &mut self.combo_text_btn, tl!("custom-edit")),
            row_value(
                tl!("custom-autoplay-text"),
                Some(autoplay_text.into()),
                &mut self.autoplay_text_btn,
                tl!("custom-edit"),
            ),
            row_value(tl!("custom-select-bg"), Some(bg_name.into()), &mut self.custom_bg_btn, tl!("custom-select")),
            row_arrow(tl!("custom-reset-bg"), None, &mut self.reset_bg_btn),
            // ---- 右栏 ----
            row_value(tl!("custom-home-bgm"), Some(bgm_name.into()), &mut self.custom_bgm_btn, tl!("custom-select")),
            row_arrow(tl!("custom-reset-home-bgm"), None, &mut self.reset_bgm_btn),
            row_value(
                tl!("custom-crash-title"),
                Some(crash_title.into()),
                &mut self.custom_crash_title_btn,
                tl!("custom-edit"),
            ),
            row_value(
                tl!("custom-crash-code"),
                Some(crash_code.into()),
                &mut self.custom_crash_code_btn,
                tl!("custom-edit"),
            ),
            row_value(
                tl!("custom-crash-reason"),
                Some(crash_reason.into()),
                &mut self.custom_crash_reason_btn,
                tl!("custom-edit"),
            ),
            row_value(
                tl!("custom-trigger-crash"),
                Some(tl!("custom-trigger-crash-sub")),
                &mut self.custom_crash_btn,
                tl!("custom-trigger"),
            ),
            row_choose(tl!("item-font"), Some(tl!("item-font-sub")), &mut self.font_btn, 0.22),
            row_value(
                tl!("item-import-font"),
                Some(tl!("item-import-font-sub")),
                &mut self.import_font_btn,
                tl!("font-import"),
            ),
            row_value(
                tl!("sx-home-video"),
                Some(video_name.into()),
                &mut self.custom_video_btn,
                tl!("custom-select").into_owned(),
            ),
            row_arrow(tl!("sx-home-video-reset"), None, &mut self.reset_video_btn),
        ]
    }
    pub fn render_top(&mut self, ui: &mut Ui, t: f32) {
        self.home_bg_btn.render_top(ui, t, 1.);
        self.font_btn.render_top(ui, t, 1.);
    }
    pub fn top_touch(&mut self, touch: &Touch, t: f32) -> bool {
        if self.home_bg_btn.top_touch(touch, t) {
            return true;
        }
        if self.font_btn.top_touch(touch, t) {
            return true;
        }
        false
    }
    pub fn touch(&mut self, touch: &Touch, t: f32) -> Result<Option<bool>> {
        let data = get_data_mut();
        let config = &mut data.config;
        if self.home_bg_btn.touch(touch, t) {
            return Ok(Some(false));
        }
        if self.font_btn.touch(touch, t) {
            return Ok(Some(false));
        }
        if self.import_font_btn.touch(touch, t) {
            request_file("_import_font");
            return Ok(Some(false));
        }
        if self.custom_bg_btn.touch(touch, t) {
            request_file("custom_bg");
            return Ok(Some(false));
        }
        if self.custom_video_btn.touch(touch, t) {
            request_file("home_video");
            return Ok(Some(false));
        }
        if self.reset_video_btn.touch(touch, t) {
            data.home_video_path = None;
            crate::scene::HOME_VIDEO_UPDATED.store(true, Ordering::Relaxed);
            show_message(tl!("sx-home-video-reset-done")).ok();
            return Ok(Some(true));
        }
        if self.reset_bg_btn.touch(touch, t) {
            data.custom_background_path = None;
            BACKGROUND_UPDATED.store(true, Ordering::Relaxed);
            show_message(tl!("custom-bg-reset")).ok();
            return Ok(Some(true));
        }
        if self.custom_bgm_btn.touch(touch, t) {
            request_file("custom_bgm");
            return Ok(Some(false));
        }
        if self.reset_bgm_btn.touch(touch, t) {
            data.custom_bgm_path = None;
            data.custom_startup_bgm_path = None;
            BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
            show_message(tl!("custom-bgm-reset")).ok();
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
        if self.autoplay_text_btn.touch(touch, t) {
            request_input("autoplay-text", InputBox::new().default_text(&data.config.autoplay_display_text));
            return Ok(Some(true));
        }
        if self.custom_crash_title_btn.touch(touch, t) {
            let current = config.custom_crash_title.clone();
            request_input("custom_crash_title", InputBox::new().default_text(&current));
            return Ok(Some(true));
        }
        if self.custom_crash_code_btn.touch(touch, t) {
            let current = config.custom_crash_code.to_string();
            request_input("custom_crash_code", InputBox::new().default_text(&current));
            return Ok(Some(true));
        }
        if self.custom_crash_reason_btn.touch(touch, t) {
            let current = config.custom_crash_reason.clone();
            request_input("custom_crash_reason", InputBox::new().default_text(&current));
            return Ok(Some(true));
        }
        if self.custom_crash_btn.touch(touch, t) {
            self.custom_crash_requested = true;
            return Ok(Some(true));
        }
        if self.show_score_btn.touch(touch, t) {
            config.show_score ^= true;
            return Ok(Some(true));
        }
        if self.show_combo_btn.touch(touch, t) {
            config.show_combo ^= true;
            return Ok(Some(true));
        }
        if self.show_acc_btn.touch(touch, t) {
            config.show_acc ^= true;
            return Ok(Some(true));
        }
        if self.hud_btn.touch(touch, t) {
            // 进入 HUD 自定义编辑模式：回主菜单并开启编辑（切页由 MainScene 处理）。
            prpr::hud::set_edit(true);
            prpr::hud::request_goto(prpr::hud::PageId::Home);
            return Ok(Some(true));
        }
        Ok(None)
    }
    pub fn update(&mut self, t: f32) -> Result<bool> {
        self.home_bg_btn.update(t);
        if self.home_bg_btn.changed() {
            let idx = self.home_bg_btn.selected().min(BUILTIN_HOME_BACKGROUNDS.len() - 1);
            let data = get_data_mut();
            data.config.home_background = Some(BUILTIN_HOME_BACKGROUNDS[idx].to_string());
            BACKGROUND_UPDATED.store(true, Ordering::Relaxed);
            return Ok(true);
        }
        // 字体列表刷新
        let expected = get_data().fonts.len() + 1;
        if self.font_options_count != expected {
            self.font_options_count = expected;
            let mut opts = vec![tl!("font-default").to_string()];
            opts.extend(get_data().fonts.iter().map(|f| f.name.clone()));
            self.font_btn.set_options(opts);
            if get_data().font_id >= expected {
                get_data_mut().font_id = 0;
            }
        }
        self.font_btn.update(t);
        if self.font_btn.changed() {
            let data = get_data_mut();
            let idx = self.font_btn.selected();
            data.font_id = idx;
            let custom = idx.checked_sub(1).and_then(|i| data.fonts.get(i)).cloned();
            self.font_task = Some(Box::pin(async move {
                let builtin = FontArc::try_from_vec(load_file("fonts/font.ttf").await?)?;
                let (main_font, fallback) = match custom {
                    Some(f) => {
                        let root = dir::fonts()?;
                        let bytes = std::fs::read(format!("{root}/{}", f.file))?;
                        (FontArc::try_from_vec(bytes)?, Some(builtin))
                    }
                    None => (builtin, None),
                };
                MAIN_FONT.with(|it| *it.borrow_mut() = Some(TextPainter::new(main_font, fallback)));
                Ok(())
            }));
            return Ok(true);
        }
        if let Some(task) = &mut self.font_task {
            if let Some(res) = poll_future(task.as_mut()) {
                self.font_task = None;
                match res {
                    Ok(()) => {
                        show_message(tl!("font-switched")).ok();
                    }
                    Err(err) => {
                        show_error(err.context(tl!("font-switch-failed")));
                    }
                }
            }
        }
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
            if id == "autoplay-text" {
                let data = get_data_mut();
                data.config.autoplay_display_text = text;
                return Ok(true);
            }
            if id == "custom_crash_title" {
                let data = get_data_mut();
                data.config.custom_crash_title = text;
                return Ok(true);
            } else if id == "custom_crash_code" {
                let data = get_data_mut();
                if let Ok(code) = text.parse::<u32>() {
                    data.config.custom_crash_code = code;
                }
                return Ok(true);
            } else if id == "custom_crash_reason" {
                let data = get_data_mut();
                data.config.custom_crash_reason = text;
                return Ok(true);
            }
            return_input(id, text);
        }
        if let Some((id, file)) = take_file() {
            if id == "custom_bg" {
                let data = get_data_mut();
                data.custom_background_path = Some(file);
                BACKGROUND_UPDATED.store(true, Ordering::Relaxed);
                show_message(tl!("custom-bg-set")).ok();
                return Ok(true);
            } else if id == "home_video" {
                let data = get_data_mut();
                data.home_video_path = Some(file);
                HOME_VIDEO_UPDATED.store(true, Ordering::Relaxed);
                show_message(tl!("sx-home-video-set")).ok();
                return Ok(true);
            } else if id == "custom_bgm" {
                let data = get_data_mut();
                data.custom_bgm_path = Some(file);
                // 合并后只保留一个自定义音乐；顺手清掉旧的开屏路径。
                data.custom_startup_bgm_path = None;
                BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
                show_message(tl!("custom-bgm-set")).ok();
                return Ok(true);
            } else {
                return_file(id, file);
            }
        }
        Ok(false)
    }
    /// 若用户点击了「切换角色」，返回进入角色立绘页的跳转。
    pub fn next_page(&mut self) -> Option<NextPage> {
        None
    }
}
