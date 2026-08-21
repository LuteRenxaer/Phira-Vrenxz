//! Spine 骨骼动画模型加载与渲染
//!
//! 支持 Spine 3.8 和 4.x 二进制格式 (.skel) 及 .atlas 文件，使用自定义 shader 渲染到 macroquad 界面。
//! 自动检测版本并选择对应的运行时后端。

use macroquad::prelude::*;
use macroquad::window::miniquad::*;
use std::sync::Arc;
use tracing::warn;

// ========== 公共 Shader ==========

mod shader {
    use macroquad::window::miniquad::*;

    pub const VERTEX: &str = r#"#version 100
attribute vec2 position;
attribute vec2 uv;
attribute vec4 color0;
attribute vec4 color1;
uniform mat4 mvp;
varying lowp vec2 f_uv;
varying lowp vec4 f_color;
varying lowp vec4 f_dark_color;
void main() {
    gl_Position = mvp * vec4(position, 0.0, 1.0);
    f_uv = uv;
    f_color = color0;
    f_dark_color = color1;
}"#;

    pub const FRAGMENT: &str = r#"#version 100
precision lowp float;
varying lowp vec2 f_uv;
varying lowp vec4 f_color;
varying lowp vec4 f_dark_color;
uniform sampler2D tex;
uniform float alpha;
void main() {
    lowp vec4 tex_color = texture2D(tex, f_uv);
    gl_FragColor = vec4(
        ((tex_color.a - 1.0) * f_dark_color.a + 1.0 - tex_color.rgb) * f_dark_color.rgb + tex_color.rgb * f_color.rgb,
        tex_color.a * f_color.a * alpha
    );
}"#;

    pub fn meta() -> ShaderMeta {
        ShaderMeta {
            images: vec!["tex".to_string()],
            uniforms: UniformBlockLayout {
                uniforms: vec![
                    UniformDesc::new("mvp", UniformType::Mat4),
                    UniformDesc::new("alpha", UniformType::Float1),
                ],
            },
        }
    }

    #[repr(C)]
    pub struct Uniforms {
        pub mvp: glam::Mat4,
        pub alpha: f32,
    }
}

// ========== 公共顶点格式 ==========

#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Vertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
    dark_color: [f32; 4],
}

// ========== 版本检测 ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpineVersion {
    V38,
    V4x,
}

/// 检测 .skel 文件的 Spine 版本。
/// - 4.x: 前 8 字节是二进制 hash，偏移 8 是版本字符串长度，随后是 "4.x"
/// - 3.8: 偏移 0 是 hash 字符串长度，版本字符串在 hash 之后
fn detect_skel_version(skel_data: &[u8]) -> Option<SpineVersion> {
    if skel_data.len() < 12 {
        return None;
    }

    // 尝试检测 4.x：前 8 字节是二进制 hash，偏移 8 是版本字符串长度，随后以 "4." 开头
    {
        let ver_len = skel_data[8] as usize;
        if ver_len >= 2 && 9 + 2 <= skel_data.len() {
            if skel_data[9] == b'4' && skel_data[10] == b'.' {
                return Some(SpineVersion::V4x);
            }
        }
    }

    // 尝试检测 3.8：偏移 0 是 hash 字符串长度，版本字符串在 hash 之后，以 "3." 开头
    {
        let hash_len = skel_data[0] as usize;
        if hash_len >= 16 && hash_len <= 64 {
            let ver_len_offset = 1 + hash_len;
            if ver_len_offset + 2 < skel_data.len() {
                let ver_len = skel_data[ver_len_offset] as usize;
                if ver_len >= 2 && ver_len_offset + 1 + 2 <= skel_data.len() {
                    if skel_data[ver_len_offset + 1] == b'3'
                        && skel_data[ver_len_offset + 2] == b'.'
                    {
                        return Some(SpineVersion::V38);
                    }
                }
            }
        }
    }

    None
}

// ========== 公共工具函数 ==========

