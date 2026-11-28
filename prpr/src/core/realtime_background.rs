//! 实时演算背景
//!
//! 谱面目录内若存在 `.pbfx`（Phira Background FX）文件，则忽略曲绘动态背景设置，
//! 由本模块在运行时逐帧计算并绘制背景。`.pbfx` 是一个 JSON 参数表：
//! 渐变底色、星点、呼吸环境几何体、按 BPM 在随机位置释放的图形/光环/粒子。
//!
//! 与 `visual-fx.html` 的效果一一对应，但完全由引擎原生绘制（无脚本、无视频）。
//!
//! 单位约定：
//! - 位置：画面归一化坐标（0~1）
//! - 尺寸：占画面短边的比例
//! - 像素类参数（线宽、粒子半径等）：以 1080p 为基准的相对像素

use crate::ext::SafeTexture;
use anyhow::{Context, Result};
use macroquad::prelude::*;
use miniquad::{BlendFactor, BlendState, BlendValue, Equation, FilterMode, PipelineParams};
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};
use std::path::Path;

// 纯顶点色着色器 + 加法混合，模拟 HTML 版本里的 `globalCompositeOperation = "lighter"` 发光叠加
const FX_VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying vec4 color;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1.0);
    color = color0 / 255.0;
}"#;

const FX_FRAGMENT: &str = r#"#version 100
precision mediump float;

varying vec4 color;

uniform float brightness;

void main() {
    gl_FragColor = vec4(color.rgb * brightness, color.a);
}"#;

/// 谱面内实时背景效果文件的扩展名（不带点）。
pub const EFFECT_EXTENSION: &str = "pbfx";

/// 判断文件名是否为实时演算效果文件（扩展名不区分大小写）。
pub fn is_effect_file(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|it| it.to_str())
        .is_some_and(|it| it.eq_ignore_ascii_case(EFFECT_EXTENSION))
}

/// 在谱面根目录文件列表中查找效果文件。
pub fn find_effect_file(files: &[String]) -> Option<String> {
    files.iter().find(|it| is_effect_file(it)).cloned()
}

const DEFAULT_PALETTE: [f32; 9] = [8., 38., 60., 150., 190., 215., 265., 300., 330.];

/// 实时演算效果的完整参数表。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RealtimeFx {
    pub version: u32,
    pub background: BackgroundFx,
    pub palette: Vec<f32>,
    pub stars: StarFx,
    pub ambient: AmbientFx,
    pub burst: BurstFx,
    pub ring: RingFx,
    pub particle: ParticleFx,
    pub floater: FloaterFx,
    pub limit: LimitFx,
}

