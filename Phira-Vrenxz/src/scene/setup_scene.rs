
use crate::{
    get_data, get_data_mut,
    login::Login,
    mp::theme,
    page::render_switch,
    popup::ChooseButton,
    save_data,
    scene::{BGM_VOLUME_UPDATED, StartupLoadingScene},
    sync_data,
};
use anyhow::Result;
use macroquad::prelude::*;
use prpr::{
    scene::{NextScene, Scene},
    time::TimeManager,
    ui::{DRectButton, FontArc, Slider, Ui, PREFER_REDUCED_MOTION},
};
use prpr_l10n::{LanguageIdentifier, LANG_IDENTS, LANG_NAMES};
use std::sync::atomic::Ordering;

#[cfg(feature = "hykb")]
use crate::icons::Icons;
#[cfg(feature = "hykb")]
use prpr::task::Task;
#[cfg(feature = "hykb")]
use std::sync::Arc;

const FADE_IN: f32 = 0.45;

const ROW_PITCH: f32 = 0.17;

const ROW_PITCH_MAX_WIDE: f32 = 0.28;

const ROW_PITCH_MAX_TALL: f32 = 0.5;

const LABEL_W: f32 = 0.52;

const COL_GAP: f32 = 0.06;

const FS_LABEL: f32 = 0.55;

const FS_COUNTER: f32 = 0.3;

const CTRL_H: f32 = 0.1;

const FIELD_MIN_W: f32 = 0.34;
const FIELD_MAX_W: f32 = 0.7;

const INPUT_MIN_W: f32 = 0.4;
const INPUT_MAX_W: f32 = 0.9;

const BTN_W: f32 = 0.26;
const BTN_GAP: f32 = 0.05;

const FS_BUTTON: f32 = 0.45;

const SW_W: f32 = 0.28;
const SW_H: f32 = 0.1;

const SLIDER_TRACK_MAX: f32 = 0.86;

const SAVE_DELAY: f32 = 0.5;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {

    Language,

    Login,

    Volume,

    General,

    Finish,
}

impl Step {
    const ALL: [Step; 5] = [Step::Language, Step::Login, Step::Volume, Step::General, Step::Finish];

    #[inline]
    fn index(self) -> usize {
        Self::ALL.iter().position(|it| *it == self).unwrap_or(0)
    }

    fn title(self) -> String {
        match self {
            Step::Language => l10n::login::select_language(),
            Step::Login => l10n::login::login(),
            Step::Volume => l10n::settings::audio(),
            Step::General => l10n::settings::general(),
            Step::Finish => l10n::settings::tutorial(),
        }
    }

