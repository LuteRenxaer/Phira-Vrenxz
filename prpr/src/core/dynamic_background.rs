//! 动态背景：基于曲绘代表色的弥散流体效果
//!
//! 模式 1：从曲绘提取 5 个代表色，生成随机色块并通过噪声平滑移动、交融，
//!         经低分辨率渲染 + 线性放大实现重度模糊，再叠加暗角遮罩。
//! 模式 2：在模式 1 基础上，通过 Goertzel 算法实时提取音乐低音能量，
//!         驱动背景亮度与饱和度平滑“呼吸”。

use crate::ext::SafeTexture;
use anyhow::{Context, Result};
use image::DynamicImage;
use macroquad::prelude::*;
use miniquad::{FilterMode, RenderPass, Texture, TextureFormat, TextureParams, TextureWrap};
use symphonia::core::audio::AudioBufferRef;

// 着色器代码 ---------------------------------------------------------------

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;

varying vec2 uv;

uniform mat4 Model;
uniform mat4 Projection;

void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    uv = texcoord;
}"#;

const MAX_BLOBS: usize = 14;

const BLOB_FRAGMENT: &str = r#"#version 100
precision highp float;

varying vec2 uv;

uniform float time;
uniform float aspect;
uniform float energy;
uniform float mode;
uniform float blob_count;
uniform vec4 color0;
uniform vec4 color1;
uniform vec4 color2;
uniform vec4 color3;
uniform vec4 color4;
uniform vec4 color5;
uniform vec4 color6;
uniform vec4 color7;
uniform vec4 color8;
uniform vec4 color9;
uniform vec4 color10;
uniform vec4 color11;
uniform vec4 color12;
uniform vec4 color13;

float hash(vec2 p) {
    return fract(sin(dot(p, vec2(12.9898, 78.233))) * 43758.5453);
}

float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    f = f * f * (3.0 - 2.0 * f);
    float a = hash(i);
    float b = hash(i + vec2(1.0, 0.0));
    float c = hash(i + vec2(0.0, 1.0));
    float d = hash(i + vec2(1.0, 1.0));
    return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

vec4 color_for(int i) {
    if (i == 0) return color0;
    if (i == 1) return color1;
    if (i == 2) return color2;
    if (i == 3) return color3;
    if (i == 4) return color4;
    if (i == 5) return color5;
    if (i == 6) return color6;
    if (i == 7) return color7;
    if (i == 8) return color8;
    if (i == 9) return color9;
    if (i == 10) return color10;
    if (i == 11) return color11;
    if (i == 12) return color12;
    return color13;
}

void main() {
    vec3 avg_color = (color0.rgb + color1.rgb + color2.rgb + color3.rgb + color4.rgb) * 0.2;

    vec3 acc = vec3(0.0);
    float wsum = 0.0;
    int count = int(clamp(blob_count, 1.0, 14.0));

    // 使用常量上界循环 + break，避免 GLES 2.0 不允许的动态循环索引导致驱动崩溃
    for (int i = 0; i < 14; ++i) {
        if (i >= count) { break; }
        vec4 col = color_for(i);
        float fi = float(i);
        float t = time * (0.10 + noise(vec2(fi, 0.0)) * 0.07);

        vec2 center = vec2(
            noise(vec2(fi * 1.7, t)),
            noise(vec2(fi * 1.7 + 100.0, t))
        );

        vec2 diff = uv - center;
        diff.x *= aspect;

        float radius = 0.44 + noise(vec2(fi, 1.0)) * 0.28;
        radius += (noise(uv * 3.0 + t * 0.7 + fi) - 0.5) * 0.16;
        radius += energy * 0.14;

        float d = length(diff) / radius;
        d += (noise(uv * 5.0 - t * 0.4 + fi * 2.0) - 0.5) * 0.24;

        float w = exp(-d * d * 2.2);
        acc += col.rgb * w;
        wsum += w;
    }

    vec3 blob_col = wsum > 0.001 ? acc / wsum : avg_color * 0.55;
    float blend = clamp(wsum * 1.5 + 0.14, 0.0, 1.0);
    vec3 col = mix(avg_color * (0.32 + (1.0 - mode) * 0.12), blob_col, blend);
    col += (noise(uv * 7.0 + time * 0.08) - 0.5) * 0.05;

    float base = 1.0 + (1.0 - mode) * 0.26 - mode * 0.10;
    float amp = mode * 0.70;
    col *= base + energy * amp;

    gl_FragColor = vec4(col, 1.0);
}"#;

