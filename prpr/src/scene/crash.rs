//! Crash scene for fun/debug purposes.
//! Displays a crash message with an error code and a retry button.
//! Visual style and animations are a direct copy of the ending scene.

use super::{draw_background, NextScene, Scene};
use crate::{
    config::Config,
    ext::{
        create_audio_manger, draw_parallelogram, draw_parallelogram_ex, draw_text_aligned, open_url,
        screen_aspect, SafeTexture, PARALLELOGRAM_SLOPE,
    },
    judge::Judge,
    time::TimeManager,
    ui::{Dialog, Ui},
};
use anyhow::Result;
use macroquad::prelude::*;
use sasa::{AudioClip, AudioManager, Music, MusicParams};
use std::env;
prpr_l10n::tl_file!("crash");

/// 内嵌报错页 BGM（编译期打入二进制，避免 Android 上 std::fs 读不到 assets 而 panic）。
const CRASH_BGM: &[u8] = include_bytes!("../../../assets/bgm/gameerror.mp3");
/// 内嵌报错页背景图。
const CRASH_BG_PNG: &[u8] = include_bytes!("../../../assets/errorbackground.png");



/// Error codes for the crash scene.
#[derive(Clone, Debug)]
pub enum CrashCode {
    // === 原有崩溃类型 ===
    ChartLoadTimeout,
    ResPackLoadTimeout,
    ManualCrash,
    /// The app caught a panic on the main thread and entered the crash screen
    /// instead of aborting.
    UnexpectedPanic {
        message: String,
    },
    Custom {
        code: u32,
        reason: String,
    },

    // === 资源加载类 (1000-1999) ===
    /// 图片/纹理加载失败
    ImageLoadFailed { message: String },
    /// 音频加载失败
    AudioLoadFailed { message: String },
    /// 字体加载失败
    FontLoadFailed { message: String },
    /// 资源包加载失败
    ResPackLoadFailed { message: String },
    /// 谱面文件加载失败
    ChartLoadFailed { message: String },
    /// 资源文件缺失
    AssetNotFound { path: String },

    // === 网络类 (2000-2999) ===
    /// 网络连接失败
    NetworkError { message: String },
    /// 服务器响应超时
    NetworkTimeout { message: String },
    /// API 请求失败
    ApiRequestFailed { message: String },
    /// 下载失败
    DownloadFailed { message: String },

    // === 解析类 (3000-3999) ===
    /// JSON 解析失败
    JsonParseError { message: String },
    /// 谱面解析失败
    ChartParseError { message: String },
    /// 配置文件解析失败
    ConfigParseError { message: String },

    // === 文件系统类 (4000-4999) ===
    /// 文件读取失败
    FileReadError { message: String },
    /// 文件写入失败
    FileWriteError { message: String },
    /// 存储空间不足
    StorageFull,

    // === 渲染类 (5000-5999) ===
    /// 纹理创建失败
    TextureCreateFailed { message: String },
    /// 着色器编译失败
    ShaderCompileFailed { message: String },
    /// 渲染上下文丢失
    RenderContextLost,

    // === 音频类 (6000-6999) ===
    /// 音频设备初始化失败
    AudioInitFailed { message: String },
    /// 音频播放失败
    AudioPlayFailed { message: String },

    // === 游戏逻辑类 (7000-7999) ===
    /// 游戏状态异常
    InvalidGameState { message: String },
    /// 判定系统错误
    JudgeSystemError { message: String },

    // === 系统类 (8000-8999) ===
    /// 内存不足
    OutOfMemory,
    /// 线程恐慌
    ThreadPanic { message: String },
    /// 空指针解引用
    NullPointerDeref,
    /// 索引越界
    IndexOutOfBounds { message: String },
    /// 算术溢出
    ArithmeticOverflow,

    // === 认证类 (9000-9999) ===
    /// 登录失败
    LoginFailed { message: String },
    /// Token 失效
    TokenExpired,
}

