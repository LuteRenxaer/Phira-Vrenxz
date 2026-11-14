//! 开屏 / 主页共用的主视觉：左侧主图 + 右侧半圆黑场 + Phigros TriGrid 几何背景。
//!
//! 开屏与主页是同一画面：同一张主图、同一个半圆黑场、同一个 TriGrid 背景、同一片粒子，
//! 只有面板里的 UI 换掉，所以点「点击屏幕开始」之后画面是连着的（不黑屏、不跳图）。
//!
//! - 半圆黑场：圆心在屏幕右缘中点，`smoothstep` 超宽羽化，中间是连续渐变、没有分段色带。
//! - TriGrid：照搬 qaqFei/phispler 的 `shaders/trigrid.glsl`（Phigros 主菜单的三角网格背景），
//!   用 `TriGridBase.png` 的 R/G/B 三通道分别当网格线 / 辉光 / 面色，按时间做呼吸脉冲。
//!   黑场和网格在同一个 fragment 里合成，只占一次全屏填充。
//! - 小方块：粒子发射器，从屏幕右侧缓缓飘出、越往左越淡，飘到封面位置之前消失。

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use macroquad::prelude::*;
use miniquad::{BlendFactor, BlendState, BlendValue, Equation, UniformType};
use prpr::{
    core::PGR_FONT,
    ext::{semi_white, SafeTexture, ScaleType},
    ui::Ui,
};

/// TriGrid 脉冲周期：2π / 1.6（shader 里 sin((uv2.y + t) * 1.6) 的周期）
const TRI_PERIOD: f32 = 3.926_990_8;

/// 面板内容中心（屏幕比例）
pub const CONTENT_CX: f32 = 0.775;

thread_local! {
    static KEY_ART: RefCell<Option<SafeTexture>> = const { RefCell::new(None) };
    static KEY_ART_BLUR: RefCell<Option<SafeTexture>> = const { RefCell::new(None) };
    static TRI_GRID: RefCell<Option<SafeTexture>> = const { RefCell::new(None) };
    /// 标题贴图（icons/phiravrenxz_icon.png，已裁透明边）
    static LOGO_TEX: RefCell<Option<SafeTexture>> = const { RefCell::new(None) };
}

/// 开屏 / 主页共用的主图（loginbg.png，Phigros 4.0 主视觉）
pub fn set_key_art(tex: SafeTexture) {
    KEY_ART.with(|k| *k.borrow_mut() = Some(tex));
}

pub fn key_art() -> Option<SafeTexture> {
    KEY_ART.with(|k| k.borrow().clone())
}

/// 主图的模糊版（缩到很小再线性放大），设置页当底图用
pub fn set_key_art_blur(tex: SafeTexture) {
    KEY_ART_BLUR.with(|k| *k.borrow_mut() = Some(tex));
}

pub fn key_art_blur() -> Option<SafeTexture> {
    KEY_ART_BLUR.with(|k| k.borrow().clone())
}

/// TriGrid 用的三通道贴图（textures/TriGridBase.png）
pub fn set_tri_grid(tex: SafeTexture) {
    TRI_GRID.with(|k| *k.borrow_mut() = Some(tex));
}

pub fn tri_grid() -> Option<SafeTexture> {
    TRI_GRID.with(|k| k.borrow().clone())
}

/// 主页背景左侧主图的默认素材（开屏 / 主页 / 所有页面共用）。
const DEFAULT_KEY_ART: &str = "backgrounds/loginbg.png";

static KEY_ART_FALLBACK_WARNED: AtomicBool = AtomicBool::new(false);

fn warn_key_art_fallback(msg: String) {
    if !KEY_ART_FALLBACK_WARNED.swap(true, Ordering::Relaxed) {
        warn!("failed to load key art: {msg}, falling back to {DEFAULT_KEY_ART}");
    }
}