const COMPOSITE_FRAGMENT: &str = r#"#version 100
precision highp float;

varying vec2 uv;

uniform sampler2D screenTexture;
uniform float dim;
uniform float energy;
uniform float mode;

vec3 rgb2hsv(vec3 c) {
    vec4 K = vec4(0.0, -1.0 / 3.0, 2.0 / 3.0, -1.0);
    vec4 p = mix(vec4(c.bg, K.wz), vec4(c.gb, K.xy), step(c.b, c.g));
    vec4 q = mix(vec4(p.xyw, c.r), vec4(c.r, p.yzx), step(p.x, c.r));
    float d = q.x - min(q.w, q.y);
    float e = 1.0e-10;
    return vec3(abs(q.z + (q.w - q.y) / (6.0 * d + e)), d / (q.x + e), q.x);
}

vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

void main() {
    vec3 col = texture2D(screenTexture, uv).rgb;

    // 模式 2：全频段 RMS 能量驱动饱和度与亮度“呼吸”
    if (mode > 0.5) {
        vec3 hsv = rgb2hsv(col);
        hsv.y *= 1.0 + energy * 0.85;
        hsv.z *= 0.80 + energy * 0.55;
        col = hsv2rgb(hsv);
    }

    // 原曲背景暗度叠加。静态亮度模式下弱化暗度，让色块更明亮。
    float final_dim = dim * (mode < 1.5 ? 0.03 : 0.08);
    col *= 1.0 - final_dim;

    // 轻微暗角，保留更多亮度
    vec2 vuv = (uv - 0.5) * 1.4;
    float vig = 1.0 - dot(vuv, vuv);
    vig = 0.7 + 0.3 * smoothstep(0.0, 0.85, vig);
    col *= vig;

    gl_FragColor = vec4(col, 1.0);
}"#;

// 音频频谱 ---------------------------------------------------------------

/// 预解码的单声道音频数据，用于模式 2 的全频段 RMS 检测。
#[derive(Clone)]
pub struct AudioSpectrum {
    sample_rate: u32,
    samples: Vec<f32>,
    global_rms: f32,
}

impl AudioSpectrum {
    /// 计算当前位置 100ms 窗口的全频段 RMS 能量，相对于整曲平均 RMS 归一化到 [0, 1]。
    pub fn full_band_rms(&self, position: f64) -> f32 {
        let center = (position * self.sample_rate as f64) as usize;
        let half = (self.sample_rate as f64 * 0.05) as usize; // 100ms 窗口
        let start = center.saturating_sub(half);
        let end = (center + half).min(self.samples.len());
        if end <= start {
            return 0.0;
        }
        let window = &self.samples[start..end];
        let n = window.len();
        if n == 0 {
            return 0.0;
        }

        let rms = (window.iter().map(|s| s * s).sum::<f32>() / n as f32).sqrt();
        let scale = (self.global_rms * 1.5).max(0.001);
        (rms / scale).min(1.0).max(0.0)
    }
}

// 动态背景 ---------------------------------------------------------------

pub struct DynamicBackground {
    mode: u8,
    dim: f32,
    colors: [Color; MAX_BLOBS],
    blob_count: usize,
    blob_material: Material,
    composite_material: Material,
    blob_target: RenderTarget,
    output_texture: SafeTexture,
    output_pass: RenderPass,
    audio: Option<AudioSpectrum>,
    bass_smooth: f32,
    last_viewport: (i32, i32, i32, i32),
}