impl CrashCode {
    pub fn code(&self) -> u32 {
        match self {
            // 原有
            CrashCode::ChartLoadTimeout => 404,
            CrashCode::ResPackLoadTimeout => 501,
            CrashCode::ManualCrash => 951,
            CrashCode::UnexpectedPanic { .. } => 500,
            CrashCode::Custom { code, .. } => *code,

            // 资源加载类
            CrashCode::ImageLoadFailed { .. } => 1001,
            CrashCode::AudioLoadFailed { .. } => 1002,
            CrashCode::FontLoadFailed { .. } => 1003,
            CrashCode::ResPackLoadFailed { .. } => 1004,
            CrashCode::ChartLoadFailed { .. } => 1005,
            CrashCode::AssetNotFound { .. } => 1006,

            // 网络类
            CrashCode::NetworkError { .. } => 2001,
            CrashCode::NetworkTimeout { .. } => 2002,
            CrashCode::ApiRequestFailed { .. } => 2003,
            CrashCode::DownloadFailed { .. } => 2004,

            // 解析类
            CrashCode::JsonParseError { .. } => 3001,
            CrashCode::ChartParseError { .. } => 3002,
            CrashCode::ConfigParseError { .. } => 3003,

            // 文件系统类
            CrashCode::FileReadError { .. } => 4001,
            CrashCode::FileWriteError { .. } => 4002,
            CrashCode::StorageFull => 4003,

            // 渲染类
            CrashCode::TextureCreateFailed { .. } => 5001,
            CrashCode::ShaderCompileFailed { .. } => 5002,
            CrashCode::RenderContextLost => 5003,

            // 音频类
            CrashCode::AudioInitFailed { .. } => 6001,
            CrashCode::AudioPlayFailed { .. } => 6002,

            // 游戏逻辑类
            CrashCode::InvalidGameState { .. } => 7001,
            CrashCode::JudgeSystemError { .. } => 7002,

            // 系统类
            CrashCode::OutOfMemory => 8001,
            CrashCode::ThreadPanic { .. } => 8002,
            CrashCode::NullPointerDeref => 8003,
            CrashCode::IndexOutOfBounds { .. } => 8004,
            CrashCode::ArithmeticOverflow => 8005,

            // 认证类
            CrashCode::LoginFailed { .. } => 9001,
            CrashCode::TokenExpired => 9002,
        }
    }

    pub fn reason(&self) -> String {
        match self {
            // 原有
            CrashCode::ChartLoadTimeout => tl!("reason-chart-load-timeout").to_string(),
            CrashCode::ResPackLoadTimeout => tl!("reason-respack-load-timeout").to_string(),
            CrashCode::ManualCrash => tl!("reason-manual-crash").to_string(),
            CrashCode::UnexpectedPanic { message } => message.clone(),
            CrashCode::Custom { reason, .. } => {
                if reason.is_empty() {
                    tl!("reason-custom-default").to_string()
                } else {
                    reason.clone()
                }
            }

            // 资源加载类
            CrashCode::ImageLoadFailed { message } => tl!("reason-image-load-failed", "message" => message.clone()),
            CrashCode::AudioLoadFailed { message } => tl!("reason-audio-load-failed", "message" => message.clone()),
            CrashCode::FontLoadFailed { message } => tl!("reason-font-load-failed", "message" => message.clone()),
            CrashCode::ResPackLoadFailed { message } => tl!("reason-respack-load-failed", "message" => message.clone()),
            CrashCode::ChartLoadFailed { message } => tl!("reason-chart-load-failed", "message" => message.clone()),
            CrashCode::AssetNotFound { path } => tl!("reason-asset-not-found", "path" => path.clone()),

            // 网络类
            CrashCode::NetworkError { message } => tl!("reason-network-error", "message" => message.clone()),
            CrashCode::NetworkTimeout { message } => tl!("reason-network-timeout", "message" => message.clone()),
            CrashCode::ApiRequestFailed { message } => tl!("reason-api-request-failed", "message" => message.clone()),
            CrashCode::DownloadFailed { message } => tl!("reason-download-failed", "message" => message.clone()),

            // 解析类
            CrashCode::JsonParseError { message } => tl!("reason-json-parse-error", "message" => message.clone()),
            CrashCode::ChartParseError { message } => tl!("reason-chart-parse-error", "message" => message.clone()),
            CrashCode::ConfigParseError { message } => tl!("reason-config-parse-error", "message" => message.clone()),

            // 文件系统类
            CrashCode::FileReadError { message } => tl!("reason-file-read-error", "message" => message.clone()),
            CrashCode::FileWriteError { message } => tl!("reason-file-write-error", "message" => message.clone()),
            CrashCode::StorageFull => tl!("reason-storage-full").to_string(),

            // 渲染类
            CrashCode::TextureCreateFailed { message } => tl!("reason-texture-create-failed", "message" => message.clone()),
            CrashCode::ShaderCompileFailed { message } => tl!("reason-shader-compile-failed", "message" => message.clone()),
            CrashCode::RenderContextLost => tl!("reason-render-context-lost").to_string(),

            // 音频类
            CrashCode::AudioInitFailed { message } => tl!("reason-audio-init-failed", "message" => message.clone()),
            CrashCode::AudioPlayFailed { message } => tl!("reason-audio-play-failed", "message" => message.clone()),

            // 游戏逻辑类
            CrashCode::InvalidGameState { message } => tl!("reason-invalid-game-state", "message" => message.clone()),
            CrashCode::JudgeSystemError { message } => tl!("reason-judge-system-error", "message" => message.clone()),

            // 系统类
            CrashCode::OutOfMemory => tl!("reason-out-of-memory").to_string(),
            CrashCode::ThreadPanic { message } => tl!("reason-thread-panic", "message" => message.clone()),
            CrashCode::NullPointerDeref => tl!("reason-null-pointer-deref").to_string(),
            CrashCode::IndexOutOfBounds { message } => tl!("reason-index-out-of-bounds", "message" => message.clone()),
            CrashCode::ArithmeticOverflow => tl!("reason-arithmetic-overflow").to_string(),

            // 认证类
            CrashCode::LoginFailed { message } => tl!("reason-login-failed", "message" => message.clone()),
            CrashCode::TokenExpired => tl!("reason-token-expired").to_string(),
        }
    }

