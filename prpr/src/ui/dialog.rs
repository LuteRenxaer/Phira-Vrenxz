prpr_l10n::tl_file!("dialog");

use super::{DRectButton, RectButton, Scroll, Ui, PREFER_REDUCED_MOTION};
use crate::{
    core::BOLD_FONT,
    ext::{draw_parallelogram, draw_parallelogram_ex, semi_white, PARALLELOGRAM_SLOPE},
    scene::show_message,
};
use anyhow::Error;
use macroquad::prelude::*;
use std::sync::atomic::Ordering;

const WIDTH_RADIO: f32 = 0.5;
const HEIGHT_RATIO: f32 = 0.7;

type DialogListener = dyn FnMut(&mut Dialog, i32) -> bool;

#[must_use]
pub struct Dialog {
    title: String,
    message: String,
    buttons: Vec<String>,
    /// listener function returns `false` to close the dialog, `true` to keep it open
    /// the parameter is the *index* of the button clicked, `-1` for outside click, `-2` for text
    listener: Option<Box<DialogListener>>,

    text_btn: RectButton,

    h: Option<f32>,
    enter_time: f32,

    scroll: Scroll,
    window_rect: Option<Rect>,
    rect_buttons: Vec<DRectButton>,
}

impl Default for Dialog {
    fn default() -> Self {
        Self {
            title: tl!("notice").to_string(),
            message: String::new(),
            buttons: vec![tl!("ok").to_string()],
            listener: None,

            text_btn: RectButton::new(),

            h: None,
            enter_time: f32::NAN,

            scroll: Scroll::new(),
            window_rect: None,
            rect_buttons: vec![DRectButton::new()],
        }
    }
}

