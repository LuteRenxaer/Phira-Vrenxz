//! Phigros 第九章 SecretChallengeLife LIFE Bar 真实 GLSL shader 渲染。
//!
//! 依据：
//! - 真实 shader 反编译：life_bar_real_glsl.glsl（Phigros2_Level_SecretChallenge
//!   LIFE Bar，HLSLCC 转译 GLSL 300 es）。FRAGMENT 主体逐行移植到 macroquad
//!   GLSL 100（in→varying、texture→texture2D、SV_Target0→gl_FragColor、
//!   uint 条件标记→int 三元）；VERTEX 去掉 Unity ObjectToWorld/MatrixVP，
//!   改用 macroquad 2D Projection*Model，保留 _OpticalFOV 计算
//!   vs_TEXCOORD1 = 540 / (tan(clamp(_OpticalFOV,1,140)*0.00872664619) * 1.6)。
//! - 材质参数：F:\Phigros_Unity2022.3\...\materials\SecretChallengeLifeBar.mat
//!   (_Life/_BaseColor(0.741,0,0)/_NoiseColor(1,1,1)/_StrokeColor(1,0.402,0.402)/
//!    _StrokeWidth(3)/_GlowColor(1,0,0)/_GlowRadius(200)/_GlowExposure(1)/
//!    _NoiseDensity(7.42)/_NoiseContrast(15.6)/_NoiseThreshold(0.591)/
//!    _OpticalFOV(98.2)/_ShowInnerRectangle(1)/_TurbulenceOffset(960,0,0,0)/
//!    _TurbulenceFlow(0,0,0,0)/_TurbulenceFade(0.5))
//! - 噪声纹理：SecretChallengeLifeNoiseA.png / SecretChallengeLifeNoiseB.png（64x64）
//! - 管线模式：参照 prpr/src/core/block_shader.rs 的 load_material/set_uniform/
//!   set_texture/gl_use_material；顶点 UV 沿用 draw_rectangle 的 0..1 插值。
//!
//! fragment 语义（原版）：vs_TEXCOORD1（FOV 距离系数）与屏幕像素距离
//! (uv*1920,1080 - (960,288)) 反算角度缩放系数 coef（弧形来源）；核心条为
//! _Life*432.5 半宽、条中心 u_y=-188、半高 200 的中央弧形距离场（满血宽 865px 居中）；
//! _ShowInnerRectangle 开启时内部湍流噪声填充
//! （_NoiseA/_NoiseB 按 _TurbulenceFlow 流动、_TurbulenceFade 混合、对比度
//! _NoiseContrast、阈值 _NoiseThreshold）；描边 _StrokeWidth；条外辉光按
//! _GlowRadius 指数衰减、_GlowExposure 曝光；BackgroundMask 中央横带掩码
//! （u_y*coef+303，半宽 1206、半高 123、边缘 35px smoothstep）把条带裁成贴顶细弧带；
//! 最终乘 vs_COLOR0.w（顶点 alpha）。

use macroquad::prelude::*;
use miniquad::{BlendFactor, BlendState, BlendValue, Equation, UniformType};

// 只读嵌入素材（随项目 assets 目录一起打包进 exe，不再硬编码外部绝对路径）。
const NOISE_A_PNG: &[u8] = include_bytes!("../../../assets/textures/SecretChallengeLifeNoiseA.png");
const NOISE_B_PNG: &[u8] = include_bytes!("../../../assets/textures/SecretChallengeLifeNoiseB.png");

/// LIFE Bar 真实 shader 材质（材质 + 双噪声纹理）。
pub struct LifeBarMaterial {
    pub material: Material,
    pub noise_a: Texture2D,
    pub noise_b: Texture2D,
}

fn load_noise(bytes: &[u8]) -> Texture2D {
    // Exported PNG rows are top-down; Unity GLES samples with (0,0) bottom-left.
    let source = image::load_from_memory(bytes).unwrap().to_rgba8();
    let source = image::imageops::flip_vertical(&source);
    let texture = Texture2D::from_rgba8(source.width() as u16, source.height() as u16, source.as_raw());
    // 湍流云斑用双线性，避免 64x64 纹素块状化。
    texture.set_filter(FilterMode::Linear);
    texture
        .raw_miniquad_texture_handle()
        .set_wrap(unsafe { get_internal_gl() }.quad_context, miniquad::TextureWrap::Repeat);
    texture
}

