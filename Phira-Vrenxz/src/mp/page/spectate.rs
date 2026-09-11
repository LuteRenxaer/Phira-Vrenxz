//! 观战页（整屏）：正在游玩的谱面 + 各玩家实时统计，点行选择同步观战目标。
//!
//! 版面（对齐房间页）：页头是标题 + 无底色的「退出观战」，内容区顶部一张概要卡
//! （谱面 / 人数 / 已开局玩家数 / 最高分），下面是可以滚动的玩家实时榜，
//! 底部一条操作条——主按钮是「同步观战：<目标名>」，目标名写在按钮上，
//! 因此"现在到底在同步谁"一眼可见（原来的版本只有一个孤零零的「同步观战」）。
//!
//! 每一行的信息层级：头像 + 名字（+ 房主 / 我自己 / 同步中徽标）在左，
//! 右侧两行——大号分数、下面一行准确率 / 最大连击 / P·G·B·M；
//! 还没开始打的玩家在右侧显示「等待中」，而不是一排 0。

use std::collections::HashMap;

use macroquad::prelude::*;
use prpr::{
    ext::SafeTexture,
    ui::{DRectButton, Scroll, Ui},
};

use super::super::{
    spectate::SpectateStat,
    theme::{self, *},
};
use crate::{client::UserManager, mp::L10N_LOCAL};

/// 观战页可执行的动作。
pub enum Action {
    Back,
    /// 「同步观战」：加载对方谱面并按对方视角同步播放
    Watch,
    /// 「退出观战」：离开房间
    Exit,
    /// 点玩家行：选择同步观战的目标
    Select(i32),
}

/// 观战页需要的只读状态。
pub struct View<'a> {
    /// 房间号（概要卡用）
    pub room_id: Option<&'a str>,
    /// 正在游玩的谱面名（服务端补发选谱消息时记录）
    pub chart_name: Option<&'a str>,
    /// 当前同步观战目标
    pub target: Option<i32>,
    /// 房间是否已经开局（没开局时只有等待提示，没有可同步的画面）
    pub playing: bool,
    /// 玩家 id 与名字（顺序即渲染顺序）
    pub players: &'a [(i32, String)],
    /// 各玩家实时统计
    pub stats: &'a HashMap<i32, SpectateStat>,
    /// 用户头像的默认贴图
    pub icon: &'a SafeTexture,
}

/// 玩家行的行高：一行名字 + 两行数据。
const ROW_H: f32 = 0.24 * SCALE;

#[derive(Default)]
pub struct SpectatePage {
    back: DRectButton,
    exit: DRectButton,
    watch: DRectButton,
    scroll: Scroll,
    /// 行按钮（索引与 `ids` 对齐）
    rows: Vec<DRectButton>,
    /// 渲染时记录的用户 id：触摸只按这份数据索引，与渲染同一来源
    ids: Vec<i32>,
}

