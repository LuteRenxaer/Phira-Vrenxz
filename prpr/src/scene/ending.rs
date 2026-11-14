prpr_l10n::tl_file!("ending");

use super::{
    challenge::ChallengeEndData,
    draw_background, game::SimpleRecord, loading::UploadFn, NextScene, Scene,
};
use crate::{
    config::Config,
    ext::{create_audio_manger, draw_parallelogram, draw_parallelogram_ex, draw_parallelogram_lr, draw_text_aligned, screen_aspect, SafeTexture, PARALLELOGRAM_SLOPE},
    info::ChartInfo,
    judge::{icon_index, Judge, PlayResult},
    scene::show_message,
    task::Task,
    time::TimeManager,
    ui::{Dialog, MessageHandle, Ui},
};
use anyhow::Result;
use macroquad::prelude::*;
use sasa::{AudioClip, AudioManager, Music, MusicParams};
use serde::Deserialize;
use std::{cell::RefCell, ops::DerefMut};

#[derive(Deserialize)]
pub struct RecordUpdateState {
    pub best: bool,
    pub improvement: u32,
    pub gain_exp: f32,
    pub new_rks: Option<f32>,
}

fn draw_illustration(tex: Texture2D, x: f32, y: f32, w: f32, h: f32, color: Color, cover: bool) -> Rect {
    let scale = 0.076;
    let w = scale * 13. * w;
    let h = scale * 7. * h;
    let r = Rect::new(x - w / 2., y - h / 2., w, h);
    let tex_rect = if cover {
        let tex_ratio = tex.width() / tex.height();
        let rect_ratio = w / h;
        if tex_ratio > rect_ratio {
            let new_w = rect_ratio / tex_ratio;
            Rect::new((1. - new_w) / 2., 0., new_w, 1.)
        } else {
            let new_h = tex_ratio / rect_ratio;
            Rect::new(0., (1. - new_h) / 2., 1., new_h)
        }
    } else {
        Rect::new(0., 0., 1., 1.)
    };
    draw_parallelogram(r, Some((tex, tex_rect)), color, true);
    r
}

pub struct EndingScene {
    background: SafeTexture,
    illustration: SafeTexture,
    player: SafeTexture,
    icons: [SafeTexture; 8],
    arc_icon: SafeTexture,
    icon_retry: SafeTexture,
    icon_proceed: SafeTexture,
    target: Option<RenderTarget>,
    audio: AudioManager,
    bgm: Music,
    info: ChartInfo,
    result: PlayResult,
    player_name: String,
    player_rks: Option<f32>,
    autoplay: bool,
    speed: f32,
    next: u8,
    update_state: Option<RecordUpdateState>,
    rated: bool,
    arcaea_mode: bool,
    upload_fn: Option<UploadFn>,
    upload_task: Option<(Task<Result<RecordUpdateState>>, MessageHandle)>,
    record_data: Option<Vec<u8>>,
    record: Option<SimpleRecord>,
    /// [课题模式] 课题结算数据（所选谱面记录 + 总分色档）；普通游玩为 None
    challenge: Option<ChallengeEndData>,
    /// [课题模式] 结算谱面记录列表当前页码（0-based，分页展示）
    challenge_page: usize,
}

