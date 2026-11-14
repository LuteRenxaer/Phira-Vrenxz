use crate::page::Fader;
use macroquad::prelude::*;
use nalgebra::Translation2;
use prpr::{
    core::Matrix,
    ext::{semi_black, semi_white, RectExt},
    ui::{button_hit, rounded_rect_shadow, DRectButton, RectButton, Scroll, ShadowConfig, Ui},
};

/// 设置页两栏布局里的「值」方块：深色底 + 极细描边 + 右侧下拉箭头
pub fn chip(ui: &mut Ui, r: Rect, text: &str, size: f32) {
    ui.fill_rect(r, Color::new(0.025, 0.027, 0.032, 0.62));
    let b = 0.0016;
    let c = semi_white(0.16);
    ui.fill_rect(Rect::new(r.x, r.y, r.w, b), c);
    ui.fill_rect(Rect::new(r.x, r.bottom() - b, r.w, b), c);
    ui.fill_rect(Rect::new(r.x, r.y, b, r.h), c);
    ui.fill_rect(Rect::new(r.right() - b, r.y, b, r.h), c);
    ui.text(text)
        .pos(r.x + 0.018, r.center().y)
        .anchor(0., 0.5)
        .size(size)
        .max_width(r.w - 0.036)
        .color(semi_white(0.92))
        .draw();
}

/// 值方块右侧的小三角
pub fn chip_caret(ui: &mut Ui, r: Rect) {
    use lyon::math::point;
    let cx = r.right() - 0.026;
    let cy = r.center().y;
    let mut b = lyon::path::Path::builder();
    b.begin(point(cx - 0.010, cy - 0.004));
    b.line_to(point(cx + 0.010, cy - 0.004));
    b.line_to(point(cx, cy + 0.008));
    b.end(true);
    ui.fill_path(&b.build(), semi_white(0.75));
}

pub struct Popup {    scroll: Scroll,
    rect: Rect,
    showing: bool,
    options: Vec<(String, RectButton)>,
    selected: usize,
    left: f32,
    size: f32,
    height: f32,
    fader: Fader,
    changed: bool,
    auto_dismiss: bool,
    pending_dismiss: bool,
    auto_adjust: Option<Rect>,
}

impl Popup {
    pub fn new() -> Self {
        Self {
            scroll: Scroll::new(),
            rect: Rect::default(),
            showing: false,
            options: Vec::new(),
            selected: usize::MAX,
            left: 0.024,
            size: 0.6,
            height: 0.1,
            fader: Fader::new().with_time(0.4).with_distance(0.04),
            changed: false,
            auto_dismiss: true,
            pending_dismiss: false,
            auto_adjust: None,
        }
    }

    #[inline]
    pub fn with_options(mut self, options: Vec<String>) -> Self {
        self.set_options(options);
        self
    }

    #[inline]
    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    #[inline]
    pub fn selected(&self) -> usize {
        self.selected
    }

    #[inline]
    pub fn set_options(&mut self, options: Vec<String>) {
        self.options = options.into_iter().map(|it| (it, RectButton::new())).collect();
    }

    #[inline]
    pub fn set_selected(&mut self, selected: usize) {
        self.selected = selected;
    }

    #[inline]
    pub fn set_auto_dismiss(&mut self, auto_dismiss: bool) {
        self.auto_dismiss = auto_dismiss;
    }

    #[inline]
    pub fn set_auto_adjust(&mut self, auto_adjust: Option<Rect>) {
        self.auto_adjust = auto_adjust;
    }

