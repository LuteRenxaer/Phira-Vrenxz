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

/// 内嵌报错页 BGM（编译期打入二进制，避免 Android 上 std::fs 读不到 assets 而 panic）。
const CRASH_BGM: &[u8] = include_bytes!("../../../assets/bgm/gameerror.mp3");
/// 内嵌报错页背景图。
const CRASH_BG_PNG: &[u8] = include_bytes!("../../../assets/backgrounds/errorbackground.png");



/// Error codes for the crash scene.
#[derive(Clone, Debug)]
pub enum CrashCode {
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
}

impl CrashCode {
    pub fn code(&self) -> u32 {
        match self {
            CrashCode::ChartLoadTimeout => 404,
            CrashCode::ResPackLoadTimeout => 501,
            CrashCode::ManualCrash => 951,
            CrashCode::UnexpectedPanic { .. } => 500,
            CrashCode::Custom { code, .. } => *code,
        }
    }

    pub fn reason(&self) -> String {
        match self {
            CrashCode::ChartLoadTimeout => "加载铺面或者加载游戏太慢(至少1分钟)而崩溃".to_string(),
            CrashCode::ResPackLoadTimeout => "玩家加载资源包过久而崩溃".to_string(),
            CrashCode::ManualCrash => "玩家在设置中,点击崩溃按钮".to_string(),
            CrashCode::UnexpectedPanic { message } => message.clone(),
            CrashCode::Custom { reason, .. } => reason.clone(),
        }
    }

    pub fn from_panic_message(message: &str) -> Self {
        CrashCode::UnexpectedPanic {
            message: message.to_string(),
        }
    }
}

pub struct CrashScene {
    code: CrashCode,
    enter_time: f32,
    background: Option<SafeTexture>,
    tip: &'static str,
    audio: Option<AudioManager>,
    bgm: Option<Music>,
    black_duration: f32,
    custom_title: String,
    copy_toast_time: f32,
}