impl Default for RealtimeFx {
    fn default() -> Self {
        Self {
            version: 1,
            background: BackgroundFx::default(),
            palette: DEFAULT_PALETTE.to_vec(),
            stars: StarFx::default(),
            ambient: AmbientFx::default(),
            burst: BurstFx::default(),
            ring: RingFx::default(),
            particle: ParticleFx::default(),
            floater: FloaterFx::default(),
            limit: LimitFx::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BackgroundFx {
    /// 中心色（#rrggbb）
    pub inner: String,
    /// 过渡色（#rrggbb）
    pub mid: String,
    /// 边缘色（#rrggbb）
    pub outer: String,
    /// 色相漂移速度（度/秒），0 表示静态
    pub hue_drift: f32,
    /// 中心亮区半径（纹理 UV 比例）
    pub radius: f32,
    /// 整体亮度（补偿游玩时 backgroundDim 的压暗）
    pub brightness: f32,
}

impl Default for BackgroundFx {
    fn default() -> Self {
        Self {
            inner: "#2a1830".to_owned(),
            mid: "#0b0810".to_owned(),
            outer: "#040309".to_owned(),
            hue_drift: 4.,
            radius: 0.75,
            brightness: 2.,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StarFx {
    pub enabled: bool,
    /// 密度系数，数量约为 画面面积 / 14000 * density
    pub density: f32,
    /// 半径范围（1080p 像素）
    pub radius: [f32; 2],
    /// 闪烁速度范围（rad/s）
    pub twinkle: [f32; 2],
    pub alpha: f32,
}

impl Default for StarFx {
    fn default() -> Self {
        Self {
            enabled: true,
            density: 1.,
            radius: [0.5, 1.9],
            twinkle: [0.2, 1.0],
            alpha: 0.7,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AmbientFx {
    pub enabled: bool,
    /// 数量
    pub count: f32,
    /// 图形种类：diamond / hex / tri / circle / star / ring
    pub types: Vec<String>,
    /// 半径范围（短边比例）
    pub radius: [f32; 2],
    /// 自转速度（rad/s）
    pub spin: f32,
    /// 呼吸速度范围（rad/s）
    pub breathe: [f32; 2],
    /// 漂移速度（短边比例/秒）
    pub drift: f32,
    pub alpha: f32,
    /// 描边宽度（1080p 像素）
    pub width: f32,
}

impl Default for AmbientFx {
    fn default() -> Self {
        Self {
            enabled: true,
            count: 12.,
            types: default_types(),
            radius: [0.028, 0.09],
            spin: 0.16,
            breathe: [0.22, 0.6],
            drift: 0.004,
            alpha: 0.15,
            width: 1.3,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BurstFx {
    pub enabled: bool,
    pub bpm: f32,
    pub bpm_variance: f32,
    /// 每次释放的图形数量范围
    pub count: [f32; 2],
    /// 释放点周围散布半径（短边比例）
    pub spread: f32,
    /// 图形起始半径范围（短边比例）
    pub start_radius: [f32; 2],
    /// 图形结束半径范围（短边比例）
    pub end_radius: [f32; 2],
    /// 生存时间范围（秒）
    pub lifetime: [f32; 2],
    /// 自转速度（rad/s）
    pub spin: f32,
    pub alpha: [f32; 2],
    /// 色相抖动（度）
    pub hue_jitter: f32,
    pub types: Vec<String>,
    /// 释放位置边距（0~0.5）
    pub margin: f32,
}

impl Default for BurstFx {
    fn default() -> Self {
        Self {
            enabled: true,
            bpm: 64.,
            bpm_variance: 0.06,
            count: [8., 13.],
            spread: 0.28,
            start_radius: [0.004, 0.012],
            end_radius: [0.018, 0.065],
            lifetime: [2.2, 4.0],
            spin: 1.1,
            alpha: [0.18, 0.55],
            hue_jitter: 30.,
            types: default_types(),
            margin: 0.12,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RingFx {
    pub enabled: bool,
    /// 同一次释放的光环数量范围
    pub count: [u32; 2],
    /// 起始半径范围（短边比例）
    pub start_radius: [f32; 2],
    /// 结束半径范围（短边比例）
    pub end_radius: [f32; 2],
    pub lifetime: [f32; 2],
    /// 线宽范围（1080p 像素）
    pub width: [f32; 2],
    pub alpha: [f32; 2],
    /// 同一次释放内各光环的启动延迟（秒）
    pub delay: f32,
    /// 淡入时长占总寿命的比例
    pub fade_in: f32,
    pub hue_jitter: f32,
}

impl Default for RingFx {
    fn default() -> Self {
        Self {
            enabled: true,
            count: [1, 2],
            start_radius: [0.014, 0.036],
            end_radius: [0.57, 0.89],
            lifetime: [3.4, 5.4],
            width: [1.0, 2.0],
            alpha: [0.14, 0.24],
            delay: 0.2,
            fade_in: 0.125,
            hue_jitter: 20.,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ParticleFx {
    pub enabled: bool,
    /// 每个图形对应的粒子数量
    pub per_shape: f32,
    /// 初速度范围（短边比例/秒）
    pub speed: [f32; 2],
    pub lifetime: [f32; 2],
    /// 半径范围（1080p 像素）
    pub radius: [f32; 2],
    pub drag: [f32; 2],
    pub hue_jitter: f32,
    pub alpha: f32,
}

impl Default for ParticleFx {
    fn default() -> Self {
        Self {
            enabled: true,
            per_shape: 0.9,
            speed: [0.028, 0.14],
            lifetime: [1.2, 2.8],
            radius: [1.0, 3.4],
            drag: [0.7, 1.6],
            hue_jitter: 40.,
            alpha: 0.8,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LimitFx {
    pub shapes: usize,
    pub particles: usize,
    pub rings: usize,
    pub trails: usize,
}

impl Default for LimitFx {
    fn default() -> Self {
        Self {
            shapes: 120,
            particles: 260,
            rings: 12,
            trails: 220,
        }
    }
}

/// 漂浮图形：随机游走的大号图形，可带小圆拖尾。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FloaterFx {
    pub enabled: bool,
    pub count: f32,
    pub types: Vec<String>,
    /// 半径范围（短边比例）
    pub radius: [f32; 2],
    /// 移动速度范围（短边比例/秒）
    pub speed: [f32; 2],
    /// 方向随机游走强度（最大角速度，rad/s）
    pub wander: f32,
    /// 角速度变化率（rad/s²，越小转向越顺滑）
    pub wander_jerk: f32,
    /// 自转速度（rad/s）
    pub spin: f32,
    /// 描边宽度（1080p 像素）
    pub width: f32,
    pub alpha: f32,
    pub trail: TrailFx,
}

impl Default for FloaterFx {
    fn default() -> Self {
        Self {
            enabled: false,
            count: 2.,
            types: vec!["star4".to_owned()],
            radius: [0.045, 0.065],
            speed: [0.025, 0.05],
            wander: 0.9,
            wander_jerk: 0.8,
            spin: 0.6,
            width: 2.,
            alpha: 0.6,
            trail: TrailFx::default(),
        }
    }
}

/// 漂浮图形留下的小圆拖尾。
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TrailFx {
    pub enabled: bool,
    /// 每移动多少（短边比例）落一个点
    pub spacing: f32,
    /// 点的半径范围（短边比例）
    pub radius: [f32; 2],
    /// 点的寿命范围（秒）
    pub life: [f32; 2],
    /// 生成点相对图形的随机散开半径（短边比例）
    pub scatter: f32,
    pub alpha: f32,
    pub hue_jitter: f32,
}

impl Default for TrailFx {
    fn default() -> Self {
        Self {
            enabled: true,
            spacing: 0.018,
            radius: [0.004, 0.009],
            life: [1., 2.],
            scatter: 0.012,
            alpha: 0.55,
            hue_jitter: 18.,
        }
    }
}

fn default_types() -> Vec<String> {
    ["diamond", "hex", "tri", "circle", "star", "ring"]
        .iter()
        .map(|it| (*it).to_owned())
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ShapeKind {
    Diamond,
    Square,
    Pentagon,
    Hex,
    Octagon,
    Tri,
    Circle,
    Ring,
    Star,
    Star4,
    Cross,
    Dart,
    Capsule,
}

impl ShapeKind {
    fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "diamond" => Some(Self::Diamond),
            "square" | "rect" => Some(Self::Square),
            "pentagon" => Some(Self::Pentagon),
            "hex" | "hexagon" => Some(Self::Hex),
            "octagon" => Some(Self::Octagon),
            "tri" | "triangle" => Some(Self::Tri),
            "circle" | "dot" => Some(Self::Circle),
            "ring" => Some(Self::Ring),
            "star" | "star5" => Some(Self::Star),
            "star4" | "star-4" => Some(Self::Star4),
            "cross" | "plus" => Some(Self::Cross),
            "dart" | "arrow" => Some(Self::Dart),
            "capsule" | "pill" => Some(Self::Capsule),
            _ => None,
        }
    }
}

fn parse_kinds(types: &[String]) -> Vec<ShapeKind> {
    let kinds = types.iter().filter_map(|it| ShapeKind::parse(it)).collect::<Vec<_>>();
    if kinds.is_empty() {
        vec![
            ShapeKind::Diamond,
            ShapeKind::Hex,
            ShapeKind::Tri,
            ShapeKind::Circle,
            ShapeKind::Star,
            ShapeKind::Ring,
        ]
    } else {
        kinds
    }
}

/// 轻量 xorshift 随机数，避免依赖线程 RNG（需保持对象可跨线程持有）。
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x >> 32) as u32
    }

    fn unit(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    fn range(&mut self, a: f32, b: f32) -> f32 {
        if b <= a {
            a
        } else {
            a + (b - a) * self.unit()
        }
    }

    fn range_u32(&mut self, a: u32, b: u32) -> u32 {
        if b <= a {
            a
        } else {
            a + self.next_u32() % (b - a + 1)
        }
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[(self.next_u32() as usize) % items.len()]
    }
}

/// 世界坐标空间：x ∈ [-1, 1]，y ∈ [-top, top]。
#[derive(Clone, Copy)]
struct Space {
    w: f32,
    h: f32,
    top: f32,
    unit: f32,
    px: f32,
}

impl Space {
    fn new(viewport: (i32, i32, i32, i32)) -> Self {
        let w = viewport.2.max(1) as f32;
        let h = viewport.3.max(1) as f32;
        Self {
            w,
            h,
            top: h / w,
            unit: 2. / w,
            px: h / 1080.,
        }
    }

    /// 归一化坐标 → 世界坐标
    fn point(&self, x: f32, y: f32) -> Vec2 {
        vec2(x * 2. - 1., (y * 2. - 1.) * self.top)
    }

    /// 占短边比例 → 世界坐标长度
    fn frac(&self, f: f32) -> f32 {
        f * self.w.min(self.h) * self.unit
    }

    /// 1080p 像素 → 世界坐标长度
    fn pxs(&self, p: f32) -> f32 {
        (p * self.px * self.unit).max(1e-4)
    }
}

struct Star {
    x: f32,
    y: f32,
    r: f32,
    phase: f32,
    speed: f32,
    hue: f32,
}

struct Ambient {
    x: f32,
    y: f32,
    r: f32,
    kind: ShapeKind,
    rot: f32,
    spin: f32,
    phase: f32,
    speed: f32,
    hue: f32,
    dx: f32,
    dy: f32,
}

struct Shape {
    x: f32,
    y: f32,
    r0: f32,
    r1: f32,
    t0: f32,
    life: f32,
    spin: f32,
    rot: f32,
    kind: ShapeKind,
    hue: f32,
    alpha: f32,
}

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    t0: f32,
    life: f32,
    r: f32,
    hue: f32,
    drag: f32,
    alpha: f32,
}

struct Ring {
    x: f32,
    y: f32,
    t0: f32,
    delay: f32,
    life: f32,
    r0: f32,
    r1: f32,
    hue: f32,
    width: f32,
    alpha: f32,
}

/// 随机游走的漂浮图形。
struct Floater {
    x: f32,
    y: f32,
    dir: f32,
    speed: f32,
    rot: f32,
    spin: f32,
    r: f32,
    kind: ShapeKind,
    hue: f32,
    trail_acc: f32,
    wander_v: f32,
}

/// 漂浮图形留下的小圆点。
struct Trail {
    x: f32,
    y: f32,
    r: f32,
    t0: f32,
    life: f32,
    hue: f32,
    alpha: f32,
}

/// 实时演算背景：解析 `.pbfx` 并逐帧渲染到一张背景纹理。
pub struct RealtimeBackground {
    fx: RealtimeFx,
    burst_kinds: Vec<ShapeKind>,
    ambient_kinds: Vec<ShapeKind>,

    target: RenderTarget,
    texture: SafeTexture,
    viewport: (i32, i32, i32, i32),
    material: Material,

    gradient: SafeTexture,
    gradient_image: Image,
    gradient_t: Vec<f32>,
    gradient_size: u16,
    last_drift: f32,
    bg_colors: [Color; 3],

    stars: Vec<Star>,
    ambients: Vec<Ambient>,
    shapes: Vec<Shape>,
    particles: Vec<Particle>,
    rings: Vec<Ring>,
    floaters: Vec<Floater>,
    trails: Vec<Trail>,

    rng: Rng,
    last_time: f32,
    next_beat: f32,
    pending_burst: bool,
}

impl RealtimeBackground {
    /// 从 `.pbfx` 内容创建实时背景。
    pub fn load(data: &[u8], viewport: (i32, i32, i32, i32)) -> Result<Self> {
        anyhow::ensure!(viewport.2 > 0 && viewport.3 > 0, "invalid viewport size");
        let fx: RealtimeFx = serde_json::from_slice(data).context("failed to parse realtime effect file")?;
        Ok(Self::from_fx(fx, viewport))
    }

    /// 用已经解析好的参数创建背景。
    pub fn from_fx(mut fx: RealtimeFx, viewport: (i32, i32, i32, i32)) -> Self {
        sanitize_fx(&mut fx);
        let viewport = (viewport.0, viewport.1, viewport.2.max(1), viewport.3.max(1));

        let burst_kinds = parse_kinds(&fx.burst.types);
        let ambient_kinds = parse_kinds(&fx.ambient.types);

        let target = render_target(viewport.2 as u32, viewport.3 as u32);
        target.texture.set_filter(FilterMode::Linear);
        let texture = SafeTexture::from(target.texture);

        let material = load_material(
            FX_VERTEX,
            FX_FRAGMENT,
            MaterialParams {
                uniforms: vec![("brightness".to_owned(), UniformType::Float1)],
                pipeline_params: PipelineParams {
                    color_blend: Some(BlendState::new(Equation::Add, BlendFactor::Value(BlendValue::SourceAlpha), BlendFactor::One)),
                    alpha_blend: Some(BlendState::new(Equation::Add, BlendFactor::Zero, BlendFactor::One)),
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .expect("failed to create realtime fx material");
        material.set_uniform("brightness", fx.background.brightness.clamp(0.1, 4.));

        let gradient_size = 64u16;
        let n = gradient_size as usize;
        let bg_radius = fx.background.radius.clamp(0.05, 4.);
        let mut gradient_t = vec![0f32; n * n];
        for y in 0..n {
            for x in 0..n {
                let u = (x as f32 + 0.5) / n as f32 - 0.5;
                let v = (y as f32 + 0.5) / n as f32 - 0.42;
                gradient_t[y * n + x] = ((u * u + v * v).sqrt() / bg_radius).clamp(0., 1.);
            }
        }
        let gradient_image = Image {
            bytes: vec![0; n * n * 4],
            width: gradient_size,
            height: gradient_size,
        };
        let gradient_tex = Texture2D::from_image(&gradient_image);
        gradient_tex.set_filter(FilterMode::Linear);
        let gradient = SafeTexture::from(gradient_tex);

        let bg_colors = [
            parse_hex(&fx.background.inner),
            parse_hex(&fx.background.mid),
            parse_hex(&fx.background.outer),
        ];

        let seed = (get_time() * 1_000_000.) as u64;
        let mut this = Self {
            fx,
            burst_kinds,
            ambient_kinds,
            target,
            texture,
            viewport,
            material,
            gradient,
            gradient_image,
            gradient_t,
            gradient_size,
            last_drift: f32::NAN,
            bg_colors,
            stars: Vec::new(),
            ambients: Vec::new(),
            shapes: Vec::new(),
            particles: Vec::new(),
            rings: Vec::new(),
            floaters: Vec::new(),
            trails: Vec::new(),
            rng: Rng::new(seed),
            last_time: 0.,
            next_beat: 0.,
            pending_burst: false,
        };
        this.init_stars();
        this.init_ambients();
        this.init_floaters();
        this.next_beat = 0.;
        this.last_time = 0.;
        this
    }

    /// 背景纹理（渲染目标尺寸变化时会重建，调用方每帧重新获取）。
    pub fn texture(&self) -> SafeTexture {
        self.texture.clone()
    }

    /// 原地切换效果参数（用于 `.cfg` 播放列表切换），会清空当前粒子并重建环境图形。
    pub fn set_fx(&mut self, mut fx: RealtimeFx) {
        sanitize_fx(&mut fx);
        self.burst_kinds = parse_kinds(&fx.burst.types);
        self.ambient_kinds = parse_kinds(&fx.ambient.types);
        self.bg_colors = [
            parse_hex(&fx.background.inner),
            parse_hex(&fx.background.mid),
            parse_hex(&fx.background.outer),
        ];
        self.material.set_uniform("brightness", fx.background.brightness.clamp(0.1, 4.));
        self.fx = fx;
        self.last_drift = f32::NAN; // 下一帧重算渐变
        self.shapes.clear();
        self.particles.clear();
        self.rings.clear();
        self.pending_burst = true; // 切换后立刻来一次释放，避免新效果开场空白
        self.init_stars();
        self.init_ambients();
        self.init_floaters();
    }

    fn init_stars(&mut self) {
        self.stars.clear();
        if !self.fx.stars.enabled {
            return;
        }
        let count = (self.viewport.2 as f32 * self.viewport.3 as f32 / 14000. * self.fx.stars.density).round() as usize;
        let radius = self.fx.stars.radius;
        let twinkle = self.fx.stars.twinkle;
        for _ in 0..count.max(1) {
            let r = self.rng.range(radius[0], radius[1]);
            let speed = self.rng.range(twinkle[0], twinkle[1]);
            let x = self.rng.unit();
            let y = self.rng.unit();
            let phase = self.rng.range(0., TAU);
            let hue = self.rng_hue();
            self.stars.push(Star { x, y, r, phase, speed, hue });
        }
    }

    fn init_ambients(&mut self) {
        self.ambients.clear();
        if !self.fx.ambient.enabled {
            return;
        }
        let count = self.fx.ambient.count.max(0.).round() as usize;
        let cfg = self.fx.ambient.clone();
        for _ in 0..count {
            let kind = self.rng.pick(&self.ambient_kinds);
            let r = self.rng.range(cfg.radius[0], cfg.radius[1]);
            let spin = self.rng.range(-cfg.spin, cfg.spin);
            let speed = self.rng.range(cfg.breathe[0], cfg.breathe[1]);
            let x = self.rng.unit();
            let y = self.rng.unit();
            let phase = self.rng.range(0., TAU);
            let hue = self.rng_hue();
            let dx = if self.rng.unit() < 0.5 { -cfg.drift } else { cfg.drift };
            let dy = if self.rng.unit() < 0.5 { -cfg.drift } else { cfg.drift };
            let rot = self.rng.range(0., TAU);
            self.ambients.push(Ambient {
                x,
                y,
                r,
                kind,
                rot,
                spin,
                phase,
                speed,
                hue,
                dx,
                dy,
            });
        }
    }

    fn rng_hue(&mut self) -> f32 {
        let base = self.rng.pick(&self.fx.palette);
        (base + self.rng.range(-14., 14.)).rem_euclid(360.)
    }

    /// 初始化漂浮图形（随机游走 + 拖尾）。
    fn init_floaters(&mut self) {
        self.floaters.clear();
        self.trails.clear();
        let cfg = self.fx.floater.clone();
        if !cfg.enabled {
            return;
        }
        let kinds = parse_kinds(&cfg.types);
        let count = cfg.count.max(0.).round() as usize;
        for i in 0..count {
            let angle = i as f32 / count.max(1) as f32 * TAU + self.rng.range(0., 0.6);
            let r = self.rng.range(cfg.radius[0], cfg.radius[1]);
            let speed = self.rng.range(cfg.speed[0], cfg.speed[1]);
            let dir = self.rng.range(0., TAU);
            let spin = self.rng.range(-cfg.spin, cfg.spin);
            let kind = self.rng.pick(&kinds);
            let hue = self.rng_hue();
            let rot = self.rng.range(0., TAU);
            self.floaters.push(Floater {
                x: 0.5 + angle.cos() * 0.28,
                y: 0.5 + angle.sin() * 0.28,
                dir,
                speed,
                rot,
                spin,
                r,
                kind,
                hue,
                trail_acc: 0.,
                wander_v: 0.,
            });
        }
    }

    fn beat_interval(&mut self) -> f32 {
        let bpm = self.fx.burst.bpm.max(1.);
        let v = self.fx.burst.bpm_variance;
        (60. / bpm) * self.rng.range(1. - v, 1. + v)
    }

    /// 更新并渲染背景，应在绘制背景前每帧调用一次。
    ///
    /// `time` 是以「音频开始播放 + 谱面/info 延迟」为 0 点的谱面时间，可为负：
    /// 负值表示音频还没到拍点起点（前置空白期），此时只画静态背景、不计拍。
    pub fn update(&mut self, time: f32, viewport: (i32, i32, i32, i32)) {
        if viewport.2 <= 0 || viewport.3 <= 0 {
            return;
        }
        self.ensure_target(viewport);

        if time < 0. {
            // 尚未进入谱面拍点：保持首帧静态画面，避免前奏空白期就提前爆发
            self.render(0.);
            return;
        }

        // 时间回退（重开 / 跳转）时清空，避免残留
        if time < self.last_time - 0.01 {
            self.shapes.clear();
            self.particles.clear();
            self.rings.clear();
            self.next_beat = time;
        }
        let dt = (time - self.last_time).clamp(0., 0.1);
        self.last_time = time;

        if self.fx.burst.enabled {
            let mut guard = 0;
            while time >= self.next_beat && guard < 32 {
                let beat = self.next_beat;
                self.spawn_beat(beat);
                self.next_beat += self.beat_interval();
                guard += 1;
            }
            // 跳转/长卡顿后不要一次性补发大量释放，直接对齐当前时间
            if time - self.next_beat > 1. {
                self.next_beat = time;
            }
        }

        if self.pending_burst {
            self.pending_burst = false;
            if self.fx.burst.enabled && self.shapes.is_empty() {
                self.spawn_beat(time);
                self.next_beat = time + self.beat_interval();
            }
        }

        // 环境几何体：呼吸 / 自转 / 漂移（碰边反向，避免瞬移）
        for a in &mut self.ambients {
            a.phase += dt * a.speed;
            a.rot += dt * a.spin;
            a.x += a.dx * dt;
            a.y += a.dy * dt;
            if a.x < -0.1 {
                a.x = -0.1;
                a.dx = a.dx.abs();
            } else if a.x > 1.1 {
                a.x = 1.1;
                a.dx = -a.dx.abs();
            }
            if a.y < -0.1 {
                a.y = -0.1;
                a.dy = a.dy.abs();
            } else if a.y > 1.1 {
                a.y = 1.1;
                a.dy = -a.dy.abs();
            }
        }

        // 漂浮图形：平滑随机游走 + 小圆拖尾（拖尾带随机散开）
        if self.fx.floater.enabled {
            let cfg = self.fx.floater.clone();
            let limit = self.fx.limit.trails;
            let spacing = cfg.trail.spacing.max(0.002);
            for i in 0..self.floaters.len() {
                let jerk = self.rng.range(-cfg.wander_jerk, cfg.wander_jerk) * dt;
                let f = &mut self.floaters[i];
                // 角速度连续变化（有阻尼），方向不会一帧一跳
                f.wander_v = (f.wander_v + jerk).clamp(-cfg.wander, cfg.wander);
                f.wander_v *= (1. - 0.8 * dt).max(0.);
                f.dir = (f.dir + f.wander_v * dt).rem_euclid(TAU);
                f.rot += f.spin * dt;

                // 靠近边缘时平滑转向画面内侧，而不是硬反弹
                let margin = 0.12;
                let mut steer_x = 0.;
                let mut steer_y = 0.;
                if f.x < margin {
                    steer_x += (margin - f.x) / margin;
                } else if f.x > 1. - margin {
                    steer_x -= (f.x - (1. - margin)) / margin;
                }
                if f.y < margin {
                    steer_y += (margin - f.y) / margin;
                } else if f.y > 1. - margin {
                    steer_y -= (f.y - (1. - margin)) / margin;
                }
                if steer_x != 0. || steer_y != 0. {
                    let target = steer_y.atan2(steer_x);
                    f.dir = lerp_angle(f.dir, target, 1. - (-6. * dt).exp());
                }

                let dx = f.dir.cos() * f.speed * dt;
                let dy = f.dir.sin() * f.speed * dt;
                f.x = (f.x + dx).clamp(-0.02, 1.02);
                f.y = (f.y + dy).clamp(-0.02, 1.02);

                if cfg.trail.enabled {
                    f.trail_acc += (dx * dx + dy * dy).sqrt();
                    while f.trail_acc >= spacing {
                        f.trail_acc -= spacing;
                        // 让轨迹散成一小团，而不是一条线
                        let scatter_angle = self.rng.range(0., TAU);
                        let scatter_dist = self.rng.range(0., cfg.trail.scatter);
                        let x = f.x + scatter_angle.cos() * scatter_dist;
                        let y = f.y + scatter_angle.sin() * scatter_dist;
                        let r = self.rng.range(cfg.trail.radius[0], cfg.trail.radius[1]);
                        let life = self.rng.range(cfg.trail.life[0], cfg.trail.life[1]).max(0.05);
                        let jitter = self.rng.range(-cfg.trail.hue_jitter, cfg.trail.hue_jitter);
                        let hue = (f.hue + jitter).rem_euclid(360.);
                        push_capped(
                            &mut self.trails,
                            Trail {
                                x,
                                y,
                                r,
                                t0: time,
                                life,
                                hue,
                                alpha: cfg.trail.alpha,
                            },
                            limit,
                        );
                    }
                }
            }
        }

        // 粒子：积分 + 超出画面立即回收
        let mut i = self.particles.len();
        while i > 0 {
            i -= 1;
            let p = &mut self.particles[i];
            if time - p.t0 >= p.life {
                self.particles.swap_remove(i);
                continue;
            }
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            let decay = (-p.drag * dt).exp();
            p.vx *= decay;
            p.vy *= decay;
            if p.x < -0.15 || p.x > 1.15 || p.y < -0.15 || p.y > 1.15 {
                self.particles.swap_remove(i);
            }
        }

        // 图形与光环：到期立即回收
        self.shapes.retain(|it| {
            let age = time - it.t0;
            (0. ..it.life).contains(&age)
        });
        self.rings.retain(|it| {
            let age = time - it.t0 - it.delay;
            (-it.delay..it.life).contains(&age)
        });
        self.trails.retain(|it| (0. ..it.life).contains(&(time - it.t0)));

        self.render(time);
    }

    fn ensure_target(&mut self, viewport: (i32, i32, i32, i32)) {
        if viewport == self.viewport || viewport.2 <= 0 || viewport.3 <= 0 {
            return;
        }
        self.viewport = viewport;
        let target = render_target(viewport.2 as u32, viewport.3 as u32);
        target.texture.set_filter(FilterMode::Linear);
        self.texture = SafeTexture::from(target.texture);
        self.target = target;
        self.init_stars();
    }

    fn spawn_beat(&mut self, t0: f32) {
        let margin = self.fx.burst.margin.clamp(0., 0.49);
        let (cx, cy) = (self.rng.range(margin, 1. - margin), self.rng.range(margin, 1. - margin));
        let base_hue = self.rng.pick(&self.fx.palette);
        let burst = self.fx.burst.clone();
        let ring = self.fx.ring.clone();
        let particle = self.fx.particle.clone();
        let shape_limit = self.fx.limit.shapes;
        let particle_limit = self.fx.limit.particles;
        let ring_limit = self.fx.limit.rings;

        if burst.enabled {
            let n = self.rng.range(burst.count[0], burst.count[1]).round().max(0.) as usize;
            for _ in 0..n {
                let kind = self.rng.pick(&self.burst_kinds);
                let ang = self.rng.range(0., TAU);
                let dist = self.rng.range(0., burst.spread);
                let x = cx + ang.cos() * dist;
                let y = cy + ang.sin() * dist;
                let r0 = self.rng.range(burst.start_radius[0], burst.start_radius[1]);
                let r1 = self.rng.range(burst.end_radius[0], burst.end_radius[1]);
                let life = self.rng.range(burst.lifetime[0], burst.lifetime[1]).max(0.2);
                let spin = self.rng.range(-burst.spin, burst.spin);
                let rot = self.rng.range(0., TAU);
                let alpha = self.rng.range(burst.alpha[0], burst.alpha[1]);
                let hue = (base_hue + self.rng.range(-burst.hue_jitter, burst.hue_jitter)).rem_euclid(360.);
                push_capped(
                    &mut self.shapes,
                    Shape {
                        x,
                        y,
                        r0,
                        r1,
                        t0,
                        life,
                        spin,
                        rot,
                        kind,
                        hue,
                        alpha,
                    },
                    shape_limit,
                );
            }

            if particle.enabled {
                let pn = (n as f32 * particle.per_shape).round().max(0.) as usize;
                for _ in 0..pn {
                    let ang = self.rng.range(0., TAU);
                    let spd = self.rng.range(particle.speed[0], particle.speed[1]);
                    let life = self.rng.range(particle.lifetime[0], particle.lifetime[1]).max(0.2);
                    let r = self.rng.range(particle.radius[0], particle.radius[1]);
                    let drag = self.rng.range(particle.drag[0], particle.drag[1]);
                    let hue = (base_hue + self.rng.range(-particle.hue_jitter, particle.hue_jitter)).rem_euclid(360.);
                    push_capped(
                        &mut self.particles,
                        Particle {
                            x: cx,
                            y: cy,
                            vx: ang.cos() * spd,
                            vy: ang.sin() * spd,
                            t0,
                            life,
                            r,
                            hue,
                            drag,
                            alpha: particle.alpha,
                        },
                        particle_limit,
                    );
                }
            }
        }

        if ring.enabled {
            let n = self.rng.range_u32(ring.count[0], ring.count[1].max(ring.count[0])) as usize;
            for i in 0..n {
                let delay = i as f32 * ring.delay + self.rng.range(0., ring.delay * 0.5);
                let r0 = self.rng.range(ring.start_radius[0], ring.start_radius[1]);
                let r1 = self.rng.range(ring.end_radius[0], ring.end_radius[1]);
                let life = self.rng.range(ring.lifetime[0], ring.lifetime[1]).max(0.2);
                let width = self.rng.range(ring.width[0], ring.width[1]);
                let alpha = self.rng.range(ring.alpha[0], ring.alpha[1]);
                let hue = (base_hue + self.rng.range(-ring.hue_jitter, ring.hue_jitter)).rem_euclid(360.);
                push_capped(
                    &mut self.rings,
                    Ring {
                        x: cx,
                        y: cy,
                        t0,
                        delay,
                        life,
                        r0,
                        r1,
                        hue,
                        width,
                        alpha,
                    },
                    ring_limit,
                );
            }
        }
    }

    fn render(&mut self, time: f32) {
        let viewport = self.viewport;
        let space = Space::new(viewport);
        self.update_gradient(time);

        let target = self.target;
        push_camera_state();
        set_camera(&Camera2D {
            zoom: vec2(1., -(viewport.2 as f32 / viewport.3 as f32)),
            render_target: Some(target),
            ..Default::default()
        });
        clear_background(BLACK);

        // 渐变底色
        draw_texture_ex(
            *self.gradient,
            -1.,
            -space.top,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(2., space.top * 2.)),
                ..Default::default()
            },
        );

        // 星点
        gl_use_material(self.material);
        if self.fx.stars.enabled {
            let alpha = self.fx.stars.alpha;
            for s in &self.stars {
                let tw = 0.3 + 0.7 * (0.5 + 0.5 * (time * s.speed + s.phase).sin());
                let p = space.point(s.x, s.y);
                let r = space.pxs(s.r);
                draw_circle_smooth(p, r, hsl(s.hue, 0.95, 0.7, tw * alpha), circle_segments(&space, r));
            }
        }

        // 呼吸环境几何体
        if self.fx.ambient.enabled {
            let cfg = &self.fx.ambient;
            let thickness = space.pxs(cfg.width);
            for a in &self.ambients {
                let breathe = 1. + 0.28 * a.phase.sin();
                let p = space.point(a.x, a.y);
                let r = space.frac(a.r) * breathe;
                let color = hsl(a.hue, 1., 0.64, cfg.alpha + 0.04 * (breathe - 1.));
                draw_shape_outline(a.kind, p, r, a.rot, thickness, color, circle_segments(&space, r));
            }
        }

        // 扩散光环
        if self.fx.ring.enabled {
            let fade_in = self.fx.ring.fade_in.max(0.001);
            for r in &self.rings {
                let k = (time - r.t0 - r.delay) / r.life;
                if !(0. ..1.).contains(&k) {
                    continue;
                }
                let radius = space.frac(r.r0 + (r.r1 - r.r0) * ease_out_cubic(k));
                let alpha = r.alpha * (1. - k) * (k / fade_in).min(1.);
                let p = space.point(r.x, r.y);
                let thickness = space.pxs(r.width * (1. - k * 0.8));
                draw_circle_stroke(p, radius, thickness, hsl(r.hue, 1., 0.68, alpha), circle_segments(&space, radius));
            }
        }

        // 爆发图形
        for s in &self.shapes {
            let k = (time - s.t0) / s.life;
            if !(0. ..1.).contains(&k) {
                continue;
            }
            let size = space.frac(s.r0 + (s.r1 - s.r0) * ease_out_cubic(k));
            let fade_in = ease_out_cubic((k * 1.6).min(1.));
            let fade_out = 1. - ease_in_out_cubic(((k - 0.72) / 0.28).clamp(0., 1.));
            let alpha = s.alpha * fade_in * fade_out;
            let p = space.point(s.x, s.y);
            let rot = s.rot + s.spin * k * 2.6;
            let color = hsl(s.hue, 1., 0.62, alpha);
            match s.kind {
                ShapeKind::Circle => draw_circle_smooth(p, size, color, circle_segments(&space, size)),
                ShapeKind::Ring => draw_circle_stroke(p, size, (size * 0.12).max(1e-4), color, circle_segments(&space, size)),
                _ => draw_shape_filled(s.kind, p, size, rot, color),
            }
        }

        // 粒子拖尾
        if self.fx.particle.enabled {
            for p in &self.particles {
                let k = (time - p.t0) / p.life;
                if !(0. ..1.).contains(&k) {
                    continue;
                }
                let alpha = (1. - k) * p.alpha * (k * 6.).min(1.);
                let pos = space.point(p.x, p.y);
                let r = space.pxs(p.r * (1. - k * 0.6));
                draw_circle_smooth(pos, r, hsl(p.hue, 1., 0.72, alpha), circle_segments(&space, r));
            }
        }

        // 漂浮图形及其小圆拖尾
        if self.fx.floater.enabled {
            let cfg = &self.fx.floater;
            for t in &self.trails {
                let k = (time - t.t0) / t.life;
                if !(0. ..1.).contains(&k) {
                    continue;
                }
                let p = space.point(t.x, t.y);
                let r = space.frac(t.r);
                let color = hsl(t.hue, 1., 0.72, t.alpha * (1. - k));
                draw_circle_smooth(p, r, color, circle_segments(&space, r));
            }
            for f in &self.floaters {
                let p = space.point(f.x, f.y);
                let r = space.frac(f.r);
                let fill = hsl(f.hue, 1., 0.62, cfg.alpha * 0.5);
                match f.kind {
                    ShapeKind::Circle => draw_circle_smooth(p, r, fill, circle_segments(&space, r)),
                    ShapeKind::Ring => draw_circle_stroke(p, r, (r * 0.12).max(1e-4), fill, circle_segments(&space, r)),
                    _ => {
                        draw_shape_filled(f.kind, p, r, f.rot, fill);
                        draw_shape_outline(f.kind, p, r, f.rot, space.pxs(cfg.width), hsl(f.hue, 1., 0.7, cfg.alpha), circle_segments(&space, r));
                    }
                }
            }
        }

        gl_use_default_material();
        pop_camera_state();
    }

    /// 只有色相漂移变化时才重算渐变纹理，避免无谓的像素上传。
    fn update_gradient(&mut self, time: f32) {
        let drift = self.fx.background.hue_drift * time;
        if drift.is_finite() && (drift - self.last_drift).abs() < 0.25 {
            return;
        }
        self.last_drift = drift;

        let inner = rotate_hue(self.bg_colors[0], drift);
        let mid = rotate_hue(self.bg_colors[1], drift);
        let outer = rotate_hue(self.bg_colors[2], drift);
        let mut lut = [[0u8; 4]; 256];
        for (i, slot) in lut.iter_mut().enumerate() {
            let c = gradient_color(inner, mid, outer, i as f32 / 255.);
            *slot = [(c.r * 255.) as u8, (c.g * 255.) as u8, (c.b * 255.) as u8, 255];
        }

        let bytes = &mut self.gradient_image.bytes;
        for (i, t) in self.gradient_t.iter().enumerate() {
            let idx = (t * 255.).round().clamp(0., 255.) as usize;
            bytes[i * 4..i * 4 + 4].copy_from_slice(&lut[idx]);
        }
        let image = Image {
            bytes: std::mem::take(bytes),
            width: self.gradient_size,
            height: self.gradient_size,
        };
        self.gradient.update(&image);
        self.gradient_image.bytes = image.bytes;
    }
}

impl Drop for RealtimeBackground {
    fn drop(&mut self) {
        self.material.delete();
    }
}

/// 修正非法/缺失的参数，保证渲染不会 panic。
fn sanitize_fx(fx: &mut RealtimeFx) {
    if fx.palette.is_empty() {
        fx.palette = DEFAULT_PALETTE.to_vec();
    }
    if !(fx.burst.bpm > 0.) {
        fx.burst.bpm = 64.;
    }
    fx.burst.bpm_variance = fx.burst.bpm_variance.clamp(0., 0.9);
    fx.limit.shapes = fx.limit.shapes.max(1);
    fx.limit.particles = fx.limit.particles.max(1);
    fx.limit.rings = fx.limit.rings.max(1);
}

fn push_capped<T>(pool: &mut Vec<T>, item: T, max: usize) {
    if pool.len() >= max {
        pool.remove(0);
    }
    pool.push(item);
}

fn ease_out_cubic(x: f32) -> f32 {
    1. - (1. - x).powi(3)
}

fn ease_in_out_cubic(x: f32) -> f32 {
    if x < 0.5 {
        4. * x * x * x
    } else {
        1. - (-2. * x + 2.).powi(3) / 2.
    }
}

fn hsl(h: f32, s: f32, l: f32, a: f32) -> Color {
    let h = h.rem_euclid(360.) / 60.;
    let c = (1. - (2. * l - 1.).abs()) * s;
    let x = c * (1. - (h % 2. - 1.).abs());
    let (r, g, b) = match h as i32 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let m = l - c / 2.;
    Color::new(r + m, g + m, b + m, a)
}

fn rgb_to_hsl(c: Color) -> (f32, f32, f32) {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let l = (max + min) / 2.;
    let d = max - min;
    if d < 1e-6 {
        return (0., 0., l);
    }
    let s = if l > 0.5 { d / (2. - max - min) } else { d / (max + min) };
    let h = if max == c.r {
        ((c.g - c.b) / d + if c.g < c.b { 6. } else { 0. }) * 60.
    } else if max == c.g {
        ((c.b - c.r) / d + 2.) * 60.
    } else {
        ((c.r - c.g) / d + 4.) * 60.
    };
    (h, s, l)
}

fn rotate_hue(color: Color, degrees: f32) -> Color {
    if degrees == 0. {
        return color;
    }
    let (h, s, l) = rgb_to_hsl(color);
    hsl(h + degrees, s, l, color.a)
}

fn parse_hex(value: &str) -> Color {
    let text = value.trim().trim_start_matches('#');
    if text.len() >= 6 && text.is_ascii() {
        let channel = |i: usize| u8::from_str_radix(&text[i..i + 2], 16).ok().map(|v| v as f32 / 255.);
        if let (Some(r), Some(g), Some(b)) = (channel(0), channel(2), channel(4)) {
            return Color::new(r, g, b, 1.);
        }
    }
    Color::new(0.02, 0.02, 0.03, 1.)
}

fn gradient_color(inner: Color, mid: Color, outer: Color, t: f32) -> Color {
    if t < 0.55 {
        lerp_color(inner, mid, t / 0.55)
    } else {
        lerp_color(mid, outer, (t - 0.55) / 0.45)
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::new(a.r + (b.r - a.r) * t, a.g + (b.g - a.g) * t, a.b + (b.b - a.b) * t, a.a + (b.a - a.a) * t)
}

/// 角度插值（走最短弧，避免 ±π 处突跳）。
fn lerp_angle(from: f32, to: f32, t: f32) -> f32 {
    let mut diff = (to - from).rem_euclid(TAU);
    if diff > PI {
        diff -= TAU;
    }
    from + diff * t
}

/// 生成图形顶点（不含圆形，圆形单独绘制）。
fn shape_vertices(kind: ShapeKind, r: f32, rot: f32, out: &mut [Vec2; 24]) -> usize {
    let (sin, cos) = rot.sin_cos();
    let rotate = |x: f32, y: f32| vec2(x * cos - y * sin, x * sin + y * cos);
    let regular = |n: usize, inner: f32, out: &mut [Vec2; 24]| {
        let count = if inner > 0. { n * 2 } else { n };
        for i in 0..count {
            let a = i as f32 / count as f32 * TAU - PI / 2.;
            let radius = if inner > 0. && i % 2 == 1 { r * inner } else { r };
            out[i] = rotate(a.cos() * radius, a.sin() * radius);
        }
        count
    };
    match kind {
        ShapeKind::Circle | ShapeKind::Ring => 0,
        ShapeKind::Diamond => {
            let pts = [(0., -1.), (0.72, 0.), (0., 1.), (-0.72, 0.)];
            for (i, (x, y)) in pts.iter().enumerate() {
                out[i] = rotate(x * r, y * r);
            }
            4
        }
        ShapeKind::Tri => regular(3, 0., out),
        ShapeKind::Square => regular(4, 0., out),
        ShapeKind::Pentagon => regular(5, 0., out),
        ShapeKind::Hex => regular(6, 0., out),
        ShapeKind::Octagon => regular(8, 0., out),
        ShapeKind::Star => regular(5, 0.45, out),
        ShapeKind::Star4 => regular(4, 0.3, out),
        ShapeKind::Cross => {
            let t = r * 0.34;
            let pts = [
                (-t, -r),
                (t, -r),
                (t, -t),
                (r, -t),
                (r, t),
                (t, t),
                (t, r),
                (-t, r),
                (-t, t),
                (-r, t),
                (-r, -t),
                (-t, -t),
            ];
            for (i, (x, y)) in pts.iter().enumerate() {
                out[i] = rotate(*x, *y);
            }
            12
        }
        ShapeKind::Dart => {
            let pts = [(0., -1.), (0.85, 0.55), (0., 0.2), (-0.85, 0.55)];
            for (i, (x, y)) in pts.iter().enumerate() {
                out[i] = rotate(x * r, y * r);
            }
            4
        }
        ShapeKind::Capsule => {
            // 横向胶囊：两段半圆 + 两条直边
            let half = r * 1.6;
            let steps = 8;
            let mut n = 0;
            for k in 0..=steps {
                let a = -PI / 2. + PI * k as f32 / steps as f32;
                out[n] = rotate(half + a.cos() * r, a.sin() * r);
                n += 1;
            }
            for k in 0..=steps {
                let a = PI / 2. + PI * k as f32 / steps as f32;
                out[n] = rotate(-half + a.cos() * r, a.sin() * r);
                n += 1;
            }
            n
        }
    }
}

/// 按屏幕像素半径决定圆的分段数，避免大半径下出现明显的正多边形。
fn circle_segments(space: &Space, radius: f32) -> usize {
    let px = radius / space.unit;
    (px * 0.35).clamp(16., 160.) as usize
}

/// 平滑实心圆（macroquad 的 draw_circle 固定 20 段，这里按半径自适应）。
fn draw_circle_smooth(center: Vec2, r: f32, color: Color, segments: usize) {
    if r <= 0. {
        return;
    }
    let step = TAU / segments as f32;
    let mut prev = center + vec2(r, 0.);
    for i in 1..=segments {
        let a = step * i as f32;
        let cur = center + vec2(a.cos(), a.sin()) * r;
        draw_triangle(center, prev, cur, color);
        prev = cur;
    }
}

/// 平滑圆环描边（用两圈顶点构成环形三角带）。
fn draw_circle_stroke(center: Vec2, r: f32, thickness: f32, color: Color, segments: usize) {
    let half = (thickness * 0.5).max(1e-5);
    let inner = (r - half).max(0.);
    let outer = r + half;
    if outer <= 0. {
        return;
    }
    let step = TAU / segments as f32;
    let mut prev_i = center + vec2(inner, 0.);
    let mut prev_o = center + vec2(outer, 0.);
    for i in 1..=segments {
        let a = step * i as f32;
        let dir = vec2(a.cos(), a.sin());
        let cur_i = center + dir * inner;
        let cur_o = center + dir * outer;
        draw_triangle(prev_i, prev_o, cur_o, color);
        draw_triangle(prev_i, cur_o, cur_i, color);
        prev_i = cur_i;
        prev_o = cur_o;
    }
}

fn draw_shape_filled(kind: ShapeKind, center: Vec2, r: f32, rot: f32, color: Color) {
    let mut pts = [Vec2::ZERO; 24];
    let n = shape_vertices(kind, r, rot, &mut pts);
    for i in 0..n {
        draw_triangle(center, center + pts[i], center + pts[(i + 1) % n], color);
    }
}

fn draw_shape_outline(kind: ShapeKind, center: Vec2, r: f32, rot: f32, thickness: f32, color: Color, segments: usize) {
    if matches!(kind, ShapeKind::Circle | ShapeKind::Ring) {
        draw_circle_stroke(center, r, thickness, color, segments);
        return;
    }
    let mut pts = [Vec2::ZERO; 24];
    let n = shape_vertices(kind, r, rot, &mut pts);
    for i in 0..n {
        let a = center + pts[i];
        let b = center + pts[(i + 1) % n];
        draw_line(a.x, a.y, b.x, b.y, thickness, color);
    }
}
