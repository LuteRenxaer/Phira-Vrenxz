use super::{ChartItem, NextPage, Page, SFader, SharedState};
use crate::icons::Icons;
use anyhow::Result;
use inputbox::InputBox;
use macroquad::prelude::*;
use prpr::{
    config::{HpMode, Mods},
    ext::{poll_future, semi_black, semi_white, LocalTask, RectExt, ScaleType},
    scene::{
        request_input, show_error, show_message, take_input, ChallengeConfig, ChallengeSession,
        GameMode, HpConfig, HpLoseCondition, NextScene,
    },
    ui::{DRectButton, RectButton, Scroll, Ui},
};
use crate::scene::SongScene;
use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};

/// 课题模式页面（普通课题 / Re:课题 / HP 血量模式）
pub struct ChallengePage {
    icons: Arc<Icons>,
    charts: Vec<ChartItem>,

    next_page: Option<NextPage>,

    // 曲目列表
    scroll: Scroll,

    // 槽位（普通固定3，Re 无限）: 存 charts 索引
    slots: Vec<usize>,
    re: bool,

    // 自定义判定（毫秒）
    judge: (u32, u32, u32),

    // HP
    hp_on: bool,
    hp_idx: usize, // 0..5 -> 1/20/50/100/200
    hp_lose: HpLoseCondition,

    back_btn: DRectButton,
    re_btn: DRectButton,
    hp_btn: DRectButton,
    hp_val_btn: DRectButton,
    hp_cond_btn: DRectButton,
    judge_p_btn: DRectButton,
    judge_g_btn: DRectButton,
    judge_b_btn: DRectButton,
    undo_btn: DRectButton,
    clear_btn: DRectButton,
    start_btn: DRectButton,

    // 启动任务
    launch_task: LocalTask<Result<NextScene>>,
    sf: SFader,
    auto_started: bool,

    // 挑战会话
    session: Option<Arc<Mutex<ChallengeSession>>>,
    result_text: Option<String>,
    start_time: f32,
    entering: bool,
}

const HP_MAX_OPTIONS: [f32; 5] = [1., 20., 50., 100., 200.];

impl ChallengePage {
    pub fn new(icons: Arc<Icons>, charts: Vec<ChartItem>) -> Result<Self> {
        Ok(Self {
            icons,
            charts,
            next_page: None,
            scroll: Scroll::new(),
            slots: Vec::new(),
            re: false,
            judge: (40, 75, 140),
            hp_on: true,
            hp_idx: 3,
            hp_lose: HpLoseCondition::BadOrMiss,
            back_btn: DRectButton::new().with_delta(-0.004),
            re_btn: DRectButton::new().with_delta(-0.004),
            hp_btn: DRectButton::new().with_delta(-0.004),
            hp_val_btn: DRectButton::new().with_delta(-0.004),
            hp_cond_btn: DRectButton::new().with_delta(-0.004),
            judge_p_btn: DRectButton::new().with_delta(-0.004),
            judge_g_btn: DRectButton::new().with_delta(-0.004),
            judge_b_btn: DRectButton::new().with_delta(-0.004),
            undo_btn: DRectButton::new().with_delta(-0.004),
            clear_btn: DRectButton::new().with_delta(-0.004),
            start_btn: DRectButton::new().with_delta(-0.004),
            launch_task: None,
            sf: SFader::new(),
            auto_started: false,
            session: None,
            result_text: None,
            start_time: f32::NAN,
            entering: true,
        })
    }

    /// 带预选谱面构造课题页：按传入顺序填进普通课题的三个槽位。
    ///
    /// 用于「谱面库里选 3 首后开始课题」的流程；原有的
    /// [`ChallengePage::new`] 语义保持不变（空槽位、左侧列表自选）。
    pub fn new_with_selection(
        icons: Arc<Icons>,
        charts: Vec<ChartItem>,
        selection: Vec<ChartItem>,
    ) -> Result<Self> {
        let mut page = Self::new(icons, charts)?;
        page.apply_selection(selection);
        Ok(page)
    }

