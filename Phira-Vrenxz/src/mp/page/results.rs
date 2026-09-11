//! 对局结算页（整屏）：概要卡 + 名次榜。
//!
//! 版面（与房间页/观战页同一套骨架）：
//!
//! ```text
//! ‹  对局结算   ·   千本桜
//! ┌───────────────────────────────────────────────┐
//! │ 3 名玩家  ·  2 人 FC  ·  最高分 1,234,567       │  ← 概要卡（带主色竖条）
//! └───────────────────────────────────────────────┘
//! 🥇 #1  alice  [我]                    1,234,567 │  ← 名次 + 名字 + 大号分数
//!                                        98.72% · FC · 1234x
//!                                        P1200 G3 B0 M1
//! ✕      bob                            放弃      │  ← 未完成的行单独灰掉
//!                        [返回]
//! ```
//!
//! 名次用行左侧的强调条 + 奖牌来表示（前三名用主色高亮行），自己的行额外带「我」徽标，
//! 因此"我在第几"不需要一行行找名字。

use macroquad::prelude::*;
use phira_mp_common::RoomResultEntry;
use prpr::ui::{DRectButton, Scroll, Ui};

use super::super::theme::{self, *};
use crate::mp::L10N_LOCAL;

/// 结算页可执行的动作。
pub enum Action {
    Back,
}

/// 三行信息（名字 / 分数 / 明细）所需的行高。
const ROW_H: f32 = 0.2;

#[derive(Default)]
pub struct ResultsPage {
    back: DRectButton,
    /// 页内返回按钮（与页头返回等价）
    back_bar: DRectButton,
    scroll: Scroll,
    entries: Vec<RoomResultEntry>,
    /// 本局谱面名（概要用，可能没有）
    chart_name: Option<String>,
}

impl ResultsPage {
    /// 收到 `RoomResults`：写入结算数据（会话随后切到本页）。
    pub fn show(&mut self, entries: Vec<RoomResultEntry>, chart_name: Option<String>) {
        self.entries = entries;
        self.chart_name = chart_name;
        self.scroll.y_scroller.reset();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.chart_name = None;
    }

    #[inline]
    pub fn has_data(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.back_bar.invalidate();
    }