const VERTEX: &str = r#"#version 100
attribute vec3 position;
attribute vec2 texcoord;
attribute vec4 color0;
uniform mat4 Model;
uniform mat4 Projection;
uniform float _OpticalFOV;
varying vec2 vs_TEXCOORD0;
varying float vs_TEXCOORD1;
varying vec4 vs_COLOR0;
void main() {
    gl_Position = Projection * Model * vec4(position, 1);
    // 原版 VERTEX：fov = clamp(_OpticalFOV,1,140)*0.00872664619;
    // vs_TEXCOORD1 = 540 / (tan(fov) * 1.6)
    float fov = clamp(_OpticalFOV, 1.0, 140.0) * 0.00872664619;
    float tan_fov = sin(fov) / cos(fov);
    vs_TEXCOORD1 = 540.0 / (tan_fov * 1.60000002);
    vs_TEXCOORD0 = texcoord;
    vs_COLOR0 = color0;
}
"#;

const FRAGMENT: &str = r#"#version 100
precision highp float;
precision highp int;

uniform float _NoiseDensity;
uniform float _NoiseContrast;
uniform float _NoiseThreshold;
uniform float _Life;
uniform float _ShowInnerRectangle;
uniform vec4 _BaseColor;
uniform vec4 _NoiseColor;
uniform vec4 _StrokeColor;
uniform float _StrokeWidth;
uniform vec4 _GlowColor;
uniform float _GlowRadius;
uniform float _GlowExposure;
uniform vec4 _TurbulenceOffset;
uniform vec4 _TurbulenceFlow;
uniform float _TurbulenceFade;
uniform float _DebugUV;
uniform float _DebugMode;
uniform sampler2D _NoiseA;
uniform sampler2D _NoiseB;

varying vec2 vs_TEXCOORD0;
varying float vs_TEXCOORD1;
varying vec4 vs_COLOR0;