impl Dialog {
    pub fn simple(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            ..Default::default()
        }
    }

    pub fn plain(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            ..Default::default()
        }
    }

    pub fn error(error: Error) -> Self {
        let error = format!("{error:?}");
        Self {
            title: tl!("error").to_string(),
            message: error.clone(),
            buttons: vec![tl!("error-copy").to_string(), tl!("ok").to_string()],
            listener: Some(Box::new(move |_dialog, pos| {
                if pos == 0 {
                    unsafe { get_internal_gl() }.quad_context.clipboard_set(&error);
                    show_message(tl!("error-copied")).ok();
                }
                false
            })),

            rect_buttons: vec![DRectButton::new(); 2],
            ..Default::default()
        }
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.set_message(message);
        self
    }

    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = message.into();
    }

    pub fn buttons(mut self, buttons: Vec<String>) -> Self {
        self.set_buttons(buttons);
        self
    }

    pub fn set_buttons(&mut self, buttons: Vec<String>) {
        self.buttons = buttons;
        self.rect_buttons = vec![DRectButton::new(); self.buttons.len()];
    }

    pub fn listener(mut self, f: impl FnMut(&mut Dialog, i32) -> bool + 'static) -> Self {
        self.listener = Some(Box::new(f));
        self
    }

    pub fn show(self) {
        crate::scene::DIALOG.with(|it| *it.borrow_mut() = Some(self));
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> bool {
        self.scroll.touch(touch, t);
        let mut exit = false;
        for (index, btn) in self.rect_buttons.iter_mut().enumerate() {
            if btn.touch(touch, t) {
                if let Some(mut listener) = self.listener.take() {
                    if !listener(self, index as i32) {
                        exit = true;
                    }
                    self.listener = Some(listener);
                    break;
                } else {
                    exit = true;
                    break;
                }
            }
        }
        if self.text_btn.touch(touch) {
            if let Some(mut listener) = self.listener.take() {
                listener(self, -2);
                self.listener = Some(listener);
            }
        }
        if exit {
            return false;
        }

        if self
            .window_rect
            .is_none_or(|rect| rect.contains(touch.position) || touch.phase != TouchPhase::Started)
        {
            true
        } else {
            if let Some(mut listener) = self.listener.take() {
                let result = listener(self, -1);
                self.listener = Some(listener);
                if result {
                    return true;
                }
            }
            false
        }
    }

    pub fn update(&mut self, t: f32) {
        self.scroll.update(t);
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32) {
        if self.enter_time.is_nan() {
            self.enter_time = t;
        }
        let p = if PREFER_REDUCED_MOTION.load(Ordering::Relaxed) {
            1.
        } else {
            ((t - self.enter_time) / 0.22).clamp(0., 1.)
        };
        let ease = 1. - (1. - p).powi(3);

        // 遮罩淡入
        ui.fill_rect(ui.screen_rect(), Color::new(0., 0., 0., 0.62 * ease));

        let mh = ui.top * 2. * HEIGHT_RATIO;
        let pad = 0.03;
        let bh = 0.09;
        let s = 0.02;

        if self.h.is_none() {
            self.h = Some(
                (ui.text(&self.message)
                    .size(0.5)
                    .max_width(2. * WIDTH_RADIO - pad * 3.)
                    .multiline()
                    .measure()
                    .h
                    + ui.text(&self.title).size(0.95).no_baseline().measure().h
                    + bh
                    + 0.28)
                    .min(mh),
            );
        }
        let h = self.h.unwrap();
        // 弹出:轻微缩放
        let scale = 0.94 + 0.06 * ease;
        let ww = 2. * WIDTH_RADIO * scale;
        let wh = h * scale;
        let wr = Rect::new(-ww / 2., -wh / 2., ww, wh);
        // 点击外部关闭用的矩形(最终尺寸)
        self.window_rect = Some(ui.rect_to_global(Rect::new(-WIDTH_RADIO, -h / 2., 2. * WIDTH_RADIO, h)));

        // 主体平行四边形:顶部略亮、底部略暗,带阴影
        draw_parallelogram_ex(
            wr,
            None,
            Color::new(0.20, 0.23, 0.29, 0.97 * ease),
            Color::new(0.09, 0.11, 0.15, 0.97 * ease),
            true,
        );
        // 描边
        draw_parallelogram(wr, None, Color::new(1., 1., 1., 0.09 * ease), false);
        // 顶部高亮线
        let l = wr.h * PARALLELOGRAM_SLOPE;
        ui.fill_rect(Rect::new(wr.x + l, wr.y, wr.w - l, 0.004), Color::new(1., 1., 1., 0.12 * ease));

        // 内容安全区域(避开左侧斜切)
        let content_x = wr.x + l + pad;
        let content_w = wr.w - l - pad * 2.;

        // 标题
        let tr = ui
            .text(&self.title)
            .pos(content_x, wr.y + pad)
            .anchor(0., 0.)
            .size(0.95)
            .max_width(content_w)
            .no_baseline()
            .color(WHITE)
            .draw_using(&BOLD_FONT);

        // 消息(可滚动)
        let scroll_top = wr.y + pad + tr.h + s * 2.;
        let scroll_h = wr.bottom() - bh - s - scroll_top;
        ui.scope(|ui| {
            ui.dx(content_x);
            ui.dy(scroll_top);
            self.scroll.size((content_w - pad, scroll_h));
            self.scroll.render(ui, |ui| {
                let r = ui
                    .text(&self.message)
                    .pos(0., 0.)
                    .size(0.5)
                    .max_width(content_w - pad)
                    .multiline()
                    .color(semi_white(0.85))
                    .draw();
                self.text_btn.set(ui, r);
                (r.w, r.h + 0.04)
            });
        });

        // 按钮:平行四边形斜切 + 右侧高亮条
        // 按钮位于窗口底部,而底部边从 wr.x 收斜到 wr.right() - l,
        // 所以按底部安全区排列,避免右下角探出窗口斜切边缘。
        let n = self.buttons.len();
        let area_x = wr.x + pad;
        let area_w = wr.w - l - pad * 2.;
        let bw = (area_w - pad * (n as f32 - 1.)) / n as f32;
        let mut r = Rect::new(area_x, wr.bottom() - bh - s, bw, bh);
        for (text, btn) in self.buttons.iter().zip(self.rect_buttons.iter_mut()) {
            btn.inner.set(ui, r);
            let pressed = btn.inner.touching();
            let v = if pressed { 0.30 } else { 0.16 };
            let bg = Color::new(v, v, v + 0.05, 0.9 * ease);
            draw_parallelogram(r, None, bg, true);
            let ss = 0.05;
            draw_parallelogram(
                Rect::new(r.x + r.w * (1. - ss), r.y, r.w * ss, r.h),
                None,
                Color::new(1., 1., 1., 0.9 * ease),
                false,
            );
            let ct = r.center();
            ui.text(text)
                .pos(ct.x, ct.y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(0.5)
                .color(WHITE)
                .max_width(r.w)
                .draw();
            r.x += bw + pad;
        }
    }
}