/// 主页背景左侧那张图从哪里来：
/// 自定义背景（`custom_background_path`）> 设置里选的内置背景（`config.home_background`）
/// > 默认主视觉 `loginbg.png`。
///
/// 自定义背景只替换左侧主图，右侧黑场 / TriGrid / 粒子仍是主页那一套。
fn key_art_source() -> PathBuf {
    if let Some(path) = crate::get_data().custom_background_path.clone() {
        let p = PathBuf::from(&path);
        if p.is_file() {
            return p;
        }
        warn_key_art_fallback(format!("custom background {path} is not accessible"));
    }
    if let Some(name) = crate::get_data().config.home_background.clone() {
        let p = crate::scene::assets_root().join(format!("backgrounds/{name}"));
        if p.is_file() {
            return p;
        }
        warn_key_art_fallback(format!("home background {name} is missing"));
    }
    crate::scene::assets_root().join(DEFAULT_KEY_ART)
}

/// 只读主图字节（后台线程安全，不碰 OpenGL）。
pub async fn load_key_art_bytes() -> Result<Vec<u8>> {
    let path = key_art_source();
    match std::fs::read(&path) {
        Ok(data) => Ok(data),
        Err(err) => {
            warn_key_art_fallback(format!("cannot read {}: {err}", path.display()));
            let fallback = crate::scene::assets_root().join(DEFAULT_KEY_ART);
            std::fs::read(&fallback).with_context(|| format!("read {}", fallback.display()))
        }
    }
}

/// 把任意图片加工成主图贴图：右侧镜像补一条（垫在面板底下，避免右缘被渐隐区切出硬边），
/// 再缩出一张 96 宽的小图当模糊版。
fn build_key_art(bytes: &[u8]) -> Result<(SafeTexture, SafeTexture)> {
    let img = image::load_from_memory(bytes).context("decode key art")?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let pad = (w / 4).max(1);
    let stride = (w + pad) as usize * 4;
    let mut buf = vec![0u8; stride * h as usize];
    let raw = rgba.as_raw();
    for y in 0..h {
        let src = &raw[y as usize * w as usize * 4..(y as usize + 1) * w as usize * 4];
        let dst = &mut buf[y as usize * stride..(y as usize + 1) * stride];
        dst[..(w * 4) as usize].copy_from_slice(src);
        for x in w..(w + pad) {
            let sx = (2 * w - 2 - x) as usize * 4;
            let off = (x * 4) as usize;
            dst[off..off + 4].copy_from_slice(&src[sx..sx + 4]);
        }
    }
    let tex = Texture2D::from_rgba8((w + pad) as u16, h as u16, &buf);
    tex.set_filter(FilterMode::Linear);

    let pad_img = image::RgbaImage::from_raw(w + pad, h, buf).unwrap_or_else(|| rgba.clone());
    let bw = 96u32;
    let bh = (h * bw / (w + pad)).max(1);
    let small = image::imageops::resize(&pad_img, bw, bh, image::imageops::FilterType::Triangle);
    let blur_tex = Texture2D::from_rgba8(bw as u16, bh as u16, small.as_raw());
    blur_tex.set_filter(FilterMode::Linear);

    Ok((SafeTexture::from(tex), SafeTexture::from(blur_tex)))
}

/// 上传主图并替换全局那份；解码失败时自动退回默认主图。
pub fn install_key_art(bytes: &[u8]) -> Result<SafeTexture> {
    match build_key_art(bytes) {
        Ok((tex, blur)) => {
            set_key_art(tex.clone());
            set_key_art_blur(blur);
            Ok(tex)
        }
        Err(err) => {
            warn_key_art_fallback(format!("cannot decode key art: {err:#}"));
            let fallback = crate::scene::assets_root().join(DEFAULT_KEY_ART);
            let data = std::fs::read(&fallback).with_context(|| format!("read {}", fallback.display()))?;
            let (tex, blur) = build_key_art(&data)?;
            set_key_art(tex.clone());
            set_key_art_blur(blur);
            Ok(tex)
        }
    }
}

/// 读当前设置对应的主图并上传（开屏启动与设置页改完之后的重新装载共用）。
pub async fn load_key_art() -> Result<SafeTexture> {
    let bytes = load_key_art_bytes().await?;
    install_key_art(&bytes)
}