impl Clone for DynamicBackground {
    /// 克隆动态背景：复用颜色与音频数据，重新创建材质与渲染目标。
    /// 用于 LoadingScene 与 GameScene 分别持有独立实例。
    fn clone(&self) -> Self {
        let blob_material = load_material(
            VERTEX,
            BLOB_FRAGMENT,
            MaterialParams {
                uniforms: vec![
                    ("time".to_owned(), UniformType::Float1),
                    ("aspect".to_owned(), UniformType::Float1),
                    ("energy".to_owned(), UniformType::Float1),
                    ("mode".to_owned(), UniformType::Float1),
                    ("blob_count".to_owned(), UniformType::Float1),
                    ("color0".to_owned(), UniformType::Float4),
                    ("color1".to_owned(), UniformType::Float4),
                    ("color2".to_owned(), UniformType::Float4),
                    ("color3".to_owned(), UniformType::Float4),
                    ("color4".to_owned(), UniformType::Float4),
                    ("color5".to_owned(), UniformType::Float4),
                    ("color6".to_owned(), UniformType::Float4),
                    ("color7".to_owned(), UniformType::Float4),
                    ("color8".to_owned(), UniformType::Float4),
                    ("color9".to_owned(), UniformType::Float4),
                    ("color10".to_owned(), UniformType::Float4),
                    ("color11".to_owned(), UniformType::Float4),
                    ("color12".to_owned(), UniformType::Float4),
                    ("color13".to_owned(), UniformType::Float4),
                ],
                ..Default::default()
            },
        )
        .expect("failed to clone blob material");
        for (i, &c) in self.colors.iter().enumerate() {
            blob_material.set_uniform(&format!("color{i}"), c);
        }
        blob_material.set_uniform("mode", self.mode as f32);
        blob_material.set_uniform("blob_count", self.blob_count as f32);

        let composite_material = load_material(
            VERTEX,
            COMPOSITE_FRAGMENT,
            MaterialParams {
                uniforms: vec![
                    ("dim".to_owned(), UniformType::Float1),
                    ("energy".to_owned(), UniformType::Float1),
                    ("mode".to_owned(), UniformType::Float1),
                ],
                textures: vec!["screenTexture".to_owned()],
                ..Default::default()
            },
        )
        .expect("failed to clone composite material");
        composite_material.set_uniform("dim", self.dim);
        composite_material.set_uniform("mode", self.mode as f32);

        let (blob_target, output_texture, output_pass) = Self::create_targets(self.last_viewport);

        Self {
            mode: self.mode,
            dim: self.dim,
            colors: self.colors,
            blob_count: self.blob_count,
            blob_material,
            composite_material,
            blob_target,
            output_texture,
            output_pass,
            audio: self.audio.clone(),
            bass_smooth: self.bass_smooth,
            last_viewport: self.last_viewport,
        }
    }
}

impl DynamicBackground {
    pub fn mode(&self) -> u8 {
        self.mode
    }

    pub fn texture(&self) -> SafeTexture {
        self.output_texture.clone()
    }

    pub fn set_audio(&mut self, audio: AudioSpectrum) {
        self.audio = Some(audio);
    }

    /// 从曲绘创建动态背景，模式 0 应直接返回 None（调用方判断）。
    /// viewport 用于创建固定大小的渲染目标，避免后续替换纹理导致外部 SafeTexture 引用失效。
    pub fn new(mode: u8, image: &DynamicImage, dim: f32, viewport: (i32, i32, i32, i32)) -> Result<Self> {
        anyhow::ensure!(mode == 1 || mode == 2, "unsupported dynamic background mode: {mode}");
        anyhow::ensure!(viewport.2 > 0 && viewport.3 > 0, "invalid viewport size");

        let rgb = image.to_rgb8();
        // color-thief 的 quality 参数范围必须是 1..=10，之前误传 14 会在 Android 上触发断言崩溃
        let palette = color_thief::get_palette(&rgb, color_thief::ColorFormat::Rgb, 10, 5).context("failed to extract color palette")?;
        let mut colors = [BLACK; MAX_BLOBS];
        for i in 0..MAX_BLOBS {
            let index = if i < palette.len() { i } else { palette.len().max(1) - 1 };
            let c = &palette[index];
            colors[i] = Color::from_rgba(c.r, c.g, c.b, 255);
        }

        let blob_count = Self::blob_count_for(viewport);

        let blob_material = load_material(
            VERTEX,
            BLOB_FRAGMENT,
            MaterialParams {
                uniforms: vec![
                    ("time".to_owned(), UniformType::Float1),
                    ("aspect".to_owned(), UniformType::Float1),
                    ("energy".to_owned(), UniformType::Float1),
                    ("mode".to_owned(), UniformType::Float1),
                    ("blob_count".to_owned(), UniformType::Float1),
                    ("color0".to_owned(), UniformType::Float4),
                    ("color1".to_owned(), UniformType::Float4),
                    ("color2".to_owned(), UniformType::Float4),
                    ("color3".to_owned(), UniformType::Float4),
                    ("color4".to_owned(), UniformType::Float4),
                    ("color5".to_owned(), UniformType::Float4),
                    ("color6".to_owned(), UniformType::Float4),
                    ("color7".to_owned(), UniformType::Float4),
                    ("color8".to_owned(), UniformType::Float4),
                    ("color9".to_owned(), UniformType::Float4),
                    ("color10".to_owned(), UniformType::Float4),
                    ("color11".to_owned(), UniformType::Float4),
                    ("color12".to_owned(), UniformType::Float4),
                    ("color13".to_owned(), UniformType::Float4),
                ],
                ..Default::default()
            },
        )
        .context("failed to load blob material")?;
        for (i, &c) in colors.iter().enumerate() {
            blob_material.set_uniform(&format!("color{i}"), c);
        }
        blob_material.set_uniform("mode", mode as f32);
        blob_material.set_uniform("blob_count", blob_count as f32);

        let composite_material = load_material(
            VERTEX,
            COMPOSITE_FRAGMENT,
            MaterialParams {
                uniforms: vec![
                    ("dim".to_owned(), UniformType::Float1),
                    ("energy".to_owned(), UniformType::Float1),
                    ("mode".to_owned(), UniformType::Float1),
                ],
                textures: vec!["screenTexture".to_owned()],
                ..Default::default()
            },
        )
        .context("failed to load composite material")?;
        composite_material.set_uniform("dim", dim);
        composite_material.set_uniform("mode", mode as f32);

        let (blob_target, output_texture, output_pass) = Self::create_targets(viewport);

        Ok(Self {
            mode,
            dim,
            colors,
            blob_count,
            blob_material,
            composite_material,
            blob_target,
            output_texture,
            output_pass,
            audio: None,
            bass_smooth: 0.0,
            last_viewport: viewport,
        })
    }