    /// 把已选谱面按顺序填入槽位；不在列表里的谱面追加到列表末尾。
    pub fn apply_selection(&mut self, selection: Vec<ChartItem>) {
        self.slots.clear();
        for item in selection {
            let index = match self.charts.iter().position(|it| Self::same_chart(it, &item)) {
                Some(index) => index,
                None => {
                    self.charts.push(item);
                    self.charts.len() - 1
                }
            };
            if !self.slots.contains(&index) {
                self.slots.push(index);
            }
        }
    }

    fn same_chart(a: &ChartItem, b: &ChartItem) -> bool {
        if a.local_path.is_some() && a.local_path == b.local_path {
            return true;
        }
        matches!((a.info.id, b.info.id), (Some(x), Some(y)) if x == y)
    }

    fn add_slot(&mut self, idx: usize) {
        if self.slots.contains(&idx) {
            return;
        }
        if !self.re && self.slots.len() >= 3 {
            show_message("普通课题最多选择 9 张谱面").warn();
            return;
        }
        self.slots.push(idx);
    }

    fn hp_option(&self) -> Option<HpConfig> {
        if self.hp_on {
            Some(HpConfig {
                max_hp: HP_MAX_OPTIONS[self.hp_idx],
                lose_condition: self.hp_lose,
            })
        } else {
            None
        }
    }

    /// 启动第 index 局
    fn start_round(&mut self, index: usize) -> Result<()> {
        let Some(&chart_idx) = self.slots.get(index) else {
            anyhow::bail!(
                "课题槽位 {} 不存在（当前已选 {} 张谱面），无法开始",
                index + 1,
                self.slots.len()
            );
        };
        let Some(chart) = self.charts.get(chart_idx) else {
            anyhow::bail!(
                "课题槽位 {} 的谱面索引 {} 已失效（本地谱列表已变化），请重新选择谱面",
                index + 1,
                chart_idx
            );
        };
        let Some(local_path) = chart.local_path.clone() else {
            anyhow::bail!(
                "课题槽位 {} 的谱面「{}」无本地路径，无法开始",
                index + 1,
                chart.info.name
            );
        };
        let id = chart.info.id;
        let total = self.slots.len();

        // 会话（首局创建）：paths 必须与所选槽位一一对应，
        // 任一槽位谱面缺失/索引失效都直接报错阻止，严禁静默丢谱导致 total 变小跳过连局。
        let session = match &self.session {
            Some(it) => Arc::clone(it),
            None => {
                let charts_root = crate::dir::charts()?;
                let mut paths = Vec::with_capacity(self.slots.len());
                let mut round_names = Vec::with_capacity(self.slots.len());
                for (slot_no, &i) in self.slots.iter().enumerate() {
                    let chart = self.charts.get(i).ok_or_else(|| {
                        anyhow::anyhow!(
                            "课题槽位 {} 的谱面索引 {} 已失效，无法开始课题，请重新选择谱面",
                            slot_no + 1,
                            i
                        )
                    })?;
                    let p = chart.local_path.clone().ok_or_else(|| {
                        anyhow::anyhow!(
                            "课题槽位 {} 的谱面「{}」无本地路径，无法开始课题",
                            slot_no + 1,
                            chart.info.name
                        )
                    })?;
                    paths.push(format!("{charts_root}/{p}"));
                    round_names.push((chart.info.name.clone(), chart.info.level.clone()));
                }
                let st = ChallengeSession {
                    re: self.re,
                    paths,
                    round_names,
                    results: Vec::new(),
                    round_counts: Vec::new(),
                    round_early_late: Vec::new(),
                    round_max_combo: Vec::new(),
                    round_fc: Vec::new(),
                    round_illustrations: Vec::new(),
                    completed_rounds: 0,
                    aborted: false,
                    hp: self.hp_option(),
                    hp_left: None,
                    judge_ms: if self.re { Some(self.judge) } else { None },
                };
                let it = Arc::new(Mutex::new(st));
                self.session = Some(Arc::clone(&it));
                it
            }
        };
        {
            let mut st = session.lock().unwrap();
            st.re = self.re;
            st.hp = self.hp_option();
            st.judge_ms = if self.re { Some(self.judge) } else { None };
        }

        let config = ChallengeConfig {
            re: self.re,
            total,
            index,
            custom_limits: if self.re {
                Some(ChallengeConfig::limits_from_ms(self.judge.0, self.judge.1, self.judge.2))
            } else {
                None
            },
            scoring: true,
            hp: self.hp_option(),
            session,
        };

        let task = SongScene::global_launch_preview(
            id,
            &local_path,
            Mods::empty(),
            HpMode::Off,
            GameMode::Challenge,
            None,
            None,
            None,
            false,
            false,
            false,
            None,
            false,
            Some(config),
        )?;
        self.launch_task = task;
        Ok(())
    }