impl EndingScene {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        background: SafeTexture,
        illustration: SafeTexture,
        player: SafeTexture,
        icons: [SafeTexture; 8],
        arc_icon: SafeTexture,
        icon_retry: SafeTexture,
        icon_proceed: SafeTexture,
        _mod_icons: [SafeTexture; 7],
        info: ChartInfo,
        result: PlayResult,
        config: &Config,
        bgm: AudioClip,
        upload_fn: Option<UploadFn>,
        player_rks: Option<f32>,
        historic_best: u32,
        record_data: Option<Vec<u8>>,
        best_record: Option<SimpleRecord>,
        _avg_fps: Option<f32>,
        challenge: Option<ChallengeEndData>,
    ) -> Result<Self> {
        let mut audio = create_audio_manger(config)?;
        let bgm = audio.create_music(
            bgm,
            MusicParams {
                amplifier: config.volume_music,
                loop_mix_time: 0.,
                ..Default::default()
            },
        )?;
        let upload_task = upload_fn
            .as_ref()
            .and_then(|f| record_data.clone().map(|data| (f(data), show_message(tl!("uploading")).handle())));
        let update_state = if upload_task.is_some() {
            None
        } else {
            let (best, improvement) = if result.score > historic_best {
                (true, result.score - historic_best)
            } else {
                (false, 0)
            };
            Some(RecordUpdateState {
                best,
                improvement,
                gain_exp: 0.,
                new_rks: None,
            })
        };
        Ok(Self {
            background,
            illustration,
            player,
            icons,
            arc_icon,
            icon_retry,
            icon_proceed,
            target: None,
            audio,
            bgm,
            update_state,
            rated: upload_task.is_some(),
            arcaea_mode: info.arcaea_judgement || config.arcaea_judgement,
            info,
            result,
            player_name: config.player_name.clone(),
            player_rks,
            autoplay: config.autoplay(),
            speed: config.speed,
            next: 0,
            upload_fn,
            upload_task,
            record_data,
            record: best_record,
            challenge,
            challenge_page: 0,
        })
    }
}

thread_local! {
    static RE_UPLOAD: RefCell<bool> = RefCell::default();
}

