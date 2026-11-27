prpr_l10n::tl_file!("dialog");

use super::{DRectButton, RectButton, Scroll, Ui, PREFER_REDUCED_MOTION};
use crate::{
    core::BOLD_FONT,
    ext::{draw_parallelogram, draw_parallelogram_band, semi_white, PARALLELOGRAM_SLOPE},
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

        ui.fill_rect(ui.screen_rect(), Color::new(0., 0., 0., 0.62 * ease));

        let mh = ui.top * 2. * HEIGHT_RATIO;
        let pad = 0.045;
        let bh = 0.085;
        let s = 0.02;
        let header_h = 0.16;

        if self.h.is_none() {
            self.h = Some(
                (ui.text(&self.message)
                    .size(0.46)
                    .max_width(2. * WIDTH_RADIO - pad * 3.)
                    .multiline()
                    .measure()
                    .h
                    + header_h
                    + bh
                    + 0.16)
                    .min(mh),
            );
        }
        let h = self.h.unwrap();
        let scale = 0.94 + 0.06 * ease;
        let ww = 2. * WIDTH_RADIO * scale;
        let wh = h * scale;
        let wr = Rect::new(-ww / 2., -wh / 2., ww, wh);
        self.window_rect = Some(ui.rect_to_global(Rect::new(-WIDTH_RADIO, -h / 2., 2. * WIDTH_RADIO, h)));

        // 主体：深灰面板 + 顶部近黑标题条（同一斜率的平行四边形）
        draw_parallelogram(wr, None, Color::new(0.185, 0.185, 0.20, 0.97 * ease), true);
        draw_parallelogram_band(wr, wr.y, wr.y + header_h, Color::new(0.05, 0.05, 0.06, 0.98 * ease));

        let l = wr.h * PARALLELOGRAM_SLOPE;
        let content_x = wr.x + l + pad;
        // 斜边收缩：给定 y 处面板的实际右边界（已扣 pad）
        let right_at = |y: f32| wr.right() - l * ((y - wr.y) / wr.h).clamp(0., 1.) - pad;
        let content_w = wr.w - l - pad * 2.;

        // 标题
        ui.text(&self.title)
            .pos(content_x + 0.01, wr.y + header_h / 2.)
            .anchor(0., 0.5)
            .size(0.62)
            .max_width(content_w)
            .no_baseline()
            .color(WHITE)
            .draw_using(&BOLD_FONT);

        // 正文区
        let body_top = wr.y + header_h + 0.035;
        let scroll_h = (wr.bottom() - bh - s - body_top - 0.03).max(0.05);
        ui.scope(|ui| {
            ui.dx(content_x);
            ui.dy(body_top);
            self.scroll.size(((right_at(body_top) - content_x).max(0.1), scroll_h));
            self.scroll.render(ui, |ui| {
                let r = ui
                    .text(&self.message)
                    .pos(0., 0.)
                    .size(0.46)
                    .max_width((right_at(body_top) - content_x).max(0.1))
                    .multiline()
                    .color(semi_white(0.9))
                    .draw();
                self.text_btn.set(ui, r);
                (r.w, r.h + 0.04)
            });
        });

        // 按钮：右下角纯文字（不再画条纹块）
        let mut rx = right_at(wr.bottom() - bh / 2. - s);
        let by = wr.bottom() - bh / 2. - s;
        for (text, btn) in self.buttons.iter().zip(self.rect_buttons.iter_mut()).rev() {
            let tr = ui
                .text(text)
                .pos(rx, by)
                .anchor(1., 0.5)
                .no_baseline()
                .size(0.46)
                .max_width(0.4)
                .color(WHITE)
                .measure_using(&BOLD_FONT);
            btn.inner.set(ui, Rect::new(rx - tr.w - 0.03, by - 0.05, tr.w + 0.06, 0.1));
            let warm = if btn.inner.touching() { WHITE } else { semi_white(0.92) };
            ui.text(text)
                .pos(rx, by)
                .anchor(1., 0.5)
                .no_baseline()
                .size(0.46)
                .max_width(0.4)
                .color(warm)
                .draw_using(&BOLD_FONT);
            rx -= tr.w + 0.09;
        }
    }
}