impl SpectatePage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.exit.invalidate();
        self.watch.invalidate();
        for b in self.rows.iter_mut() {
            b.invalidate();
        }
    }

    pub fn update(&mut self, t: f32) {
        self.scroll.update(t);
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, v: &View) {
        let accent = ui.accent();
        let f = theme::frame(ui, BAR_BTN_H);
        match v.chart_name {
            Some(name) => {
                let sub = mtl!("spectate-chart", "chart" => name.to_owned());
                theme::header(ui, f.header, &mut self.back, &mut DRectButton::new(), t, &mtl!("spectate-title"), Some(&sub), None);
            }
            None => {
                theme::header(ui, f.header, &mut self.back, &mut DRectButton::new(), t, &mtl!("spectate-title"), None, None);
            }
        }
        // 「退出观战」只保留底部操作条里的那一个（页头再摆一个无底色红字，跟房间里
        // "两个退出按钮"是同一个毛病）。

        // —— 概要卡：谱面 / 人数 / 已开局人数 / 最高分 ——
        let summary_h = 0.2f32.min(f.body.h * 0.3 * SCALE);
        let summary = Rect::new(f.body.x, f.body.y, f.body.w, summary_h);
        theme::card_accented(ui, summary, card(), accent);
        let scoring: Vec<&SpectateStat> = v.stats.values().filter(|s| s.total() > 0).collect();
        let best = scoring.iter().map(|s| s.score()).max().unwrap_or(0);
        let mut parts: Vec<String> = vec![mtl!("mp-n-players", "n" => v.players.len() as u64)];
        if !v.playing {
            parts.push(mtl!("mp-state-choose").into_owned());
        } else if scoring.is_empty() {
            parts.push(mtl!("spectate-waiting").into_owned());
        }
        if best > 0 {
            parts.push(mtl!("spectate-best", "score" => format!("{best:07}")));
        }
        let title = match (v.room_id, v.chart_name) {
            (Some(id), _) => mtl!("mp-room-tag", "id" => id.to_owned()),
            (None, Some(name)) => name.to_owned(),
            (None, None) => mtl!("spectate-title").into_owned(),
        };
        theme::text_left(ui, summary.x + CARD_PAD + 0.012, summary.y + summary.h * 0.32, FS_SECTION, text(), &title, summary.w - CARD_PAD * 2.);
        let sub = parts.join("  ·  ");
        theme::text_left(ui, summary.x + CARD_PAD + 0.012, summary.y + summary.h * 0.72, FS_SMALL, text_muted(), &sub, summary.w - CARD_PAD * 2.);

        // —— 玩家实时榜 ——
        let list_y = summary.bottom() + SECTION_GAP;
        let list_h = (f.body.bottom() - list_y - 0.05).max(0.1);
        theme::section_label(ui, f.body.x, list_y - 0.045, &mtl!("mp-player-count", "n" => v.players.len() as u64));
        let list_w = f.body.w.min(theme::MAX_LIST_W);
        let list_x = f.body.x + (f.body.w - list_w) / 2.;
        let step = ROW_H + ROW_GAP;
        let n = v.players.len();
        self.ids.clear();
        self.rows.resize_with(n, DRectButton::new);

        if n == 0 {
            theme::text_left(ui, list_x, list_y + 0.03, FS_BODY, text_muted(), &mtl!("spectate-none"), list_w);
        }
        ui.scope(|ui| {
            ui.dx(list_x);
            ui.dy(list_y);
            self.scroll.size((list_w, list_h));
            self.scroll.render(ui, |ui| {
                for (i, (id, name)) in v.players.iter().enumerate() {
                    let rr = Rect::new(0., i as f32 * step, list_w, ROW_H);
                    let selected = Some(*id) == v.target;
                    let st = v.stats.get(id).copied().unwrap_or_default();
                    theme::row_button(ui, &mut self.rows[i], t, rr, selected, accent, |ui, r| {
                        // 左：头像 + 名字 + 徽标
                        let avr = (r.h * 0.26).min(0.044 * SCALE);
                        let cx = r.x + CARD_PAD + avr;
                        ui.avatar(cx, r.center().y, avr, t, UserManager::opt_avatar(*id, v.icon));
                        let name_x = cx + avr + 0.028;
                        let mut tags_right = r.x + r.w * 0.52;
                        if selected {
                            let s = mtl!("spectate-syncing");
                            tags_right = theme::tag_right(ui, tags_right, name_x, r.y + r.h * 0.3, &s, tag_accent(accent), WHITE);
                        }
                        theme::text_left(ui, name_x, r.y + r.h * 0.3, FS_BODY, text(), name, (tags_right - name_x).max(0.05));
                        // 右：分数 + 明细
                        let right = r.right() - CARD_PAD;
                        if st.total() == 0 {
                            theme::text_right(ui, right, r.center().y, FS_SMALL, text_muted(), &mtl!("spectate-waiting"), r.w * 0.45);
                        } else {
                            let score = format!("{:07}", st.score());
                            theme::text_right(ui, right, r.y + r.h * 0.3, FS_SECTION, text(), &score, r.w * 0.45);
                            let detail = format!(
                                "{:.2}%  ·  {}x  ·  P{} G{} B{} M{}",
                                st.accuracy() * 100.,
                                st.max_combo,
                                st.perfect,
                                st.good,
                                st.bad,
                                st.miss
                            );
                            theme::text_right(ui, right, r.y + r.h * 0.72, FS_SMALL, text_dim(), &detail, r.w * 0.46);
                        }
                    });
                    self.ids.push(*id);
                }
                (list_w, n as f32 * step)
            });
        });
        for i in n..self.rows.len() {
            self.rows[i].invalidate();
        }

        theme::text_left(
            ui,
            f.body.x,
            f.body.bottom() - 0.015,
            FS_SMALL,
            text_muted(),
            &mtl!("mp-spectate-hint"),
            f.body.w * 0.55,
        );

        // —— 底部操作条：主按钮带上目标名 ——
        let target_name = v
            .target
            .and_then(|id| v.players.iter().find(|(pid, _)| *pid == id).map(|(_, name)| name.clone()));
        let (watch_label, watch_color) = match &target_name {
            // 目标名写在按钮上："现在同步的是谁"一眼可见
            Some(name) => (format!("{}：{name}", mtl!("spectate-watch")), primary(accent)),
            None => (mtl!("spectate-watch").into_owned(), primary(accent)),
        };
        let bw = (f.bar.w.min(1.5) - BAR_COL_GAP) * 0.62;
        let br = Rect::new(f.bar.x, f.bar.y, bw, BAR_BTN_H);
        if v.playing || target_name.is_some() {
            theme::button(ui, &mut self.watch, t, br, watch_label, FS_BUTTON, watch_color, WHITE);
        } else {
            theme::button_static(ui, br, mtl!("spectate-watch"), FS_BUTTON, secondary(), text_muted());
        }
        let xr = Rect::new(br.right() + BAR_COL_GAP, f.bar.y, f.bar.w - bw - BAR_COL_GAP, BAR_BTN_H);
        theme::button(ui, &mut self.exit, t, xr, mtl!("spectate-exit"), FS_BUTTON, danger(), WHITE);
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<Action> {
        if self.back.touch(touch, t) {
            return Some(Action::Back);
        }
        if self.watch.touch(touch, t) {
            return Some(Action::Watch);
        }
        if self.exit.touch(touch, t) {
            return Some(Action::Exit);
        }
        if self.scroll.contains(touch) && self.scroll.touch(touch, t) {
            for b in self.rows.iter_mut() {
                b.inner.cancel();
            }
            return None;
        }
        for (i, b) in self.rows.iter_mut().enumerate() {
            if b.touch(touch, t) {
                return self.ids.get(i).copied().map(Action::Select);
            }
        }
        None
    }
}
