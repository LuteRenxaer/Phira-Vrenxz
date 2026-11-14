use super::startup::{ASSET_PACKAGES, ExtractProgress};
use super::MainScene;
use crate::get_data;
prpr_l10n::tl_file!("login");
use prpr::{
    ext::{poll_future, semi_white, LocalTask, SafeTexture, ScaleType},
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::{FontArc, Ui, UI_AUDIO},
};
#[cfg(feature = "video")]
use prpr::core::{Anim, Video};
use anyhow::Result;
use futures_util::Future;
use macroquad::prelude::*;
use sasa::{AudioClip, Music, MusicParams};
use std::pin::Pin;
use std::sync::{mpsc, Arc, Mutex};

// ==== 视觉时间线（BootScene 层级：Background→Logo→Warning，场景内连续渐显）====
// Phigros BootScene.unity 的 Canvas 层级为 Background(全屏底)→Logo(居中)→Warning(全屏警示图) 叠加。
const LOGO_WAIT: f32 = 0.3; // WaitForSeconds(0.3f)：Logo 显示前黑屏等待（BootSceneControl）
const FADE_IN: f32 = 0.8; // LogoSplash fadeIn：alpha 0→1 线性
const HOLD: f32 = 1.2; // LogoSplash hold：保持
const FADE_OUT: f32 = 0.6; // LogoSplash fadeOut：alpha 1→0 线性
const LOGO_END: f32 = LOGO_WAIT + FADE_IN + HOLD + FADE_OUT; // 2.9s：Logo 总时长（与 Phigros 一致）
const TIP_START: f32 = LOGO_WAIT + FADE_IN + HOLD; // 2.3s：与 Logo 淡出起点重叠（Warning 渐显）
const TIP_FADE: f32 = 0.5; // Anim(warning)：温馨提示图线性渐显时长
const TIP_SHOW: f32 = 5.0; // Warning 完整显示时长：3 秒后开始渐变消失（不等到白闪后）
const TIP_FADE_OUT: f32 = 0.5; // Warning 淡出时长
const BGM_FADE: f32 = 2.0; // 音频淡入：2 秒音量 0→1 线性（BootScene SplashScene3AudioPlayer Volume=0 语义）
// ==== 白闪（固定卡点，非点击驱动）====
// BGM 在 LOGO_END=2.9s 起播（fade_in 2s 到满音量），白闪卡在 BGM 第 8 秒，即场景 2.9+8.0=10.9s。
const FLASH_AT: f32 = LOGO_END + 8.0; // 10.9s：BGM 卡点白闪
/// 白闪卡点对应的 BGM 播放位置（BGM 从 LOGO_END 起播，第 8 秒是卡点）
const BGM_FLASH_POS: f64 = 8.0;
/// 白闪总时长：起白 → 模糊主图 → 逐渐清晰（对齐参考视频开屏的「白→花」过渡，约 1 秒）
const REVEAL_TIME: f32 = 1.05;
// ==== Login 开始界面（白闪结束即完全清晰，之后可点击继续）====
const LOGIN_START: f32 = FLASH_AT + REVEAL_TIME;
// ==== 切主页过渡（点击「继续」后先等过渡播完，再真正 Replace 到主页）====
const SWITCH_FADE: f32 = 0.02; // 点击「继续」后到切场景之间的过渡时长
// ==== Login 背景网格流动 + 贴图叠加（Phigros 网格背景风格）====
// 网格背景层与贴图叠加层共用同一流动幅度；颜色固定为原版银白菱形网格（不循环变色）。
const GRID_FLOW: Vec2 = vec2(0.01, 0.0); // 网格/贴图统一水平流动（极慢，约每秒 1% 屏宽，对齐参考图静态观感）
const GRID_CELL: Vec2 = vec2(0.05, 0.0889); // 菱形单元密度：横向 20 个、纵向 11.25 个（16:9 下正菱形，对齐参考图横20纵12）