impl CrashScene {
    const TIPS: &[&str] = &[
        "哎呀,sensei的Phira-Vrenxz又崩溃了呀",
        "不!",
        "wow,你又可以向Lute_Rencai投诉了呢",
        "you game error!",
        "你可以数一数Phira-Vrenxz有多少个bug啦",
        "你肯定不知道,Phira-Vrenxz其实是rust项目",
        "哈哈哈,你去玩Phira吧",
        "我们的bug真多呀",
        "恭喜你,中大奖了!",
        "kskbl",
        "zdjd",
        "如何解决error,先把游戏删掉,然后启动phira玩去",
        "是时候检查一下你的AP记录了",
        "你的游戏被error先生占领了",
        "这个bug是故意留下来逗你玩的",
        "建议：重启游戏，如果还不行就重装系统",
        "你确定你玩的是Phira-Vrenxz不是Phira？",
        "哈哈，你也遇到了这个bug？",
        "error先生今天心情不好",
        "试试把手机倒过来？",
        "你的手指是不是太快了？",
        "别担心，这个bug已经提交给开发组了",
        "你一定是打开了新世界的大门",
        "这个错误是隐藏彩蛋",
        "现在你知道为什么叫Phira-Vrenxz了吧？",
        "加油，你离游戏崩溃次数记录只差一次了！",
        "你获得了‘游戏崩溃大师’称号",
        "建议：退出游戏，去写作业",
        "这个bug已经被标记为‘不会修复’",
        "你的AP记录可能已经飞走了",
        "error先生正在嘲笑你",
        "你玩的是Phira-Vrenxz，不是Phira",
        "这个错误是Lute_Rencai的锅",
        "建议：把你的电脑砸了",
        "恭喜你成功让游戏崩溃了",
        "别慌，这只是个开始",
        "你可能需要重新安装你的游戏",
        "建议：去玩Phira吧，那里没那么多bug",
        "error先生今天又开party了",
        "你的手速太快，游戏跟不上",
        "是不是你刚刚按了什么奇怪的按钮？",
        "好消息：你获得了崩溃成就！",
        "坏消息：这个成就没奖励",
        "你的AP记录已经乘坐火箭飞走了",
        "建议：先冷静一下，然后重启游戏",
        "这个bug是Lute_Rencai的错，不是你的",
        "你的游戏正在尝试修复自己……但失败了",
        "哈哈，你被error先生盯上了",
        "这个错误代码是幸运数字，你赚了",
        "你可以把这个截图发给作者，他会感谢你",
        "这个bug…和我一样会哈气",
        "哈欠…又是崩溃啊…",
        "哼！区区崩溃，本大人根本不放在眼里！",
        "……麻烦，下次直接砸了电脑吧",
        "嘻嘻，sensei又被bug捉弄了呢～",
        "风纪委员长在此！立即修复bug！",
        "阳奈大人说得对，我来记录这个错误",
        "老师！您怎么又搞崩溃了！",
        "……这个错误，我会用十字军解决",
        "呜…要不我们先去玩别的游戏吧…",
        "正义实现部出击！正义的修复！",
        "これは、エラーです。勇者よ、立ち上がれ！",
        "…我已经记不清这是第几次了",
        "这个bug的颜色…不够白，不合格",
        "水…我需要水来冷静…然后重启",
        "夏莱的科技真是深不可测啊",
        "把它吃掉就不会再崩溃了…大概",
        "主啊，请保佑这个游戏不再崩溃",
        "我已记录，下次更新会修复…大概",
        "哇！又崩了！再来一次！",
        "我…我藏进柜子里了，你们玩",
        "任务失败，撤退",
        "好累…这个bug也…太顽固了",
        "阿罗娜来了！虽然我也不知道怎么修",
        "分析完成…需要重启",
        "前辈…好困…但也要帮你重启",
        "把这个错误当成目标，击碎它",
        "我可是法外狂徒！岂会被bug打败！",
        "……（默默重启）",
        "下次我要在bug里塞点惊喜～",
        "我命令你，立刻恢复！",
        "已经重启了！",
        "老师！不要再按那个按钮了！",
        "这个错误…我记住了",
        "这是勇者试炼！爱丽丝会突破！",
        "我已经麻木了…继续吧",
        "让我用白色覆盖这个错误！",
        "水…水…好了，重启成功",
        "这个bug的构造…很有趣",
        "吃掉它！吃掉它就不见了！",
        "主啊，请赐予我重启的力量",
        "错误代码已记录，已通知开发组",
        "我还会再回来的！",
        "下次我不会让它跑掉",
        "好麻烦…但是必须重启",
        "阿罗娜会加油的！",
        "冷静…重启…再重启",
        "前辈…你习惯就好",
        "把这个bug当作狩猎目标",
        "哼！这种小事，不值一提！",
        "……我不会在同一个坑里摔倒两次",
        "sensei，你又给我提供了快乐素材",
        "再崩溃我就把电脑没收",
        "遵命！已经准备好重启方案",
        "老师！您是不是又偷偷改代码了！",
        "我会用我的方式解决它",
        "这是爱丽丝的觉悟！",
        "……我已经懒得数了",
        "我觉得…我们可以先去玩别的…",
        "这是第几号bug来着？我先查查台账",
        "sensei，这次真的不是我干的",
        "报告！错误已被正义实现部登记在案",
        "error先生说他今天想休息一下",
        "已自动上报，开发组正在围观",
        "别急，先截图发群里乐一乐",
        "恭喜你解锁了隐藏剧情：崩溃篇",
        "这个bug会呼吸，先别打扰它",
        "重启能解决90%的问题，剩下10%重装",
        "你的存档还活着，别慌",
        "阿罗娜表示：这不是我的计算失误",
        "要不要先喝口水冷静一下？",
        "阳奈大人正在赶来修复的路上",
        "建议深呼吸三次，然后重启",
        "这个错误已经被我瞪过了，没用",
        "下次更新一定修，大概吧",
        "其实这是个防沉迷系统，休息一下吧",
        "夏莱的服务器刚刚打了个喷嚏",
        "爱丽丝会把它当成BOSS击破的！",
        "别担心，这只是程序在卖萌",
        "重启吧，重启完又是一条好汉",
        "你离‘崩溃十连’只差一步了",
        "这个bug正在申请加班费",
        "建议重启游戏，再不行重启人生",
        "sensei，请下指令：重启 or 继续崩溃？",
        "风纪委员长已记录，违规bug将被处分",
        "error先生只是想引起你的注意",
        "你的AP记录此刻非常安全",
        "好，我宣布：这是特性，不是bug",
        "阿罗娜正在重新计算你的好运",
        "再按一次，说不定就修好了",
        "这个bug有它自己的想法",
        "冷静，我们先看一眼错误代码再笑",
        "重启的按钮已经为你准备好了",
        "恭喜，本次崩溃已计入年度统计",
        "也许……是时候去喝杯奶茶了",
        "阳奈大人说：区区错误，不足为惧",
        "让夏莱的科技帮你重启！",
        "别盯着看了，它不会自己好的",
        "正义实现部，出击！目标是这个bug",
    ];