    /// 根据 panic 消息匹配对应的崩溃原因
    pub fn from_panic_message(message: &str) -> Self {
        let msg = message.to_lowercase();

        // 索引越界
        if msg.contains("index out of bounds") || msg.contains("out of range") {
            return CrashCode::IndexOutOfBounds { message: message.to_string() };
        }

        // 算术溢出
        if msg.contains("overflow") || msg.contains("underflow") || msg.contains("divide by zero") {
            return CrashCode::ArithmeticOverflow;
        }

        // 空指针
        if msg.contains("null") || msg.contains("none") || msg.contains("unwrap on none") {
            return CrashCode::NullPointerDeref;
        }

        // 内存不足
        if msg.contains("out of memory") || msg.contains("oom") || msg.contains("allocation failed") {
            return CrashCode::OutOfMemory;
        }

        // 图片加载
        if msg.contains("image") && (msg.contains("load") || msg.contains("decode") || msg.contains("format")) {
            return CrashCode::ImageLoadFailed { message: message.to_string() };
        }
        if msg.contains("texture") {
            return CrashCode::TextureCreateFailed { message: message.to_string() };
        }

        // 音频
        if msg.contains("audio") || msg.contains("sound") || msg.contains("music") || msg.contains("ogg") || msg.contains("mp3") {
            if msg.contains("init") || msg.contains("device") {
                return CrashCode::AudioInitFailed { message: message.to_string() };
            }
            return CrashCode::AudioLoadFailed { message: message.to_string() };
        }

        // 字体
        if msg.contains("font") || msg.contains("ttf") {
            return CrashCode::FontLoadFailed { message: message.to_string() };
        }

        // 资源包
        if msg.contains("respack") || msg.contains("resource pack") {
            return CrashCode::ResPackLoadFailed { message: message.to_string() };
        }

        // 谱面
        if msg.contains("chart") || msg.contains("beatmap") || msg.contains("pec") || msg.contains("pgr") || msg.contains("rpe") {
            if msg.contains("parse") || msg.contains("decode") {
                return CrashCode::ChartParseError { message: message.to_string() };
            }
            return CrashCode::ChartLoadFailed { message: message.to_string() };
        }

        // 网络
        if msg.contains("network") || msg.contains("connection") || msg.contains("disconnected") {
            return CrashCode::NetworkError { message: message.to_string() };
        }
        if msg.contains("timeout") || msg.contains("timed out") {
            return CrashCode::NetworkTimeout { message: message.to_string() };
        }
        if msg.contains("download") {
            return CrashCode::DownloadFailed { message: message.to_string() };
        }
        if msg.contains("http") || msg.contains("api") || msg.contains("request") {
            return CrashCode::ApiRequestFailed { message: message.to_string() };
        }

        // JSON 解析
        if msg.contains("json") || msg.contains("serde") || msg.contains("parse") {
            return CrashCode::JsonParseError { message: message.to_string() };
        }

        // 文件操作
        if msg.contains("file") || msg.contains("io error") || msg.contains("read") || msg.contains("write") {
            if msg.contains("write") || msg.contains("create") {
                return CrashCode::FileWriteError { message: message.to_string() };
            }
            return CrashCode::FileReadError { message: message.to_string() };
        }

        // 渲染
        if msg.contains("shader") || msg.contains("glsl") {
            return CrashCode::ShaderCompileFailed { message: message.to_string() };
        }
        if msg.contains("render") || msg.contains("opengl") || msg.contains("vulkan") || msg.contains("gpu") {
            return CrashCode::RenderContextLost;
        }

        // 登录/认证
        if msg.contains("login") || msg.contains("sign in") || msg.contains("auth") {
            return CrashCode::LoginFailed { message: message.to_string() };
        }
        if msg.contains("token") || msg.contains("unauthorized") || msg.contains("401") {
            return CrashCode::TokenExpired;
        }

        // 线程
        if msg.contains("thread") || msg.contains("panic") {
            return CrashCode::ThreadPanic { message: message.to_string() };
        }

        // 游戏状态
        if msg.contains("state") || msg.contains("invalid") {
            return CrashCode::InvalidGameState { message: message.to_string() };
        }

        // 判定系统
        if msg.contains("judge") || msg.contains("judgment") {
            return CrashCode::JudgeSystemError { message: message.to_string() };
        }

        // 资源缺失
        if msg.contains("not found") || msg.contains("no such file") || msg.contains("missing") {
            return CrashCode::AssetNotFound { path: message.to_string() };
        }

        // 默认：意外崩溃
        CrashCode::UnexpectedPanic { message: message.to_string() }
    }
}