pub struct StudioLogoScene {
    next_scene: Option<Box<dyn Scene>>,
    logo: Option<SafeTexture>,
    tip: Option<SafeTexture>,
    /// 开屏主视觉（loginbg.png 原图；右侧压暗成面板，与主页共用同一版式）
    art: Option<SafeTexture>,
    /// 开屏/主页共用的主视觉底板（右侧深色网格面板）
    backdrop: crate::splash_ui::Backdrop,
    #[cfg(feature = "video")]
    home_video: Option<Video>,
    #[cfg(feature = "video")]
    home_video_start: f64,
    #[cfg(feature = "video")]
    home_video_path: Option<String>,
    #[cfg(feature = "video")]
    home_video_rx: Option<mpsc::Receiver<Vec<u8>>>,
    bgm: Option<Music>,
    bgm_started: bool,
    enter_time: f32,
    // 白闪：flash_start 为固定卡点（BGM 第 7.5s）触发后的起点，None=尚未到白闪时刻
    flash_start: Option<f32>,
    // Login 阶段用户点击「继续」后请求切换主界面（白闪后不自动进 Home）
    switch_pending: bool,
    // 切主页过渡起点：点击「继续」的时刻；过渡动画（白色淡出）播完才真正 Replace 到主页
    switch_start: Option<f32>,
    // ==== 开屏期间后台加载主界面（对齐 Phigros：BootScene 为唯一启动场景，加载完成后直接 LoadScene）====
    load_task: LocalTask<Result<MainScene>>,
    ready_scene: Option<Box<dyn Scene>>,
    load_error: Option<String>,
    extract_progress: Option<Arc<Mutex<ExtractProgress>>>,
    extract_handle: Option<std::thread::JoinHandle<()>>,
    fallback_font: Option<FontArc>,
}