fn find_texture_data<'a>(
    page_name: &str,
    textures: &'a [(String, Vec<u8>)],
) -> Option<&'a (String, Vec<u8>)> {
    let page_name_lower = page_name.to_lowercase();
    textures
        .iter()
        .find(|(name, _)| {
            let a = std::path::Path::new(name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(name)
                .to_lowercase();
            a == page_name_lower
        })
        .or_else(|| {
            let prefix = page_name_lower.trim_end_matches(".png");
            textures.iter().find(|(name, _)| {
                let a = std::path::Path::new(name)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(name)
                    .to_lowercase();
                a.starts_with(prefix) && a.ends_with(".png")
            })
        })
        .or_else(|| textures.first())
}

fn detect_premultiplied_alpha(atlas_data: &[u8]) -> bool {
    let atlas_str = String::from_utf8_lossy(atlas_data);
    atlas_str.contains("pma:true") || atlas_str.contains("pma: true")
}

fn create_render_resources(
    premultiplied_alpha: bool,
) -> anyhow::Result<(Pipeline, Buffer, Buffer)> {
    let InternalGlContext { quad_context: ctx, .. } = unsafe { get_internal_gl() };

    let shader = Shader::new(ctx, shader::VERTEX, shader::FRAGMENT, shader::meta())?;
    let default_blend = if premultiplied_alpha {
        BlendState::new(
            Equation::Add,
            BlendFactor::One,
            BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
        )
    } else {
        BlendState::new(
            Equation::Add,
            BlendFactor::Value(BlendValue::SourceAlpha),
            BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
        )
    };

    let pipeline = Pipeline::with_params(
        ctx,
        &[BufferLayout::default()],
        &[
            VertexAttribute::new("position", VertexFormat::Float2),
            VertexAttribute::new("uv", VertexFormat::Float2),
            VertexAttribute::new("color0", VertexFormat::Float4),
            VertexAttribute::new("color1", VertexFormat::Float4),
        ],
        shader,
        PipelineParams {
            color_blend: Some(default_blend),
            alpha_blend: Some(BlendState::new(
                Equation::Add,
                BlendFactor::One,
                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
            )),
            cull_face: CullFace::Nothing,
            ..Default::default()
        },
    );

    let vertex_buffer = Buffer::stream(
        ctx,
        BufferType::VertexBuffer,
        10000 * std::mem::size_of::<Vertex>(),
    );
    let index_buffer = Buffer::stream(
        ctx,
        BufferType::IndexBuffer,
        10000 * std::mem::size_of::<u16>(),
    );

    Ok((pipeline, vertex_buffer, index_buffer))
}

// ========== 宏：生成后端实现 ==========