    fn rows(self) -> usize {
        match self {
            Step::Language => 1,

            Step::Login => 4,
            Step::Volume | Step::General => 3,
            Step::Finish => 2,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LoginField {
    Email,
    Name,
    Pwd,
}

#[derive(Clone, Copy)]
struct Row {
    label: Rect,
    ctrl: Rect,
}

pub struct SetupScene {

    fallback: FontArc,
    step: Step,
    enter_time: f32,

    lang_btn: ChooseButton,

    login: Option<Login>,

    #[cfg(feature = "hykb")]
    icons_task: Option<Task<Result<Arc<Icons>>>>,

    t_email: String,
    t_name: String,
    t_pwd: String,
    active: Option<LoginField>,
    input_email: DRectButton,
    input_name: DRectButton,
    input_pwd: DRectButton,
    login_btn: DRectButton,
    register_btn: DRectButton,

    music_slider: Slider,
    sfx_slider: Slider,
    bgm_slider: Slider,
    offset_slider: Slider,
    perf_btn: ChooseButton,
    reduced_motion_btn: DRectButton,
    tutorial_btn: DRectButton,

    prev_btn: DRectButton,
    next_btn: DRectButton,

    save_time: f32,

    pending: Option<NextScene>,
}

impl SetupScene {
    pub fn new(fallback: FontArc) -> Self {

        let lang_index = get_data()
            .language
            .as_ref()
            .and_then(|it| it.parse::<LanguageIdentifier>().ok())
            .and_then(|ident| LANG_IDENTS.iter().position(|it| *it == ident))
            .unwrap_or_default();

        let step = std::env::var("PHIRA_SETUP_STEP")
            .ok()
            .and_then(|it| it.parse::<usize>().ok())
            .and_then(|n| Step::ALL.get(n.saturating_sub(1)).copied())
            .unwrap_or(Step::Language);
        Self {
            fallback,
            step,
            enter_time: f32::NAN,
            lang_btn: ChooseButton::new()
                .with_options(LANG_NAMES.iter().map(|it| it.to_string()).collect())
                .with_selected(lang_index),
            login: None,
            #[cfg(feature = "hykb")]
            icons_task: None,
            t_email: String::new(),
            t_name: String::new(),
            t_pwd: String::new(),
            active: None,
            input_email: DRectButton::new(),
            input_name: DRectButton::new(),
            input_pwd: DRectButton::new(),
            login_btn: DRectButton::new(),
            register_btn: DRectButton::new(),
            music_slider: Slider::new(0.0..2.0, 0.05),
            sfx_slider: Slider::new(0.0..2.0, 0.05),
            bgm_slider: Slider::new(0.0..2.0, 0.05),
            offset_slider: Slider::new(-0.5..0.5, 0.005),

            perf_btn: ChooseButton::new()
                .with_options(l10n::settings::perf_tiers())
                .with_selected(get_data().config.performance.min(5) as usize),
            reduced_motion_btn: DRectButton::new(),
            tutorial_btn: DRectButton::new(),
            prev_btn: DRectButton::new(),
            next_btn: DRectButton::new(),
            save_time: f32::INFINITY,
            pending: None,
        }
    }

    fn rows(body: Rect, n: usize, wide: bool) -> Vec<Row> {
        let n = n.max(1);
        let max_pitch = if wide { ROW_PITCH_MAX_WIDE } else { ROW_PITCH_MAX_TALL };
        let pitch = ((body.h - ROW_PITCH) / n as f32).clamp(ROW_PITCH, max_pitch);
        let total = pitch * n as f32;
        let y0 = body.y + (body.h - total) / 2.;
        (0..n)
            .map(|i| {
                let y = y0 + i as f32 * pitch;
                Row {
                    label: Rect::new(body.x, y, LABEL_W, pitch),
                    ctrl: Rect::new(
                        body.x + LABEL_W + COL_GAP,
                        y,
                        (body.w - LABEL_W - COL_GAP).max(0.2),
                        pitch,
                    ),
                }
            })
            .collect()
    }

    fn ctrl_rect(r: Rect, w: f32, h: f32) -> Rect {
        Rect::new(r.x, r.center().y - h / 2., w, h)
    }

    fn slider_rect(r: Rect) -> Rect {
        let left = r.x + 0.14;
        let track = (r.w - 0.14 - 0.132).clamp(0.24, SLIDER_TRACK_MAX);
        let w = track / 1.2;
        Rect::new(left + 0.166 + 0.2 * w, r.center().y - 0.04, w, 0.08)
    }

    fn label(ui: &mut Ui, r: Rect, s: &str) {
        theme::text_left(ui, r.x, r.center().y, FS_LABEL, theme::text_dim(), s, r.w);
    }

    fn flat_button(ui: &mut Ui, btn: &mut DRectButton, t: f32, r: Rect, label: &str, fill: Color, fg: Color) {
        btn.build(ui, t, r, |ui, path| {
            ui.fill_path(&path, fill);
            ui.text(label)
                .pos(r.center().x, r.center().y)
                .anchor(0.5, 0.5)
                .no_baseline()
                .size(FS_BUTTON)
                .color(fg)
                .max_width(r.w * 0.92)
                .draw();
        });
    }

    fn render_header(&self, ui: &mut Ui, f: &theme::Frame) {
        let cy = f.header.center().y;
        let counter = format!("{} / {}", self.step.index() + 1, Step::ALL.len());
        theme::text_left(ui, f.header.x, cy, FS_LABEL, theme::text(), &self.step.title(), f.header.w * 0.7);
        theme::text_right(ui, f.header.right(), cy, FS_COUNTER, theme::text_muted(), &counter, f.header.w * 0.25);
    }

    fn render_body(&mut self, ui: &mut Ui, t: f32, body: Rect, wide: bool) {

        let n = if self.step == Step::Login && get_data().me.is_some() {
            1
        } else {
            self.step.rows()
        };
        let rows = Self::rows(body, n, wide);
        self.render_rows(ui, t, &rows);
    }

    fn render_rows(&mut self, ui: &mut Ui, t: f32, rows: &[Row]) {
        let accent = ui.accent();
        match self.step {
            Step::Language => {
                let w = Self::field_w(rows[0].ctrl);
                Self::label(ui, rows[0].label, &l10n::settings::lang());
                self.lang_btn.render(ui, Self::ctrl_rect(rows[0].ctrl, w, CTRL_H), t);
            }
            Step::Login => match &get_data().me {

                Some(me) => {
                    let name = me.name.clone();
                    let logged_in = l10n::login::logged_in();
                    let r = rows[0].label;
                    let cy = r.center().y;

                    let w = rows[0].ctrl.right() - r.x;
                    theme::text_left(ui, r.x, cy - 0.055, FS_LABEL, theme::text(), &name, w);
                    theme::text_left(ui, r.x, cy + 0.055, FS_COUNTER, theme::text_muted(), &logged_in, w);
                }

                None => {

                    let d_email = self.field_display(LoginField::Email);
                    let d_name = self.field_display(LoginField::Name);
                    let d_pwd = self.field_display(LoginField::Pwd);
                    let email = l10n::login::email();
                    let name = l10n::login::username();
                    let pwd = l10n::login::password();
                    let w = Self::input_w(rows[0].ctrl);
                    Self::label(ui, rows[0].label, &email);
                    self.input_email
                        .render_input(ui, Self::ctrl_rect(rows[0].ctrl, w, CTRL_H), t, &d_email, &email, 0.5);
                    Self::label(ui, rows[1].label, &name);
                    self.input_name
                        .render_input(ui, Self::ctrl_rect(rows[1].ctrl, w, CTRL_H), t, &d_name, &name, 0.5);
                    Self::label(ui, rows[2].label, &pwd);
                    self.input_pwd
                        .render_input(ui, Self::ctrl_rect(rows[2].ctrl, w, CTRL_H), t, &d_pwd, &pwd, 0.5);

                    let busy = self.login.as_ref().is_some_and(|it| it.busy());
                    let login_fill = if busy {
                        theme::color_alpha(accent, 0.5)
                    } else {
                        theme::primary(accent)
                    };
                    let register_fill = if busy {
                        theme::color_alpha(theme::secondary(), theme::secondary().a * 0.5)
                    } else {
                        theme::secondary()
                    };
                    let login_label = l10n::login::login();
                    let register_label = l10n::login::register();
                    let r = rows[3].ctrl;
                    let (login_r, register_r) = Self::button_pair(r);
                    Self::flat_button(ui, &mut self.login_btn, t, login_r, &login_label, login_fill, WHITE);
                    Self::flat_button(ui, &mut self.register_btn, t, register_r, &register_label, register_fill, theme::text());
                }
            },
            Step::Volume => {
                {
                    let v = get_data_mut().config.volume_music;
                    Self::label(ui, rows[0].label, &l10n::settings::music());
                    self.music_slider.render(ui, Self::slider_rect(rows[0].ctrl), t, v, format!("{:.2}", v));
                }
                {
                    let v = get_data_mut().config.volume_sfx;
                    Self::label(ui, rows[1].label, &l10n::settings::sfx());
                    self.sfx_slider.render(ui, Self::slider_rect(rows[1].ctrl), t, v, format!("{:.2}", v));
                }
                {
                    let v = get_data_mut().config.volume_bgm;
                    Self::label(ui, rows[2].label, &l10n::settings::bgm());
                    self.bgm_slider.render(ui, Self::slider_rect(rows[2].ctrl), t, v, format!("{:.2}", v));
                }
            }
            Step::General => {
                {

                    let v = get_data_mut().config.offset;
                    Self::label(ui, rows[0].label, &l10n::settings::cali());
                    self.offset_slider
                        .render(ui, Self::slider_rect(rows[0].ctrl), t, v, format!("{:.0}ms", v * 1000.));
                }
                {
                    let w = Self::field_w(rows[1].ctrl);
                    Self::label(ui, rows[1].label, &l10n::settings::perf());
                    self.perf_btn.render(ui, Self::ctrl_rect(rows[1].ctrl, w, CTRL_H), t);
                }
                {
                    let on = get_data().prefer_reduced_motion;
                    Self::label(ui, rows[2].label, &l10n::settings::reduced_motion());
                    render_switch(ui, Self::ctrl_rect(rows[2].ctrl, SW_W, SW_H), t, &mut self.reduced_motion_btn, on);
                }
            }
            Step::Finish => {
                {

                    let (value, color) = match &get_data().me {
                        Some(me) => (me.name.clone(), theme::text()),
                        None => (l10n::home::not_logged_in(), theme::text_muted()),
                    };
                    Self::label(ui, rows[0].label, &l10n::login::login());
                    theme::text_left(ui, rows[0].ctrl.x, rows[0].ctrl.center().y, FS_LABEL, color, &value, rows[0].ctrl.w);
                }
                {

                    let on = get_data().play_tutorial;
                    Self::label(ui, rows[1].label, &l10n::settings::tutorial());
                    render_switch(ui, Self::ctrl_rect(rows[1].ctrl, SW_W, SW_H), t, &mut self.tutorial_btn, on);
                }
            }
        }
    }

    fn button_pair(r: Rect) -> (Rect, Rect) {
        let h = CTRL_H;
        let y = r.center().y - h / 2.;
        (
            Rect::new(r.x, y, BTN_W, h),
            Rect::new(r.x + BTN_W + BTN_GAP, y, BTN_W, h),
        )
    }

    fn field_w(ctrl: Rect) -> f32 {
        ctrl.w.clamp(FIELD_MIN_W, FIELD_MAX_W)
    }

    fn input_w(ctrl: Rect) -> f32 {
        ctrl.w.clamp(INPUT_MIN_W, INPUT_MAX_W)
    }

    fn render_nav(&mut self, ui: &mut Ui, t: f32, f: &theme::Frame) {
        let accent = ui.accent();
        let last = self.step == Step::Finish;
        let labels = vec![
            l10n::library::prev_page(),
            if last { l10n::common::confirm() } else { l10n::library::next_page() },
        ];
        let (_, rects) = theme::button_bar(
            ui,
            &labels,
            f.bar.x,
            f.bar.right(),
            f.bar.bottom(),
            f.bar.h,
            theme::BAR_ROW_GAP,
            theme::BAR_COL_GAP,
            theme::BarAlign::Fill,
        );

        if self.step == Step::Language {
            self.prev_btn.invalidate();

            let dim = theme::secondary();
            Self::flat_button(
                ui,
                &mut self.prev_btn,
                t,
                rects[0],
                &labels[0],
                Color { a: dim.a * 0.5, ..dim },
                theme::text_muted(),
            );
        } else {
            Self::flat_button(ui, &mut self.prev_btn, t, rects[0], &labels[0], theme::secondary(), theme::text());
        }
        Self::flat_button(ui, &mut self.next_btn, t, rects[1], &labels[1], theme::primary(accent), WHITE);
    }

    #[cfg(not(feature = "hykb"))]
    fn ensure_login(&mut self, _t: f32) {
        if self.login.is_none() {
            self.login = Some(Login::new_bare());
        }
    }

    #[cfg(feature = "hykb")]
    fn ensure_login(&mut self, _t: f32) {
        if self.login.is_some() {
            return;
        }
        if self.icons_task.is_none() {
            self.icons_task = Some(Task::new(async { Ok(Arc::new(Icons::new().await?)) }));
            return;
        }
        let Some(task) = &mut self.icons_task else { return };
        let Some(res) = task.take() else { return };
        self.icons_task = None;
        match res {
            Ok(icons) => self.login = Some(Login::new(icons)),
            Err(err) => prpr::scene::show_error(err),
        }
    }

    fn submit_login(&mut self, t: f32) {
        self.finish_editing();
        if self.login.as_ref().is_some_and(|it| it.busy()) {
            return;
        }
        self.ensure_login(t);
        let email = self.t_email.clone();
        let pwd = self.t_pwd.clone();
        if let Some(login) = &mut self.login {
            login.submit_email_login(email, pwd);
        }
    }

    fn submit_register(&mut self, t: f32) {
        self.finish_editing();
        if self.login.as_ref().is_some_and(|it| it.busy()) {
            return;
        }
        self.ensure_login(t);
        let email = self.t_email.clone();
        let name = self.t_name.clone();
        let pwd = self.t_pwd.clone();
        if let Some(login) = &mut self.login {
            login.submit_email_register(email, name, pwd);
        }
    }

    fn activate(&mut self, f: LoginField) {
        self.active = Some(f);
        unsafe { get_internal_gl() }.quad_context.show_keyboard(true);
    }

    fn finish_editing(&mut self) {
        if self.active.take().is_some() {
            unsafe { get_internal_gl() }.quad_context.show_keyboard(false);
        }
    }

    fn field_mut(&mut self, f: LoginField) -> &mut String {
        match f {
            LoginField::Email => &mut self.t_email,
            LoginField::Name => &mut self.t_name,
            LoginField::Pwd => &mut self.t_pwd,
        }
    }

    fn field_display(&self, f: LoginField) -> String {
        let raw = match f {
            LoginField::Email => &self.t_email,
            LoginField::Name => &self.t_name,
            LoginField::Pwd => &self.t_pwd,
        };
        let shown = if f == LoginField::Pwd {
            "*".repeat(raw.chars().count())
        } else {
            raw.clone()
        };
        if self.active == Some(f) {
            format!("{shown}|")
        } else {
            shown
        }
    }

    fn update_typing(&mut self) {
        let Some(f) = self.active else { return };
        let mut batch = String::new();
        loop {
            match get_char_pressed() {
                None => break,
                Some(c) => {
                    if c == '\r' || c == '\n' {
                        self.finish_editing();
                        return;
                    }
                    if c == '\u{8}' || c == '\u{7f}' || c.is_control() {
                        continue;
                    }
                    batch.push(c);
                }
            }
        }
        if !batch.is_empty() {
            let text = self.field_mut(f);
            for c in batch.chars().rev() {
                text.push(c);
            }
        }
        if is_key_pressed(KeyCode::Backspace) {
            self.field_mut(f).pop();
        }
    }

    fn mark_dirty(&mut self, t: f32) {
        self.save_time = t;
    }

    fn goto(&mut self, t: f32, step: Step) {
        self.finish_editing();
        self.step = step;
        if step == Step::Login {

            self.ensure_login(t);
        }
    }

    fn step_prev(&mut self, t: f32) {
        let i = self.step.index();
        if i > 0 {
            self.goto(t, Step::ALL[i - 1]);
        }
    }

    fn step_next(&mut self, t: f32) {
        let i = self.step.index();
        if i + 1 < Step::ALL.len() {
            self.goto(t, Step::ALL[i + 1]);
            return;
        }
        {
            let data = get_data_mut();
            data.initial_setup_done = true;

            data.has_chosen_language = true;
        }
        let _ = save_data();
        self.pending = Some(NextScene::Replace(Box::new(StartupLoadingScene::new(self.fallback.clone()))));
    }
}

impl Scene for SetupScene {
    fn enter(&mut self, tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        if self.enter_time.is_nan() {
            self.enter_time = tm.now() as f32;
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: &Touch) -> Result<bool> {
        let t = tm.now() as f32;

        if self.lang_btn.top_touch(touch, t) {
            return Ok(true);
        }
        if self.perf_btn.top_touch(touch, t) {
            return Ok(true);
        }
        if self.prev_btn.touch(touch, t) {
            self.step_prev(t);
            return Ok(true);
        }
        if self.next_btn.touch(touch, t) {
            self.step_next(t);
            return Ok(true);
        }

        match self.step {
            Step::Language => {
                if self.lang_btn.touch(touch, t) {
                    return Ok(true);
                }
            }
            Step::Login => {

                if get_data().me.is_some() {
                    return Ok(false);
                }
                if self.input_email.touch(touch, t) {
                    self.activate(LoginField::Email);
                    return Ok(true);
                }
                if self.input_name.touch(touch, t) {
                    self.activate(LoginField::Name);
                    return Ok(true);
                }
                if self.input_pwd.touch(touch, t) {
                    self.activate(LoginField::Pwd);
                    return Ok(true);
                }
                if self.login_btn.touch(touch, t) {
                    self.submit_login(t);
                    return Ok(true);
                }
                if self.register_btn.touch(touch, t) {
                    self.submit_register(t);
                    return Ok(true);
                }
            }
            Step::Volume => {
                let config = &mut get_data_mut().config;
                if self.music_slider.touch(touch, t, &mut config.volume_music).is_some() {
                    self.mark_dirty(t);
                    return Ok(true);
                }
                if self.sfx_slider.touch(touch, t, &mut config.volume_sfx).is_some() {
                    self.mark_dirty(t);
                    return Ok(true);
                }
                let old = config.volume_bgm;
                if self.bgm_slider.touch(touch, t, &mut config.volume_bgm).is_some() {
                    if (config.volume_bgm - old).abs() > 0.001 {

                        BGM_VOLUME_UPDATED.store(true, Ordering::Relaxed);
                    }
                    self.mark_dirty(t);
                    return Ok(true);
                }
            }
            Step::General => {
                {
                    let config = &mut get_data_mut().config;
                    if self.offset_slider.touch(touch, t, &mut config.offset).is_some() {
                        self.mark_dirty(t);
                        return Ok(true);
                    }
                }
                if self.perf_btn.touch(touch, t) {
                    return Ok(true);
                }
                if self.reduced_motion_btn.touch(touch, t) {
                    let data = get_data_mut();
                    data.prefer_reduced_motion ^= true;

                    PREFER_REDUCED_MOTION.store(data.prefer_reduced_motion, Ordering::Relaxed);
                    self.mark_dirty(t);
                    return Ok(true);
                }
            }
            Step::Finish => {
                if self.tutorial_btn.touch(touch, t) {
                    get_data_mut().play_tutorial ^= true;
                    self.mark_dirty(t);
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = tm.now() as f32;
        if self.enter_time.is_nan() {
            self.enter_time = t;
        }
        self.lang_btn.update(t);
        self.perf_btn.update(t);
        self.update_typing();

        if let Some(login) = &mut self.login {
            login.update(t)?;
        }

        if self.active.is_none() && get_data().me.is_some() && !self.t_pwd.is_empty() {
            self.t_pwd.clear();
        }

        if self.lang_btn.changed() {
            let data = get_data_mut();
            data.language = Some(LANG_IDENTS[self.lang_btn.selected()].to_string());
            data.has_chosen_language = true;
            sync_data();
            self.mark_dirty(t);
        }

        if self.perf_btn.changed() {
            get_data_mut().config.performance = self.perf_btn.selected().min(5) as u8;
            self.mark_dirty(t);
        }

        if self.save_time.is_finite() && t > self.save_time + SAVE_DELAY {
            let _ = save_data();
            self.save_time = f32::INFINITY;
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        let t = tm.now() as f32;
        if self.enter_time.is_nan() {
            self.enter_time = t;
        }

        let f = theme::frame(ui, theme::BAR_BTN_H);

        set_camera(&ui.bg_camera());
        let bg = ui.background();
        ui.fill_rect(ui.screen_rect(), bg);
        set_camera(&ui.camera());

        let alpha = ((t - self.enter_time) / FADE_IN).clamp(0., 1.);
        ui.alpha(alpha, |ui| {
            self.render_header(ui, &f);
            self.render_body(ui, t, f.body, f.wide);
            self.render_nav(ui, t, &f);
        });

        self.lang_btn.render_top(ui, t, 1.);
        self.perf_btn.render_top(ui, t, 1.);

        {
            use once_cell::sync::Lazy;
            use std::sync::atomic::{AtomicU32, Ordering};
            static SHOT: Lazy<Option<String>> = Lazy::new(|| std::env::var("PHIRA_SHOT").ok());
            static N: AtomicU32 = AtomicU32::new(0);
            if let Some(path) = SHOT.as_ref() {
                if N.fetch_add(1, Ordering::Relaxed) % 300 == 0 {
                    macroquad::texture::get_screen_data().export_png(path);
                }
            }
        }
        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        self.pending.take().unwrap_or_default()
    }
}

mod l10n {

    pub(crate) mod login {
        prpr_l10n::tl_file!("login" tll);

        pub(crate) fn select_language() -> String {
            tll!("startup-select-language").into_owned()
        }

        pub(crate) fn login() -> String {
            tll!("login").into_owned()
        }

        pub(crate) fn email() -> String {
            tll!("email").into_owned()
        }

        pub(crate) fn username() -> String {
            tll!("username").into_owned()
        }

        pub(crate) fn password() -> String {
            tll!("password").into_owned()
        }

        pub(crate) fn register() -> String {
            tll!("register").into_owned()
        }

        pub(crate) fn logged_in() -> String {
            tll!("action-success", "action" => "login")
        }
    }

    pub(crate) mod settings {
        prpr_l10n::tl_file!("settings" tsl);

        pub(crate) fn lang() -> String {
            tsl!("item-lang").into_owned()
        }

        pub(crate) fn audio() -> String {
            tsl!("audio").into_owned()
        }

        pub(crate) fn general() -> String {
            tsl!("general").into_owned()
        }

        pub(crate) fn music() -> String {
            tsl!("item-music").into_owned()
        }

        pub(crate) fn sfx() -> String {
            tsl!("item-sfx").into_owned()
        }

        pub(crate) fn bgm() -> String {
            tsl!("item-bgm").into_owned()
        }

        pub(crate) fn cali() -> String {
            tsl!("item-cali").into_owned()
        }

        pub(crate) fn perf() -> String {
            tsl!("item-perf").into_owned()
        }

        pub(crate) fn perf_tiers() -> Vec<String> {
            vec![
                tsl!("item-perf-off").into_owned(),
                tsl!("item-perf-light").into_owned(),
                tsl!("item-perf-medium").into_owned(),
                tsl!("item-perf-full").into_owned(),
                tsl!("item-perf-ultra").into_owned(),
                tsl!("item-perf-custom").into_owned(),
            ]
        }

        pub(crate) fn reduced_motion() -> String {
            tsl!("item-prefer-reduced-motion").into_owned()
        }

        pub(crate) fn tutorial() -> String {
            tsl!("tutorial").into_owned()
        }
    }

    pub(crate) mod common {
        prpr_l10n::tl_file!("common" tcl);

        pub(crate) fn confirm() -> String {
            tcl!("confirm").into_owned()
        }
    }

    pub(crate) mod home {
        prpr_l10n::tl_file!("home" thl);

        pub(crate) fn not_logged_in() -> String {
            thl!("not-logged-in").into_owned()
        }
    }

    pub(crate) mod library {
        prpr_l10n::tl_file!("library" tyl);

        pub(crate) fn prev_page() -> String {
            tyl!("prev-page").into_owned()
        }

        pub(crate) fn next_page() -> String {
            tyl!("next-page").into_owned()
        }
    }
}