impl StudioLogoScene {
    pub async fn new(next_scene: Option<Box<dyn Scene>>, fallback: FontArc) -> Self {
        let logo = match load_texture("icons/Studio_Logo.png").await {
            Ok(tex) => Some(tex.into()),
            Err(e) => {
                warn!("failed to load Studio_Logo.png: {:?}", e);
                None
            }
        };
        let tip = match load_texture("icons/zh_CNwxts.png").await {
            Ok(tex) => Some(tex.into()),
            Err(e) => {
                warn!("failed to load zh_CNwxts.png: {:?}", e);
                None
            }
        };
        // 开屏主视觉：与主页共用同一张左侧主图（自定义背景 > 内置背景 > loginbg.png）。
        // 自定义背景只换左边这张图，右侧黑场 / TriGrid / 粒子仍由 splash_ui::Backdrop 绘制。
        let art = match crate::splash_ui::load_key_art().await {
            Ok(tex) => Some(tex),
            Err(err) => {
                warn!("failed to load key art: {err:?}");
                None
            }
        };

        // TriGrid 三通道贴图（R=网格线 / G=辉光 / B=面色），Phigros 主菜单三角网格背景
        if let Ok(tex) = load_texture("textures/TriGridBase.png").await {
            tex.set_filter(FilterMode::Linear);
            crate::splash_ui::set_tri_grid(SafeTexture::from(tex));
        }

        // 标题贴图：读原图、裁掉透明边再上传，开屏和主页共用
        match load_file("icons/phiravrenxz_icon.png").await {
            Ok(bytes) => match image::load_from_memory(&bytes) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
                    for (x, y, p) in rgba.enumerate_pixels() {
                        if p[3] > 8 {
                            x0 = x0.min(x);
                            y0 = y0.min(y);
                            x1 = x1.max(x + 1);
                            y1 = y1.max(y + 1);
                        }
                    }
                    if x1 > x0 && y1 > y0 {
                        let crop = image::imageops::crop_imm(&rgba, x0, y0, x1 - x0, y1 - y0).to_image();
                        let tex = Texture2D::from_rgba8(crop.width() as u16, crop.height() as u16, crop.as_raw());
                        tex.set_filter(FilterMode::Linear);
                        crate::splash_ui::set_logo_tex(Some(tex.into()));
                    }
                }
                Err(e) => warn!("failed to decode phiravrenxz_icon.png: {:?}", e),
            },
            Err(e) => warn!("failed to load phiravrenxz_icon.png: {:?}", e),
        }

        // 开屏和主页共用同一首 BGM（设置里的自定义音乐，或默认开屏曲）。
        // 进主页时不会重新加载，而是把这一个 Music 直接交给 MainScene 继续播。
        let bgm = match crate::scene::load_shared_bgm_bytes()
            .and_then(|data| AudioClip::new(data).map_err(Into::into))
        {
            Ok(clip) => UI_AUDIO
                .with(|it| {
                    it.borrow_mut().create_music(
                        clip,
                        MusicParams {
                            // 开屏 / 主页共用 BGM 音量：和设置里的 BGM 音量保持一致。
                            amplifier: get_data().config.volume_bgm,
                            loop_mix_time: 0.,
                            ..Default::default()
                        },
                    )
                })
                .ok(),
            Err(err) => {
                warn!("failed to load shared BGM: {err:#}");
                None
            }
        };

        // 初始化主界面加载任务（复用 StartupLoadingScene 的解压 + MainScene 加载逻辑）
        let assets_dir = super::assets_root();
        let marker = assets_dir.join(".extracted_builtin");
        let need_extract = match std::fs::read_to_string(&marker) {
            Ok(stamp) => stamp.trim() != super::startup::extract_stamp(&assets_dir),
            Err(_) => !super::startup::all_packages_complete(&assets_dir),
        };

        let (extract_progress, extract_handle, load_task, fallback_font) = if need_extract {
            let progress = Arc::new(Mutex::new(ExtractProgress {
                current: 0,
                total: ASSET_PACKAGES.len(),
                message: "准备解压...".to_string(),
                done: false,
            }));
            let progress_clone = progress.clone();
            let handle = std::thread::spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    super::startup::extract_builtin_assets(&progress_clone);
                }));
                if result.is_err() {
                    if let Ok(mut p) = progress_clone.lock() {
                        p.done = true;
                        p.message = "解压时发生错误".to_string();
                    }
                }
            });
            (Some(progress), Some(handle), None, Some(fallback))
        } else {
            let task: Pin<Box<dyn Future<Output = Result<MainScene>>>> =
                Box::pin(async move { MainScene::new(fallback).await });
            (None, None, Some(task), None)
        };

        Self {
            next_scene,
            logo,
            tip,
            art,
            backdrop: crate::splash_ui::Backdrop::new(),
            #[cfg(feature = "video")]
            home_video: None,
            #[cfg(feature = "video")]
            home_video_start: 0.,
            #[cfg(feature = "video")]
            home_video_path: None,
            #[cfg(feature = "video")]
            home_video_rx: None,
            bgm,
            bgm_started: false,
            enter_time: f32::NAN,
            flash_start: None,
            switch_pending: false,
            switch_start: None,
            load_task,
            ready_scene: None,
            load_error: None,
            extract_progress,
            extract_handle,
            fallback_font,
        }
    }

    /// 跳过开屏：把画面时钟直接挪到白闪卡点，并把 BGM seek 到同一拍。
    /// 开屏也使用玩家设置的主页动态背景视频。
    #[cfg(feature = "video")]
    fn update_video(&mut self, now: f64) {
        let path = get_data().home_video_path.clone();
        if path != self.home_video_path {
            self.home_video_path = path.clone();
            self.home_video = None;
            self.home_video_rx = None;
            if let Some(path) = path {
                let (tx, rx) = mpsc::channel();
                std::thread::spawn(move || {
                    if let Ok(data) = std::fs::read(&path) {
                        let _ = tx.send(data);
                    }
                });
                self.home_video_rx = Some(rx);
            }
        }

        if let Some(rx) = self.home_video_rx.take() {
            match rx.try_recv() {
                Ok(data) => match Video::new(
                    data,
                    0.,
                    ScaleType::CropCenter,
                    Anim::fixed(1.),
                    Anim::fixed(0.),
                ) {
                    Ok(video) => {
                        self.home_video_start = now;
                        self.home_video = Some(video);
                    }
                    Err(err) => warn!("failed to open splash video: {err:#}"),
                },
                Err(mpsc::TryRecvError::Empty) => self.home_video_rx = Some(rx),
                Err(mpsc::TryRecvError::Disconnected) => {}
            }
        }

        if let Some(video) = &mut self.home_video {
            if now < self.home_video_start || now - self.home_video_start >= video.duration {
                self.home_video_start = now;
                let _ = video.reset();
            }
            let _ = video.update(now - self.home_video_start);
        }
    }

    fn skip_to_flash(&mut self, tm: &mut TimeManager) {
        let now = tm.now() as f32;
        self.enter_time = now - FLASH_AT;
        self.flash_start = Some(FLASH_AT);
        let bgm_was_started = self.bgm_started;
        self.bgm_started = true;
        if let Some(bgm) = &mut self.bgm {
            let _ = bgm.seek_to(BGM_FLASH_POS);
            if !bgm_was_started {
                let _ = bgm.fade_in(0.2);
            }
        }
    }
}