macro_rules! impl_spine_backend {
    ($mod_name:ident, $spine_crate:ident, $update_stmt:expr) => {
        mod $mod_name {
            use super::*;
            use $spine_crate::{
                controller::{SkeletonController, SkeletonControllerSettings},
                draw::{ColorSpace, CullDirection},
                AnimationStateData, Atlas, BlendMode, SkeletonBinary,
            };

            fn blend_state(mode: BlendMode, premultiplied_alpha: bool) -> BlendState {
                match mode {
                    BlendMode::Normal => {
                        if premultiplied_alpha {
                            BlendState::new(
                                Equation::Add,
                                BlendFactor::One,
                                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                            )
                        } else {
                            BlendState::new(
                                Equation::Add,
                                BlendFactor::Value(BlendValue::SourceAlpha),
                                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                            )
                        }
                    }
                    BlendMode::Additive => {
                        if premultiplied_alpha {
                            BlendState::new(Equation::Add, BlendFactor::One, BlendFactor::One)
                        } else {
                            BlendState::new(
                                Equation::Add,
                                BlendFactor::Value(BlendValue::SourceAlpha),
                                BlendFactor::One,
                            )
                        }
                    }
                    BlendMode::Multiply => BlendState::new(
                        Equation::Add,
                        BlendFactor::Value(BlendValue::DestinationColor),
                        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                    ),
                    BlendMode::Screen => BlendState::new(
                        Equation::Add,
                        BlendFactor::One,
                        BlendFactor::OneMinusValue(BlendValue::SourceColor),
                    ),
                }
            }

            pub struct Backend {
                controller: SkeletonController,
                pipeline: Pipeline,
                vertex_buffer: Buffer,
                index_buffer: Buffer,
                textures: Vec<Texture2D>,
                premultiplied_alpha: bool,
            }

            impl Backend {
                pub fn from_memory(
                    atlas_data: &[u8],
                    skel_data: &[u8],
                    textures: &[(String, Vec<u8>)],
                    animation: &str,
                ) -> anyhow::Result<Self> {
                    let atlas = Arc::new(Atlas::new(atlas_data, "")?);
                    let premultiplied_alpha = detect_premultiplied_alpha(atlas_data);

                    let skeleton_binary = SkeletonBinary::new(atlas.clone());
                    let skeleton_data = Arc::new(skeleton_binary.read_skeleton_data(skel_data)?);

                    let animations: Vec<String> = skeleton_data
                        .animations()
                        .map(|a| a.name().to_string())
                        .collect();

                    let mut animation_state_data =
                        AnimationStateData::new(skeleton_data.clone());
                    animation_state_data.set_default_mix(0.0);
                    let animation_state_data = Arc::new(animation_state_data);

                    let mut controller =
                        SkeletonController::new(skeleton_data, animation_state_data)
                            .with_settings(SkeletonControllerSettings {
                                premultiplied_alpha,
                                cull_direction: CullDirection::CounterClockwise,
                                color_space: ColorSpace::SRGB,
                            });

                    if let Err(e) = controller
                        .animation_state
                        .set_animation_by_name(0, animation, true)
                    {
                        warn!("Failed to set animation '{}': {}", animation, e);
                        if let Some(first) = animations.first() {
                            let _ = controller
                                .animation_state
                                .set_animation_by_name(0, first, true);
                        }
                    }

                    // 更新一次（版本差异：3.8 单参数，4.x 需 Physics）
                    let c = &mut controller;
                    $update_stmt(c, 0.0);

                    // 加载纹理
                    let mut gpu_textures = Vec::new();
                    for (i, page) in atlas.pages().enumerate() {
                        let page_name = page.name();
                        let texture_data = find_texture_data(&page_name, textures);
                        let texture = if let Some((_, data)) = texture_data {
                            Texture2D::from_file_with_format(data, Some(ImageFormat::Png))
                        } else {
                            warn!(
                                "Atlas page '{}' 未找到匹配的纹理文件，使用空白纹理",
                                page_name
                            );
                            Texture2D::from_file_with_format(&[0u8; 0], Some(ImageFormat::Png))
                        };
                        page.renderer_object().set(i);
                        gpu_textures.push(texture);
                    }

                    let (pipeline, vertex_buffer, index_buffer) =
                        create_render_resources(premultiplied_alpha)?;

                    Ok(Self {
                        controller,
                        pipeline,
                        vertex_buffer,
                        index_buffer,
                        textures: gpu_textures,
                        premultiplied_alpha,
                    })
                }

                pub fn update(&mut self, dt: f32) {
                    let c = &mut self.controller;
                    $update_stmt(c, dt);
                }

                pub fn set_animation(&mut self, name: &str, loop_: bool) {
                    self.controller.animation_state.clear_track(0);
                    let _ = self
                        .controller
                        .animation_state
                        .set_animation_by_name(0, name, loop_);
                }

                pub fn set_expression(&mut self, index: u32) {
                    set_expression_on_skeleton(&mut self.controller.skeleton, index);
                }

                pub fn render(&mut self, scale: f32, position: glam::Vec2, alpha: f32) {
                    let renderables = self.controller.combined_renderables();
                    if renderables.is_empty() {
                        return;
                    }

                    let mut gl = unsafe { get_internal_gl() };
                    gl.flush();
                    let ctx = gl.quad_context;

                    let projection = gl.quad_gl.get_projection_matrix();
                    let model = glam::Mat4::from_translation(position.extend(0.0))
                        * glam::Mat4::from_scale(glam::Vec3::new(scale, -scale, 1.0));
                    let mvp = projection * model;

                    ctx.apply_pipeline(&self.pipeline);
                    ctx.apply_uniforms(&shader::Uniforms { mvp, alpha });

                    for renderable in &renderables {
                        if renderable.vertices.is_empty() || renderable.indices.is_empty() {
                            continue;
                        }

                        let vertices: Vec<Vertex> = renderable
                            .vertices
                            .iter()
                            .zip(renderable.uvs.iter())
                            .zip(renderable.colors.iter())
                            .zip(renderable.dark_colors.iter())
                            .map(|(((pos, uv), color), dark_color)| Vertex {
                                position: *pos,
                                uv: *uv,
                                color: *color,
                                dark_color: *dark_color,
                            })
                            .collect();

                        self.vertex_buffer.update(ctx, &vertices);
                        self.index_buffer.update(ctx, &renderable.indices);

                        let bs = blend_state(renderable.blend_mode, self.premultiplied_alpha);
                        ctx.set_blend(
                            Some(bs),
                            Some(BlendState::new(
                                Equation::Add,
                                BlendFactor::One,
                                BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                            )),
                        );

                        let texture_handle =
                            if let Some(renderer_obj) = renderable.attachment_renderer_object {
                                let idx = unsafe { *(renderer_obj as *const usize) };
                                self.textures
                                    .get(idx)
                                    .map(|t| t.raw_miniquad_texture_handle())
                                    .unwrap_or(self.textures[0].raw_miniquad_texture_handle())
                            } else {
                                self.textures[0].raw_miniquad_texture_handle()
                            };

                        let bindings = Bindings {
                            vertex_buffers: vec![self.vertex_buffer],
                            index_buffer: self.index_buffer,
                            images: vec![texture_handle],
                        };
                        ctx.apply_bindings(&bindings);
                        ctx.draw(0, renderable.indices.len() as i32, 1);
                    }

                    gl.quad_gl.texture(None);
                    gl.quad_gl.pipeline(None);
                    gl.quad_gl.draw_mode(DrawMode::Triangles);
                    gl.quad_gl.scissor(None);
                }
            }

            /// 在指定 skeleton 上设置表情（两个后端的 Skeleton 类型不同，但 API 一致）
            fn set_expression_on_skeleton(skeleton: &mut $spine_crate::Skeleton, index: u32) {
                fn extract_num(name: &str) -> Option<u32> {
                    let bytes = name.as_bytes();
                    let mut i = 0;
                    while i < bytes.len() {
                        if bytes[i].is_ascii_digit() {
                            let start = i;
                            while i < bytes.len() && bytes[i].is_ascii_digit() {
                                i += 1;
                            }
                            return name[start..i].parse::<u32>().ok();
                        }
                        i += 1;
                    }
                    None
                }

                fn is_default_placeholder(name: &str) -> bool {
                    let lower = name.to_lowercase();
                    lower.contains("default") || lower.contains("defualt")
                }

                let target_num = index + 1;

                let mut slot_map: std::collections::HashMap<String, (String, Vec<String>)> =
                    std::collections::HashMap::new();
                {
                    for slot in skeleton.slots() {
                        let slot_name = slot.data().name().to_string();
                        let default = slot.data().attachment_name().unwrap_or("").to_string();
                        slot_map.insert(slot_name, (default, Vec::new()));
                    }
                    for skin in skeleton.data().skins() {
                        for entry in skin.attachments() {
                            let slot_idx = entry.slot_index as usize;
                            if let Some(slot) = skeleton.slots().nth(slot_idx) {
                                let slot_name = slot.data().name().to_string();
                                if let Some((_, list)) = slot_map.get_mut(&slot_name) {
                                    list.push(entry.attachment.name().to_string());
                                }
                            }
                        }
                    }
                }

                for (slot_name, (default, list)) in slot_map {
                    if list.len() <= 1 {
                        continue;
                    }
                    let target = list
                        .iter()
                        .find(|n| extract_num(n) == Some(target_num))
                        .cloned()
                        .or_else(|| {
                            let prefix = format!("{:02}", target_num);
                            list.iter().find(|n| n.starts_with(&prefix)).cloned()
                        })
                        .unwrap_or_else(|| {
                            if is_default_placeholder(&default) {
                                list.iter()
                                    .find(|n| !is_default_placeholder(n))
                                    .cloned()
                                    .unwrap_or(default)
                            } else {
                                default
                            }
                        });
                    skeleton.set_attachment(&slot_name, Some(&target));
                }
            }
        }
    };
}