    pub fn set_bottom(&mut self, bottom: bool) {
        self.fader.distance = self.fader.distance.abs() * if bottom { 1. } else { -1. };
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn show(&mut self, ui: &mut Ui, t: f32, r: Rect) {
        let rect = ui.rect_to_global(r);
        self.show_global(t, rect);
    }

    /// 用已经换算好的屏幕坐标弹窗。
    /// `render_flat` 里的行坐标带着滚动容器的偏移，必须先转成全局坐标再判断
    /// 下方放不放得下，否则会把「本地坐标」和 `ui.top` 混着比，弹窗翻到屏幕外。
    pub fn show_global(&mut self, t: f32, mut rect: Rect) {
        if let Some(area) = self.auto_adjust {
            rect.x = rect.x.clamp(area.x, area.right() - rect.w);
            rect.y = rect.y.clamp(area.y, area.bottom() - rect.h);
        }
        self.rect = rect;
        self.showing = true;
        self.fader.sub(t);
    }

    pub fn dismiss(&mut self, t: f32) {
        self.showing = false;
        self.fader.back(t);
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32, alpha: f32) {
        if !self.fader.transiting() && !self.showing {
            return;
        }
        let r = self.rect;
        self.scroll.size((r.w, r.h));
        self.fader.reset();
        ui.abs_scope(|ui| {
            ui.dx(r.x);
            ui.dy(r.y);
            self.fader.for_sub(|f| {
                f.render(ui, t, |ui| {
                    let r = Rect::new(0., 0., r.w, r.h);
                    let mut cfg = ShadowConfig {
                        radius: 0.01,
                        elevation: 0.01,
                        ..Default::default()
                    };
                    cfg.base *= alpha;
                    rounded_rect_shadow(ui, r, &cfg);
                    ui.fill_path(&r.rounded(0.01), Color { a: alpha, ..ui.background() });
                    self.scroll.render(ui, |ui| {
                        for (id, (opt, btn)) in self.options.iter_mut().enumerate() {
                            if id != 0 {
                                ui.fill_rect(Rect::new(0.02, -0.001, r.w - 0.04, 0.002), semi_white(0.7 * alpha));
                            }
                            let r = Rect::new(0., 0., r.w, self.height);
                            btn.set(ui, r);
                            let chosen = id == self.selected;
                            if chosen {
                                ui.fill_rect(r.feather(-0.007), semi_black(0.4 * alpha));
                            }
                            ui.text(opt.as_str())
                                .pos(self.left, self.height / 2.)
                                .anchor(0., 0.5)
                                .no_baseline()
                                .size(self.size)
                                .max_width(r.w - self.left * 2.)
                                .color(semi_white(alpha))
                                .draw();
                            ui.dy(self.height);
                        }
                        (r.w, self.options.len() as f32 * self.height)
                    });
                });
            });
        });
    }

    pub fn update(&mut self, t: f32) {
        if self.showing {
            let old_matrix = self.scroll.matrix();
            let mut transform = Matrix::identity();
            transform *= Translation2::new(self.rect.x, self.rect.y).to_homogeneous();
            if let Some(inv) = transform.try_inverse() {
                self.scroll.set_matrix(Some(inv));
            }
            self.scroll.update(t);
            self.scroll.set_matrix(old_matrix);
        } else {
            self.scroll.update(t);
        }
        self.fader.done(t);
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> bool {
        if self.pending_dismiss {
            if touch.phase == TouchPhase::Ended {
                self.dismiss(t);
                self.pending_dismiss = false;
            }
            return true;
        }
        if self.showing {
            if touch.phase != TouchPhase::Started || self.rect.contains(touch.position) {
                if self.scroll.touch(touch, t) {
                    return true;
                }
                if self.rect.contains(touch.position) {
                    for (id, (_, btn)) in self.options.iter_mut().enumerate() {
                        if btn.touch(touch) {
                            button_hit();
                            if self.selected != id {
                                self.selected = id;
                                self.changed = true;
                            }
                            if self.auto_dismiss {
                                self.dismiss(t);
                            }
                            return true;
                        }
                    }
                    return true;
                }
                false
            } else if touch.phase == TouchPhase::Started {
                self.pending_dismiss = true;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    #[inline]
    pub fn showing(&self) -> bool {
        self.showing
    }

    #[inline]
    pub fn changed(&mut self) -> bool {
        if self.changed {
            self.changed = false;
            true
        } else {
            false
        }
    }
}

pub struct ChooseButton {
    btn: DRectButton,
    popup: Popup,
    width: Option<f32>,
    height: f32,
    need_to_show: bool,
}

impl ChooseButton {
    pub fn new() -> Self {
        Self {
            btn: DRectButton::new(),
            popup: Popup::new(),
            width: None,
            height: 0.34,
            need_to_show: false,
        }
    }

    #[inline]
    pub fn with_options(mut self, options: Vec<String>) -> Self {
        self.popup = self.popup.with_options(options);
        self
    }

    #[inline]
    pub fn with_selected(mut self, selected: usize) -> Self {
        self.popup.selected = selected;
        self
    }

    #[inline]
    pub fn set_options(&mut self, options: Vec<String>) {
        self.popup.set_options(options);
    }

    #[inline]
    pub fn with_height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    /// 弹窗宽度（不改变按钮本身的宽度）。选项文字很长时用。
    #[inline]
    pub fn with_width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    #[inline]
    pub fn selected(&self) -> usize {
        self.popup.selected
    }

    pub fn set_selected(&mut self, selected: usize) {
        self.popup.selected = selected;
    }

    #[inline]
    pub fn changed(&mut self) -> bool {
        self.popup.changed()
    }

    pub fn render(&mut self, ui: &mut Ui, r: Rect, t: f32) {
        self.btn
            .render_text_para(ui, r, t, &self.popup.options[self.popup.selected].0, self.popup.size, false);
        if self.need_to_show {
            let pad = 0.007;
            let mut rr = Rect::new(r.x, r.bottom() + pad, self.width.unwrap_or(r.w), self.height);
            let delta = 0.1;
            rr.x -= delta;
            rr.w += delta;
            self.popup.set_bottom(true);
            self.popup.show(ui, t, rr);
            self.need_to_show = false;
        }
    }

    #[inline]
    pub fn render_top(&mut self, ui: &mut Ui, t: f32, alpha: f32) {
        self.popup.render(ui, t, alpha);
    }

    /// 扁平样式：深色值方块 + 当前选项 + 下拉箭头（设置页两栏布局用）
    pub fn render_flat(&mut self, ui: &mut Ui, r: Rect, t: f32, size: f32) {
        let text = self.popup.options[self.popup.selected].0.clone();
        self.btn.build(ui, t, r, |ui, _| {
            chip(ui, r, &text, size);
            chip_caret(ui, r);
        });
        if self.need_to_show {
            let pad = 0.007;
            let h = self.height.min(ui.top * 2. - 0.06);
            let w = self.width.unwrap_or(r.w);
            // 先把按钮和「往下展开后的底边」都换到屏幕坐标，再决定往哪边弹；
            // r 是滚动容器里的局部坐标，直接和 ui.top 比会把弹窗翻到屏幕外。
            let gr = ui.rect_to_global(r);
            let open_up = gr.y + gr.h + pad + h > ui.top - 0.02;
            let mut target = if open_up {
                Rect::new(gr.x, gr.y - h - pad, w, h)
            } else {
                Rect::new(gr.x, gr.y + gr.h + pad, w, h)
            };
            target.y = target.y.clamp(-ui.top + 0.02, ui.top - 0.02 - h);
            target.x -= 0.1;
            target.w += 0.1;
            self.popup.set_bottom(!open_up);
            self.popup.show_global(t, target);
            self.need_to_show = false;
        }
    }

    pub fn update(&mut self, t: f32) {
        self.popup.update(t);
    }

    pub fn top_touch(&mut self, touch: &Touch, t: f32) -> bool {
        if self.popup.showing() {
            if self.popup.touch(touch, t) {
                return true;
            }
            self.popup.rect.contains(touch.position)
        } else {
            self.popup.fader.transiting()
        }
    }

    pub fn touch(&mut self, touch: &Touch, t: f32) -> bool {
        if self.btn.touch(touch, t) {
            self.need_to_show = true;
            true
        } else {
            false
        }
    }
}