pub struct CrashScene {
    code: CrashCode,
    enter_time: f32,
    background: Option<SafeTexture>,
    tip: String,
    audio: AudioManager,
    bgm: Music,
    black_duration: f32,
    custom_title: String,
}

const TIP_COUNT: usize = 149;

fn random_tip() -> String {
    use ::rand::Rng;
    let idx = ::rand::thread_rng().gen_range(0..TIP_COUNT);
    let key = format!("tip-{:03}", idx + 1);
    tl!(key).to_string()
}

impl CrashScene {
    pub fn new(code: CrashCode, custom_title: String) -> Self {
        let tip = random_tip();

        let config = Config::default();
        let mut audio = create_audio_manger(&config).expect("创建音频管理器失败");

        // 从编译期内嵌字节加载，避免 Android 上 std::fs 读不到 APK 内 assets 而 panic。
        let bgm_data = CRASH_BGM.to_vec();
        let clip = AudioClip::new(bgm_data).expect("音频数据解析失败");
        let bgm = audio
            .create_music(
                clip,
                MusicParams {
                    amplifier: 1.0,
                    loop_mix_time: 0.0,
                    ..Default::default()
                },
            )
            .expect("创建音乐失败");

        Self {
            code,
            enter_time: f32::NAN,
            background: None,
            tip,
            audio,
            bgm,
            black_duration: 0.3,
            custom_title,
        }
    }


    fn load_background(&mut self) {
        if self.background.is_some() {
            return;
        }

        // 直接用编译期内嵌的 PNG，避免 Android 上 std::fs 读不到 assets。
        if let Ok(img) = image::load_from_memory(CRASH_BG_PNG) {
            let rgba = img.to_rgba8();
            let (w, h) = (rgba.width(), rgba.height());
            let pixels = rgba.into_raw();
            let tex = Texture2D::from_rgba8(w as u16, h as u16, &pixels);
            self.background = Some(SafeTexture::from(tex));
            tracing::info!("成功加载内嵌背景图片");
        } else {
            tracing::warn!("无法加载内嵌 errorbackground.png，使用纯色背景");
        }
    }


    fn ran(t: f32, l: f32, r: f32) -> f32 {
        ((t - l) / (r - l)).clamp(0., 1.)
    }


    fn ease(t: f32) -> f32 {
        1. - (1. - t).powi(3)
    }


    fn tran(gl: &mut QuadGl, x: f32) {
        gl.push_model_matrix(Mat4::from_translation(vec3(x * 2., 0., 0.)));
    }