// 3.8 后端：update(dt)
impl_spine_backend!(v38, rusty_spine_38, |c: &mut rusty_spine_38::controller::SkeletonController, dt: f32| {
    c.update(dt);
});

// 4.x 后端：update(dt, Physics::Update)
impl_spine_backend!(v4x, rusty_spine_4x, |c: &mut rusty_spine_4x::controller::SkeletonController, dt: f32| {
    use rusty_spine_4x::Physics;
    c.update(dt, Physics::Update);
});

// ========== 统一 SpineModel ==========

enum SpineBackend {
    V38(v38::Backend),
    V4x(v4x::Backend),
}

pub struct SpineModel {
    inner: SpineBackend,
    /// 模型缩放
    pub scale: f32,
    /// 模型位置（屏幕坐标，中心）
    pub position: glam::Vec2,
    /// 整体透明度
    pub alpha: f32,
}

impl SpineModel {
    /// 从内存数据加载 Spine 模型，自动检测版本（3.8 / 4.x）
    pub fn from_memory(
        atlas_data: &[u8],
        skel_data: &[u8],
        textures: &[(String, Vec<u8>)],
        animation: &str,
    ) -> anyhow::Result<Self> {
        let version = detect_skel_version(skel_data);
        let inner = match version {
            Some(SpineVersion::V4x) => {
                SpineBackend::V4x(v4x::Backend::from_memory(atlas_data, skel_data, textures, animation)?)
            }
            Some(SpineVersion::V38) | None => {
                // 检测不到版本时默认尝试 3.8（兼容旧格式）
                SpineBackend::V38(v38::Backend::from_memory(atlas_data, skel_data, textures, animation)?)
            }
        };

        Ok(Self {
            inner,
            scale: 1.0,
            position: glam::Vec2::ZERO,
            alpha: 1.0,
        })
    }

