//! 观战页（整屏）：正在游玩的谱面 + 各玩家实时统计（P/G/B/M、连击、近似准确率与
//! 分数），点行选择同步观战目标，底部「同步观战 / 退出观战」。
//!
//! 原本这是居中浮层，现在是整屏页面。

use std::collections::HashMap;

use macroquad::prelude::*;
use prpr::ui::{DRectButton, Scroll, Ui};

use super::super::{
    spectate::SpectateStat,
    theme::{self, *},
};
use crate::mp::L10N_LOCAL;

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
    /// 正在游玩的谱面名（服务端补发选谱消息时记录）
    pub chart_name: Option<&'a str>,
    /// 当前同步观战目标
    pub target: Option<i32>,
    /// 玩家 id 与名字（顺序即渲染顺序）
    pub players: &'a [(i32, String)],
    /// 各玩家实时统计
    pub stats: &'a HashMap<i32, SpectateStat>,
}

#[derive(Default)]
pub struct SpectatePage {
    back: DRectButton,
    watch: DRectButton,
    exit: DRectButton,
    scroll: Scroll,
    /// 行按钮（索引与 `ids` 对齐）
    rows: Vec<DRectButton>,
    /// 渲染时记录的用户 id：触摸只按这份数据索引，与渲染同一来源
    ids: Vec<i32>,
}

impl SpectatePage {
    pub fn invalidate(&mut self) {
        self.back.invalidate();
        self.watch.invalidate();
        self.exit.invalidate();
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
        let sub = v.chart_name.map(|it| mtl!("spectate-chart", "chart" => it.to_owned()));
        theme::header(
            ui,
            f.header,
            &mut self.back,
            &mut DRectButton::new(),
            t,
            &mtl!("spectate-title"),
            sub.as_deref(),
            None,
        );

        let list_w = f.body.w.min(theme::MAX_LIST_W);
        let list_x = f.body.x + (f.body.w - list_w) / 2.;
        let row_h = ROW_TALL * 1.2;
        let step = row_h + ROW_GAP;
        let n = v.players.len();
        self.ids.clear();
        self.rows.resize_with(n, DRectButton::new);

        if n == 0 {
            theme::text_left(ui, list_x, f.body.y + 0.05, FS_SECTION, text_muted(), &mtl!("spectate-none"), list_w);
        }
        ui.scope(|ui| {
            ui.dx(list_x);
            ui.dy(f.body.y);
            self.scroll.size((list_w, f.body.h));
            self.scroll.render(ui, |ui| {
                for (i, (id, name)) in v.players.iter().enumerate() {
                    let rr = Rect::new(0., i as f32 * step, list_w, row_h);
                    let selected = Some(*id) == v.target;
                    let st = v.stats.get(id).copied().unwrap_or_default();
                    theme::row_button(ui, &mut self.rows[i], t, rr, selected, accent, |ui, r| {
                        theme::text_left(ui, r.x + CARD_PAD, r.y + r.h * 0.32, FS_SECTION, text(), name, r.w * 0.42);
                        let line = if st.total() == 0 {
                            mtl!("spectate-waiting").into_owned()
                        } else {
                            format!("{:07} · {:.2}% · {}x", st.score(), st.accuracy() * 100., st.max_combo)
                        };
                        theme::text_right(ui, r.right() - CARD_PAD, r.y + r.h * 0.32, FS_BODY, text_dim(), &line, r.w * 0.52);
                        let detail = format!("P{}  G{}  B{}  M{}", st.perfect, st.good, st.bad, st.miss);
                        theme::text_right(ui, r.right() - CARD_PAD, r.y + r.h * 0.76, FS_SMALL, text_muted(), &detail, r.w * 0.52);
                    });
                    self.ids.push(*id);
                }
                (list_w, n as f32 * step)
            });
        });

        theme::text_left(
            ui,
            list_x,
            f.body.bottom() - 0.015,
            FS_SMALL,
            text_muted(),
            &mtl!("mp-spectate-hint"),
            list_w,
        );

        let bw = (f.bar.w.min(1.2) - 0.04) / 2.;
        let x = f.bar.center().x - (bw * 2. + 0.04) / 2.;
        theme::button(ui, &mut self.watch, t, Rect::new(x, f.bar.y, bw, BAR_BTN_H), mtl!("spectate-watch"), FS_BUTTON, primary(accent), WHITE);
        theme::button(
            ui,
            &mut self.exit,
            t,
            Rect::new(x + bw + 0.04, f.bar.y, bw, BAR_BTN_H),
            mtl!("spectate-exit"),
            FS_BUTTON,
            danger(),
            WHITE,
        );
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
        if self.scroll.touch(touch, t) {
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