/// 屏幕横向比例 → UI 世界坐标 x
#[inline]
pub fn sx(v: f32) -> f32 {
    v * 2. - 1.
}

/// 屏幕纵向比例 → UI 世界坐标 y
#[inline]
pub fn sy(ui: &Ui, v: f32) -> f32 {
    (v * 2. - 1.) * ui.top
}

struct Particle {
    y: f32,
    speed: f32,
    phase: f32,
    size: f32,
    bright: f32,
}

/// 开屏 / 主页共用的底板
pub struct Backdrop {
    material: RefCell<Option<Material>>,
    tried: Cell<bool>,
    particles: Vec<Particle>,
}

impl Backdrop {
    pub fn new() -> Self {
        let mut seed = 0x51ed_270bu32;
        let mut rnd = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / 16_777_216.
        };
        let mut particles = Vec::new();
        for _ in 0..44 {
            let y = 0.03 + rnd() * 0.94;
            let speed = 0.010 + rnd() * 0.020;
            let phase = rnd();
            let size = 0.0080 + rnd() * 0.0080;
            let bright = 0.45 + rnd() * 0.55;
            particles.push(Particle { y, speed, phase, size, bright });
        }
        Self {
            material: RefCell::new(None),
            tried: Cell::new(false),
            particles,
        }
    }

    /// 材质必须在渲染线程、GL 上下文可用时创建，所以放到第一次绘制时惰性创建。
    fn ensure(&self) {
        if self.tried.get() {
            return;
        }
        self.tried.set(true);
        *self.material.borrow_mut() = load_material(
            shader::VERTEX,
            shader::FRAGMENT,
            MaterialParams {
                pipeline_params: miniquad::PipelineParams {
                    color_blend: Some(BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::SourceAlpha),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    )),
                    ..Default::default()
                },
                uniforms: vec![
                    ("t".to_owned(), UniformType::Float1),
                    ("uAlpha".to_owned(), UniformType::Float1),
                    ("uAspect".to_owned(), UniformType::Float1),
                ],
                textures: vec!["uTex".to_owned()],
                ..Default::default()
            },
        )
        .map_err(|e| {
            warn!("backdrop shader error: {e:?}");
            e
        })
        .ok();
    }

    /// 绘制底板：主图 + 半圆黑场 & TriGrid + 飘动的小方块粒子。
    pub fn draw(&self, ui: &mut Ui, art: Option<&SafeTexture>, time: f32, alpha: f32) {
        if alpha <= 0.001 {
            return;
        }
        let top = ui.top;
        let full = Rect::new(-1., -top, 2., 2. * top);

        // 先铺一层不透明底色：主图 / 黑场 shader 在左侧都不是完全不透明，
        // 不铺底会把上一帧或上一个页面的画面透出来（切页残影）。
        ui.fill_rect(full, Color::new(0.060, 0.070, 0.090, 1.0));

        // 主图：默认主视觉（16:9）铺满高度、右缘贴住屏幕右缘，主体（人物）落在左半屏；
        // 玩家自定义的竖图 / 窄图则贴左显示，空出来的地方用同一张图的大模糊版补满，
        // 避免人像被右侧黑场整个盖住。
        if let Some(tex) = art {
            let ar = (tex.width() / tex.height()).max(0.2);
            let h = 2. * top;
            let w = h * ar;
            let screen_ar = full.w / full.h;
            let r = if ar + 0.02 < screen_ar {
                if let Some(blur) = key_art_blur() {
                    let gr = ui.rect_to_global(full);
                    ui.fill_rect(
                        full,
                        (*blur, gr, ScaleType::CropCenter, Color::new(1., 1., 1., 0.55 * alpha)),
                    );
                }
                Rect::new(full.x, full.y, w, h)
            } else {
                Rect::new(full.right() - w, full.y, w, h)
            };
            ui.fill_rect(r, (**tex, r, ScaleType::default(), Color::new(1., 1., 1., alpha)));
        }

        self.draw_dome(ui, time, alpha);
    }

    /// 只绘制穹顶/右侧黑场/TriGrid/粒子，用于视频背景之上。
    pub fn draw_dome(&self, ui: &mut Ui, time: f32, alpha: f32) {
        let top = ui.top;
        let full = Rect::new(-1., -top, 2., 2. * top);
        // 半圆黑场 + TriGrid（单 pass）
        self.ensure();
        if let Some(m) = &*self.material.borrow() {
            if let Some(tg) = tri_grid() {
                m.set_texture("uTex", *tg);
            }
            m.set_uniform("t", time.rem_euclid(TRI_PERIOD));
            m.set_uniform("uAlpha", alpha);
            m.set_uniform("uAspect", full.w / full.h);
            let r = ui.rect_to_global(full);
            gl_use_material(*m);
            draw_rectangle(r.x, r.y, r.w, r.h, WHITE);
            gl_use_default_material();
        }

        // 粒子：从右侧缓缓向左飘，越往左越淡，到封面（fx≈0.5）之前消失
        for p in &self.particles {
            let travel = (time * p.speed + p.phase).fract();
            let x = 1.03 - travel * 0.78;
            let fade_in = ((1.0 - x) / 0.14).clamp(0., 1.);
            let fade_out = ((x - 0.44) / 0.12).clamp(0., 1.);
            let a = fade_in * fade_out * p.bright * alpha;
            if a <= 0.01 {
                continue;
            }
            let s = p.size;
            ui.fill_rect(Rect::new(sx(x), sy(ui, p.y), s, s / ui.top * 0.62), Color::new(1., 1., 1., a));
        }
    }
}