    /// 更新动画
    pub fn update(&mut self, dt: f32) {
        match &mut self.inner {
            SpineBackend::V38(b) => b.update(dt),
            SpineBackend::V4x(b) => b.update(dt),
        }
    }

    /// 设置动画
    pub fn set_animation(&mut self, name: &str, loop_: bool) {
        match &mut self.inner {
            SpineBackend::V38(b) => b.set_animation(name, loop_),
            SpineBackend::V4x(b) => b.set_animation(name, loop_),
        }
    }

    /// 设置表情
    pub fn set_expression(&mut self, index: u32) {
        match &mut self.inner {
            SpineBackend::V38(b) => b.set_expression(index),
            SpineBackend::V4x(b) => b.set_expression(index),
        }
    }

    /// 渲染模型
    pub fn render(&mut self) {
        let (scale, position, alpha) = (self.scale, self.position, self.alpha);
        match &mut self.inner {
            SpineBackend::V38(b) => b.render(scale, position, alpha),
            SpineBackend::V4x(b) => b.render(scale, position, alpha),
        }
    }
}

// ========== 原始模型数据 ==========

pub struct SpineModelRawData {
    pub atlas_data: Vec<u8>,
    pub skel_data: Vec<u8>,
    /// (纹理文件名, 纹理数据) 列表，支持多纹理
    pub textures: Vec<(String, Vec<u8>)>,
}