    fn blob_count_for((_, _, w, h): (i32, i32, i32, i32)) -> usize {
        let area = w as f32 * h as f32;
        let scale = (area / 130_000.0).sqrt();
        let count = (scale * 2.8).ceil() as usize;
        count.clamp(7, MAX_BLOBS)
    }

    /// 将音乐文件解码为单声道数据，用于模式 2。
    pub fn decode_audio(data: &[u8]) -> Result<AudioSpectrum> {
        use std::io::Cursor;
        use symphonia::core::codecs::DecoderOptions;
        use symphonia::core::errors::Error;
        use symphonia::core::formats::FormatOptions;
        use symphonia::core::io::MediaSourceStream;
        use symphonia::core::meta::MetadataOptions;
        use symphonia::core::probe::Hint;

        let mss = MediaSourceStream::new(Box::new(Cursor::new(data.to_vec())), Default::default());
        let hint = Hint::new();
        let format_opts = FormatOptions::default();
        let metadata_opts = MetadataOptions::default();
        let decoder_opts = DecoderOptions::default();

        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &format_opts, &metadata_opts)
            .context("failed to probe audio format")?;
        let mut format = probed.format;
        let track = format.default_track().context("no audio track")?;
        let sample_rate = track.codec_params.sample_rate.context("unknown sample rate")?;
        let mut decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &decoder_opts)
            .context("failed to create audio decoder")?;
        let track_id = track.id;

        let mut samples = Vec::new();
        loop {
            let packet = match format.next_packet() {
                Ok(p) => p,
                Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e.into()),
            };
            if packet.track_id() != track_id {
                continue;
            }
            let decoded = decoder.decode(&packet)?;
            append_mono(decoded, &mut samples);
        }

        let global_rms = if samples.is_empty() {
            0.0
        } else {
            (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
        };

        Ok(AudioSpectrum {
            sample_rate,
            samples,
            global_rms,
        })
    }

    /// 更新背景纹理。应在渲染背景前每帧调用一次。
    /// 渲染目标在创建时固定，避免外部 SafeTexture 引用失效；视口变化时由 draw_background 做缩放裁剪。
    pub fn update(&mut self, time: f32, viewport: (i32, i32, i32, i32), music_position: Option<f32>) {
        if viewport.2 <= 0 || viewport.3 <= 0 {
            return;
        }

        let energy = if self.mode >= 2 {
            if let Some(audio) = &self.audio {
                let raw = audio.full_band_rms(music_position.unwrap_or(0.0) as f64);
                // 平滑过渡，避免突变
                self.bass_smooth = self.bass_smooth * 0.88 + raw * 0.12;
                self.bass_smooth
            } else {
                self.bass_smooth * 0.95
            }
        } else {
            0.0
        };

        let aspect = viewport.2 as f32 / viewport.3 as f32;

        // 1) 在低分辨率目标上绘制色块（低分辨率即重度模糊）
        self.blob_material.set_uniform("time", time);
        self.blob_material.set_uniform("aspect", aspect);
        self.blob_material.set_uniform("energy", energy);

        push_camera_state();
        set_camera(&Camera2D {
            zoom: vec2(1.0, -1.0),
            ..Default::default()
        });

        if viewport != self.last_viewport {
            let (blob_target, output_texture, output_pass) = Self::create_targets(viewport);
            self.blob_target = blob_target;
            self.output_texture = output_texture;
            self.output_pass = output_pass;
            self.last_viewport = viewport;
        }

        let blob_count = Self::blob_count_for(viewport);
        if blob_count != self.blob_count {
            self.blob_count = blob_count;
            self.blob_material.set_uniform("blob_count", blob_count as f32);
        }

        let mut gl = unsafe { get_internal_gl() };
        gl.flush();
        let old_pass = gl.quad_gl.get_active_render_pass();
        let old_viewport = gl.quad_gl.get_viewport();

        gl.quad_gl.render_pass(Some(self.blob_target.render_pass));
        gl.quad_gl.viewport(None);
        clear_background(BLACK);
        gl_use_material(self.blob_material);
        draw_rectangle(-1.0, -1.0, 2.0, 2.0, WHITE);
        gl_use_default_material();
        gl.flush();

        // 2) 合成：放大低分辨率色块并叠加暗角/暗度/能量
        self.composite_material.set_uniform("energy", energy);
        self.composite_material.set_texture("screenTexture", self.blob_target.texture);

        gl.quad_gl.render_pass(Some(self.output_pass));
        gl.quad_gl.viewport(None);
        clear_background(BLACK);
        gl_use_material(self.composite_material);
        draw_rectangle(-1.0, -1.0, 2.0, 2.0, WHITE);
        gl_use_default_material();
        gl.flush();

        // 恢复原有渲染目标
        gl.quad_gl.render_pass(old_pass);
        gl.quad_gl.viewport(old_viewport);
        pop_camera_state();
    }

    fn create_targets(viewport: (i32, i32, i32, i32)) -> (RenderTarget, SafeTexture, RenderPass) {
        let (w, h) = (viewport.2 as u32, viewport.3 as u32);
        // 色块目标为视口 1/4，用线性放大实现模糊；至少 128x128 防止过度马赛克
        let blob_w = (w / 3).max(128);
        let blob_h = (h / 3).max(128);

        let blob_target = render_target(blob_w, blob_h);
        blob_target.texture.set_filter(FilterMode::Linear);

        let gl = unsafe { get_internal_gl() };
        let tex = Texture::new_render_texture(
            gl.quad_context,
            TextureParams {
                width: w,
                height: h,
                format: TextureFormat::RGBA8,
                filter: FilterMode::Linear,
                wrap: TextureWrap::Clamp,
                ..Default::default()
            },
        );
        let pass = RenderPass::new(gl.quad_context, tex, None);
        let texture = SafeTexture::from(Texture2D::from_miniquad_texture(tex));

        (blob_target, texture, pass)
    }
}

impl Drop for DynamicBackground {
    fn drop(&mut self) {
        self.blob_material.delete();
        self.composite_material.delete();
    }
}

/// 将任意格式的解码音频统一转为单声道 f32，使用 SampleBuffer 避免处理各种位深。
fn append_mono(buf: AudioBufferRef, out: &mut Vec<f32>) {
    use symphonia::core::audio::SampleBuffer;

    let frames = buf.frames();
    let channels = buf.spec().channels.count();
    if frames == 0 || channels == 0 {
        return;
    }

    // 一次性转换为 f32 交错样本
    let mut sample_buf = SampleBuffer::<f32>::new(frames as u64, *buf.spec());
    sample_buf.copy_interleaved_ref(buf);

    // 每帧取多声道平均，得到单声道
    let div = channels as f32;
    for frame in sample_buf.samples().chunks(channels) {
        let sum: f32 = frame.iter().sum();
        out.push(sum / div);
    }
}