    fn chart_list_rect() -> Rect {
        Rect::new(-0.96, -0.88, 0.62, 1.7)
    }

    fn slot_rects(&self) -> Vec<Rect> {
        let n = if self.re { self.slots.len().max(1) } else { 3 };
        let (x, w, h) = (-0.28, 1.22, 0.19);
        let y0 = 0.85;
        let gap = 0.03;
        (0..n)
            .map(|i| Rect::new(x, y0 - (h + gap) * i as f32, w, h))
            .collect()
    }

    fn control_rect(row: f32) -> Rect {
        Rect::new(-0.28, 0.14 - row as f32 * 0.13, 1.22, 0.1)
    }

    fn small_ctrl_rect(row: f32, col: usize) -> Rect {
        let w = 0.39;
        let x = -0.28 + col as f32 * (w + 0.025);
        Rect::new(x, 0.14 - row as f32 * 0.13, w, 0.1)
    }

    /// 开始挑战（原生玻璃 UI 按钮）
    pub fn set_hp(&mut self, on: bool, idx: usize, lose: HpLoseCondition) {
        self.hp_on = on;
        self.hp_idx = idx.min(HP_MAX_OPTIONS.len() - 1);
        self.hp_lose = lose;
    }

    /// 由谱面库直接开始课题时调用：不显示课题页，下一次 next_scene 直接进游戏。
    pub fn start_now(&mut self) -> bool {
        self.auto_started = true;
        self.start_challenge()
    }

    fn start_challenge(&mut self) -> bool {
        if (!self.re && self.slots.len() < 3) || (self.re && self.slots.is_empty()) {
            show_message(if self.re {
                "Re:课题至少选择 1 张谱面"
            } else {
                "普通课题需要选择 3 张谱面"
            })
            .warn();
            return false;
        }
        self.session = None;
        self.result_text = None;
        if let Err(err) = self.start_round(0) {
            show_error(err);
            return false;
        }
        true
    }
}

/// 会话推进：已中止则作废、全部完成则结算、否则启动下一局（幂等，重复调用安全）
fn advance_session_ext(me: &mut ChallengePage) -> Result<()> {
    if let Some(session) = me.session.clone() {
        let (aborted, completed, total) = {
            let lock = session.lock().unwrap();
            (lock.aborted, lock.completed_rounds, lock.paths.len())
        };
        if aborted {
            me.result_text = Some("挑战失败：中途退出，整场作废".to_string());
            me.session = None;
            return Ok(());
        }
        if completed >= total {
            let sum: u32 = {
                let lock = session.lock().unwrap();
                lock.results.iter().map(|(s, _)| *s).sum()
            };
            let (rank, _) = ChallengeConfig::color_rank(sum);
            let name = match rank {
                0 => "彩",
                1 => "金",
                2 => "红",
                3 => "蓝",
                4 => "绿",
                _ => "无",
            };
            me.result_text = Some(format!("挑战完成！总分 {}，色档 {}", sum, name));
            me.session = None;
            return Ok(());
        }
        if me.launch_task.is_none() {
            me.start_round(completed)?;
        }
    }
    Ok(())
}