    pub fn new(code: CrashCode, custom_title: String) -> Self {
        tracing::info!("CrashScene::new called");
        // On Android, rand::thread_rng() uses a once_cell::Lazy that may be
        // poisoned by a previous panic, causing random_tip() to panic. Guard with catch_unwind.
        let tip = std::panic::catch_unwind(|| {
            let mut rng = ::rand::thread_rng();
            Self::TIPS[::rand::Rng::gen_range(&mut rng, 0..Self::TIPS.len())]
        }).unwrap_or("哎呀,sensei的Phira-Vrenxz又崩溃了呀");

        let config = Config::default();
        let audio = match create_audio_manger(&config) {
            Ok(mut audio) => {
                // 从编译期内嵌字节加载，避免 Android 上 std::fs 读不到 APK 内 assets 而 panic。
                let bgm_data = CRASH_BGM.to_vec();
                match AudioClip::new(bgm_data) {
                    Ok(clip) => {
                        match audio.create_music(clip, MusicParams {
                            amplifier: 1.0,
                            loop_mix_time: 0.0,
                            ..Default::default()
                        }) {
                            Ok(bgm) => {
                                Self {
                                    code,
                                    enter_time: f32::NAN,
                                    background: None,
                                    tip,
                                    audio: Some(audio),
                                    bgm: Some(bgm),
                                    black_duration: 0.3,
                                    custom_title,
                                    copy_toast_time: 0.0,
                                }
                            }
                            Err(e) => {
                                tracing::warn!("创建音乐失败: {}", e);
                                Self {
                                    code,
                                    enter_time: f32::NAN,
                                    background: None,
                                    tip,
                                    audio: Some(audio),
                                    bgm: None,
                                    black_duration: 0.3,
                                    custom_title,
                                    copy_toast_time: 0.0,
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("音频数据解析失败: {}", e);
                        Self {
                            code,
                            enter_time: f32::NAN,
                            background: None,
                            tip,
                            audio: Some(audio),
                            bgm: None,
                            black_duration: 0.3,
                            custom_title,
                            copy_toast_time: 0.0,
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("创建音频管理器失败: {}", e);
                Self {
                    code,
                    enter_time: f32::NAN,
                    background: None,
                    tip,
                    audio: None,
                    bgm: None,
                    black_duration: 0.3,
                    custom_title,
                    copy_toast_time: 0.0,
                }
            }
        };
        audio
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
        tracing::info!("CrashScene::enter called");

        tm.reset();
        tm.seek_to(0.0);
        self.enter_time = tm.now() as f32;

        self.load_background();

        if let Some(bgm) = &mut self.bgm {
            if let Err(e) = bgm.play() {
                tracing::warn!("播放背景音乐失败: {}", e);
            }
        }
        Ok(())
    }

    fn update(&mut self, _tm: &mut TimeManager) -> Result<()> {
        if let Some(audio) = &mut self.audio {
            if let Err(e) = audio.recover_if_needed() {
                tracing::warn!("音频恢复失败: {}", e);
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        tracing::info!("CrashScene::render called");
        // Guard the entire render with catch_unwind. On Android, some Lazy
        // static (e.g. rand::thread_rng) may be poisoned by a previous panic,
        // causing render to panic. Show a simplified fallback if it does.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
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
            "CRASH Lv.Error_555",
            r.right() - r.h / 7. * 13. * 0.13 - 0.01,
            r.bottom() - top / 20.,
            (1., 1.),
            0.46,
            Color::new(1., 1., 1., illus_progress),
        );
        let p = (r.x + 0.04, r.bottom() - top / 20.);
        let mw = rr.x - 0.02 - p.0;
        let code_text = format!("错误代码:{}////", self.code.code());
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
            "哇!你的PhiLie崩溃啦!看来error先生愤怒了呢"
        } else {
            &self.custom_title
        };
        draw_text_aligned(
            ui,
            title,
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
            CrashCode::ChartLoadTimeout => "加载超时 ( > 60秒 )",
            CrashCode::ResPackLoadTimeout => "资源包加载超时",
            CrashCode::ManualCrash => "手动触发",
            CrashCode::UnexpectedPanic { .. } => "意外崩溃",
            CrashCode::Custom { .. } => "自定义崩溃",
        };
        let dy = 0.025;
        draw_text_aligned(
            ui,
            detail,
            s1.x + dx,
            s1.bottom() - dy,
            (0., 1.),
            0.34,
            Color::new(1., 1., 1., s1_progress),
        );
        draw_text_aligned(
            ui,
            "建议：重启游戏",
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
            "投诉",
            complain_rect.center().x,
            complain_rect.center().y,
            (0.5, 0.5),
            0.38,
            WHITE,
        );
        gl.pop_model_matrix();


        let copy_rect = Rect::new(-1. - h * slope, -top + dy_btn + h + 0.01, w, h);
        Self::tran(&mut gl, -btn_p * 0.085);
        draw_parallelogram(copy_rect, None, btn_bg, true);
        draw_parallelogram(
            Rect::new(copy_rect.x + copy_rect.w * (1. - s), copy_rect.y, copy_rect.w * s, copy_rect.h),
            None,
            WHITE,
            false,
        );
        draw_text_aligned(
            ui,
            "复制报告",
            copy_rect.center().x,
            copy_rect.center().y,
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
            "强制重启",
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
            "退出游戏",
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
                        Dialog::plain("投诉", "是否前往投诉页面？")
                            .buttons(vec!["取消".to_string(), "前往投诉".to_string()])
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
                    if copy_rect.contains(touch.position) {
                        let report = format!(
                            "=== Phira-Vrenxz Crash Report ===\nError Code: {}\nReason: {}\nTitle: {}\nTip: {}",
                            self.code.code(),
                            self.code.reason(),
                            if self.custom_title.is_empty() { "哇!你的PhiLie崩溃啦!看来error先生愤怒了呢" } else { &self.custom_title },
                            self.tip,
                        );
                        unsafe { get_internal_gl() }.quad_context.clipboard_set(&report);
                        self.copy_toast_time = now;
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

        // 复制成功提示（显示2秒后淡出）
        if self.copy_toast_time > 0. {
            let toast_elapsed = now - self.copy_toast_time;
            if toast_elapsed < 2.0 {
                let toast_alpha = if toast_elapsed < 1.5 { 1.0 } else { (2.0 - toast_elapsed) / 0.5 };
                draw_text_aligned(
                    ui,
                    "崩溃报告已复制到剪贴板",
                    0.,
                    -top + 0.1,
                    (0.5, 0.5),
                    0.4,
                    Color::new(0.3, 1.0, 0.5, toast_alpha),
                );
            }
        }

            Ok(())
        }));
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => {
                tracing::warn!("CrashScene::render panicked, showing fallback UI");
                let asp = screen_aspect();
                let top = 1. / asp;
                draw_rectangle(-1., -top, 2., top * 2., Color::new(0.03, 0.03, 0.06, 1.));
                draw_text_aligned(ui, "CRASH", 0., 0.1, (0.5, 0.5), 0.6, WHITE);
                draw_text_aligned(ui, &format!("Error Code: {}", self.code.code()), 0., -0.1, (0.5, 0.5), 0.3, Color::new(1., 1., 1., 0.8));
                draw_text_aligned(ui, "Tap to return", 0., -0.3, (0.5, 0.5), 0.25, Color::new(1., 1., 1., 0.5));
                Ok(())
            }
        }
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        NextScene::None
    }
}