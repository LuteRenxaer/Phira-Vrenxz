//! 对局结算页（整屏）：排名列表 + 分数 / 准确率 / 连击。

use macroquad::prelude::*;
use phira_mp_common::RoomResultEntry;
use prpr::ui::{DRectButton, Scroll, Ui};

use super::super::theme::{self, *};
use crate::mp::L10N_LOCAL;

/// 结算页可执行的动作。
pub enum Action {
    Back,
}

#[derive(Default)]
pub struct ResultsPage {
    back: DRectButton,
    /// 页内返回按钮（与页头返回等价）
    back_bar: DRectButton,
    scroll: Scroll,
    entries: Vec<RoomResultEntry>,
}

impl ResultsPage {
    /// 收到 `RoomResults`：写入结算数据（会话随后切到本页）。
    pub fn show(&mut self, entries: Vec<RoomResultEntry>) {
        self.entries = entries;
        self.scroll.y_scroller.reset();
    }

    pub fn clear(&mut self) {
        self.entries.clear();
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

    pub fn render(&mut self, ui: &mut Ui, t: f32) {
        let accent = ui.accent();
        let f = theme::frame(ui, BAR_BTN_H);
        let sub = mtl!("mp-n-players", "n" => self.entries.len() as u64);
        theme::header(
            ui,
            f.header,
            &mut self.back,
            &mut DRectButton::new(),
            t,
            &mtl!("results-title"),
            Some(&sub),
            None,
        );

        let list_w = f.body.w.min(theme::MAX_LIST_W);
        let list_x = f.body.x + (f.body.w - list_w) / 2.;
        let row_h = ROW_TALL * 1.15;
        let step = row_h + ROW_GAP;
        let n = self.entries.len();
        if n == 0 {
            theme::text_left(ui, list_x, f.body.y + 0.05, FS_SECTION, text_muted(), &mtl!("mp-msg-none"), list_w);
        }
        ui.scope(|ui| {
            ui.dx(list_x);
            ui.dy(f.body.y);
            self.scroll.size((list_w, f.body.h));
            let entries = &self.entries;
            self.scroll.render(ui, |ui| {
                for (i, r) in entries.iter().enumerate() {
                    let rr = Rect::new(0., i as f32 * step, list_w, row_h);
                    if r.aborted {
                        theme::row_static(ui, rr, false, accent);
                    } else if i < 3 {
                        theme::row_static(ui, rr, true, accent);
                    } else {
                        theme::row_static(ui, rr, false, accent);
                    }
                    // 名次
                    let medal = if r.aborted {
                        "✕".to_owned()
                    } else if i == 0 {
                        "🥇".to_owned()
                    } else if i == 1 {
                        "🥈".to_owned()
                    } else if i == 2 {
                        "🥉".to_owned()
                    } else {
                        format!("#{}", i + 1)
                    };
                    theme::text_left(ui, rr.x + CARD_PAD, rr.y + rr.h * 0.32, FS_BODY, text_dim(), &medal, 0.22);
                    let fg = if r.aborted { text_muted() } else { text() };
                    let right = rr.right() - CARD_PAD;
                    if r.aborted {
                        theme::text_right(ui, right, rr.y + rr.h * 0.32, FS_SMALL, text_muted(), &mtl!("results-aborted"), rr.w * 0.5);
                    } else {
                        let score = format!("{:07}", r.score);
                        theme::text_right(ui, right, rr.y + rr.h * 0.32, FS_BIG, fg, &score, rr.w * 0.4);
                        let mut detail = format!("{:.2}%", r.accuracy * 100.);
                        if r.full_combo {
                            detail.push_str(" · FC");
                        }
                        if r.max_combo > 0 {
                            detail.push_str(&format!(" · {}combo", r.max_combo));
                        }
                        theme::text_left(ui, rr.x + CARD_PAD + 0.22, rr.y + rr.h * 0.76, FS_SMALL, text_dim(), &detail, rr.w - 0.3);
                    }
                    theme::text_left(
                        ui,
                        rr.x + CARD_PAD + 0.22,
                        rr.y + rr.h * 0.32,
                        FS_SECTION,
                        fg,
                        &r.user_name,
                        rr.w - 0.22 - CARD_PAD * 2. - rr.w * 0.4,
                    );
                }
                (list_w, n as f32 * step)
            });
        });

        theme::button(
            ui,
            &mut self.back_bar,
            t,
            Rect::new(f.bar.center().x - 0.35, f.bar.y, 0.7, BAR_BTN_H),
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