impl Drop for Backdrop {
    fn drop(&mut self) {
        if let Some(mut m) = self.material.borrow_mut().take() {
            m.delete();
        }
    }
}

fn logo_parts(ui: &mut Ui, size: f32) -> (f32, f32, f32, f32) {
    let gap = size * 0.07;
    let suffix_size = size * 0.26;
    let mut t = ui.text("PHIRA").size(size);
    let w_main = t.measure_using(&PGR_FONT).w;
    let mut t = ui.text("vrenxz").size(suffix_size);
    let w_suffix = t.measure_using(&PGR_FONT).w;
    (w_main, gap, w_suffix, suffix_size)
}

pub fn logo_width(ui: &mut Ui, size: f32) -> f32 {
    let (a, b, c, _) = logo_parts(ui, size);
    a + b + c
}

/// 以左边缘 x 绘制字标，返回总宽度。
/// 整串一次绘制 + 两层辉光，避免每帧几十次逐字排版造成的掉帧。
pub fn draw_logo_left(ui: &mut Ui, x: f32, cy: f32, size: f32, alpha: f32) -> f32 {
    let (w_main, gap, w_suffix, suffix_size) = logo_parts(ui, size);
    for (k, a) in [(1.08f32, 0.10f32), (1.03, 0.14)] {
        ui.text("PHIRA")
            .pos(x, cy)
            .anchor(0., 0.5)
            .size(size * k)
            .color(Color::new(0.45, 0.75, 1., a * alpha))
            .draw_using(&PGR_FONT);
    }
    ui.text("PHIRA")
        .pos(x, cy)
        .anchor(0., 0.5)
        .size(size)
        .color(Color::new(1., 1., 1., alpha))
        .draw_using(&PGR_FONT);
    ui.text("vrenxz")
        .pos(x + w_main + gap, cy + size * 0.05)
        .anchor(0., 0.5)
        .size(suffix_size)
        .color(Color::new(0.72, 0.86, 1., 0.9 * alpha))
        .draw_using(&PGR_FONT);
    w_main + gap + w_suffix
}

