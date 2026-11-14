use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    config::Config,
    ext::{
        create_audio_manger, semi_black, semi_white,
        SafeTexture, BLACK_TEXTURE, ScaleType, PARALLELOGRAM_SLOPE,
    },
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::{RectButton, Ui},
};
use sasa::{AudioClip, AudioManager, Music, MusicParams, PlaySfxParams, Sfx};
use std::sync::Arc;

use crate::icons::Icons;

/// 剧情背景 slash.png（编译期内嵌，避免运行时读取 assets 失败）。
const STORY_BG_PNG: &[u8] = include_bytes!("../../../assets/backgrounds/slash.png");
/// 剧情背景音乐 Ambient #4002.wav。
const STORY_BGM: &[u8] = include_bytes!("../../../assets/sfx/Ambient #4002.wav");
/// 剧情信息音效 message.wav。
const STORY_MSG_SFX: &[u8] = include_bytes!("../../../assets/sfx/message.wav");

/// 各条对话在文字显现完毕后自动停留的秒数；0 表示等待点击（开场与选项处）。
const AUTO_DELAYS: [f32; 10] = [
    0.0, // 0 Restart?
    0.0, // 1 StartAgain?（此条之后显示 A/B 选项，等用户选择）
    2.0, // 2 Call out...
    2.0, // 3 Error
    2.0, // 4 Decipher...
    5.0, // 5 Accomplish!
    3.0, // 6 Loading...
    3.0, // 7 404
    3.0, // 8 Load again...
    3.0, // 9 Accomplish!
];

// ==================== 剧情场景 ====================

/// 平行四边形路径（与 profile.rs 一致），配合 ui.fill_path 在 Ui 坐标系绘制。
fn parallelogram(r: Rect, shear: f32) -> lyon::path::Path {
    use lyon::math::point;
    let mut p = lyon::path::Path::builder();
    p.begin(point(r.x + shear, r.y));
    p.line_to(point(r.right() + shear, r.y));
    p.line_to(point(r.right() - shear, r.bottom()));
    p.line_to(point(r.x - shear, r.bottom()));
    p.end(true);
    p.build()
}

pub struct StoryScene {
    background: SafeTexture,
    dialogue_index: usize,
    text_progress: f32,
    choice_a: RectButton,
    choice_b: RectButton,
    show_choices: bool,
    finished: bool,
    audio: Option<AudioManager>,
    bgm: Option<Music>,
    msg_sfx: Option<Sfx>,
    icons: Arc<Icons>,
    rank_icons: [SafeTexture; 8],
    enter_time: f32,
    choice_show_time: f32,
    auto_start: f32,
}

impl StoryScene {
    fn load_bg() -> SafeTexture {
        match image::load_from_memory(STORY_BG_PNG) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (w, h) = (rgba.width(), rgba.height());
                let tex = Texture2D::from_rgba8(w as u16, h as u16, &rgba.into_raw());
                SafeTexture::from(tex)
            }
            Err(e) => {
                tracing::warn!("加载剧情背景 slash.png 失败: {}", e);
                BLACK_TEXTURE.clone()
            }
        }
    }

    fn play_msg(&mut self) {
        if let Some(sfx) = &mut self.msg_sfx {
            let _ = sfx.play(PlaySfxParams::default());
        }
    }

    fn ease_out_quart(t: f32) -> f32 {
        1. - (1. - t).powi(4)
    }

    pub fn new(icons: Arc<Icons>, rank_icons: [SafeTexture; 8]) -> Self {
        Self {
            background: Self::load_bg(),
            dialogue_index: 0,
            text_progress: 0.0,
            choice_a: RectButton::new(),
            choice_b: RectButton::new(),
            show_choices: false,
            finished: false,
            audio: create_audio_manger(&Config::default()).ok(),
            bgm: None,
            msg_sfx: None,
            icons,
            rank_icons,
            enter_time: 0.0,
            choice_show_time: 0.0,
            auto_start: 0.0,
        }
    }
}