impl Scene for StudioLogoScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.enter_time.is_nan() {
            self.enter_time = tm.now() as f32;
        }
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = tm.now() as f32;
        #[cfg(feature = "video")]
        self.update_video(macroquad::time::get_time() as f64);
        // 对齐 Phigros：Logo 淡出后（Warning 渐显时）AudioFadeIn 启动 BGM
        if !self.bgm_started && !self.enter_time.is_nan() && t - self.enter_time >= LOGO_END {
            self.bgm_started = true;
            if let Some(bgm) = &mut self.bgm {
                let _ = bgm.fade_in(BGM_FADE as f64);
            }
        }

        // 白闪：BGM 播放到第 7.5 秒固定卡点触发（点击不提前），白闪 1.2s 后进入 Login；BGM 全程不停
        let elapsed = if self.enter_time.is_nan() { 0. } else { t - self.enter_time };
        // 键盘也能跳过开屏，直接到白闪卡点
        if self.flash_start.is_none()
            && elapsed < FLASH_AT
            && (is_key_pressed(KeyCode::Space) || is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Escape))
        {
            self.skip_to_flash(tm);
        }
        let elapsed = if self.enter_time.is_nan() { 0. } else { t - self.enter_time };
        if self.flash_start.is_none() && elapsed >= FLASH_AT {
            self.flash_start = Some(elapsed);
        }

        // Login 阶段（白闪结束后）：点击「继续」才请求切换主界面，不自动进 Home
        if !self.switch_pending && elapsed >= LOGIN_START {
            let any_input = is_mouse_button_pressed(MouseButton::Left)
                || is_key_pressed(KeyCode::Space)
                || is_key_pressed(KeyCode::Enter)
                || is_key_pressed(KeyCode::Escape);
            if any_input {
                self.switch_pending = true;
                self.switch_start = Some(elapsed);
            }
        }

        // 开屏播放期间后台推进主界面加载
        if let Some(progress) = &self.extract_progress {
            let done = progress.lock().map(|p| p.done).unwrap_or(true);
            if done && self.load_task.is_none() {
                if let Some(fallback) = self.fallback_font.take() {
                    let task: Pin<Box<dyn Future<Output = Result<MainScene>>>> =
                        Box::pin(async move { MainScene::new(fallback).await });
                    self.load_task = Some(task);
                }
            }
        }
        if let Some(future) = self.load_task.as_mut() {
            if let Some(res) = poll_future(future.as_mut()) {
                self.load_task = None;
                match res {
                    Ok(scene) => {
                        self.ready_scene = Some(Box::new(scene));
                    }
                    Err(err) => {
                        self.load_error = Some(format!("{err:#}"));
                    }
                }
            }
        }

        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if touch.phase == TouchPhase::Started {
            let elapsed = if self.enter_time.is_nan() { 0. } else { tm.now() as f32 - self.enter_time };
            // 开屏阶段点击任意位置：跳到白闪卡点；Login 阶段才是点击继续。
            if self.flash_start.is_none() && elapsed < FLASH_AT {
                self.skip_to_flash(tm);
                return Ok(true);
            }
            // 仅 Login 阶段（白闪结束后）的触摸才触发切换「继续」；开屏/白闪期间触摸不提前白闪
            if elapsed >= LOGIN_START && !self.switch_pending {
                self.switch_pending = true;
                self.switch_start = Some(elapsed);
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        let t = tm.now() as f32;

        // ==== [临时调试] 纯 GL clear 红色验证底层管线 ====
        unsafe {
            let gl = get_internal_gl();
            gl.quad_context.clear(Some((1.0, 0.0, 0.0, 1.0)), None, None);
        }

        set_camera(&ui.bg_camera());
        let full = ui.screen_rect();
        ui.fill_rect(full, BLACK);

        set_camera(&ui.camera());

        let elapsed = if self.enter_time.is_nan() { 0. } else { t - self.enter_time };

        // ==== Logo 阶段（对齐 LogoSplash 的 fadeIn/hold/fadeOut 三段渐变）====
        if elapsed < LOGO_END {
            let alpha = if elapsed < LOGO_WAIT {
                0.
            } else if elapsed < LOGO_WAIT + FADE_IN {
                ((elapsed - LOGO_WAIT) / FADE_IN).clamp(0., 1.)
            } else if elapsed < LOGO_WAIT + FADE_IN + HOLD {
                1.
            } else {
                ((LOGO_END - elapsed) / FADE_OUT).clamp(0., 1.)
            };

            if let Some(logo) = &self.logo {
                let max_w = 1.2f32;
                let ratio = logo.height() / logo.width();
                let w = max_w;
                let h = w * ratio;
                let r = Rect::new(-w / 2., -h / 2., w, h);
                let r_global = ui.rect_to_global(r);

                draw_texture_ex(
                    **logo,
                    r_global.x,
                    r_global.y,
                    Color::new(1., 1., 1., alpha),
                    DrawTextureParams {
                        dest_size: Some(vec2(r_global.w, r_global.h)),
                        ..Default::default()
                    },
                );
            } else {
                ui.text("Studio Logo")
                    .pos(0., 0.)
                    .anchor(0.5, 0.5)
                    .size(0.6)
                    .color(Color::new(1., 1., 1., alpha))
                    .draw();
            }
        }
        // ==== 温馨提示阶段（对齐 Phigros Warning：与 Logo 淡出重叠、同画面连续渐显；完整显示 3 秒后渐变消失）====
        if elapsed >= TIP_START {
            if let Some(tip) = &self.tip {
                let appear = ((elapsed - TIP_START) / TIP_FADE).clamp(0., 1.);
                let fade_out_start = TIP_START + TIP_FADE + TIP_SHOW; // 2.3+0.5+3.0=5.8s：完整显示 3 秒后开始淡出
                let alpha = if elapsed < fade_out_start {
                    appear
                } else {
                    (1. - (elapsed - fade_out_start) / TIP_FADE_OUT).clamp(0., 1.)
                };
                let full = ui.screen_rect();
                let img_asp = tip.width() / tip.height();
                let screen_asp = full.w / full.h;
                let (w, h) = if screen_asp > img_asp {
                    (full.h * img_asp, full.h)
                } else {
                    (full.w, full.w / img_asp)
                };
                let r = Rect::new(full.x + (full.w - w) / 2., full.y + (full.h - h) / 2., w, h);
                let r_global = ui.rect_to_global(r);

                draw_texture_ex(
                    **tip,
                    r_global.x,
                    r_global.y,
                    Color::new(1., 1., 1., alpha),
                    DrawTextureParams {
                        dest_size: Some(vec2(r_global.w, r_global.h)),
                        ..Default::default()
                    },
                );
            }
        }

        // 跳过提示：白闪前点击/按键直接跳到卡点
        if self.flash_start.is_none() && elapsed < FLASH_AT {
            let a = (elapsed / 0.6).clamp(0., 1.);
            let skip_y = ui.top - 0.045;
            ui.text(tl!("startup-tap-to-skip"))
                .pos(0.96, skip_y)
                .anchor(1., 1.)
                .size(0.30)
                .color(semi_white(0.65 * a))
                .draw();
        }

        // ==== 开屏主画面：白闪开始时出现（先被白幕+模糊盖住），随白幕褪去逐渐清晰 ====
        if elapsed >= FLASH_AT {
            let p = ((elapsed - FLASH_AT) / REVEAL_TIME).clamp(0., 1.);
            // 内容：前 55% 时间内渐显，之后完全清晰
            let alpha = (p / 0.55).min(1.);
            ui.alpha(alpha, |ui| {
                #[cfg(feature = "video")]
                let drew_video = if let Some(video) = &self.home_video {
                    let top = ui.top;
                    let full = Rect::new(-1., -top, 2., 2. * top);
                    ui.fill_rect(full, Color::new(0., 0., 0., alpha));
                    push_camera_state();
                    set_camera(&ui.camera());
                    let elapsed = macroquad::time::get_time() as f64 - self.home_video_start;
                    video.render(elapsed, 1. / top, Color::new(1., 1., 1., alpha));
                    pop_camera_state();
                    ui.fill_rect(full, Color::new(0., 0., 0., 0.28));
                    self.backdrop.draw_dome(ui, macroquad::time::get_time() as f32, alpha);
                    true
                } else {
                    false
                };
                #[cfg(not(feature = "video"))]
                let drew_video = false;
                if !drew_video {
                    self.backdrop.draw(ui, self.art.as_ref(), macroquad::time::get_time() as f32, alpha);
                }
                let cx = crate::splash_ui::sx(crate::splash_ui::CONTENT_CX);
                // 版本号：贴在字标右上（右对齐）
                let lw = crate::splash_ui::logo_width(ui, 1.50);
                ui.text(format!("Version : {}", env!("CARGO_PKG_VERSION")))
                    .pos(cx + lw / 2., -0.135)
                    .anchor(1., 0.5)
                    .size(0.30)
                    .color(semi_white(0.75 * alpha))
                    .draw();
                // 字标：PHIRA + 小字 vrenxz
                crate::splash_ui::draw_logo(ui, cx, -0.02, 1.50, alpha);
                // 点击屏幕开始（轻微呼吸）
                let blink = ((t * 1.6).sin() * 0.5 + 0.5) * 0.35 + 0.65;
                let tap = tl!("startup-tap-to-continue");
                let tw = crate::splash_ui::text_width(ui, &tap, 0.48, 0.0);
                crate::splash_ui::draw_text_tracked(ui, &tap, cx - tw / 2., 0.15, 0.48, 0.0, semi_white(blink * alpha));
            });
        }

        // ==== 白闪：起白 → 模糊主图压在上面 → 一起褪去（对齐参考视频的开屏白闪，不是硬切全白）====
        if let Some(flash_at) = self.flash_start {
            let fp = (elapsed - flash_at).max(0.);
            if fp < REVEAL_TIME {
                let p = (fp / REVEAL_TIME).clamp(0., 1.);
                let full = ui.screen_rect();
                // 模糊层：前 70% 时间内淡出
                let blur_a = (1. - (p / 0.7).min(1.)).powi(2);
                if blur_a > 0.002 {
                    let blur = crate::splash_ui::key_art_blur().or_else(|| self.art.clone());
                    if let Some(tex) = blur {
                        let r = ui.rect_to_global(full);
                        ui.fill_rect(full, (*tex, r, ScaleType::CropCenter, Color::new(1., 1., 1., 0.9 * blur_a)));
                    }
                }
                // 白幕：起手接近全白，OutQuad 褪掉
                let white = (1. - p).powi(2) * 0.96;
                ui.fill_rect(full, Color::new(1., 1., 1., white));
            }
        }

        // 加载失败提示（仅在异常时可见，不影响正常开屏画面）
        if let Some(err) = &self.load_error {
            ui.text("启动加载失败")
                .pos(0., 0.)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.5)
                .color(RED)
                .draw();
            ui.text(err)
                .pos(0., 0.08)
                .anchor(0.5, 0.)
                .max_width(1.4)
                .size(0.3)
                .color(semi_white(0.7))
                .draw();
        }

        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        // Login 阶段点击「继续」后先播切主页过渡动画（白色淡出），动画播完才真正 Replace；
        // 未点击则一直停留 Login（白闪后不自动进 Home）
        if let Some(start) = self.switch_start {
            let elapsed = if self.enter_time.is_nan() { 0. } else { tm.now() as f32 - self.enter_time };
            if elapsed - start < SWITCH_FADE {
                return NextScene::None; // 过渡动画播放中：保持当前场景，避免生硬直切
            }
            // 优先切业务前置场景（登录 / 首次设置）
            if let Some(scene) = self.next_scene.take() {
                return NextScene::Replace(scene);
            }
            // 默认路径：开屏期间主界面已后台加载完成，直接进入（不再经过独立加载场景）
            if let Some(scene) = self.ready_scene.take() {
                // 把正在播的开屏 BGM 交给主界面，切场景时不断音、不重播。
                if let Some(bgm) = self.bgm.take() {
                    crate::scene::carry_bgm(bgm);
                }
                return NextScene::Replace(scene);
            }
            // 主界面未就绪（如首次解压资源包）：保持 Login 画面等待加载完成后再切，避免黑屏断档
        }
        NextScene::None
    }
}