impl Scene for EndingScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        tm.reset();
        tm.seek_to(-0.4);
        self.target = target;
        Ok(())
    }

    fn pause(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.bgm.pause()?;
        tm.pause();
        Ok(())
    }

    fn resume(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.bgm.play()?;
        tm.resume();
        Ok(())
    }

    fn touch(&mut self, _tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        if crate::hud::edit_active() {
            crate::hud::editor_touch(touch);
            return Ok(true);
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        self.audio.recover_if_needed()?;
        if crate::hud::edit_active() {
            if let Some(p) = crate::hud::take_goto() {
                if p != crate::hud::PageId::Ending {
                    crate::hud::request_goto(p);
                    self.next = 1;
                }
            }
        }
        if tm.now() >= 0. && self.target.is_none() && self.bgm.paused() {
            self.bgm.play()?;
        }
        if RE_UPLOAD.with(|it| std::mem::replace(it.borrow_mut().deref_mut(), false)) && self.upload_task.is_none() {
            self.upload_task = self
                .record_data
                .clone()
                .map(|data| ((self.upload_fn.as_ref().unwrap())(data), show_message(tl!("uploading")).handle()));
        }
        if let Some((task, handle)) = &mut self.upload_task {
            if let Some(result) = task.take() {
                handle.cancel();
                match result {
                    Err(err) => {
                        let error = format!("{:?}", err.context(tl!("upload-failed")));
                        Dialog::plain(tl!("upload-failed"), error)
                            .buttons(vec![tl!("upload-cancel").to_string(), tl!("upload-retry").to_string()])
                            .listener(move |_dialog, pos| {
                                if pos == 1 {
                                    RE_UPLOAD.with(|it| *it.borrow_mut() = true);
                                }
                                false
                            })
                            .show();
                    }
                    Ok(state) => {
                        self.update_state = Some(state);
                        show_message(tl!("uploaded")).ok();
                    }
                }
                self.upload_task = None;
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        let challenge_mode = self.challenge.is_some();
        let asp = screen_aspect();
        let top = 1. / asp;
        let now = tm.now() as f32;
        let gl = unsafe { get_internal_gl() }.quad_gl;
        let res = &self.result;
        set_camera(&Camera2D {
            zoom: vec2(1., -asp),
            render_target: self.target,
            ..Default::default()
        });
        crate::hud::set_cur_page(crate::hud::PageId::Ending);
        crate::hud::begin_frame();
        draw_background(*self.background);
        fn ran(t: f32, l: f32, r: f32) -> f32 {
            ((t - l) / (r - l)).clamp(0., 1.)
        }
        fn tran(gl: &mut QuadGl, x: f32) {
            gl.push_model_matrix(Mat4::from_translation(vec3(x * 2., 0., 0.)));
        }
        tran(gl, (1. - ran(now, 0.1, 1.3)).powi(3));
        let slope = PARALLELOGRAM_SLOPE;
        // [课题模式] 不绘制左侧大曲绘（用户要求去掉“大封面”），坐标改用占位矩形供后续面板布局
        let r = if !challenge_mode {
            let r = draw_illustration(*self.illustration, -0.38, 0., 1., 1.2, WHITE, true);
            let ratio = 0.2;
            draw_parallelogram_ex(
                Rect::new(r.x, r.y + r.h * (1. - ratio), r.w - r.h * (1. - ratio) * slope, r.h * ratio),
                None,
                Color::default(),
                Color::new(0., 0., 0., 0.7),
                false,
            );
            r
        } else {
            Rect::new(0.28, -top, 1.44, 2. * top)
        };
        if !challenge_mode {
            let rr = draw_text_aligned(ui, &self.info.level, r.right() - r.h / 7. * 13. * 0.13 - 0.01, r.bottom() - top / 20., (1., 1.), 0.46, WHITE);
            let p = (r.x + 0.04, r.bottom() - top / 20.);
            let mw = rr.x - 0.02 - p.0;
            let mut text = ui.text(&self.info.name).pos(p.0, p.1).anchor(0., 1.).size(0.7);
            if text.measure().w <= mw {
                text.draw();
            } else {
                drop(text);
                ui.text(&self.info.name).pos(p.0, p.1).anchor(0., 1.).size(0.5).max_width(mw).draw();
            }
        }
        gl.pop_model_matrix();
        let dx = 0.06;
        let c = Color::new(0., 0., 0., 0.6);
        tran(gl, (1. - ran(now, 0.2, 1.3)).powi(3));
        let mut main = Rect::new(r.right() - 0.05, r.y, r.w * 0.84, r.h / 2.);
        // HUD 自定义：结算分数面板（可移动/缩放；默认矩形基于原布局）。
        main = crate::hud::slot_or(ui, "ending", "score", crate::hud::Cap(true, true, true), main);
        if !challenge_mode {
            draw_parallelogram(main, None, c, true);
            let spd = if (self.speed - 1.).abs() <= 1e-4 {
                String::new()
            } else {
                format!(" {:.2}x", self.speed)
            };
            let text = if self.autoplay {
                format!("Phira-Vrenxz[AUTOPLAY] {spd}")
            } else if !self.rated {
                format!("Phira-Vrenxz[UNRATED] {spd}")
            } else if let Some(state) = &self.update_state {
                format!(
                    "Phira-Vrenxz {spd}  {}",
                    if state.best {
                        format!("NEW BEST +{:07}", state.improvement)
                    } else {
                        String::new()
                    }
                )
            } else {
                "Uploading…".to_owned()
            };
            let r = draw_text_aligned(ui, &text, main.x + dx, main.bottom() - 0.035, (0., 1.), 0.34, WHITE);
            let r = draw_text_aligned(ui, &format!("{:07}", res.score), r.x, r.y - 0.023, (0., 1.), 1., WHITE);
            let is_arc_full = self.arcaea_mode && res.counts[0] == res.num_of_notes;
            let icon = if is_arc_full {
                6
            } else {
                icon_index(res.score, res.num_of_notes == res.max_combo)
            };
            let p = ran(now, 1.4, 1.9).powi(2);
            let s = main.h * 0.67;
            let ct = (main.right() - main.h * slope - s / 2., r.bottom() + 0.02 - s / 2.);
            let s = s + s * (1. - p) * 0.3;
            let icon_tex = if is_arc_full {
                &*self.arc_icon
            } else {
                &*self.icons[icon]
            };
            draw_texture_ex(
                *icon_tex,
                ct.0 - s / 2.,
                ct.1 - s / 2.,
                Color::new(1., 1., 1., p),
                DrawTextureParams {
                    dest_size: Some(vec2(s, s)),
                    ..Default::default()
                },
            );
        }
        gl.pop_model_matrix();
        tran(gl, (1. - ran(now, 0.4, 1.5)).powi(3));
        let d = r.h / 16.;
        let mut s1 = Rect::new(main.x - d * 4. * slope, main.bottom() + d, main.w - d * 5. * slope, d * 3.);
        // HUD 自定义：结算统计条（Max Combo / Accuracy，可移动/缩放）。
        s1 = crate::hud::slot_or(ui, "ending", "stats1", crate::hud::Cap(true, true, true), s1);
        if !challenge_mode {
        draw_parallelogram(s1, None, c, true);
        {
            let dy = 0.025;
            let r = draw_text_aligned(ui, "Max Combo", s1.x + dx, s1.bottom() - dy, (0., 1.), 0.34, WHITE);
            draw_text_aligned(ui, &res.max_combo.to_string(), r.x, r.y - 0.01, (0., 1.), 0.7, WHITE);
            let r = draw_text_aligned(ui, "Accuracy", s1.right() - dx, s1.bottom() - dy, (1., 1.), 0.34, WHITE);
            draw_text_aligned(ui, &format!("{:.2}%", res.accuracy * 100.), r.right(), r.y - 0.01, (1., 1.), 0.7, WHITE);
        }
        }
        gl.pop_model_matrix();
        tran(gl, (1. - ran(now, 0.5, 1.7)).powi(3));
        let mut s2 = Rect::new(s1.x - d * 4. * slope, s1.bottom() + d, s1.w, s1.h);
        // HUD 自定义：结算判定计数条（Perfect/Good/Bad/Miss/Early/Late，可移动/缩放）。
        s2 = crate::hud::slot_or(ui, "ending", "stats2", crate::hud::Cap(true, true, true), s2);
        if !challenge_mode {
        draw_parallelogram(s2, None, c, true);
        {
            let dy = 0.025;
            let dy2 = 0.015;
            let bg = 0.57;
            let sm = 0.26;
            let draw_count = |ui: &mut Ui, ratio: f32, name: &str, count: u32| {
                let r = draw_text_aligned(ui, name, s2.x + s2.w * ratio, s2.bottom() - dy, (0.5, 1.), sm, WHITE);
                draw_text_aligned(ui, &count.to_string(), r.center().x, r.y - dy2, (0.5, 1.), bg, WHITE);
            };
            draw_count(ui, 0.14, "Perfect", res.counts[0]);
            draw_count(ui, 0.33, "Good", res.counts[1]);
            draw_count(ui, 0.46, "Bad", res.counts[2]);
            draw_count(ui, 0.59, "Miss", res.counts[3]);
            let sm = 0.3;
            let l = s2.x + s2.w * 0.72;
            let rt = s2.x + s2.w * 0.94;
            let cy = s2.center().y;
            let r = draw_text_aligned(ui, "Early", l, cy - dy2 / 2., (0., 1.), sm, WHITE);
            draw_text_aligned(ui, &res.early.to_string(), rt, r.bottom(), (1., 1.), sm, WHITE);
            let r = draw_text_aligned(ui, "Late", l, cy + dy2 / 2., (0., 0.), 0.3, WHITE);
            draw_text_aligned(ui, &res.late.to_string(), rt, r.y, (1., 0.), sm, WHITE);
        }
        }
        gl.pop_model_matrix();
        // [课题模式] 课题结算：按 Phigros 官方 ChallengeModeOver 预制布局（对齐参考图）
        // 三条 SettleAccount（官方 1456×194，宽高比 7.5）阶梯堆叠：条 1 最上最右，逐条左移 61px、下移 228px；
        // 条内：左侧封面条带（官方 755/1456≈0.5186）+ 左下曲名，右侧 7 位分数 / IN·Lv.15 / 判定四列 / Early·Late；
        // 条区下方 Sum 总分条（官方 718×140）+ 右侧溢出的 Ratings 色档徽章。
        // Re 课题谱面可能超官方 3 条上限，保留分页（每页最多 3 条，控件置于 Sum 条下方）
        if let Some(ch) = &self.challenge {
            const PER_PAGE: usize = 3;
            let total = ch.rounds.len().max(1);
            let pages = (total + PER_PAGE - 1) / PER_PAGE;
            let page = self.challenge_page.min(pages.saturating_sub(1));
            tran(gl, (1. - ran(now, 0.6, 1.8)).powi(3));
            {
                // 难度拆分：difficulty "IN 15" → ("IN", "Lv.15")
                let split_diff = |diff: &str| -> (String, String) {
                    // level 文本本身可能已含 "Lv."（如 "EZ Lv.0.2" / "Lv.Lv.0.2"），
                    // 这里先剥掉所有重复前缀，最终只补一个 "Lv."，避免渲染成 "Lv.Lv.0.2"
                    let strip_lv = |s: &str| -> String {
                        let mut t = s.trim();
                        loop {
                            let next = t
                                .strip_prefix("Lv.")
                                .or_else(|| t.strip_prefix("lv."))
                                .or_else(|| t.strip_prefix("LV."))
                                .map(|rest| rest.trim_start());
                            match next {
                                Some(rest) => t = rest,
                                None => break,
                            }
                        }
                        t.to_owned()
                    };
                    match diff.split_whitespace().collect::<Vec<_>>().as_slice() {
                        [d0, l0, ..] => (d0.to_string(), format!("Lv.{}", strip_lv(l0))),
                        [d0] => {
                            let s = strip_lv(d0);
                            if s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
                                (String::new(), format!("Lv.{s}"))
                            } else {
                                (s, String::new())
                            }
                        }
                        _ => ("?".to_string(), String::new()),
                    }
                };
                // 配色：结算条 / 总和条的底板统一用黑色半透明（与用户信息面板、按钮同一套底色）
                let c_label = Color::new(1., 1., 1., 0.62);
                let c_diff = Color::new(1., 1., 1., 0.88);
                // 官方比例换算（按屏幕高度缩放，保证任意宽高比下三条 + 总分条都放得下）
                let bh = 0.332 * top; // 条高（参考图 247/1488 屏高）
                let bw = bh * 1456. / 194.; // 条宽（官方 7.5 倍条高）
                let x_step = bw * 61. / 1456.; // 阶梯左移
                let y_step = bh * 228. / 194.; // 阶梯下移
                let band_right = 0.855; // 条 1 右缘
                let band_top = -0.677 * top; // 条 1 上缘
                // ==== 三条结算条（SettleAccount）====
                for row in 0..PER_PAGE {
                    let gi = page * PER_PAGE + row;
                    if gi >= total {
                        break;
                    }
                    let rec = &ch.rounds[gi];
                    let band = Rect::new(band_right - x_step * row as f32 - bw, band_top + y_step * row as f32, bw, bh);
                    draw_parallelogram(band, None, c, true);
                    // 封面条带：官方 755×194 占条内左侧 0~0.5186 宽，用真实曲绘中央裁条
                    let cw = bw * 0.5186;
                    if let Some(cover) = ch.covers.get(gi) {
                        let tw = cover.width().max(1.);
                        let th = cover.height().max(1.);
                        let target_ar = cw / bh;
                        let src_ar = tw / th;
                        let (sx, sy, sw, sh) = if src_ar > target_ar {
                            let sw0 = th * target_ar;
                            ((tw - sw0) / 2. / tw, 0., sw0 / tw, 1.)
                        } else {
                            let sh0 = tw / target_ar;
                            (0., (th - sh0) / 2. / th, 1., sh0 / th)
                        };
                        draw_parallelogram(Rect::new(band.x, band.y, cw, bh), Some((**cover, Rect::new(sx, sy, sw, sh))), WHITE, true);
                        // 封面右缘渐隐进条底色（参考图里曲绘与底色不是硬切边）
                        let l = bh * slope;
                        let fade_w = cw * 0.42;
                        draw_parallelogram_lr(
                            Rect::new(band.x + cw - fade_w - l, band.y, fade_w + l, bh),
                            None,
                            Color::new(0., 0., 0., 0.),
                            Color::new(0., 0., 0., 0.6),
                            false,
                        );
                    }
                    // 曲名：条内左下（官方 SongsName 大字）
                    let name_w = bw * 0.46;
                    let mut name_t = ui
                        .text(&rec.name)
                        .pos(band.x + bw * 0.025, band.y + bh * 0.85)
                        .anchor(0., 1.)
                        .size(0.66)
                        .color(WHITE);
                    if name_t.measure().w <= name_w {
                        name_t.draw();
                    } else {
                        drop(name_t);
                        ui.text(&rec.name)
                            .pos(band.x + bw * 0.025, band.y + bh * 0.83)
                            .anchor(0., 1.)
                            .size(0.44)
                            .color(WHITE)
                            .max_width(name_w)
                            .draw();
                    }
                    // 分数：条内右上，7 位补零
                    draw_text_aligned(ui, &format!("{:07}", rec.score), band.right() - bw * 0.047, band.y + bh * 0.30, (1., 0.5), 1.09, WHITE);
                    // 难度：封面右端，IN 在上 / Lv.15 在下（右对齐）
                    let (l_diff, l_lv) = split_diff(&rec.difficulty);
                    draw_text_aligned(ui, &l_diff, band.x + bw * 0.472, band.y + bh * 0.68, (1., 0.5), 0.36, c_diff);
                    draw_text_aligned(ui, &l_lv, band.x + bw * 0.472, band.y + bh * 0.83, (1., 0.5), 0.36, c_diff);
                    // 判定四列：数字在上、标签在下
                    let jlabels = ["Perfect", "Good", "Bad", "Miss"];
                    let jx = [0.546, 0.643, 0.712, 0.781];
                    for (j, label) in jlabels.iter().enumerate() {
                        let x = band.x + bw * jx[j];
                        draw_text_aligned(ui, &rec.counts[j].to_string(), x, band.y + bh * 0.71, (0.5, 0.5), 0.40, WHITE);
                        draw_text_aligned(ui, label, x, band.y + bh * 0.83, (0.5, 0.5), 0.34, c_label);
                    }
                    // Early / Late：右侧两行，标签左、数字右对齐
                    let elx = band.x + bw * 0.840;
                    let enx = band.x + bw * 0.948;
                    draw_text_aligned(ui, "Early", elx, band.y + bh * 0.67, (0., 0.5), 0.36, c_label);
                    draw_text_aligned(ui, &rec.early.to_string(), enx, band.y + bh * 0.67, (1., 0.5), 0.40, WHITE);
                    draw_text_aligned(ui, "Late", elx, band.y + bh * 0.82, (0., 0.5), 0.36, c_label);
                    draw_text_aligned(ui, &rec.late.to_string(), enx, band.y + bh * 0.82, (1., 0.5), 0.40, WHITE);
                }
                // ==== Sum 总分条（官方 718×140）+ 右侧 Ratings 徽章 ====
                // 参考图里徽章比条更高，探出条外，因此单独按条高比例绘制
                let sum_h = bh * 0.575;
                let sum_w = sum_h * 6.22;
                let band3_bottom = band_top + y_step * 2. + bh;
                let sum_rect = Rect::new(0.471 - sum_w / 2., band3_bottom + 0.148 * top - sum_h / 2., sum_w, sum_h);
                draw_parallelogram(sum_rect, None, c, true);
                draw_text_aligned(ui, &format!("{:07}", ch.total_score), sum_rect.x + sum_w * 0.121, sum_rect.center().y, (0., 0.5), 0.98, WHITE);
                let badge_s = bh * 1.35;
                let bcx = sum_rect.right() - bw * 0.050;
                let bcy = sum_rect.center().y - bh * 0.20;
                let icon_tex = &*self.icons[(ch.rank as usize).min(7)];
                draw_texture_ex(
                    *icon_tex,
                    bcx - badge_s / 2.,
                    bcy - badge_s / 2.,
                    WHITE,
                    DrawTextureParams {
                        dest_size: Some(vec2(badge_s, badge_s)),
                        ..Default::default()
                    },
                );
                // ==== 分页（官方无；Re 课题 >3 首时保留，控件置于 Sum 条下方）====
                if pages > 1 {
                    let pg_w = bh * 0.9;
                    let pg_h = bh * 0.55;
                    let pg_cy = (sum_rect.bottom() + bh * 0.42).min(top - pg_h);
                    let prev_btn = Rect::new(sum_rect.x + sum_w * 0.12 - pg_w / 2., pg_cy - pg_h / 2., pg_w, pg_h);
                    let next_btn = Rect::new(sum_rect.x + sum_w * 0.88 - pg_w / 2., pg_cy - pg_h / 2., pg_w, pg_h);
                    draw_parallelogram(prev_btn, None, Color::new(0.12, 0.13, 0.16, 1.), true);
                    draw_parallelogram(next_btn, None, Color::new(0.12, 0.13, 0.16, 1.), true);
                    let prev_color = if page > 0 { WHITE } else { Color::new(1., 1., 1., 0.35) };
                    let next_color = if page + 1 < pages { WHITE } else { Color::new(1., 1., 1., 0.35) };
                    draw_text_aligned(ui, "‹", prev_btn.center().x, prev_btn.center().y, (0.5, 0.5), 0.30, prev_color);
                    draw_text_aligned(ui, "›", next_btn.center().x, next_btn.center().y, (0.5, 0.5), 0.30, next_color);
                    draw_text_aligned(ui, &format!("{}/{}", page + 1, pages), sum_rect.center().x, pg_cy, (0.5, 0.5), 0.22, c_label);
                    if !crate::hud::edit_active() {
                        let hit = |rect: Rect| {
                            Judge::get_touches()
                                .iter()
                                .any(|touch| touch.phase == TouchPhase::Ended && rect.contains(touch.position))
                        };
                        let mut new_page: Option<usize> = None;
                        if page > 0 && hit(prev_btn) {
                            new_page = Some(page - 1);
                        }
                        if page + 1 < pages && hit(next_btn) {
                            new_page = Some(page + 1);
                        }
                        if let Some(p) = new_page {
                            self.challenge_page = p;
                        }
                    }
                }
            }
            gl.pop_model_matrix();
        }
        fn touched(rect: Rect) -> bool {
            Judge::get_touches()
                .iter()
                .any(|touch| touch.phase == TouchPhase::Ended && rect.contains(touch.position))
        }
        let dy = 0.006;
        let w = 0.17;
        let p = (1. - ran(now, 2., 2.7)).powi(2);
        let h = 0.1;
        let s = 0.05;
        let hs = h * 0.3;
        let params = DrawTextureParams {
            dest_size: Some(vec2(hs * 2., hs * 2.)),
            ..Default::default()
        };
        tran(gl, -p * 0.085);
        let mut r = Rect::new(-1. - h * slope, -top + dy, w, h);
        // HUD 自定义：重开按钮（可移动/缩放）。
        r = crate::hud::slot_or(ui, "ending", "retry", crate::hud::Cap(true, true, true), r);
        draw_parallelogram(r, None, c, true);
        draw_parallelogram(Rect::new(r.x + r.w * (1. - s), r.y, r.w * s, r.h), None, WHITE, false);
        let ct = r.center();
        draw_texture_ex(*self.icon_retry, ct.x - hs, ct.y - hs, WHITE, params.clone());
        gl.pop_model_matrix();
        if p <= 0. && !crate::hud::edit_active() && touched(r) {
            if self.upload_task.is_some() {
                show_message(tl!("still-uploading"));
            }
            self.next = 1;
        }
        tran(gl, p * 0.085);
        let mut r = Rect::new(1. + h * slope - w, top - dy - h, w, h);
        // HUD 自定义：继续按钮（可移动/缩放）。
        r = crate::hud::slot_or(ui, "ending", "proceed", crate::hud::Cap(true, true, true), r);
        draw_parallelogram(r, None, c, true);
        draw_parallelogram(Rect::new(r.x + r.w * s, r.y, r.w * s, r.h), None, WHITE, false);
        let ct = r.center();
        draw_texture_ex(*self.icon_proceed, ct.x - hs, ct.y - hs, WHITE, params);
        gl.pop_model_matrix();
        if p <= 0. && !crate::hud::edit_active() && touched(r) {
            if self.upload_task.is_some() {
                show_message(tl!("still-uploading"));
            }
            self.next = 2;
        }
        {
        // 用户信息面板：还原原版（黑色半透明 + rks + 云端头像 + 名字）
        let alpha = ran(now, 1.5, 1.9);
        let main = Rect::new(1. - 0.28, -top + dy * 2.5, 0.35, 0.1);
        draw_parallelogram(main, None, Color::new(0., 0., 0., c.a * alpha), false);
        let sub = Rect::new(1. - 0.13, main.center().y + 0.01, 0.12, 0.03);
        let color = Color::new(1., 1., 1., alpha);
        draw_parallelogram(sub, None, color, false);
        draw_text_aligned(
            ui,
            &if let Some(new_rks) = self.update_state.as_ref().and_then(|it| it.new_rks) {
                format!("{new_rks:.2}")
            } else if let Some(rks) = &self.player_rks {
                format!("{rks:.2}")
            } else {
                "".to_owned()
            },
            sub.center().x,
            sub.center().y,
            (0.5, 0.5),
            0.37,
            Color::new(0., 0., 0., alpha),
        );
        let r = draw_illustration(*self.player, 1. - 0.21, main.center().y, 0.12 / (0.076 * 7.), 0.12 / (0.076 * 7.), color, true);
        let text = draw_text_aligned(ui, &self.player_name, r.x - 0.01, r.center().y, (1., 0.5), 0.54, color);
        draw_parallelogram(
            Rect::new(text.x - main.h * slope - 0.01, main.y, r.x - text.x + main.h * slope * 2. + 0.013, main.h),
            None,
            Color::new(0., 0., 0., c.a * alpha),
            false,
        );
        draw_text_aligned(ui, &self.player_name, r.x - 0.01, r.center().y, (1., 0.5), 0.54, color);
        }

        // 判定时间分布图：0ms 居中，左 Early / 右 Late；附推荐偏移。
        if !res.diffs.is_empty() {
            const BINS: usize = 41;
            let mut bins = [0u32; BINS];
            let mut sum = 0f64;
            for &d in &res.diffs {
                let p = (d.clamp(-0.2, 0.2) / 0.2 * 20.).round() as i32 + 20;
                bins[p.clamp(0, 40) as usize] += 1;
                sum += d as f64;
            }
            let max = bins.iter().copied().max().unwrap_or(1).max(1) as f32;
            let w = 0.62;
            let h = 0.10;
            let x0 = -w / 2.;
            let y0 = ui.top - 0.16;
            let bw = w / BINS as f32;
            for (i, &n) in bins.iter().enumerate() {
                let bh = h * n as f32 / max;
                let x = x0 + i as f32 * bw;
                let color = if i < 20 {
                    Color::new(0.35, 0.65, 1., 0.8)
                } else if i > 20 {
                    Color::new(1., 0.6, 0.3, 0.8)
                } else {
                    Color::new(1., 1., 1., 0.9)
                };
                ui.fill_rect(Rect::new(x, y0 - bh, bw * 0.9, bh), color);
            }
            let avg_ms = sum / res.diffs.len() as f64 * 1000.;
            ui.text(format!(
                "判定分布  P+ {}  G {}  O {}  M {}  推荐偏移 {:+.1} ms",
                res.perfect_plus, res.great, res.ok, res.meh, avg_ms
            ))
                .pos(0., y0 + 0.01)
                .anchor(0.5, 0.)
                .size(0.30)
                .color(Color::new(1., 1., 1., 0.75))
                .draw();
        }

        if crate::hud::edit_active() {
            crate::hud::editor_render(ui);
        }

        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        if self.next != 0 {
            let _ = self.bgm.pause();
        }
        match self.next {
            0 => NextScene::None,
            1 => NextScene::Pop,
            2 => {
                // [课题模式] 结算完成后携带课题结算数据回课题页（含所选谱面记录与总分评分）
                if let Some(ch) = &self.challenge {
                    NextScene::PopNWithResult(2, Box::new(ch.clone()))
                } else if let Some(rec) = &self.record {
                    NextScene::PopNWithResult(2, Box::new(rec.clone()))
                } else {
                    NextScene::PopN(2)
                }
            }
            _ => unreachable!(),
        }
    }
}