impl Scene for StoryScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        // 创建并播放背景音乐 Ambient #4002.wav
        if let Some(audio) = &mut self.audio {
            if self.bgm.is_none() {
                if let Ok(clip) = AudioClip::new(STORY_BGM.to_vec()) {
                    match audio.create_music(
                        clip,
                        MusicParams {
                            amplifier: 1.0,
                            loop_mix_time: 0.0,
                            ..Default::default()
                        },
                    ) {
                        Ok(bgm) => {
                            self.bgm = Some(bgm);
                            if let Some(bgm) = &mut self.bgm {
                                let _ = bgm.play();
                            }
                        }
                        Err(e) => tracing::warn!("剧情BGM创建失败: {}", e),
                    }
                }
            }
            // 创建信息音效 message.wav
            if self.msg_sfx.is_none() {
                if let Ok(clip) = AudioClip::new(STORY_MSG_SFX.to_vec()) {
                    match audio.create_sfx(clip, None) {
                        Ok(sfx) => self.msg_sfx = Some(sfx),
                        Err(e) => tracing::warn!("剧情音效创建失败: {}", e),
                    }
                }
            }
        }
        // 进入时播放第一条信息音效
        self.play_msg();
        self.enter_time = tm.now() as f32;
        self.auto_start = tm.now() as f32;
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.text_progress += 0.02;
        // 自动推进：文字显现完成后，按该对话的延迟自动进入下一条
        // （开场两条与选项处 delay=0，仍等待点击 / 用户选择）
        if self.text_progress >= 1.0 && !self.show_choices && !self.finished {
            if self.dialogue_index < AUTO_DELAYS.len() {
                let delay = AUTO_DELAYS[self.dialogue_index];
                if delay > 0.0 && tm.now() as f32 - self.auto_start >= delay {
                    self.dialogue_index += 1;
                    self.text_progress = 0.0;
                    self.auto_start = tm.now() as f32;
                    self.play_msg();
                }
            }
        }
        // 对话结束，直接弹回主页（章节谱面列表为隐藏 UI，不再跳入）
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        // 与 profile/song/chapter 等正常场景一致：显式设置 Ui 屏幕映射相机，
        // 避免残留上一场景（crash 等）的 GL 相机导致面板错位。
        set_camera(&ui.bg_camera());

        // 背景 slash.png
        ui.fill_rect(
            ui.screen_rect(),
            (*self.background, ui.screen_rect(), ScaleType::CropCenter),
        );
        // 半透明压暗，保证面板与文字可读
        ui.fill_rect(ui.screen_rect(), semi_black(0.45));

        let slope = PARALLELOGRAM_SLOPE;

        // 对话内容
        let dialogues = [
            ("系统", "Restart?"),
            ("系统", "StartAgain?"),
            ("系统", "Call out..."),
            ("High-rise", "Error"),
            ("系统", "Decipher..."),
            ("系统", "Accomplish!"),
            ("系统", "Loading..."),
            ("Error", "404"),
            ("系统", "Load again..."),
            ("系统", "Accomplish!"),
        ];

        if self.dialogue_index < dialogues.len() {
            let (speaker, text) = dialogues[self.dialogue_index];

            // 左侧白色面板：贴屏幕左缘，右缘向内倾斜（平行四边形状）。
            // 参照主页开始按钮的贴边做法：矩形左缘略伸出屏幕，配合平行四边形
            // 倾斜后左下角明显戳出屏幕（一角出屏）。用 ui.fill_path 绘制，
            // 与 ui.text 同一套 Ui 坐标系，避免 draw_parallelogram 与文字错位。
            let panel_w = 0.8;
            let panel_h = 0.3;
            let lean = panel_h * PARALLELOGRAM_SLOPE;
            // 垂直居中：面板中心 y=0（Ui 坐标系 y 向下，-panel_h/2 使上下对称）
            // 进入场景时面板从屏幕左侧以 Out Quart 缓动滑入到位
            let now = tm.now() as f32;
            let slide = Self::ease_out_quart(((now - self.enter_time) / 0.7).min(1.0));
            let panel_base_x = -1.0 - lean;
            let panel_x = panel_base_x - (1.0 - slide) * 0.9;
            let panel_r = Rect::new(panel_x, -panel_h / 2.0, panel_w, panel_h);
            ui.fill_path(
                &parallelogram(panel_r, lean),
                WHITE,
            );

            // 文字起点：相对面板左上角（贴屏幕左缘 -1.0）向内缩进，随滑入动画同步移动
            let text_x = panel_x + lean + 0.03;

            // 说话人
            ui.text(speaker)
                .pos(text_x, panel_r.y + 0.03)
                .anchor(0., 0.)
                .size(0.5)
                .color(BLACK)
                .draw();

            // 对话文本（逐字显现）
            let visible_len = (self.text_progress * text.len() as f32) as usize;
            let visible_text = &text[..visible_len.min(text.len())];
            ui.text(visible_text)
                .pos(text_x, panel_r.y + 0.15)
                .anchor(0., 0.)
                .size(0.6)
                .color(BLACK)
                .draw();

            // 右侧选择框：贴屏幕右缘，左缘向内倾斜
            if self.show_choices {
                let choice_w = 0.25;
                let choice_h = 0.1;
                let choice_lean = choice_h * PARALLELOGRAM_SLOPE;
                // 右侧选择按钮：右缘伸出屏幕（1.0 + choice_lean），配合平行四边形
                // 倾斜后右上角明显一角出屏，与左侧面板镜像对称；垂直方向与面板居中对齐。
                // 出现时从屏幕右侧以 Out Quart 缓动滑入到位
                let btn_slide = Self::ease_out_quart(((now - self.choice_show_time) / 0.5).min(1.0));
                let choice_x = 1.0 + choice_lean - choice_w;
                let choice_dx = (1.0 - btn_slide) * 0.6;
                let choice_a_r = Rect::new(choice_x + choice_dx, -0.125, choice_w, choice_h);
                let choice_b_r = Rect::new(choice_x + choice_dx, 0.025, choice_w, choice_h);

                self.choice_a.set(ui, choice_a_r);
                self.choice_b.set(ui, choice_b_r);

                ui.fill_path(
                    &parallelogram(choice_a_r, choice_a_r.h * PARALLELOGRAM_SLOPE),
                    semi_white(0.9),
                );
                ui.fill_path(
                    &parallelogram(choice_b_r, choice_b_r.h * PARALLELOGRAM_SLOPE),
                    semi_white(0.9),
                );

                ui.text("A: NO")
                    .pos(choice_a_r.center().x, choice_a_r.center().y)
                    .anchor(0.5, 0.5)
                    .size(0.55)
                    .color(BLACK)
                    .draw();

                ui.text("B: Yes")
                    .pos(choice_b_r.center().x, choice_b_r.center().y)
                    .anchor(0.5, 0.5)
                    .size(0.55)
                    .color(BLACK)
                    .draw();
            }
        } else {
            // 对话结束：直接弹回主页（章节谱面列表为隐藏 UI，不再跳入）
            self.finished = true;
        }

        let _ = slope;
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if self.finished {
            return Ok(true);
        }

        if self.show_choices {
            if self.choice_a.touch(touch) {
                // 选择 A: NO —— 继续对话（进入 Call out... 后自动播放）
                self.show_choices = false;
                self.dialogue_index += 1;
                self.text_progress = 0.0;
                self.auto_start = tm.now() as f32;
                self.play_msg();
                return Ok(true);
            }
            if self.choice_b.touch(touch) {
                // 选择 B: Yes —— 闪退
                std::process::exit(0);
            }
        } else {
            // 点击屏幕推进对话
            if self.text_progress >= 1.0 {
                if self.dialogue_index == 1 {
                    // StartAgain? 之后显示选项（停留在当前对话）
                    self.show_choices = true;
                    self.choice_show_time = tm.now() as f32;
                    self.play_msg();
                    return Ok(true);
                }
                // 自动播放阶段（delay>0）：点击不干预，按设定延迟自动推进
                if self.dialogue_index < AUTO_DELAYS.len() && AUTO_DELAYS[self.dialogue_index] > 0.0 {
                    return Ok(true);
                }
                self.dialogue_index += 1;
                self.text_progress = 0.0;
                self.auto_start = tm.now() as f32;
                self.play_msg();
            } else {
                self.text_progress = 1.0;
            }
            return Ok(true);
        }

        Ok(false)
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        if self.finished {
            // 剧情结束：弹回主页，不再进入章节谱面列表
            NextScene::Pop
        } else {
            NextScene::None
        }
    }
}