    pub fn update(&mut self, t: f32) {
        self.scroll.update(t);
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, me: Option<i32>) {
        let accent = ui.accent();
        let f = theme::frame(ui, BAR_BTN_H);
        match self.chart_name.as_deref() {
            Some(name) => {
                let sub = mtl!("spectate-chart", "chart" => name.to_owned());
                theme::header(ui, f.header, &mut self.back, &mut DRectButton::new(), t, &mtl!("results-title"), Some(&sub), None);
            }
            None => {
                theme::header(ui, f.header, &mut self.back, &mut DRectButton::new(), t, &mtl!("results-title"), None, None);
            }
        }

        // —— 概要卡：人数 / FC 数 / 最高分 ——
        let played: Vec<&RoomResultEntry> = self.entries.iter().filter(|r| !r.aborted).collect();
        let fc = played.iter().filter(|r| r.full_combo).count();
        let best = played.iter().map(|r| r.score).max().unwrap_or(0);
        let abort = self.entries.len() - played.len();
        let mut parts: Vec<String> = vec![mtl!("mp-n-players", "n" => played.len() as u64)];
        if fc > 0 {
            // FC 是圈子里的通用写法，不做本地化
            parts.push(format!("{fc} FC"));
        }
        if best > 0 {
            parts.push(mtl!("spectate-best", "score" => format!("{best:07}")));
        }
        if abort > 0 {
            parts.push(format!("{abort} {}", mtl!("results-aborted")));
        }
        let summary_h = 0.14f32.min(f.body.h * 0.3);
        let summary = Rect::new(f.body.x, f.body.y, f.body.w, summary_h);
        theme::card_accented(ui, summary, card(), accent);
        theme::text_left(
            ui,
            summary.x + CARD_PAD + 0.012,
            summary.center().y,
            FS_SECTION,
            text(),
            &parts.join("   ·   "),
            summary.w - CARD_PAD * 2.,
        );

        // —— 名次榜 ——
        let list_w = f.body.w.min(theme::MAX_LIST_W);
        let list_x = f.body.x + (f.body.w - list_w) / 2.;
        let list_y = summary.bottom() + SECTION_GAP;
        let list_h = (f.bar.y - BAR_GAP - list_y).max(0.1);
        let n = self.entries.len();
        if n == 0 {
            theme::text_left(ui, list_x, list_y + 0.03, FS_BODY, text_muted(), &mtl!("mp-msg-none"), list_w);
        }
        ui.scope(|ui| {
            ui.dx(list_x);
            ui.dy(list_y);
            self.scroll.size((list_w, list_h));
            let entries = &self.entries;
            self.scroll.render(ui, |ui| {
                let step = ROW_H + ROW_GAP;
                // 名次只按"完成的人"排：中途放弃的行不占名次
                let mut rank = 0usize;
                for (i, r) in entries.iter().enumerate() {
                    let rr = Rect::new(0., i as f32 * step, list_w, ROW_H);
                    let is_me = Some(r.user_id) == me;
                    let medal = if r.aborted {
                        None
                    } else {
                        rank += 1;
                        Some(rank)
                    };
                    let top3 = matches!(medal, Some(1..=3));
                    theme::row_static(ui, rr, is_me || top3, accent);
                    // 左侧名次条 + 名次
                    let mark = match medal {
                        None => "✕".to_owned(),
                        Some(1) => "🥇".to_owned(),
                        Some(2) => "🥈".to_owned(),
                        Some(3) => "🥉".to_owned(),
                        Some(r) => format!("#{r}"),
                    };
                    let mark_color = if r.aborted {
                        text_muted()
                    } else if top3 {
                        accent
                    } else {
                        text_dim()
                    };
                    theme::text_left(ui, rr.x + CARD_PAD, rr.y + rr.h * 0.3, FS_SECTION, mark_color, &mark, 0.24);

                    let name_x = rr.x + CARD_PAD + 0.26;
                    let mut tags_right = rr.x + rr.w * 0.62;
                    if is_me {
                        let s = mtl!("mp-you");
                        tags_right = theme::tag_right(ui, tags_right, name_x, rr.y + rr.h * 0.3, &s, tag_bg(), text_dim());
                    }
                    let fg = if r.aborted { text_muted() } else { text() };
                    theme::text_left(ui, name_x, rr.y + rr.h * 0.3, FS_BODY, fg, &r.user_name, (tags_right - name_x).max(0.05));

                    let right = rr.right() - CARD_PAD;
                    if r.aborted {
                        theme::text_right(ui, right, rr.center().y, FS_SMALL, text_muted(), &mtl!("results-aborted"), rr.w * 0.3);
                    } else {
                        let score = format!("{:07}", r.score);
                        theme::text_right(ui, right, rr.y + rr.h * 0.3, FS_SECTION, text(), &score, rr.w * 0.34);
                        let mut detail = format!("{:.2}%", r.accuracy * 100.);
                        if r.full_combo {
                            detail.push_str(" · FC");
                        }
                        if r.max_combo > 0 {
                            detail.push_str(&format!(" · {}x", r.max_combo));
                        }
                        theme::text_right(ui, right, rr.y + rr.h * 0.62, FS_SMALL, text_dim(), &detail, rr.w * 0.34);
                        let judges = format!("P{} G{} B{} M{}", r.perfect, r.good, r.bad, r.miss);
                        theme::text_right(ui, right, rr.y + rr.h * 0.86, FS_SMALL, text_muted(), &judges, rr.w * 0.34);
                    }
                }
                (list_w, n as f32 * (ROW_H + ROW_GAP))
            });
        });

        let bw = 0.7f32.min(f.bar.w);
        theme::button(
            ui,
            &mut self.back_bar,
            t,
            Rect::new(f.bar.center().x - bw / 2., f.bar.y, bw, BAR_BTN_H),
            mtl!("mp-back"),
            FS_BUTTON,
            secondary(),
            text(),
        );
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> Option<Action> {
        if self.back.touch(touch, t) || self.back_bar.touch(touch, t) {
            return Some(Action::Back);
        }
        if self.scroll.touch(touch, t) {
            return None;
        }
        None
    }
}