vec4 u_xlat0;
mediump float u_xlat16_0;
bool u_xlatb0;
vec4 u_xlat1;
int u_xlati1;
bool u_xlatb1;
vec4 u_xlat2;
vec3 u_xlat3;
vec3 u_xlat4;
vec3 u_xlat5;
bvec3 u_xlatb5;
vec2 u_xlat6;
mediump float u_xlat16_6;
bool u_xlatb6;
float u_xlat7;
bool u_xlatb7;
float u_xlat12;
vec2 u_xlat13;
float u_xlat18;
float u_xlat19;
void main()
{
    // [_DEBUG_UV] 方向自检：uv.y=0 显示红色、uv.y=1 显示蓝色，用于验证 quad UV 在屏幕上的方向。
    if(_DebugUV>0.5){
        gl_FragColor = vec4(1.0 - vs_TEXCOORD0.y, 0.0, vs_TEXCOORD0.y, 1.0);
        return;
    }
    u_xlatb0 = 9.99999975e-05>=_Life;
    if(u_xlatb0){discard;}
    u_xlat0.x = max(vs_TEXCOORD1, 9.99999975e-05);
    // 坐标约定（macroquad draw_rectangle 顶点源码核对，2026-10-31）：
    //   顶部顶点 texcoord=(0,0)、底部=(0,1)，即 uv.y=0 在屏幕顶部、uv.y=1 在底部。
    //   原版公式 u_y = uv.y*1080 - 288 天然把条带定位在顶部贴边：
    //   顶部 u_y=-288、底部 u_y=792；条中心 u_y=-188、半高 200 → 顶部 0~300px 全宽横条。
    // x 方向 u_x = uv.x*1920 - 960。
    u_xlat6.x = vs_TEXCOORD0.x * 1920.0 - 960.0;
    u_xlat6.y = vs_TEXCOORD0.y * 1080.0 - 288.0;
    u_xlat18 = dot(u_xlat6.xy, u_xlat6.xy);
    u_xlat18 = sqrt(u_xlat18);
    u_xlat1.x = u_xlat18 / u_xlat0.x;
    u_xlat7 = min(u_xlat1.x, 1.0);
    u_xlat13.x = max(u_xlat1.x, 1.0);
    u_xlat13.x = float(1.0) / u_xlat13.x;
    u_xlat7 = u_xlat13.x * u_xlat7;
    u_xlat13.x = u_xlat7 * u_xlat7;
    u_xlat19 = u_xlat13.x * 0.0208350997 + -0.0851330012;
    u_xlat19 = u_xlat13.x * u_xlat19 + 0.180141002;
    u_xlat19 = u_xlat13.x * u_xlat19 + -0.330299497;
    u_xlat13.x = u_xlat13.x * u_xlat19 + 0.999866009;
    u_xlat19 = u_xlat13.x * u_xlat7;
    u_xlatb1 = 1.0<u_xlat1.x;
    u_xlat19 = u_xlat19 * -2.0 + 1.57079637;
    u_xlat1.x = u_xlatb1 ? u_xlat19 : float(0.0);
    u_xlat1.x = u_xlat7 * u_xlat13.x + u_xlat1.x;
    u_xlat0.x = u_xlat0.x * u_xlat1.x;
    u_xlat18 = max(u_xlat18, 9.99999975e-05);
    u_xlat0.x = u_xlat0.x / u_xlat18;
    u_xlat18 = _Life;
    u_xlat18 = clamp(u_xlat18, 0.0, 1.0);
    // [2026-10-31 还原原版] 保留角度系数 coef（u_xlat0.x，弧形来源），半宽 432.5、
    // 条中心 u_y=-188、半高 200；BackgroundMask 掩码带 u_w = u_y*coef + 303
    // （半宽 1206、半高 123、边缘 35px、1-smoothstep 带内保留）把条带裁成贴顶
    // 细弧带（coef=1 时屏幕 0~108px）。掩码偏移必须用 303，用旧值 1500 会让
    // 掩码带整体出屏、屏幕所有像素落在带外被裁成透明 → 血条全不见。
    u_xlat1.x = u_xlat18 * 432.5;
    u_xlat2 = u_xlat6.xyxy * u_xlat0.xxxx + vec4(0.0, 188.0, -70.0, 303.0);
    u_xlat1.y = 200.0;
    u_xlat1.xy = (-u_xlat1.xy) + abs(u_xlat2.xy);
    u_xlat13.xy = max(u_xlat1.xy, vec2(0.0, 0.0));
    u_xlat18 = dot(u_xlat13.xy, u_xlat13.xy);
    u_xlat18 = sqrt(u_xlat18);
    u_xlat1.x = max(u_xlat1.y, u_xlat1.x);
    u_xlat1.x = min(u_xlat1.x, 0.0);
    u_xlat18 = u_xlat18 + u_xlat1.x;
    float dbg_sdf = u_xlat18;
    // 原 HLSLCC uint 条件标记（0xFFFFFFFFu/uint(0)）改为 int 三元（GLSL ES 100 无 uint）
    u_xlati1 = (u_xlat18>=0.0) ? 1 : 0;
    u_xlatb7 = _ShowInnerRectangle>=0.5;
    u_xlat13.x = u_xlatb7 ? 1.0 : float(0.0);
    u_xlat3.xyz = u_xlat13.xxx * _BaseColor.xyz;
    u_xlat3.xyz = (int(u_xlati1) != 0) ? vec3(0.0, 0.0, 0.0) : u_xlat3.xyz;
    u_xlatb1 = u_xlati1==0;
    u_xlatb1 = u_xlatb7 && u_xlatb1;
    if(u_xlatb1){
        u_xlat0.xy = u_xlat6.xy * u_xlat0.xx + vec2(960.0, 288.0);
        u_xlat0.yz = u_xlat0.xy + (-_TurbulenceOffset.xy);
        u_xlat6.x = u_xlat0.y * 0.025527427;
        u_xlat1.x = max(_NoiseDensity, 0.00999999978);
        u_xlat0.x = u_xlat6.x * u_xlat1.x;
        u_xlat1 = u_xlat0.xzxz * vec4(0.015625, 2.60416673e-05, 0.015625, 2.60416673e-05) + _TurbulenceFlow;
        u_xlat16_0 = texture2D(_NoiseA, u_xlat1.xy).x;
        u_xlat16_6 = texture2D(_NoiseB, u_xlat1.zw).x;
        u_xlat6.x = (-u_xlat16_0) + u_xlat16_6;
        u_xlat0.x = _TurbulenceFade * u_xlat6.x + u_xlat16_0;
        u_xlat0.x = u_xlat0.x + (-_NoiseThreshold);
        u_xlat0.x = u_xlat0.x * _NoiseContrast;
        u_xlat0.x = max(u_xlat0.x, 0.0);
        u_xlat1.xyz = u_xlat0.xxx * _NoiseColor.xyz;
        u_xlat1.xyz = u_xlat3.xyz * u_xlat1.xyz;
        u_xlat4.xyz = u_xlat1.xyz + u_xlat1.xyz;
        u_xlat5.xyz = (-u_xlat3.xyz) + vec3(1.0, 1.0, 1.0);
        u_xlat5.xyz = u_xlat5.xyz + u_xlat5.xyz;
        u_xlat0.xyz = (-_NoiseColor.xyz) * u_xlat0.xxx + vec3(1.0, 1.0, 1.0);
        u_xlat0.xyz = (-u_xlat5.xyz) * u_xlat0.xyz + vec3(1.0, 1.0, 1.0);
        u_xlatb5.xyz = greaterThanEqual(u_xlat3.xyzx, vec4(0.5, 0.5, 0.5, 0.0)).xyz;
        u_xlat5.x = u_xlatb5.x ? float(1.0) : 0.0;
        u_xlat5.y = u_xlatb5.y ? float(1.0) : 0.0;
        u_xlat5.z = u_xlatb5.z ? float(1.0) : 0.0;
        u_xlat0.xyz = (-u_xlat1.xyz) * vec3(2.0, 2.0, 2.0) + u_xlat0.xyz;
        u_xlat3.xyz = u_xlat5.xyz * u_xlat0.xyz + u_xlat4.xyz;
    }
    u_xlat0.x = max(_StrokeWidth, 0.0);
    u_xlat0.x = (-u_xlat0.x) * 0.5 + abs(u_xlat18);
    u_xlat0.x = max(u_xlat0.x, 0.0);
    u_xlatb6 = _StrokeWidth>=0.00100000005;
    u_xlat6.x = u_xlatb6 ? 1.0 : float(0.0);
    u_xlat12 = min(u_xlat0.x, 1.0);
    u_xlat18 = u_xlat12 * -2.0 + 3.0;
    u_xlat12 = u_xlat12 * u_xlat12;
    u_xlat12 = (-u_xlat18) * u_xlat12 + 1.0;
    u_xlat6.x = u_xlat12 * u_xlat6.x;
    float dbg_stroke = u_xlat6.x;
    u_xlat12 = _GlowRadius * 0.300000012;
    u_xlat12 = max(u_xlat12, 1.0);
    u_xlat1.xyz = u_xlat0.xxx * vec3(-8.0, -2.4000001, -0.75);
    u_xlat0.xzw = u_xlat1.xyz / vec3(u_xlat12);
    u_xlat0.xzw = exp2(u_xlat0.xzw);
    u_xlat0.xz = u_xlat0.xz * vec2(0.560000002, 0.319999993);
    u_xlat0.x = u_xlat0.z + u_xlat0.x;
    u_xlat0.x = u_xlat0.w * 0.119999997 + u_xlat0.x;
    u_xlat12 = max(_GlowExposure, 0.0);
    u_xlat0.x = u_xlat12 * u_xlat0.x;
    u_xlat0.x = u_xlat0.x * 0.340000004;
    float dbg_glow = u_xlat0.x;
    u_xlat1.xyz = u_xlat0.xxx * _GlowColor.xyz;
    u_xlat4.xyz = u_xlat3.xyz;
    u_xlat4.xyz = clamp(u_xlat4.xyz, 0.0, 1.0);
    u_xlat4.xyz = (-u_xlat4.xyz) + vec3(1.0, 1.0, 1.0);
    u_xlat1.xyz = u_xlat1.xyz;
    u_xlat1.xyz = clamp(u_xlat1.xyz, 0.0, 1.0);
    u_xlat1.xyz = (-u_xlat1.xyz) + vec3(1.0, 1.0, 1.0);
    u_xlat4.xyz = (-u_xlat4.xyz) * u_xlat1.xyz + vec3(1.0, 1.0, 1.0);
    u_xlat3.xyz = u_xlat3.xyz + vec3(-1.0, -1.0, -1.0);
    u_xlat3.xyz = max(u_xlat3.xyz, vec3(0.0, 0.0, 0.0));
    u_xlat3.xyz = u_xlat3.xyz + u_xlat4.xyz;
    u_xlat0.xzw = _GlowColor.xyz * u_xlat0.xxx + vec3(-1.0, -1.0, -1.0);
    u_xlat0.xzw = max(u_xlat0.xzw, vec3(0.0, 0.0, 0.0));
    u_xlat3.xyz = u_xlat0.xzw + u_xlat3.xyz;
    u_xlat4.xyz = min(u_xlat3.xyz, vec3(1.0, 1.0, 1.0));
    u_xlat4.xyz = (-u_xlat4.xyz) + vec3(1.0, 1.0, 1.0);
    u_xlat1.xyz = (-u_xlat4.xyz) * u_xlat1.xyz + vec3(1.0, 1.0, 1.0);
    u_xlat3.xyz = u_xlat3.xyz + vec3(-1.0, -1.0, -1.0);
    u_xlat3.xyz = max(u_xlat3.xyz, vec3(0.0, 0.0, 0.0));
    u_xlat1.xyz = u_xlat1.xyz + u_xlat3.xyz;
    u_xlat0.xzw = u_xlat0.xzw + u_xlat1.xyz;
    u_xlat1.xyz = (-u_xlat0.xzw) + _StrokeColor.xyz;
    u_xlat0.xyz = u_xlat6.xxx * u_xlat1.xyz + u_xlat0.xzw;
    // [2026-10-31 还原原版] BackgroundMask 中央横带掩码（life_bar_real_glsl.glsl 原渐隐段）：
    // u_xlat2.zw = u_y*coef+303，半宽 1206、半高 123、边缘 35px。
    // 注意方向：末端是 1-smoothstep（带内保留、带外裁为 0/discard），不是挖空——
    // 掩码带 u_y*coef∈[-426,-180]（coef=1 时屏幕 0~108px）内保留，条带主体
    // u_y∈[-388,12]（0~300px）其余部分被裁掉，只留顶部贴顶细弧带（参考图样式）。
    // 若把偏移从 303 改成 1500，掩码带出屏 → 全屏落在带外 → 整条血条被裁成透明。
    u_xlat1.xy = abs(u_xlat2.zw) + vec2(-1206.0, -123.0);
    u_xlat13.xy = max(u_xlat1.xy, vec2(0.0, 0.0));
    u_xlat18 = dot(u_xlat13.xy, u_xlat13.xy);
    u_xlat18 = sqrt(u_xlat18);
    u_xlat1.x = max(u_xlat1.y, u_xlat1.x);
    u_xlat1.x = min(u_xlat1.x, 0.0);
    u_xlat18 = u_xlat18 + u_xlat1.x;
    // [2026-10-31 v5] 掩码边缘平滑 35px→8px：原版 35px smoothstep 在深色背景上
    // 形成 35px 暗色过渡带（用户反馈"黑底/黑边"）；缩到 8px 后过渡几乎不可见，
    // 条带显示区同步从 0~73px 扩到 0~100px，更接近参考图血条厚度。
    u_xlat18 = u_xlat18 + 8.0;
    u_xlat18 = u_xlat18 * 0.0076335878;
    u_xlat18 = clamp(u_xlat18, 0.0, 1.0);
    u_xlat1.x = u_xlat18 * -2.0 + 3.0;
    u_xlat18 = u_xlat18 * u_xlat18;
    u_xlat18 = (-u_xlat1.x) * u_xlat18 + 1.0;
    u_xlat0.xyz = vec3(u_xlat18) * u_xlat0.xyz;
    // [2026-10-31 v3] RGB 不再乘 vs_COLOR0.w：顶点 alpha 只用于混合权重，避免血条被压暗成暗红
    // (之前 RGB*0.6 + alpha*0.6 双重衰减，深色背景上仍是实心暗红观感，用户反馈"没改一样")
    // [DEBUG] 四分区诊断输出（_DebugMode>0.5 时启用，验证后删除）
    //  左上: 噪声纹理直接采样 (R=NoiseA, G=NoiseB)
    //  右上: SDF 距离场（绿=条内sdf<0, 红=条外, 亮度=|sdf|/100）
    //  左下: 描边权重 dbg_stroke 灰度
    //  右下: 辉光强度 dbg_glow 灰度
    if(_DebugMode > 0.5){
        vec2 d_uv = vs_TEXCOORD0.xy;
        // [DEBUG v2 2026-10-31] 四分区压缩到条带所在顶部区域（条带 0~300px ≈ uv.y<0.28）。
        // 上一版把描边/辉光放在下半屏，而条带外 SDF 权重恒 0，导致误判"描边/辉光全黑"。
        if(d_uv.y < 0.3){
            if(d_uv.x < 0.25){
                gl_FragColor = vec4(texture2D(_NoiseA, d_uv * 16.0).r, texture2D(_NoiseB, d_uv * 16.0).r, 0.0, 1.0);
            } else if(d_uv.x < 0.5){
                float t = clamp(dbg_sdf * 0.01 + 0.5, 0.0, 1.0);
                gl_FragColor = vec4(t, 1.0 - t, 0.0, 1.0);
            } else if(d_uv.x < 0.75){
                gl_FragColor = vec4(dbg_stroke, dbg_stroke, dbg_stroke, 1.0);
            } else {
                gl_FragColor = vec4(dbg_glow, dbg_glow, dbg_glow, 1.0);
            }
        } else {
            gl_FragColor = vec4(0.0, 0.0, 0.0, 1.0);
        }
        return;
    }
    u_xlat1.x = max(u_xlat0.y, u_xlat0.x);
    u_xlat0.w = max(u_xlat0.z, u_xlat1.x);
    u_xlatb1 = 9.99999975e-06>=u_xlat0.w;
    if(u_xlatb1){discard;}
    // [2026-10-31 v13 恢复原版颜色合成·删除二次压暗] 新实机截图
    // (clipboard_1793448919824) 量化定位：亮红块之间被 v12 二次曲线压成
    // R<50 的纯黑/黑底微红，血条碎成一块块"亮红+黑间隔"，比参考图难看。
    // 参考图同位置是 R~106-122 暗红——即原版噪声暗区自然输出
    // (2*_BaseColor-1≈0.482→R≈123)，二次压暗反而破坏了原版层次。
    // v13：恢复 u_xlat0.xyz 原版合成色（噪声亮/暗交替、描边、辉光衰减全部
    // 按原版公式保留）；仅保留极暗 discard（透明底）与下方 sdf>3 条带外透明。
    if(u_xlat0.w < 0.06){ discard; }
    // [2026-10-31 v14 红底→阴影渐变] 透明度：顶部 100% → 底部 0% 垂直渐变。
    // uv.y=0（屏幕顶部）alpha=1.0，uv.y=0.1（掩码带底 ≈108px）alpha=0.0，
    // 线性下渐变形成"上实下虚"的阴影效果（替代原 v13 恒定 0.6 半透明红底）。
    u_xlat0.w = clamp(1.0 - vs_TEXCOORD0.y * 10.0, 0.0, 1.0);
    // [2026-10-31 v6] 条带外彻底透明：参考图是透明底而非黑底——条带外区域
    // （sdf>3，描边/辉光内圈之外）不仅有暗红辉光残影，还叠着黑底 RGB，
    // 压 alpha 只减弱混合权重，暗色 RGB 仍会透出来。直接 RGB 归零 → alpha=0
    // → discard，条带外完全透明（保留 |sdf|≤3 的描边与内圈辉光）。
    if(dbg_sdf > 3.0){
        u_xlat0.xyz = vec3(0.0);
        u_xlat0.w = 0.0;
    }
    gl_FragColor = u_xlat0;
    return;
}
"#;