/// 默认模型目录（相对于当前工作目录的 assets 目录）
pub const DEFAULT_MODEL_DIR: &str = "assets/skel/Hoshino/Hoshino (Combat Readiness_Attack)";

/// 异步加载默认模型的原始数据
pub async fn load_default_model_data() -> anyhow::Result<SpineModelRawData> {
    load_model_from_dir(DEFAULT_MODEL_DIR).await
}

/// 从 assets/skel 目录加载默认模型（需在主线程调用，因为会创建 GPU 资源）
pub async fn load_default_model() -> anyhow::Result<SpineModel> {
    let data = load_default_model_data().await?;
    SpineModel::from_memory(&data.atlas_data, &data.skel_data, &data.textures, "Idle_01")
}

/// 从指定目录加载模型原始数据（自动查找 .atlas/.skel/.png 文件，支持多纹理）
pub async fn load_model_from_dir(dir: &str) -> anyhow::Result<SpineModelRawData> {
    use std::fs;
    let entries = fs::read_dir(dir)?;
    let mut atlas_path = None;
    let mut skel_path = None;
    let mut textures = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext {
                "atlas" => atlas_path = Some(path.clone()),
                "skel" => skel_path = Some(path.clone()),
                "png" => {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("texture.png")
                        .to_string();
                    // 过滤掉头像文件
                    if !name.contains("-avatar") && !name.contains("_avatar") {
                        textures.push((name, fs::read(&path)?));
                    }
                }
                _ => {}
            }
        }
    }
    let atlas_path = atlas_path.ok_or_else(|| anyhow::anyhow!("未找到 .atlas 文件"))?;
    let skel_path = skel_path.ok_or_else(|| anyhow::anyhow!("未找到 .skel 文件"))?;
    if textures.is_empty() {
        anyhow::bail!("未找到 .png 纹理文件");
    }
    let atlas_data = fs::read(&atlas_path)?;
    let skel_data = fs::read(&skel_path)?;
    // 版本检测：确保是受支持的格式
    match detect_skel_version(&skel_data) {
        Some(_) => {} // 3.8 或 4.x 都支持
        None => {
            warn!("无法识别 Spine 版本，将尝试按 3.8 加载");
        }
    }
    Ok(SpineModelRawData {
        atlas_data,
        skel_data,
        textures,
    })
}

/// 判断目录是否包含有效的 spine 模型文件（.atlas + .skel）
fn is_model_dir(path: &std::path::Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(path) {
        let mut has_atlas = false;
        let mut has_skel = false;
        for entry in entries.flatten() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                match ext {
                    "atlas" => has_atlas = true,
                    "skel" => has_skel = true,
                    _ => {}
                }
            }
        }
        has_atlas && has_skel
    } else {
        false
    }
}

/// 判断模型目录是否为受支持的格式（3.8 或 4.x）
fn is_supported_model_dir(path: &std::path::Path) -> bool {
    if !is_model_dir(path) {
        return false;
    }
    // 现在两个版本都支持，只要有 .atlas + .skel 即可
    true
}