impl Page for ChallengePage {
    fn label(&self) -> Cow<'static, str> {
        if self.auto_started { Cow::Borrowed("") } else { Cow::Borrowed("challenge") }
    }

    fn enter(&mut self, s: &mut SharedState) -> Result<()> {
        self.start_time = s.t;
        // 从游玩 Pop 回来由框架触发 enter：推进下一局 / 结算 / 作废判定
        advance_session_ext(self)?;
        Ok(())
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        if self.entering {
            if self.charts.is_empty() {
                self.charts = s.charts_local.clone();
            }
            self.entering = false;
        }
        let t = s.t;
        if self.start_time.is_nan() {
            self.start_time = t;
        }
        for c in &mut self.charts {
            c.illu.notify();
            c.illu.settle(t);
        }
        self.scroll.update(t);

        // 输入框回收（自定义判定）
        if let Some((id, text)) = take_input() {
            let parse = |raw: &str, old: u32| -> u32 {
                raw.trim().parse::<u32>().ok().filter(|&v| v <= 300).unwrap_or(old)
            };
            match id.as_str() {
                "ch_judge_p" => self.judge.0 = parse(&text, self.judge.0),
                "ch_judge_g" => self.judge.1 = parse(&text, self.judge.1),
                "ch_judge_b" => self.judge.2 = parse(&text, self.judge.2),
                _ => {}
            }
            if self.re && !(self.judge.0 < self.judge.1 && self.judge.1 < self.judge.2) {
                self.judge = (40, 75, 140);
                show_message("判定需满足 Perfect < Good < Bad，已重置为 40/75/140").warn();
            }
        }

        // 启动任务推进
        if let Some(task) = &mut self.launch_task {
            if let Some(res) = poll_future(task.as_mut()) {
                match res {
                    Ok(scene) => {
                        self.launch_task = None;
                        self.sf.next(t, scene);
                    }
                    Err(err) => {
                        self.launch_task = None;
                        show_error(err);
                    }
                }
            }
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        if self.launch_task.is_some() {
            return Ok(true);
        }
        if self.back_btn.touch(touch, s.t) {
            self.next_page = Some(NextPage::Pop);
            return Ok(true);
        }
        // 曲目列表点击选曲
        let list_r = Self::chart_list_rect();
        if touch.phase == TouchPhase::Started && list_r.contains(touch.position) {
            self.scroll.touch(touch, s.t);
            let off = self.scroll.y_scroller.offset;
            let row_h = 0.1;
            let idx = ((touch.position.y - list_r.y + off) / row_h - 1.) as i32;
            if idx >= 0 && (idx as usize) < self.charts.len() {
                self.add_slot(idx as usize);
                return Ok(true);
            }
        }
        if self.scroll.touch(touch, s.t) {
            return Ok(true);
        }
        // 槽位点击移除
        let slot_rects = self.slot_rects();
        if touch.phase == TouchPhase::Started {
            for (pos, r) in slot_rects.iter().enumerate() {
                if r.contains(touch.position) && pos < self.slots.len() {
                    self.slots.remove(pos);
                    return Ok(true);
                }
            }
        }
        if self.re_btn.touch(touch, s.t) {
            self.re = !self.re;
            self.slots.clear();
            return Ok(true);
        }
        if self.hp_btn.touch(touch, s.t) {
            self.hp_on = !self.hp_on;
            return Ok(true);
        }
        if self.hp_val_btn.touch(touch, s.t) && self.hp_on {
            self.hp_idx = (self.hp_idx + 1) % HP_MAX_OPTIONS.len();
            return Ok(true);
        }
        if self.hp_cond_btn.touch(touch, s.t) && self.hp_on {
            self.hp_lose = match self.hp_lose {
                HpLoseCondition::BadOrMiss => HpLoseCondition::NonPerfect,
                HpLoseCondition::NonPerfect => HpLoseCondition::BadOrMiss,
            };
            return Ok(true);
        }
        if self.re {
            if self.judge_p_btn.touch(touch, s.t) {
                request_input("ch_judge_p", InputBox::new().default_text(&self.judge.0.to_string()));
                return Ok(true);
            }
            if self.judge_g_btn.touch(touch, s.t) {
                request_input("ch_judge_g", InputBox::new().default_text(&self.judge.1.to_string()));
                return Ok(true);
            }
            if self.judge_b_btn.touch(touch, s.t) {
                request_input("ch_judge_b", InputBox::new().default_text(&self.judge.2.to_string()));
                return Ok(true);
            }
        }
        if self.undo_btn.touch(touch, s.t) {
            if !self.slots.is_empty() {
                self.slots.pop();
            }
            return Ok(true);
        }
        if self.clear_btn.touch(touch, s.t) {
            self.slots.clear();
            return Ok(true);
        }
        if self.start_btn.touch(touch, s.t) {
            self.start_challenge();
            return Ok(true);
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        if self.auto_started {
            return Ok(());
        }
        let t = s.t;
        // 玻璃拟态背景：深灰蓝渐变 + 径向光斑
        glass_background(ui, t);

        // 标题（半透明胶囊托底提升层次）
        let title = "课题模式";
        let title_w = glass_text_len(title, 0.05);
        glass_capsule(ui, 0., 0.92, title_w, 0.05 * 1.22, 0.12);
        ui.text(title)
            .pos(0., 0.92)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.05)
            .color(semi_white(1.))
            .draw();

        // 返回按钮（玻璃）
        let back_r = Rect::new(-0.95, 0.88, 0.14, 0.06);
        let press = 1. - self.back_btn.progress(t);
        self.back_btn.render_shadow(ui, back_r, t, |ui, _| {
            glass_button(ui, back_r, 0.010, Color::new(1., 1., 1., 0.10), press);
        });
        ui.text("<")
            .pos(back_r.center().x, back_r.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.04)
            .color(semi_white(1.))
            .draw();

        // 左侧曲目列表（玻璃面板）
        let list_r = Self::chart_list_rect();
        glass_panel(ui, list_r, 0.014);
        if !self.charts.is_empty() {
            self.scroll.size((list_r.w, list_r.h));
            let content_h = self.charts.len() as f32 * 0.1 + 0.04;
            let off = self.scroll.y_scroller.offset;
            self.scroll.render(ui, |ui| {
                for (i, c) in self.charts.iter().enumerate() {
                    let r = Rect::new(list_r.x + 0.02, list_r.y + 0.02 + i as f32 * 0.1 - off, list_r.w - 0.04, 0.09);
                    if self.slots.contains(&i) {
                        glass_selected(ui, r.feather(-0.004), 0.010);
                    }
                    let name = if c.info.name.is_empty() { "--" } else { &c.info.name };
                    ui.text(name)
                        .pos(r.x + 0.1, r.y + 0.015)
                        .size(0.026)
                        .max_width(r.w - 0.12)
                        .color(semi_white(0.95))
                        .draw();
                    let diff = format!("{:.2}", c.info.difficulty);
                    ui.text(&diff)
                        .pos(r.x + 0.1, r.y + 0.055)
                        .size(0.018)
                        .color(semi_white(0.6))
                        .draw();
                }
                (list_r.w, content_h)
            });
        } else {
            ui.text("无本地谱面")
                .pos(list_r.center().x, list_r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.04)
                .color(semi_white(0.5))
                .draw();
        }

        // 右侧槽位（玻璃面板）
        for (i, r) in self.slot_rects().iter().enumerate() {
            glass_panel(ui, *r, 0.016);
            if let Some(&ci) = self.slots.get(i) {
                if let Some(c) = self.charts.get(ci) {
                    let illu_r = Rect::new(r.x + 0.04, r.y + 0.02, r.h - 0.04, r.h - 0.04);
                    ui.fill_path(&illu_r.rounded(0.008), c.illu.shading(illu_r, t));
                    let name = if c.info.name.is_empty() { "--" } else { &c.info.name };
                    ui.text(name)
                        .pos(r.x + r.h + 0.04, r.y + r.h * 0.3)
                        .size(0.028)
                        .max_width(0.9)
                        .color(semi_white(1.))
                        .draw();
                    let diff = format!("定数 {:.2}", c.info.difficulty);
                    ui.text(&diff)
                        .pos(r.x + r.h + 0.04, r.y + r.h * 0.58)
                        .size(0.02)
                        .color(semi_white(0.65))
                        .draw();
                }
            } else {
                let slot_txt = format!("第{}槽", i + 1);
                let slot_w = glass_text_len(&slot_txt, 0.03);
                glass_capsule(ui, r.center().x, r.center().y, slot_w, 0.03 * 1.18, 0.06);
                ui.text(&slot_txt)
                    .pos(r.center().x, r.center().y)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(0.03)
                    .color(semi_white(0.80))
                    .draw();
            }
        }

        // 模式（玻璃按钮）
        let mode_r = Self::control_rect(0.);
        let press = 1. - self.re_btn.progress(t);
        let mode_base = if self.re { Color::new(0.62, 0.32, 0.12, 0.82) } else { Color::new(1., 1., 1., 0.09) };
        self.re_btn.render_shadow(ui, mode_r, t, |ui, _| {
            glass_button(ui, mode_r, 0.012, mode_base, press);
        });
        ui.text(if self.re { "Re:课题" } else { "普通课题" })
            .pos(mode_r.center().x, mode_r.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.03)
            .color(semi_white(1.))
            .draw();

        // 提示（弱胶囊）
        let cap_r = Self::control_rect(0.5);
        let hint = if self.re { "选曲不限（不可重复）" } else { "至少选 3 张开始" };
        let hint_w = glass_text_len(hint, 0.022);
        glass_capsule(ui, cap_r.center().x, cap_r.center().y, hint_w, 0.022 * 1.2, 0.045);
        ui.text(hint)
            .pos(cap_r.center().x, cap_r.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.022)
            .color(semi_white(0.45))
            .draw();

        // HP 开关（玻璃按钮 + 状态胶囊）
        let hp_r = Self::control_rect(1.);
        let press = 1. - self.hp_btn.progress(t);
        let hp_base = if self.hp_on { Color::new(0.55, 0.20, 0.20, 0.85) } else { Color::new(1., 1., 1., 0.09) };
        self.hp_btn.render_shadow(ui, hp_r, t, |ui, _| {
            glass_button(ui, hp_r, 0.012, hp_base, press);
        });
        let hp_txt = if self.hp_on { "HP 血量: 开" } else { "HP 血量: 关" };
        let hp_w = glass_text_len(hp_txt, 0.03);
        glass_capsule(ui, hp_r.center().x, hp_r.center().y, hp_w, 0.03 * 1.18, 0.06);
        ui.text(hp_txt)
            .pos(hp_r.center().x, hp_r.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.03)
            .color(semi_white(1.))
            .draw();

        // HP 档位 + 扣血条件（玻璃按钮 + 胶囊）
        if self.hp_on {
            let val_r = Self::control_rect(2.);
            let press = 1. - self.hp_val_btn.progress(t);
            self.hp_val_btn.render_shadow(ui, val_r, t, |ui, _| {
                glass_button(ui, val_r, 0.012, Color::new(1., 1., 1., 0.09), press);
            });
            let hp_val = HP_MAX_OPTIONS[self.hp_idx];
            let val_txt = format!("血量上限 {}", if hp_val.fract() == 0. { format!("{}", hp_val as i32) } else { format!("{}", hp_val) });
            let val_w = glass_text_len(&val_txt, 0.026);
            glass_capsule(ui, val_r.center().x, val_r.center().y, val_w, 0.026 * 1.18, 0.06);
            ui.text(&val_txt)
                .pos(val_r.center().x, val_r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.026)
                .color(semi_white(1.))
                .draw();

            let cond_r = Self::control_rect(3.);
            let press = 1. - self.hp_cond_btn.progress(t);
            self.hp_cond_btn.render_shadow(ui, cond_r, t, |ui, _| {
                glass_button(ui, cond_r, 0.012, Color::new(1., 1., 1., 0.09), press);
            });
            let cond = match self.hp_lose {
                HpLoseCondition::BadOrMiss => "Bad/Miss 扣血",
                HpLoseCondition::NonPerfect => "非Perfect 扣血",
            };
            let cond_w = glass_text_len(cond, 0.026);
            glass_capsule(ui, cond_r.center().x, cond_r.center().y, cond_w, 0.026 * 1.18, 0.06);
            ui.text(cond)
                .pos(cond_r.center().x, cond_r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.026)
                .color(semi_white(1.))
                .draw();
        }

        // Re 自定义判定（玻璃按钮 + 胶囊）
        if self.re {
            for (i, (btn, key, val)) in [
                (&mut self.judge_p_btn, "P", self.judge.0),
                (&mut self.judge_g_btn, "G", self.judge.1),
                (&mut self.judge_b_btn, "B", self.judge.2),
            ]
            .into_iter()
            .enumerate()
            {
                let r = Self::small_ctrl_rect(3., i);
                let press = 1. - btn.progress(t);
                btn.render_shadow(ui, r, t, |ui, _| {
                    glass_button(ui, r, 0.012, Color::new(1., 1., 1., 0.09), press);
                });
                let judge_txt = format!("{} {}ms", key, val);
                let judge_w = glass_text_len(&judge_txt, 0.024);
                glass_capsule(ui, r.center().x, r.center().y, judge_w, 0.024 * 1.18, 0.06);
                ui.text(&judge_txt)
                    .pos(r.center().x, r.center().y)
                    .anchor(0.5, 0.5)
                    .no_baseline()
                    .size(0.024)
                    .color(semi_white(1.))
                    .draw();
            }
        }

        // 撤销 / 清空（玻璃按钮）
        let undo_r = Self::small_ctrl_rect(4., 0);
        let press = 1. - self.undo_btn.progress(t);
        self.undo_btn.render_shadow(ui, undo_r, t, |ui, _| {
            glass_button(ui, undo_r, 0.012, Color::new(1., 1., 1., 0.09), press);
        });
        ui.text("撤销")
            .pos(undo_r.center().x, undo_r.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.028)
            .color(semi_white(1.))
            .draw();

        let clear_r = Self::small_ctrl_rect(4., 1);
        let press = 1. - self.clear_btn.progress(t);
        self.clear_btn.render_shadow(ui, clear_r, t, |ui, _| {
            glass_button(ui, clear_r, 0.012, Color::new(1., 1., 1., 0.09), press);
        });
        ui.text("清空")
            .pos(clear_r.center().x, clear_r.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.028)
            .color(semi_white(1.))
            .draw();

        // 开始按钮（玻璃 + 主题发光）
        let start_r = Self::control_rect(5.);
        let press = 1. - self.start_btn.progress(t);
        let start_base = if self.hp_on { Color::new(0.70, 0.15, 0.15, 0.90) } else { Color::new(0.15, 0.62, 0.30, 0.90) };
        self.start_btn.render_shadow(ui, start_r, t, |ui, _| {
            let glow_r = start_r.feather(0.012 + 0.006 * press);
            ui.fill_path(&glow_r.rounded(0.030), Color::new(start_base.r, start_base.g, start_base.b, 0.08 + 0.12 * press));
            glass_button(ui, start_r, 0.014, start_base, press);
        });
        ui.text("开始挑战")
            .pos(start_r.center().x, start_r.center().y)
            .anchor(0.5, 0.5)
            .no_baseline()
            .size(0.038)
            .color(semi_white(1.))
            .draw();

        // 结算提示（玻璃弹窗）
        if let Some(text) = &self.result_text {
            let pop_r = Rect::new(-0.5, -0.25, 1.0, 0.5);
            ui.fill_path(&pop_r.rounded(0.02), Color::new(0.05, 0.06, 0.10, 0.90));
            ui.stroke_path(&pop_r.rounded(0.02), 0.0025, semi_white(0.22));
            ui.text(text)
                .pos(0., 0.)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.05)
                .color(semi_white(1.))
                .draw();
        }

        self.sf.render(ui, t);
        Ok(())
    }

    fn next_page(&mut self) -> NextPage {
        self.next_page.take().unwrap_or_default()
    }

    fn next_scene(&mut self, s: &mut SharedState) -> NextScene {
        self.sf.next_scene(s.t).unwrap_or_default()
    }

    fn on_back_pressed(&mut self, _s: &mut SharedState) -> bool {
        self.next_page = Some(NextPage::Pop);
        true
    }

    fn resume(&mut self) -> Result<()> {
        // 兼容旧回调：逻辑与 enter 一致（advance_session_ext 幂等，重复调用安全）
        advance_session_ext(self)?;
        if self.auto_started {
            self.next_page = Some(NextPage::Pop);
        }
        Ok(())
    }
}