impl LifeBarMaterial {
    /// 构建 LIFE Bar 材质并设置全部常量 uniforms（_Life 等逐帧由调用方更新）。
    pub fn load() -> Result<Self, miniquad::ShaderError> {
        let noise_a = load_noise(NOISE_A_PNG);
        let noise_b = load_noise(NOISE_B_PNG);
        let params = MaterialParams {
            // [2026-10-31 修复] 自定义材质默认 pipeline 不带 color_blend，
            // alpha 通道被丢弃导致血条始终纯色实心；补上 SrcAlpha/OneMinusSrcAlpha
            // 后顶点 alpha（vs_COLOR0.w）才能真正参与混合，实现半透明（原版 Unity
            // 管线默认即为该混合模式）。
            pipeline_params: PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
            uniforms: vec![
                ("_NoiseDensity".to_owned(), UniformType::Float1),
                ("_NoiseContrast".to_owned(), UniformType::Float1),
                ("_NoiseThreshold".to_owned(), UniformType::Float1),
                ("_Life".to_owned(), UniformType::Float1),
                ("_ShowInnerRectangle".to_owned(), UniformType::Float1),
                ("_BaseColor".to_owned(), UniformType::Float4),
                ("_NoiseColor".to_owned(), UniformType::Float4),
                ("_StrokeColor".to_owned(), UniformType::Float4),
                ("_StrokeWidth".to_owned(), UniformType::Float1),
                ("_GlowColor".to_owned(), UniformType::Float4),
                ("_GlowRadius".to_owned(), UniformType::Float1),
                ("_GlowExposure".to_owned(), UniformType::Float1),
                ("_TurbulenceOffset".to_owned(), UniformType::Float4),
                ("_TurbulenceFlow".to_owned(), UniformType::Float4),
                ("_TurbulenceFade".to_owned(), UniformType::Float1),
                ("_DebugUV".to_owned(), UniformType::Float1),
                ("_DebugMode".to_owned(), UniformType::Float1),
                ("_OpticalFOV".to_owned(), UniformType::Float1),
            ],
            textures: vec!["_NoiseA".to_owned(), "_NoiseB".to_owned()],
            ..Default::default()
        };
        let material = load_material(VERTEX, FRAGMENT, params)?;
        // 常量 uniforms（SecretChallengeLifeBar.mat 原值）
        material.set_uniform("_BaseColor", vec4(0.7411765, 0.0, 0.0, 1.0));
        material.set_uniform("_NoiseColor", vec4(1.0, 1.0, 1.0, 1.0));
        material.set_uniform("_StrokeColor", vec4(1.0, 0.4023534, 0.4023534, 1.0));
        material.set_uniform("_StrokeWidth", 3.0f32);
        material.set_uniform("_GlowColor", vec4(1.0, 0.0, 0.0, 1.0));
        material.set_uniform("_GlowRadius", 200.0f32);
        material.set_uniform("_GlowExposure", 1.0f32);
        material.set_uniform("_NoiseDensity", 7.42f32);
        material.set_uniform("_NoiseContrast", 15.6f32);
        material.set_uniform("_NoiseThreshold", 0.591f32);
        material.set_uniform("_OpticalFOV", 98.2f32);
        material.set_uniform("_ShowInnerRectangle", 1.0f32);
        material.set_uniform("_TurbulenceOffset", vec4(960.0, 0.0, 0.0, 0.0));
        material.set_uniform("_TurbulenceFlow", vec4(0.0, 0.0, 0.0, 0.0));
        material.set_uniform("_TurbulenceFade", 0.5f32);
        material.set_uniform("_Life", 1.0f32);
        material.set_uniform("_DebugUV", 0.0f32);
        material.set_uniform("_DebugMode", 0.0f32); // 诊断探针已排完，恢复真实 shader 渲染
        material.set_texture("_NoiseA", noise_a);
        material.set_texture("_NoiseB", noise_b);
        Ok(Self {
            material,
            noise_a,
            noise_b,
        })
    }
}