/// 将英文模型目录名翻译为中文显示名。
/// 翻译参考 AND_Custom 角色包的中文命名风格。
/// 已经是中文的目录名直接返回。
pub fn translate_model_name(name: &str) -> String {
    match name {
        // ===== Hoshino 星野 =====
        "Hoshino" => "星野".into(),
        "Hoshino (2nd_Year_No_Scarf)" => "星野（二年级）（无围巾）".into(),
        "Hoshino (2nd_Year_Scarf)" => "星野（二年级）".into(),
        "Hoshino (Combat Readiness_Attack)" => "星野（临战）（攻击）".into(),
        "Hoshino (Combat_Readiness_Defense)" => "星野（临战）（防御）".into(),
        "Hoshino (First_Year)" => "星野（一年级）".into(),
        "Hoshino_Armed_Damaged" => "星野（武装）（战损）".into(),
        "Hoshino_Armed_Dual" => "星野（武装）（双持）".into(),
        "Hoshino_Armed_Dual_Damaged" => "星野（武装）（双持）（战损）".into(),
        "Hoshino_Armed_PistolShield" => "星野（武装）（手枪开盾）".into(),
        "Hoshino_Armed_PistolShield_Damaged" => "星野（武装）（手枪开盾）（战损）".into(),
        "Hoshino_Gym" => "星野（体操服）".into(),
        "Hoshino_Halloween" => "星野（恐怖）".into(),
        "Hoshino_Swimsuit" => "星野（泳装）".into(),

        // ===== Shiroko 白子 =====
        "Shiroko" => "白子".into(),
        "Shiroko_1st_Year" => "白子（一年级）".into(),
        "Shiroko_Cycling" => "白子（骑行）".into(),
        "Shiroko_Damaged" => "白子（战损）".into(),
        "Shiroko_Gym" => "白子（体操服）".into(),
        "Shiroko_Halloween" => "白子（恐怖）".into(),
        "Shiroko_Halloween_Damaged" => "白子（恐怖）（战损）".into(),
        "Shiroko_Halloween_WoundHeal" => "白子（恐怖）（创伤愈合）".into(),
        "Shiroko_Halloween_WoundHeal_Robber" => "白子（恐怖）（创伤愈合）（劫匪）".into(),
        "Shiroko_Swimsuit" => "白子（泳装）".into(),

        // ===== 其他角色 =====
        "Akane_Casual" => "茜（休闲装）".into(),
        "Aris_Winter" => "爱丽丝（冬装）".into(),
        "Ayane_Idol" => "绫音（偶像）".into(),
        "Chihiro_Pajama" => "千寻（睡衣）".into(),
        "Kaori_Idol" => "香织（偶像）".into(),
        "Kuchinashi" => "栀子梦".into(),
        "Maki_Pajama" => "真纪（睡衣）".into(),
        "Mari_Idol" => "玛丽（偶像）".into(),
        "Midori_Winter" => "绿（冬装）".into(),
        "Momoi_Winter" => "桃（冬装）".into(),
        "Noa_Pajama" => "诺亚（睡衣）".into(),
        "Rio_Winter" => "莉音（冬装）".into(),
        "Serika_Idol" => "芹香（偶像）".into(),
        "Smiling_Professor" => "笑面教授".into(),
        "Toki_Casual" => "时（休闲装）".into(),
        "Yuuka_Pajama" => "优香（睡衣）".into(),
        "Yuzu_Winter" => "柚子（冬装）".into(),

        // 已经是中文或未收录的名字直接返回
        _ => name.to_string(),
    }
}

/// 扫描 assets/skel 目录下所有内置模型，返回 (显示名, 完整路径) 列表。
/// Hoshino / Shiroko 是整合目录，会递归扫描其子目录中的变体。
pub fn list_builtin_models() -> Vec<(String, String)> {
    let mut models = Vec::new();
    let skel_dir = std::path::Path::new("assets/skel");
    if !skel_dir.exists() {
        return models;
    }
    if let Ok(entries) = std::fs::read_dir(skel_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if name == "Hoshino" || name == "Shiroko" {
                // 整合目录：递归扫描子目录
                if let Ok(sub_entries) = std::fs::read_dir(&path) {
                    for sub_entry in sub_entries.flatten() {
                        let sub_path = sub_entry.path();
                        if sub_path.is_dir() && is_supported_model_dir(&sub_path) {
                            let sub_name = sub_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("")
                                .to_string();
                            models.push((
                                translate_model_name(&sub_name),
                                sub_path.to_string_lossy().to_string(),
                            ));
                        }
                    }
                }
            } else if is_supported_model_dir(&path) {
                models.push((translate_model_name(&name), path.to_string_lossy().to_string()));
            }
        }
    }
    models.sort_by(|a, b| a.0.cmp(&b.0));
    models
}