// —— 玻璃拟态（Glassmorphism）绘制辅助 ——

/// 文本宽度粗估（用于胶囊底包裹文字）
fn glass_text_len(text: &str, size: f32) -> f32 {
    let em: f32 = text
        .chars()
        .map(|c| {
            if c.is_ascii() {
                if c == ' ' {
                    0.30
                } else {
                    0.55
                }
            } else {
                1.0
            }
        })
        .sum();
    size * (em + 0.55)
}

/// 径向渐变光斑
fn glass_spot(ui: &mut Ui, cx: f32, cy: f32, radius: f32, color: Color) {
    let fade = Color::new(color.r, color.g, color.b, 0.0);
    ui.fill_circle(cx, cy, radius, (color, (cx, cy), fade, radius));
}

/// 背景：深灰蓝垂直线性渐变 + 缓慢漂移的径向光斑
fn glass_background(ui: &mut Ui, t: f32) {
    ui.fill_rect(
        Rect::new(-1., -1., 2., 2.),
        (
            Color::new(0.050, 0.060, 0.100, 1.0),
            (0., -1.),
            Color::new(0.022, 0.028, 0.050, 1.0),
            (0., 1.),
        ),
    );
    let (a, b, c) = (t * 0.37, t * 0.25, t * 0.29);
    glass_spot(
        ui,
        -0.55 + 0.14 * a.sin(),
        -0.80 + 0.08 * (a * 0.9).cos(),
        0.90,
        Color::new(0.22, 0.48, 0.62, 0.16),
    );
    glass_spot(
        ui,
        0.85 + 0.12 * b.cos(),
        0.30 + 0.10 * (b * 1.2).sin(),
        1.00,
        Color::new(0.55, 0.32, 0.60, 0.12),
    );
    glass_spot(
        ui,
        0.15 + 0.10 * c.sin(),
        0.95 + 0.14 * (c * 0.8).cos(),
        0.80,
        Color::new(0.30, 0.52, 0.36, 0.10),
    );
}