/// 以 cx 居中绘制字标
pub fn draw_logo(ui: &mut Ui, cx: f32, cy: f32, size: f32, alpha: f32) {
    // 有标题贴图（icons/phiravrenxz_icon.png）就直接画图，宽度仍按字标尺寸走，
    // 这样开屏 → 主页的字标位置/大小完全一致。
    if let Some(tex) = LOGO_TEX.with(|it| it.borrow().clone()) {
        let w = logo_width(ui, size);
        let h = w * tex.height() / tex.width();
        let r = Rect::new(cx - w * 0.5, cy - h * 0.5, w, h);
        ui.fill_rect(r, (*tex, r, ScaleType::Fit, semi_white(alpha)));
        return;
    }
    let w = logo_width(ui, size);
    draw_logo_left(ui, cx - w / 2., cy, size, alpha);
}

/// 设置标题贴图（已裁掉透明边），传 None 则回到原来的字形绘制
pub fn set_logo_tex(tex: Option<SafeTexture>) {
    LOGO_TEX.with(|it| *it.borrow_mut() = tex);
}

/// 带字距的文字宽度（主字体）
pub fn text_width(ui: &mut Ui, text: &str, size: f32, tracking: f32) -> f32 {
    let mut w = 0.;
    for ch in text.chars() {
        let mut t = ui.text(ch.to_string()).size(size);
        w += t.measure().w + tracking;
    }
    w - tracking
}

/// 提示文字：整串一次绘制（不再逐字排版）
pub fn draw_text_tracked(ui: &mut Ui, text: &str, x: f32, cy: f32, _size: f32, tracking: f32, color: Color) -> f32 {
    ui.text(text)
        .pos(x, cy)
        .anchor(0., 0.5)
        .size(_size)
        .color(color)
        .draw();
    text_width(ui, text, _size, tracking)
}

/// 面板内的小字（版本号、提示之类）
pub fn draw_hint(ui: &mut Ui, cx: f32, cy: f32, text: &str, size: f32, alpha: f32) {
    ui.text(text)
        .pos(cx, cy)
        .anchor(0.5, 0.5)
        .size(size)
        .color(semi_white(0.88 * alpha))
        .draw();
}

mod shader {
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

    // 右半圆黑场 + Phigros TriGrid（照搬 phispler 的 trigrid.glsl，参数取预览默认值并整体衰减）
    pub const FRAGMENT: &str = r#"#version 100
precision mediump float;

varying lowp vec2 uv;

uniform sampler2D uTex;
uniform float t;
uniform float uAlpha;
uniform float uAspect;

void main() {
    // ---- TriGrid：斜切 + 平铺 + 时间脉冲 ----
    vec2 uv2 = uv;
    vec2 w = uv2 - 0.5;
    uv2.x += pow(abs(w.y), 2.0) * (-w.x);
    uv2.x += 0.1;
    vec2 tuv = mod(uv2 / vec2(0.2, 0.25), 1.0);
    vec4 tc = texture2D(uTex, tuv);

    float k = 0.8;
    // t 已由 CPU 按周期取模，这里保持小值，避免 mediump 的 sin 大参数精度问题
    float p = clamp((sin((uv2.y + t) * 1.6) - k) / (1.0 - k), 0.0, 1.0);

    vec3 tri =
        vec3(0.018, 0.018, 0.018) +
        vec3(1.0, 1.0, 1.0) * 0.4 * tc.r +
        vec3(0.702, 0.9449554, 1.0) * 0.1 * tc.b * (p * 2.0) +
        vec3(0.383, 0.8843222, 1.0) * (0.71 * tc.g * (p * 5.0));
    // 右侧更强、左侧几乎看不见
    tri *= mix(0.18, 1.0, smoothstep(0.15, 0.62, uv.x));

    // ---- 右半圆黑场 ----
    vec2 q = vec2((uv.x - 1.0) * uAspect, uv.y - 0.5);
    float d = length(q);
    float inside = 1.0 - smoothstep(0.42, 1.18, d);

    // 暗场打底 + 网格加色（单 pass 合成）
    vec3 rgb = vec3(0.060, 0.070, 0.090) + tri * 0.16;
    float a = clamp(inside * 0.95 + 0.10 * (tri.r + tri.g + tri.b) * (1.0 - inside), 0.0, 1.0);
    gl_FragColor = vec4(rgb, a * uAlpha);
}"#;
}
