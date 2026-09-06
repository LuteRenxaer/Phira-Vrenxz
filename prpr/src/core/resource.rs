use super::{MSRenderTarget, Matrix, Point, NOTE_WIDTH_RATIO_BASE};
use crate::{
    config::Config,
    ext::{create_audio_manger, nalgebra_to_glm, SafeTexture},
    fs::FileSystem,
    info::ChartInfo,
    particle::{AtlasConfig, ColorCurve, Emitter, EmitterConfig},
};
use anyhow::{bail, Context, Result};
use macroquad::prelude::*;
use miniquad::{
    gl::{GLuint, GL_LINEAR},
    Texture, TextureWrap,
};
use sasa::{AudioClip, AudioManager, Sfx};
use serde::Deserialize;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    ops::DerefMut,
    path::Path,
    sync::atomic::AtomicU32,
};

pub const MAX_SIZE: usize = 64;
pub static DPI_VALUE: AtomicU32 = AtomicU32::new(250);
pub const BUFFER_SIZE: usize = 1024;

/// 低分辨率 Note 贴图的缩放因子（宽高各缩小为 1/factor）
const NOTE_LOW_RES_FACTOR: u32 = 2;

/// 将贴图按 `factor` 等比例降采样（盒式平均），返回低分辨率版本。
///
/// 尺寸不足 factor 的贴图原样返回，避免放大。
fn downscale_texture(tex: &SafeTexture, factor: u32) -> SafeTexture {
    let f = factor.max(1) as usize;
    let (w, h) = (tex.width() as usize, tex.height() as usize);
    if w <= f || h <= f {
        return tex.clone();
    }
    let img = tex.get_texture_data();
    let (nw, nh) = (w / f, h / f);
    let src = &img.bytes;
    let mut dst = vec![0u8; nw * nh * 4];
    for y in 0..nh {
        let y0 = y * f;
        let y1 = (y0 + f).min(h);
        for x in 0..nw {
            let x0 = x * f;
            let x1 = (x0 + f).min(w);
            let (mut r, mut g, mut b, mut a, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for sy in y0..y1 {
                let row = sy * w;
                for sx in x0..x1 {
                    let i = (row + sx) * 4;
                    r += src[i] as u32;
                    g += src[i + 1] as u32;
                    b += src[i + 2] as u32;
                    a += src[i + 3] as u32;
                    n += 1;
                }
            }
            let i = (y * nw + x) * 4;
            dst[i] = (r / n) as u8;
            dst[i + 1] = (g / n) as u8;
            dst[i + 2] = (b / n) as u8;
            dst[i + 3] = (a / n) as u8;
        }
    }
    SafeTexture::from(Texture2D::from_rgba8(nw as u16, nh as u16, &dst)).with_filter(GL_LINEAR)
}

#[inline]
fn default_scale() -> f32 {
    1.
}

#[inline]
fn default_duration() -> f32 {
    0.5
}

#[inline]
fn default_perfect() -> u32 {
    0xe1ffec9f
}

#[inline]
fn default_good() -> u32 {
    0xebb4e1ff
}

#[inline]
fn default_tinted() -> bool {
    true
}

#[allow(dead_code)]
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ResPackInfo {
    pub name: String,
    pub author: String,

    pub hit_fx: (u32, u32),
    #[serde(default = "default_duration")]
    pub hit_fx_duration: f32,
    #[serde(default = "default_scale")]
    pub hit_fx_scale: f32,
    #[serde(default)]
    pub hit_fx_rotate: bool,
    #[serde(default)]
    pub hide_particles: bool,
    #[serde(default = "default_tinted")]
    pub hit_fx_tinted: bool,

    pub hold_atlas: (u16, u16),
    #[serde(rename = "holdAtlasMH")]
    pub hold_atlas_mh: (u16, u16),

    #[serde(default)]
    pub hold_keep_head: bool,
    #[serde(default)]
    pub hold_repeat: bool,
    #[serde(default)]
    pub hold_compact: bool,

    #[serde(default = "default_perfect")]
    color_perfect: u32,
    #[serde(default = "default_good")]
    color_good: u32,

    #[serde(default)]
    pub description: String,
}

fn parse_color_guess_alpha(c: u32) -> Color {
    if c > 0xffffff {
        Color::from_hex_argb(c)
    } else {
        Color::from_hex_rgb(c)
    }
}

impl ResPackInfo {
    pub fn verify(&self) -> Result<()> {
        if self.name.is_empty() {
            bail!("empty name");
        }
        if self.name.len() > 100 {
            bail!("name too long");
        }
        if self.description.len() > 1000 {
            bail!("description too long");
        }
        if !(1..=10240).contains(&self.hit_fx.0.saturating_mul(self.hit_fx.1)) {
            bail!("Invalid hit_fx");
        }
        Ok(())
    }
    pub fn color_perfect(&self) -> Color {
        parse_color_guess_alpha(self.color_perfect)
    }

    pub fn color_good(&self) -> Color {
        parse_color_guess_alpha(self.color_good)
    }

    pub fn fx_perfect(&self) -> Color {
        if self.hit_fx_tinted {
            self.color_perfect()
        } else {
            WHITE
        }
    }

    pub fn fx_good(&self) -> Color {
        if self.hit_fx_tinted {
            self.color_good()
        } else {
            WHITE
        }
    }
}

#[derive(Clone)]
pub struct NoteStyle {
    pub click: SafeTexture,
    pub hold: SafeTexture,
    pub flick: SafeTexture,
    pub drag: SafeTexture,
    pub hold_body: Option<SafeTexture>,
    pub hold_atlas: (u16, u16),
}

impl NoteStyle {
    pub fn verify(&self) -> Result<()> {
        if self.hold_atlas.0.saturating_add(self.hold_atlas.1) as f32 >= self.hold.height() {
            bail!("Invalid atlas");
        }
        Ok(())
    }

    /// 根据当前贴图重建 hold_body（Repeat 环绕贴图，去掉头部/尾部行）
    fn build_hold_body(&mut self) {
        let pixels = self.hold.get_texture_data();
        let width = self.hold.width() as u16;
        let height = self.hold.height() as u16;
        let atlas = self.hold_atlas;
        let res = Texture2D::from_rgba8(
            width,
            height - atlas.0 - atlas.1,
            &pixels.bytes[(atlas.0 as usize * width as usize * 4)..(pixels.bytes.len() - atlas.1 as usize * width as usize * 4)],
        );
        let context = unsafe { get_internal_gl() }.quad_context;
        res.raw_miniquad_texture_handle().set_wrap(context, TextureWrap::Repeat);
        self.hold_body = Some(res.into());
    }

    /// 生成低分辨率版本：所有贴图按 `factor` 等比例缩小，atlas 偏移同步缩放。
    ///
    /// 用于屏幕上 Note 数量较多时降分辨率渲染，降低纹理采样带宽。
    pub fn downscaled(&self, hold_repeat: bool, factor: u32) -> Self {
        let factor = factor.max(1);
        let mut low = Self {
            click: downscale_texture(&self.click, factor),
            hold: downscale_texture(&self.hold, factor),
            flick: downscale_texture(&self.flick, factor),
            drag: downscale_texture(&self.drag, factor),
            hold_body: self.hold_body.as_ref().map(|it| downscale_texture(it, factor)),
            hold_atlas: (self.hold_atlas.0 / factor as u16, self.hold_atlas.1 / factor as u16),
        };
        if hold_repeat {
            low.build_hold_body();
        }
        low
    }

    #[inline]
    fn to_uv(&self, t: u16) -> f32 {
        t as f32 / self.hold.height()
    }

    pub fn hold_ratio(&self) -> f32 {
        self.hold.height() / self.hold.width()
    }

    pub fn hold_head_rect(&self) -> Rect {
        let sy = self.to_uv(self.hold_atlas.1);
        Rect::new(0., 1. - sy, 1., sy)
    }

    pub fn hold_body_rect(&self) -> Rect {
        let sy = self.to_uv(self.hold_atlas.0);
        let ey = 1. - self.to_uv(self.hold_atlas.1);
        Rect::new(0., sy, 1., ey - sy)
    }

    pub fn hold_tail_rect(&self) -> Rect {
        let ey = self.to_uv(self.hold_atlas.0);
        Rect::new(0., 0., 1., ey)
    }
}

#[derive(Clone)]
pub struct ResourcePack {
    pub info: ResPackInfo,
    pub note_style: NoteStyle,
    pub note_style_mh: NoteStyle,
    /// 低分辨率版本（Note 数量较多时使用），与 `note_style` 一一对应
    pub note_style_low: NoteStyle,
    /// 低分辨率版本（Note 数量较多时使用），与 `note_style_mh` 一一对应
    pub note_style_mh_low: NoteStyle,
    pub sfx_click: AudioClip,
    pub challenge_texture: SafeTexture,
    pub sfx_drag: AudioClip,
    pub sfx_flick: AudioClip,
    pub ending: AudioClip,
    pub hit_fx: SafeTexture,
}

impl ResourcePack {
    /// 按 (是否多指提示, 是否低分辨率) 选择 Note 贴图样式
    pub fn style_for(&self, mh: bool, low: bool) -> &NoteStyle {
        match (mh, low) {
            (true, true) => &self.note_style_mh_low,
            (true, false) => &self.note_style_mh,
            (false, true) => &self.note_style_low,
            (false, false) => &self.note_style,
        }
    }
}

impl ResourcePack {
    pub async fn from_path<T: AsRef<Path>>(path: Option<T>) -> Result<Self> {
        Self::load(
            if let Some(path) = path {
                crate::fs::fs_from_file(path.as_ref())?
            } else {
                crate::fs::fs_from_assets("respack/")?
            }
            .deref_mut(),
        )
        .await
    }

    pub async fn load(fs: &mut dyn FileSystem) -> Result<Self> {
        macro_rules! load_tex {
            ($path:literal) => {
                SafeTexture::from(image::load_from_memory(&fs.load_file($path).await.with_context(|| format!("Missing {}", $path))?)?)
                    .with_filter(GL_LINEAR)
            };
        }
        let info: ResPackInfo = serde_yaml::from_str(&String::from_utf8(fs.load_file("info.yml").await.context("Missing info.yml")?)?)?;
        info.verify()?;
        let mut note_style = NoteStyle {
            click: load_tex!("click.png"),
            hold: load_tex!("hold.png"),
            flick: load_tex!("flick.png"),
            drag: load_tex!("drag.png"),
            hold_body: None,
            hold_atlas: info.hold_atlas,
        };
        note_style.verify()?;
        let mut note_style_mh = NoteStyle {
            click: load_tex!("click_mh.png"),
            hold: load_tex!("hold_mh.png"),
            flick: load_tex!("flick_mh.png"),
            drag: load_tex!("drag_mh.png"),
            hold_body: None,
            hold_atlas: info.hold_atlas_mh,
        };
        note_style_mh.verify()?;

        if info.hold_repeat {
            note_style.build_hold_body();
            note_style_mh.build_hold_body();
        }
        // 预生成低分辨率 Note 贴图（屏幕上 Note 数量较多时降分辨率渲染，降低纹理采样开销）
        let note_style_low = note_style.downscaled(info.hold_repeat, NOTE_LOW_RES_FACTOR);
        let note_style_mh_low = note_style_mh.downscaled(info.hold_repeat, NOTE_LOW_RES_FACTOR);
        let hit_fx = image::load_from_memory(&fs.load_file("hit_fx.png").await.context("Missing hit_fx.png")?)?.into();

        macro_rules! load_clip {
            ($path:literal) => {
                if let Some(sfx) = fs
                    .load_file(format!("{}.ogg", $path).as_str())
                    .await
                    .ok()
                    .map(|it| AudioClip::new(it))
                    .transpose()?
                {
                    sfx
                } else if let Some(sfx) = fs
                    .load_file(format!("{}.wav", $path).as_str())
                    .await
                    .ok()
                    .map(|it| AudioClip::new(it))
                    .transpose()?
                {
                    sfx
                } else if let Some(sfx) = fs
                    .load_file(format!("{}.mp3", $path).as_str())
                    .await
                    .ok()
                    .map(|it| AudioClip::new(it))
                    .transpose()?
                {
                    sfx
                } else {
                    // 全局 assets 目录（整理后的布局把音效放在 assets/sfx/ 下；
                    // 兼容旧的平铺 assets/ 根目录）
                    let name = format!("{}.ogg", $path);
                    let data = match load_file(format!("sfx/{name}").as_str()).await {
                        Ok(data) => data,
                        Err(_) => load_file(name.as_str()).await?,
                    };
                    AudioClip::new(data)?
                }
            };
        }

        let challenge_texture = load_texture("rank/rainbow.png").await?;

        Ok(Self {
            info,
            note_style,
            note_style_mh,
            note_style_low,
            note_style_mh_low,
            sfx_click: load_clip!("click"),
            sfx_drag: load_clip!("drag"),
            sfx_flick: load_clip!("flick"),
            ending: load_clip!("ending"),
            challenge_texture: challenge_texture.into(),
            hit_fx,
        })
    }
}

pub struct ParticleEmitter {
    pub scale: f32,
    pub emitter: Emitter,
    pub emitter_square: Emitter,
    pub hide_particles: bool,
    /// 粒子削减模式下的交替计数（每两次发射只发一次）
    reduce_tick: bool,
}

impl ParticleEmitter {
    pub fn new(res_pack: &ResourcePack, scale: f32, hide_particles: bool) -> Result<Self> {
        let colors_curve = {
            let start = WHITE;
            let mut mid = start;
            let mut end = start;
            mid.a *= 0.7;
            end.a = 0.;
            ColorCurve { start, mid, end }
        };
        let mut res = Self {
            scale: res_pack.info.hit_fx_scale,
            emitter: Emitter::new(EmitterConfig {
                local_coords: false,
                texture: Some(*res_pack.hit_fx),
                lifetime: res_pack.info.hit_fx_duration,
                lifetime_randomness: 0.0,
                initial_rotation_randomness: 0.0,
                initial_direction_spread: 0.0,
                initial_velocity: 0.0,
                atlas: Some(AtlasConfig::new(res_pack.info.hit_fx.0 as _, res_pack.info.hit_fx.1 as _, ..)),
                emitting: false,
                colors_curve,
                ..Default::default()
            }),
            emitter_square: Emitter::new(EmitterConfig {
                local_coords: false,
                lifetime: res_pack.info.hit_fx_duration,
                lifetime_randomness: 0.0,
                initial_direction_spread: 2. * std::f32::consts::PI,
                size_randomness: 0.3,
                emitting: false,
                initial_velocity: 2.5 * scale,
                initial_velocity_randomness: 1. / 10.,
                linear_accel: -6. / 1.,
                colors_curve,
                ..Default::default()
            }),
            hide_particles,
            reduce_tick: false,
        };
        res.set_scale(scale);
        Ok(res)
    }

    /// 普通模式发射（保留：UI/预览等场景直接使用）
    pub fn emit_at(&mut self, pt: Vec2, rotation: f32, color: Color) {
        self.emit_at_impl(pt, rotation, color, false);
    }

    /// 削减模式发射：保留 hit_fx 主粒子、去掉碎屑粒子，且连续发射每两次只发一次。
    /// 由“性能优化档位”的完全积极 / 自定义触发。
    pub(crate) fn emit_at_reduced(&mut self, pt: Vec2, rotation: f32, color: Color) {
        self.emit_at_impl(pt, rotation, color, true);
    }

    fn emit_at_impl(&mut self, pt: Vec2, rotation: f32, color: Color, reduce: bool) {
        let emit_main = if reduce {
            self.reduce_tick = !self.reduce_tick;
            self.reduce_tick
        } else {
            true
        };
        if emit_main {
            self.emitter.config.initial_rotation = rotation;
            self.emitter.config.base_color = color;
            self.emitter.emit(pt, 1);
        }
        if !reduce && !self.hide_particles {
            self.emitter_square.config.base_color = color;
            self.emitter_square.emit(pt, 4);
        }
    }

    pub fn draw(&mut self, dt: f32) {
        self.emitter.draw(vec2(0., 0.), dt);
        self.emitter_square.draw(vec2(0., 0.), dt);
    }

    /// 当前是否没有任何存活粒子
    pub fn is_empty(&self) -> bool {
        self.emitter.is_empty() && self.emitter_square.is_empty()
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.emitter.config.size = self.scale * scale / 5.;
        self.emitter_square.config.size = self.scale * scale / 44.;
    }
}

type NoteBufferMap = BTreeMap<(i8, GLuint), Vec<(Vec<Vertex>, Vec<u16>)>>;

#[derive(Default)]
pub struct NoteBuffer(NoteBufferMap);

impl NoteBuffer {
    pub fn push(&mut self, key: (i8, GLuint), vertices: [Vertex; 4]) {
        let meshes = self.0.entry(key).or_default();
        if meshes.last().is_none_or(|it| it.0.len() + 4 > MAX_SIZE * 4) {
            meshes.push(Default::default());
        }
        let last = meshes.last_mut().unwrap();
        let i = last.0.len() as u16;
        last.0.extend_from_slice(&vertices);
        last.1.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
    }

    pub fn draw_all(&mut self) {
        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let gl = gl.quad_gl;
        gl.draw_mode(DrawMode::Triangles);
        for ((_, tex_id), meshes) in std::mem::take(&mut self.0).into_iter() {
            gl.texture(Some(Texture2D::from_miniquad_texture(unsafe { Texture::from_raw_id(tex_id, miniquad::TextureFormat::RGBA8) })));
            for mesh in meshes {
                gl.geometry(&mesh.0, &mesh.1);
            }
        }
    }
}

pub type SfxMap = HashMap<String, Sfx>;

pub struct Resource {
    pub config: Config,
    pub info: ChartInfo,
    pub aspect_ratio: f32,
    pub dpi: u32,
    pub last_vp: (i32, i32, i32, i32),
    pub note_width: f32,

    pub time: f64,

    pub alpha: f32,
    pub judge_line_color: Color,

    pub camera: Camera2D,

    pub background: SafeTexture,
    pub illustration: SafeTexture,
    pub icons: [SafeTexture; 8],
    pub arc_icon: SafeTexture,
    pub mod_icons: [SafeTexture; 7],
    pub res_pack: ResourcePack,
    pub player: SafeTexture,
    pub icon_back: SafeTexture,
    pub icon_retry: SafeTexture,
    pub icon_resume: SafeTexture,
    pub icon_proceed: SafeTexture,

    pub emitter: ParticleEmitter,

    pub audio: AudioManager,
    pub music: AudioClip,
    pub track_length: f64,
    pub sfx_click: Sfx,
    pub sfx_drag: Sfx,
    pub sfx_flick: Sfx,

    pub extra_sfxs: SfxMap,

    pub chart_target: Option<MSRenderTarget>,
    pub no_effect: bool,

    /// 当前屏幕上可见的 Note 数量达到阈值（见 `LOW_RES_NOTE_THRESHOLD`），
    /// Note 渲染改用低分辨率贴图。由游戏场景每帧更新。
    pub low_res_notes: bool,
    /// 未来 1 秒内需要击打的 Note 数量过多（见 `HIT_FX_DENSITY_THRESHOLD`），
    /// 打击特效（粒子）被禁用。由游戏场景每帧更新。
    pub suppress_hit_fx: bool,

    pub note_buffer: RefCell<NoteBuffer>,

    pub model_stack: Vec<Matrix>,
}

macro_rules! loads {
    ($($path:literal),*) => {
        [$(loads!(@detail $path)),*]
    };

    (@detail $path:literal) => {
        Texture2D::from_image(&load_image($path).await?).into()
    };
}

impl Resource {
    pub async fn load_icons() -> Result<[SafeTexture; 8]> {
        Ok(loads![
            "rank/F.png",
            "rank/C.png",
            "rank/B.png",
            "rank/A.png",
            "rank/S.png",
            "rank/V.png",
            "rank/FC.png",
            "rank/phi.png"
        ])
    }
    pub async fn load_mod_icons() -> Result<[SafeTexture; 7]> {

        Ok(loads![
            "mod/flip_x.png",
            "mod/fade_out.png",
            "mod/fade_in.png",
            "mod/nightcore.png",
            "mod/rainbow.png",
            "mod/autoplay.png",
            "mod/no-shader.png"
        ])
    }

    pub async fn new(
        config: Config,
        info: ChartInfo,
        mut fs: Box<dyn FileSystem>,
        player: Option<SafeTexture>,
        background: SafeTexture,
        illustration: SafeTexture,
        has_no_effect: bool,
    ) -> Result<Self> {
        macro_rules! load_tex {
            ($path:literal) => {
                SafeTexture::from(Texture2D::from_image(&load_image($path).await?))
            };
        }
        // 整理后的布局把这类素材放进分类目录；优先新路径，回退旧的平铺 assets/ 根目录
        macro_rules! load_tex_fb {
            ($flat:literal, $org:literal) => {
                match load_image($org).await {
                    Ok(img) => SafeTexture::from(Texture2D::from_image(&img)),
                    Err(_) => load_tex!($flat),
                }
            };
        }
        let res_pack = ResourcePack::from_path(config.res_pack_path.as_ref())
            .await
            .context("Failed to load resource pack")?;
        let camera = Camera2D {
            target: vec2(0., 0.),
            zoom: vec2(1., -config.aspect_ratio.unwrap_or(info.aspect_ratio)),
            ..Default::default()
        };

        let mut audio = create_audio_manger(&config)?;
        let music = AudioClip::new(fs.load_file(&info.music).await?)?;
        let track_length = music.length();
        let buffer_size = Some(BUFFER_SIZE);
        let sfx_click = audio.create_sfx(res_pack.sfx_click.clone(), buffer_size)?;
        let sfx_drag = audio.create_sfx(res_pack.sfx_drag.clone(), buffer_size)?;
        let sfx_flick = audio.create_sfx(res_pack.sfx_flick.clone(), buffer_size)?;



        let aspect_ratio = config.aspect_ratio.unwrap_or(info.aspect_ratio);
        let note_width = config.note_scale * NOTE_WIDTH_RATIO_BASE as f32;
        let note_scale = config.note_scale;

        let emitter = ParticleEmitter::new(&res_pack, note_scale, res_pack.info.hide_particles)?;

        let no_effect = config.disable_effect || has_no_effect;

        macroquad::window::gl_set_drawcall_buffer_capacity(MAX_SIZE * 4, MAX_SIZE * 6);
        Ok(Self {
            config,
            info,
            aspect_ratio,
            dpi: DPI_VALUE.load(std::sync::atomic::Ordering::SeqCst),
            last_vp: (0, 0, 0, 0),
            note_width,

            time: 0.,

            alpha: 1.,
            judge_line_color: res_pack.info.fx_perfect(),

            camera,

            background,
            illustration,
            icons: Self::load_icons().await?,
            arc_icon: load_tex!("rank/FC_ARC.png"),
            mod_icons: Self::load_mod_icons().await?,
            res_pack,
            player: if let Some(player) = player { player } else { load_tex_fb!("player.jpg", "backgrounds/player.jpg") },
            icon_back: load_tex_fb!("back.png", "icons/back.png"),
            icon_retry: load_tex_fb!("retry.png", "icons/retry.png").with_mipmap(),
            icon_resume: load_tex_fb!("resume.png", "icons/resume.png"),
            icon_proceed: load_tex_fb!("proceed.png", "icons/proceed.png").with_mipmap(),

            emitter,

            audio,
            music,
            track_length,
            sfx_click,
            sfx_drag,
            sfx_flick,
            extra_sfxs: SfxMap::new(),

            chart_target: None,
            no_effect,

            low_res_notes: false,
            suppress_hit_fx: false,

            note_buffer: RefCell::new(NoteBuffer::default()),

            model_stack: vec![Matrix::identity()],
        })
    }

    pub fn create_sfx(&mut self, clip: AudioClip) -> Result<Sfx> {
        self.audio.create_sfx(clip, Some(BUFFER_SIZE))
    }

    pub fn emit_at_origin(&mut self, rotation: f32, color: Color) {
        if !self.config.eff_particles() || self.suppress_hit_fx {
            return;
        }
        let pt = self.world_to_screen(Point::default());
        let pt = vec2(
            if self.config.flip_x() { -pt.x } else { pt.x },
            if self.config.flip_y() { pt.y } else { -pt.y },
        );
        let rotation = if self.res_pack.info.hit_fx_rotate { rotation.to_radians() } else { 0. };
        if self.config.eff_fx_reduce() {
            self.emitter.emit_at_reduced(pt, rotation, color);
        } else {
            self.emitter.emit_at(pt, rotation, color);
        }
    }

    pub fn update_size(&mut self, vp: (i32, i32, i32, i32)) -> bool {
        if self.last_vp == vp {
            return false;
        }
        self.last_vp = vp;
        if !self.no_effect || self.config.sample_count != 1 {
            self.chart_target = Some(MSRenderTarget::new((vp.2 as u32, vp.3 as u32), self.config.sample_count));
        }
        fn viewport(aspect_ratio: f32, (x, y, w, h): (i32, i32, i32, i32)) -> (i32, i32, i32, i32) {
            let w = w as f32;
            let h = h as f32;
            let (rw, rh) = {
                let ew = h * aspect_ratio;
                if ew > w {
                    let eh = w / aspect_ratio;
                    (w, eh)
                } else {
                    (ew, h)
                }
            };
            (x + ((w - rw) / 2.).round() as i32, y + ((h - rh) / 2.).round() as i32, rw as i32, rh as i32)
        }
        let aspect_ratio = self.config.aspect_ratio.unwrap_or(self.info.aspect_ratio);
        if self.info.force_aspect_ratio {
            self.aspect_ratio = aspect_ratio;
            self.camera.viewport = Some(viewport(aspect_ratio, vp));
        } else {
            self.aspect_ratio = aspect_ratio.min(vp.2 as f32 / vp.3 as f32);
            self.camera.zoom.y = -self.aspect_ratio;
            self.camera.viewport = Some(viewport(self.aspect_ratio, vp));
        };
        true
    }

    pub fn world_to_screen(&self, pt: Point) -> Point {
        self.model_stack.last().unwrap().transform_point(&pt)
    }

    pub fn screen_to_world(&self, pt: Point) -> Point {
        self.model_stack.last().unwrap().try_inverse().unwrap().transform_point(&pt)
    }

    #[inline]
    pub fn with_model(&mut self, model: Matrix, f: impl FnOnce(&mut Self)) {
        let model = self.model_stack.last().unwrap() * model;
        self.model_stack.push(model);
        f(self);
        self.model_stack.pop();
    }

    #[inline]
    pub fn apply_model(&mut self, f: impl FnOnce(&mut Self)) {
        self.apply_model_of(&self.model_stack.last().unwrap().clone(), f);
    }

    #[inline]
    pub fn apply_model_of(&mut self, mat: &Matrix, f: impl FnOnce(&mut Self)) {
        unsafe { get_internal_gl() }.quad_gl.push_model_matrix(nalgebra_to_glm(mat));
        f(self);
        unsafe { get_internal_gl() }.quad_gl.pop_model_matrix();
    }
}