/// 玻璃面板：底部投影 + 半透明白填充 + 高光描边 + 顶部亮线
fn glass_panel(ui: &mut Ui, r: Rect, radius: f32) {
    let sh = r.feather(radius * 1.1);
    let sh = Rect::new(sh.x, sh.y + radius * 0.6, sh.w, sh.h);
    ui.fill_path(&sh.rounded(radius * 1.4), semi_black(0.28));
    let path = r.rounded(radius);
    ui.fill_path(&path, Color::new(1., 1., 1., 0.065));
    ui.stroke_path(&path, 0.0016, semi_white(0.14));
    let top = Rect::new(r.x + r.w * 0.08, r.y + r.h * 0.05, r.w * 0.84, (r.h * 0.03).max(radius * 0.3));
    ui.fill_path(&top.rounded(top.h), semi_white(0.10));
}

/// 选中项玻璃高亮：低饱和青蓝
fn glass_selected(ui: &mut Ui, r: Rect, radius: f32) {
    let path = r.rounded(radius);
    ui.fill_path(&path, Color::new(0.20, 0.34, 0.48, 0.34));
    ui.stroke_path(&path, 0.0015, Color::new(0.45, 0.72, 0.95, 0.30));
}

/// 半透明胶囊底（中心 cx/cy，文字下方）
fn glass_capsule(ui: &mut Ui, cx: f32, cy: f32, w: f32, h: f32, alpha: f32) {
    if w <= 0. || h <= 0. {
        return;
    }
    let r = Rect::new(cx - w * 0.5, cy - h * 0.5, w, h);
    let path = r.rounded(h * 0.5);
    ui.fill_path(&path, Color::new(1., 1., 1., alpha));
    ui.stroke_path(&path, 0.0014, semi_white(0.15));
}

/// 玻璃按钮主体：半透明白/主题色填充 + 高光描边，按压（hover）时提亮
fn glass_button(ui: &mut Ui, r: Rect, radius: f32, base: Color, press: f32) {
    let lift = press * 0.30;
    let fill = Color::new(
        (base.r + (1. - base.r) * lift).min(1.),
        (base.g + (1. - base.g) * lift).min(1.),
        (base.b + (1. - base.b) * lift).min(1.),
        (base.a + (1. - base.a) * press * 0.10).min(1.),
    );
    let path = r.rounded(radius);
    ui.fill_path(&path, fill);
    ui.stroke_path(&path, 0.0018 + press * 0.0010, semi_white(0.14 + press * 0.28));
}