    fn draw_illustration(&self, x: f32, y: f32, w: f32, h: f32, _color: Color) -> Rect {
        let scale = 0.076;
        let w = scale * 13. * w;
        let h = scale * 7. * h;
        let r = Rect::new(x - w / 2., y - h / 2., w, h);
        let bg_color = Color::new(0.15, 0.15, 0.2, 0.5);
        draw_parallelogram(r, None, bg_color, true);
        let border_color = Color::new(0.6, 0.6, 0.7, 0.3);
        draw_parallelogram(r, None, border_color, false);
        let text_color = Color::new(0.8, 0.3, 0.3, 1.0);
        draw_text_ex(
            "!",
            r.x + r.w * 0.35,
            r.y + r.h * 0.75,
            TextParams {
                font_size: (r.h * 0.7) as u16,
                color: text_color,
                ..Default::default()
            },
        );
        r
    }
}

impl Scene for CrashScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {

        tm.reset();
        tm.seek_to(0.0);
        self.enter_time = tm.now() as f32;

        self.load_background();

        if let Err(e) = self.bgm.play() {
            tracing::warn!("播放背景音乐失败: {}", e);
        }
        Ok(())
    }

    fn update(&mut self, _tm: &mut TimeManager) -> Result<()> {

        if let Err(e) = self.audio.recover_if_needed() {
            tracing::warn!("音频恢复失败: {}", e);
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        let asp = screen_aspect();
        let top = 1. / asp;
        let now = tm.now() as f32;
        let elapsed = now - self.enter_time;


        if elapsed < self.black_duration {

            draw_rectangle(-1., -top, 2., top * 2., Color::new(0., 0., 0., 1.));
            return Ok(());
        }


        let mut gl = unsafe { get_internal_gl() }.quad_gl;

        set_camera(&Camera2D {
            zoom: vec2(1., -asp),
            ..Default::default()
        });


        if let Some(bg_tex) = &self.background {
            draw_background(**bg_tex);
        } else {
            draw_rectangle(-1., -top, 2., top * 2., Color::new(0.03, 0.03, 0.06, 1.));
        }

        let slope = PARALLELOGRAM_SLOPE;


        let illus_progress = Self::ease(Self::ran(now, 0.1, 1.3));
        Self::tran(&mut gl, (1. - illus_progress).powi(3));

        let r = self.draw_illustration(-0.38, 0.0, 1.0, 1.2, WHITE);

        let ratio = 0.2;
        draw_parallelogram_ex(
            Rect::new(r.x, r.y + r.h * (1. - ratio), r.w - r.h * (1. - ratio) * slope, r.h * ratio),
            None,
            Color::default(),
            Color::new(0., 0., 0., 0.7 * illus_progress),
            false,
        );

        let rr = draw_text_aligned(
            ui,
            "CRASH Lv.Error_999",
            r.right() - r.h / 7. * 13. * 0.13 - 0.01,
            r.bottom() - top / 20.,
            (1., 1.),
            0.46,
            Color::new(1., 1., 1., illus_progress),
        );
        let p = (r.x + 0.04, r.bottom() - top / 20.);
        let mw = rr.x - 0.02 - p.0;
        let code_text = tl!("crash-error-code", "code" => self.code.code().to_string());
        let mut text = ui.text(&code_text).pos(p.0, p.1).anchor(0., 1.).size(0.7);
        if text.measure().w <= mw {
            text.draw();
        } else {
            drop(text);
            ui.text(&code_text).pos(p.0, p.1).anchor(0., 1.).size(0.5).max_width(mw).draw();
        }

        gl.pop_model_matrix();


        let main_progress = Self::ease(Self::ran(now, 0.2, 1.3));
        Self::tran(&mut gl, (1. - main_progress).powi(3));

        let dx = 0.06;
        let c = Color::new(0., 0., 0., 0.6 * main_progress);
        let main = Rect::new(r.right() - 0.05, r.y, r.w * 0.84, r.h / 2.);
        draw_parallelogram(main, None, c, true);


        let title = if self.custom_title.is_empty() {
            tl!("crash-title").to_string()
        } else {
            self.custom_title.clone()
        };
        draw_text_aligned(
            ui,
            &title,
            main.x + dx,
            main.bottom() - 0.035,
            (0., 1.),
            0.34,
            Color::new(1., 1., 1., main_progress),
        );

        let reason = self.code.reason();
        let reason_lines: Vec<&str> = reason.split('\n').collect();
        for (i, line) in reason_lines.iter().enumerate() {
            let y_offset = 0.085 + i as f32 * 0.04;
            draw_text_aligned(
                ui,
                line,
                main.x + dx,
                main.bottom() - y_offset,
                (0., 1.),
                0.28,
                Color::new(1., 1., 1., main_progress * 0.7),
            );
        }

        let icon_size = main.h * 0.5;
        let icon_x = main.right() - main.h * slope - icon_size * 0.6;
        let icon_y = main.center().y - icon_size / 2.;
        draw_text_ex(
            "!",
            icon_x,
            icon_y + icon_size * 0.8,
            TextParams {
                font_size: (icon_size * 1.2) as u16,
                color: Color::new(1., 0.3, 0.3, main_progress),
                ..Default::default()
            },
        );

        gl.pop_model_matrix();


        let s1_progress = Self::ease(Self::ran(now, 0.4, 1.5));
        Self::tran(&mut gl, (1. - s1_progress).powi(3));

        let d = r.h / 16.;
        let s1 = Rect::new(main.x - d * 4. * slope, main.bottom() + d, main.w - d * 5. * slope, d * 3.);
        draw_parallelogram(s1, None, c, true);

        let detail = match self.code {
            CrashCode::ChartLoadTimeout => tl!("detail-chart-load-timeout").to_string(),
            CrashCode::ResPackLoadTimeout => tl!("detail-respack-load-timeout").to_string(),
            CrashCode::ManualCrash => tl!("detail-manual-crash").to_string(),
            CrashCode::UnexpectedPanic { .. } => tl!("detail-unexpected-panic").to_string(),
            CrashCode::Custom { .. } => tl!("detail-custom").to_string(),
            CrashCode::ImageLoadFailed { .. } => tl!("detail-image-load-failed").to_string(),
            CrashCode::AudioLoadFailed { .. } => tl!("detail-audio-load-failed").to_string(),
            CrashCode::FontLoadFailed { .. } => tl!("detail-font-load-failed").to_string(),
            CrashCode::ResPackLoadFailed { .. } => tl!("detail-respack-load-failed").to_string(),
            CrashCode::ChartLoadFailed { .. } => tl!("detail-chart-load-failed").to_string(),
            CrashCode::AssetNotFound { .. } => tl!("detail-asset-not-found").to_string(),
            CrashCode::NetworkError { .. } => tl!("detail-network-error").to_string(),
            CrashCode::NetworkTimeout { .. } => tl!("detail-network-timeout").to_string(),
            CrashCode::ApiRequestFailed { .. } => tl!("detail-api-request-failed").to_string(),
            CrashCode::DownloadFailed { .. } => tl!("detail-download-failed").to_string(),
            CrashCode::JsonParseError { .. } => tl!("detail-json-parse-error").to_string(),
            CrashCode::ChartParseError { .. } => tl!("detail-chart-parse-error").to_string(),
            CrashCode::ConfigParseError { .. } => tl!("detail-config-parse-error").to_string(),
            CrashCode::FileReadError { .. } => tl!("detail-file-read-error").to_string(),
            CrashCode::FileWriteError { .. } => tl!("detail-file-write-error").to_string(),
            CrashCode::StorageFull => tl!("detail-storage-full").to_string(),
            CrashCode::TextureCreateFailed { .. } => tl!("detail-texture-create-failed").to_string(),
            CrashCode::ShaderCompileFailed { .. } => tl!("detail-shader-compile-failed").to_string(),
            CrashCode::RenderContextLost => tl!("detail-render-context-lost").to_string(),
            CrashCode::AudioInitFailed { .. } => tl!("detail-audio-init-failed").to_string(),
            CrashCode::AudioPlayFailed { .. } => tl!("detail-audio-play-failed").to_string(),
            CrashCode::InvalidGameState { .. } => tl!("detail-invalid-game-state").to_string(),
            CrashCode::JudgeSystemError { .. } => tl!("detail-judge-system-error").to_string(),
            CrashCode::OutOfMemory => tl!("detail-out-of-memory").to_string(),
            CrashCode::ThreadPanic { .. } => tl!("detail-thread-panic").to_string(),
            CrashCode::NullPointerDeref => tl!("detail-null-pointer-deref").to_string(),
            CrashCode::IndexOutOfBounds { .. } => tl!("detail-index-out-of-bounds").to_string(),
            CrashCode::ArithmeticOverflow => tl!("detail-arithmetic-overflow").to_string(),
            CrashCode::LoginFailed { .. } => tl!("detail-login-failed").to_string(),
            CrashCode::TokenExpired => tl!("detail-token-expired").to_string(),
        };
        let dy = 0.025;
        draw_text_aligned(
            ui,
            &detail,
            s1.x + dx,
            s1.bottom() - dy,
            (0., 1.),
            0.34,
            Color::new(1., 1., 1., s1_progress),
        );
        draw_text_aligned(
            ui,
            &tl!("crash-suggest-restart"),
            s1.right() - dx,
            s1.bottom() - dy,
            (1., 1.),
            0.28,
            Color::new(0.7, 0.8, 1., s1_progress * 0.7),
        );

        gl.pop_model_matrix();


        let btn_p = (1. - Self::ran(now, 2.0, 2.7)).powi(2);
        let h = 0.1;
        let w = 0.17;
        let s = 0.05;
        let dy_btn = 0.006;
        let btn_bg = Color::new(0., 0., 0., 0.6);


        let complain_rect = Rect::new(-1. - h * slope, -top + dy_btn, w, h);
        Self::tran(&mut gl, -btn_p * 0.085);
        draw_parallelogram(complain_rect, None, btn_bg, true);
        draw_parallelogram(
            Rect::new(complain_rect.x + complain_rect.w * (1. - s), complain_rect.y, complain_rect.w * s, complain_rect.h),
            None,
            WHITE,
            false,
        );
        draw_text_aligned(
            ui,
            &tl!("crash-complain"),
            complain_rect.center().x,
            complain_rect.center().y,
            (0.5, 0.5),
            0.38,
            WHITE,
        );
        gl.pop_model_matrix();


        let restart_rect = Rect::new(1. + h * slope - w, top - dy_btn - 2. * h - 0.02, w, h);
        Self::tran(&mut gl, btn_p * 0.085);
        draw_parallelogram(restart_rect, None, btn_bg, true);
        draw_parallelogram(
            Rect::new(restart_rect.x + restart_rect.w * s, restart_rect.y, restart_rect.w * s, restart_rect.h),
            None,
            WHITE,
            false,
        );
        draw_text_aligned(
            ui,
            &tl!("crash-force-restart"),
            restart_rect.center().x,
            restart_rect.center().y,
            (0.5, 0.5),
            0.38,
            WHITE,
        );
        gl.pop_model_matrix();


        let exit_rect = Rect::new(1. + h * slope - w, top - dy_btn - h, w, h);
        Self::tran(&mut gl, btn_p * 0.085);
        draw_parallelogram(exit_rect, None, btn_bg, true);
        draw_parallelogram(
            Rect::new(exit_rect.x + exit_rect.w * s, exit_rect.y, exit_rect.w * s, exit_rect.h),
            None,
            WHITE,
            false,
        );
        draw_text_aligned(
            ui,
            &tl!("crash-exit"),
            exit_rect.center().x,
            exit_rect.center().y,
            (0.5, 0.5),
            0.38,
            WHITE,
        );
        gl.pop_model_matrix();


        if btn_p <= 0. {
            for touch in Judge::get_touches() {
                if touch.phase == TouchPhase::Ended {
                    if exit_rect.contains(touch.position) {
                        std::process::exit(0);
                    }
                    if complain_rect.contains(touch.position) {
                        Dialog::plain(tl!("crash-complain-title").to_string(), tl!("crash-complain-msg").to_string())
                            .buttons(vec![tl!("crash-cancel").to_string(), tl!("crash-go-complain").to_string()])
                            .listener(|_dialog, pos| {
                                if pos == 1 {
                                    let _ = open_url("https://qm.qq.com/q/NS4qvTszCg");
                                }
                                false
                            })
                            .show();
                    }
                    if restart_rect.contains(touch.position) {

                        if let Ok(exe) = env::current_exe() {
                            let _ = std::process::Command::new(exe).spawn();
                        }
                        std::process::exit(0);
                    }
                }
            }
        }


        let tip_margin = 0.03;
        draw_text_aligned(
            ui,
            &format!("Tip: {}", self.tip),
            -1.0 + tip_margin,
            top - tip_margin,
            (0., 1.),
            0.35,
            Color::new(1., 1., 1., 0.5),
        );

        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        NextScene::None
    }
}