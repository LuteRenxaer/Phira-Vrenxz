//! 手柄输入支持（仅桌面平台）
//!
//! 手柄导航系统：左摇杆+十字键双重保障，A确认、B返回、X联机。
//! 支持 PS / Xbox 360 / Xbox One / Switch 等常见协议手柄。
//! gilrs 已将不同手柄的按键统一映射为标准 Button / Axis 枚举。

use macroquad::prelude::*;
use std::cell::RefCell;

/// Minimal focus target used by the optional gamepad menu navigation.
/// The current UI does not register focus targets yet; gameplay input works
/// without it.
#[derive(Clone, Default)]
pub struct FocusTarget {
    pub rect: Rect,
}

#[derive(Default, Clone)]
pub struct GamepadFrame {
    pub key_delta: i32,
    pub keys_down: u32,
    pub touches: Vec<Touch>,
    pub menu_pressed: bool,
}

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios", target_env = "ohos")))]
use gilrs::{Axis, Button, Gilrs};

#[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios", target_env = "ohos")))]
pub struct GamepadInput {
    gilrs: Option<Gilrs>,
    buttons: std::collections::HashSet<Button>,
    menu_down: bool,
    sticks: [Option<Vec2>; 2],
    a_was_down: bool,
    b_was_down: bool,
    x_was_down: bool,
    y_was_down: bool,
    home_was_down: bool,
    retry_timer: f32,
}

#[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios", target_env = "ohos"))]
pub struct GamepadInput {}

impl Default for GamepadInput {
    fn default() -> Self {
        Self::new()
    }
}

impl GamepadInput {
    #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios", target_env = "ohos")))]
    pub fn new() -> Self {
        Self {
            gilrs: Gilrs::new().ok(),
            buttons: Default::default(),
            menu_down: false,
            sticks: [None; 2],
            a_was_down: false,
            b_was_down: false,
            x_was_down: false,
            y_was_down: false,
            home_was_down: false,
            retry_timer: 0.0,
        }
    }

    #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios", target_env = "ohos"))]
    pub fn new() -> Self {
        Self {}
    }

    #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios", target_env = "ohos")))]
    pub fn poll(&mut self, dt: f32) -> (GamepadFrame, NavInput) {
        if self.gilrs.is_none() {
            self.retry_timer += dt;
            if self.retry_timer >= 2.0 {
                self.retry_timer = 0.0;
                if let Ok(g) = Gilrs::new() {
                    self.gilrs = Some(g);
                }
            }
            return (GamepadFrame::default(), NavInput::default());
        }

        let mut frame = GamepadFrame::default();
        let mut nav = NavInput::default();
        let Some(gilrs) = self.gilrs.as_mut() else {
            return (frame, nav);
        };

        while gilrs.next_event().is_some() {}

        const DEADZONE: f32 = 0.25;
        const NAV_DEADZONE: f32 = 0.25;
        const STICK_IDS: [u64; 2] = [u64::MAX - 2000, u64::MAX - 2001];
        const STICK_AXES: [(Axis, Axis); 2] = [(Axis::LeftStickX, Axis::LeftStickY), (Axis::RightStickX, Axis::RightStickY)];

        const HIT_BUTTONS: [Button; 12] = [
            Button::South, Button::East, Button::North, Button::West,
            Button::LeftTrigger, Button::LeftTrigger2,
            Button::RightTrigger, Button::RightTrigger2,
            Button::DPadUp, Button::DPadDown, Button::DPadLeft, Button::DPadRight,
        ];

        let mut current = std::collections::HashSet::new();
        let mut menu_down = false;

        let mut left_stick = Vec2::ZERO;
        let mut right_stick = Vec2::ZERO;
        let mut a_down = false;
        let mut b_down = false;
        let mut x_down = false;
        let mut y_down = false;
        let mut home_down = false;
        let mut lb = false;
        let mut rb = false;
        let mut lt_trigger = false;
        let mut rt_trigger = false;
        let mut dpad_up = false;
        let mut dpad_down = false;
        let mut dpad_left = false;
        let mut dpad_right = false;

        for (_, gp) in gilrs.gamepads() {
            for &btn in &HIT_BUTTONS {
                if gp.is_pressed(btn) {
                    current.insert(btn);
                }
            }
            if gp.value(Axis::LeftZ) > 0.5 {
                current.insert(Button::LeftTrigger2);
            }
            if gp.value(Axis::RightZ) > 0.5 {
                current.insert(Button::RightTrigger2);
            }

            if gp.is_pressed(Button::Start) || gp.is_pressed(Button::Select) || gp.is_pressed(Button::Mode) {
                menu_down = true;
            }

            for (i, &(ax_x, ax_y)) in STICK_AXES.iter().enumerate() {
                let raw = vec2(gp.value(ax_x), gp.value(ax_y));
                let active = raw.length_squared() > DEADZONE * DEADZONE;
                if active {
                    let pos = raw.clamp_length_max(1.0);
                    if self.sticks[i].is_none() {
                        frame.touches.push(Touch {
                            id: STICK_IDS[i], phase: TouchPhase::Started,
                            position: pos, time: f64::NEG_INFINITY,
                        });
                    }
                    frame.touches.push(Touch {
                        id: STICK_IDS[i], phase: TouchPhase::Moved,
                        position: pos, time: f64::NEG_INFINITY,
                    });
                    self.sticks[i] = Some(pos);
                } else if let Some(last) = self.sticks[i].take() {
                    frame.touches.push(Touch {
                        id: STICK_IDS[i], phase: TouchPhase::Ended,
                        position: last, time: f64::NEG_INFINITY,
                    });
                }
            }

            let raw_stick = vec2(gp.value(Axis::LeftStickX), gp.value(Axis::LeftStickY));
            if raw_stick.length_squared() > NAV_DEADZONE * NAV_DEADZONE {
                // Invert Y: gilrs up=+1, but UI coords down=+1
                left_stick = vec2(raw_stick.x, -raw_stick.y).clamp_length_max(1.0);
            }

            let raw_rstick = vec2(gp.value(Axis::RightStickX), gp.value(Axis::RightStickY));
            if raw_rstick.length_squared() > NAV_DEADZONE * NAV_DEADZONE {
                right_stick = vec2(raw_rstick.x, -raw_rstick.y).clamp_length_max(1.0);
            }

            a_down |= gp.is_pressed(Button::South);
            b_down |= gp.is_pressed(Button::East);
            x_down |= gp.is_pressed(Button::West);
            y_down |= gp.is_pressed(Button::North);
            home_down |= gp.is_pressed(Button::Mode);

            // LB/RB: only use digital button detection
            lb |= gp.is_pressed(Button::LeftTrigger);
            rb |= gp.is_pressed(Button::RightTrigger);

            // LT/RT: use analog trigger with threshold
            lt_trigger |= gp.value(Axis::LeftZ) > 0.3;
            rt_trigger |= gp.value(Axis::RightZ) > 0.3;

            dpad_up |= gp.is_pressed(Button::DPadUp);
            dpad_down |= gp.is_pressed(Button::DPadDown);
            dpad_left |= gp.is_pressed(Button::DPadLeft);
            dpad_right |= gp.is_pressed(Button::DPadRight);
        }

        if menu_down && !self.menu_down {
            frame.menu_pressed = true;
        }
        self.menu_down = menu_down;

        for btn in &current {
            if !self.buttons.contains(btn) {
                frame.key_delta += 1;
                frame.keys_down += 1;
            }
        }
        for btn in &self.buttons {
            if !current.contains(btn) {
                frame.key_delta -= 1;
            }
        }
        self.buttons = current;

        nav.a_pressed = a_down && !self.a_was_down;
        nav.b_pressed = b_down && !self.b_was_down;
        nav.x_pressed = x_down && !self.x_was_down;
        nav.y_pressed = y_down && !self.y_was_down;
        nav.home_pressed = home_down && !self.home_was_down;
        self.a_was_down = a_down;
        self.b_was_down = b_down;
        self.x_was_down = x_down;
        self.y_was_down = y_down;
        self.home_was_down = home_down;

        nav.left_stick = left_stick;
        nav.right_stick = right_stick;
        nav.dpad_dir = dpad_dir_from_buttons(dpad_up, dpad_down, dpad_left, dpad_right);
        nav.skip_combo = lb && rb && lt_trigger && rt_trigger;

        (frame, nav)
    }

    #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios", target_env = "ohos"))]
    pub fn poll(&mut self, _dt: f32) -> (GamepadFrame, NavInput) {
        (GamepadFrame::default(), NavInput::default())
    }

    #[cfg(not(any(target_arch = "wasm32", target_os = "android", target_os = "ios", target_env = "ohos")))]
    pub fn get_raw_state(&mut self) -> GamepadRawState {
        let mut raw = GamepadRawState::default();
        raw.gilrs_ready = self.gilrs.is_some();
        let Some(gilrs) = self.gilrs.as_ref() else {
            return raw;
        };
        let mut first = true;
        for (_, gp) in gilrs.gamepads() {
            raw.connected = true;
            raw.left_stick = vec2(gp.value(Axis::LeftStickX), gp.value(Axis::LeftStickY));
            raw.right_stick = vec2(gp.value(Axis::RightStickX), gp.value(Axis::RightStickY));
            raw.south |= gp.is_pressed(Button::South);
            raw.east |= gp.is_pressed(Button::East);
            raw.north |= gp.is_pressed(Button::North);
            raw.west |= gp.is_pressed(Button::West);
            // LB/RB: digital button only
            raw.lb |= gp.is_pressed(Button::LeftTrigger);
            raw.rb |= gp.is_pressed(Button::RightTrigger);
            // LT/RT: analog axis
            raw.lt = raw.lt.max(gp.value(Axis::LeftZ));
            raw.rt = raw.rt.max(gp.value(Axis::RightZ));
            raw.dpad_up |= gp.is_pressed(Button::DPadUp);
            raw.dpad_down |= gp.is_pressed(Button::DPadDown);
            raw.dpad_left |= gp.is_pressed(Button::DPadLeft);
            raw.dpad_right |= gp.is_pressed(Button::DPadRight);
            raw.start |= gp.is_pressed(Button::Start);
            raw.select |= gp.is_pressed(Button::Select);
            raw.mode |= gp.is_pressed(Button::Mode);
            raw.axis_lx = raw.axis_lx.max(gp.value(Axis::LeftStickX));
            raw.axis_ly = raw.axis_ly.max(gp.value(Axis::LeftStickY));
            raw.axis_rx = raw.axis_rx.max(gp.value(Axis::RightStickX));
            raw.axis_ry = raw.axis_ry.max(gp.value(Axis::RightStickY));
            raw.axis_lz = raw.axis_lz.max(gp.value(Axis::LeftZ));
            raw.axis_rz = raw.axis_rz.max(gp.value(Axis::RightZ));
            raw.btn_lt1 |= gp.is_pressed(Button::LeftTrigger);
            raw.btn_lt2 |= gp.is_pressed(Button::LeftTrigger2);
            raw.btn_rt1 |= gp.is_pressed(Button::RightTrigger);
            raw.btn_rt2 |= gp.is_pressed(Button::RightTrigger2);
            first = false;
        }
        if first {
            raw.connected = false;
        }
        raw
    }

    #[cfg(any(target_arch = "wasm32", target_os = "android", target_os = "ios", target_env = "ohos"))]
    pub fn get_raw_state(&mut self) -> GamepadRawState {
        GamepadRawState::default()
    }
}

fn dpad_dir_from_buttons(up: bool, down: bool, left: bool, right: bool) -> Vec2 {
    let x = (right as i32) as f32 - (left as i32) as f32;
    let y = (up as i32) as f32 - (down as i32) as f32;
    let v = vec2(x, -y);
    if v.length_squared() > 0.01 { v.normalize() } else { Vec2::ZERO }
}

#[derive(Default, Clone, Copy)]
pub struct NavInput {
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub dpad_dir: Vec2,
    pub a_pressed: bool,
    pub b_pressed: bool,
    pub x_pressed: bool,
    pub y_pressed: bool,
    pub home_pressed: bool,
    pub skip_combo: bool,
}

#[derive(Default, Clone)]
pub struct NavState {
    pub focus_index: usize,
    pub phase: f32,
    pub last_stick_dir: Vec2,
    pub nav_cooldown: f32,
}

impl NavState {
    pub fn new() -> Self {
        Self { focus_index: 0, phase: 0.0, last_stick_dir: Vec2::ZERO, nav_cooldown: 0.0 }
    }

    pub fn update(&mut self, targets: &[FocusTarget], input: NavInput, dt: f32) {
        self.phase += dt * 2.0;
        if targets.is_empty() { self.focus_index = 0; return; }
        if self.focus_index >= targets.len() { self.focus_index = 0; }
        if self.nav_cooldown > 0.0 { self.nav_cooldown -= dt; }

        let combined = if input.left_stick.length_squared() > input.dpad_dir.length_squared() {
            input.left_stick
        } else {
            input.dpad_dir
        };

        if combined.length_squared() > 0.2 * 0.2 && self.nav_cooldown <= 0.0 {
            let dir = combined.normalize();
            self.last_stick_dir = dir;
            self.nav_cooldown = 0.2;

            let current = &targets[self.focus_index];
            let cc = current.rect.center();
            let mut best_idx = self.focus_index;
            let mut best_score = f32::MAX;

            // Determine if navigation is primarily horizontal or vertical
            let is_vertical = dir.y.abs() > dir.x.abs();

            for (i, target) in targets.iter().enumerate() {
                if i == self.focus_index { continue; }
                let tc = target.rect.center();
                let diff = tc - cc;
                let len = diff.length();
                if len < 0.001 { continue; }

                let (alignment, dist) = if is_vertical {
                    // Must move in the correct Y direction
                    if diff.y * dir.y <= 0.0 { continue; }
                    let alignment = (diff.y / len).abs(); // 0..1, how vertical
                    let dist = diff.y.abs() + diff.x.abs() * 3.0; // penalize horizontal offset
                    (alignment, dist)
                } else {
                    // Must move in the correct X direction
                    if diff.x * dir.x <= 0.0 { continue; }
                    let alignment = (diff.x / len).abs(); // 0..1, how horizontal
                    let dist = diff.x.abs() + diff.y.abs() * 3.0; // penalize vertical offset
                    (alignment, dist)
                };

                let score = dist / (alignment + 0.3);
                if score < best_score { best_score = score; best_idx = i; }
            }

            if best_idx != self.focus_index && best_score < f32::MAX {
                self.focus_index = best_idx;
            }
        }

        if combined.length_squared() < 0.15 * 0.15 {
            self.last_stick_dir = Vec2::ZERO;
        }
    }

    pub fn current_target<'a>(&self, targets: &'a [FocusTarget]) -> Option<&'a FocusTarget> {
        targets.get(self.focus_index)
    }
}

#[derive(Default, Clone, Copy)]
pub struct GamepadRawState {
    pub connected: bool,
    pub gilrs_ready: bool,
    pub left_stick: Vec2,
    pub right_stick: Vec2,
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
    pub south: bool,
    pub east: bool,
    pub north: bool,
    pub west: bool,
    pub lb: bool,
    pub rb: bool,
    pub lt: f32,
    pub rt: f32,
    pub start: bool,
    pub select: bool,
    pub mode: bool,
    pub axis_lx: f32,
    pub axis_ly: f32,
    pub axis_rx: f32,
    pub axis_ry: f32,
    pub axis_lz: f32,
    pub axis_rz: f32,
    pub btn_lt1: bool,
    pub btn_lt2: bool,
    pub btn_rt1: bool,
    pub btn_rt2: bool,
}

thread_local! {
    static PENDING: RefCell<GamepadFrame> = RefCell::default();
    static NAV_STATE: RefCell<NavState> = RefCell::new(NavState::new());
    static GAMEPAD: RefCell<GamepadInput> = RefCell::new(GamepadInput::new());
    static LAST_FRAME: RefCell<GamepadFrame> = RefCell::default();
    static LAST_NAV_INPUT: RefCell<NavInput> = RefCell::new(NavInput::default());
    static LAST_RAW_STATE: RefCell<GamepadRawState> = RefCell::new(GamepadRawState::default());
    static CURSOR: RefCell<Vec2> = const { RefCell::new(Vec2::ZERO) };
    static GAMEPLAY_ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// GameScene marks itself active so the UI cursor does not fire extra touches
/// while the player is using the controller to hit notes.
pub fn set_gameplay_active(active: bool) {
    GAMEPLAY_ACTIVE.with(|it| it.set(active));
}

/// A simple virtual cursor for menus (the UI does not register focus targets).
pub fn update_virtual_cursor(dt: f32) {
    if GAMEPLAY_ACTIVE.with(|it| it.get()) || !is_connected() {
        return;
    }
    let nav = nav_input_static();
    let dir = if nav.left_stick.length_squared() > 0.04 {
        nav.left_stick
    } else {
        nav.dpad_dir
    };
    let mut clicked = None;
    CURSOR.with(|cursor| {
        let mut c = cursor.borrow_mut();
        c.x = (c.x + dir.x * dt * 1.6).clamp(-1., 1.);
        c.y = (c.y + dir.y * dt * 1.6).clamp(-1., 1.);
        if nav.a_pressed {
            clicked = Some(vec2(
                (c.x * 0.5 + 0.5) * screen_width(),
                (c.y * 0.5 + 0.5) * screen_height(),
            ));
        }
    });
    if let Some(pos) = clicked {
        push_nav_touch(pos);
    }
}

pub fn cursor_pos() -> Option<Vec2> {
    if GAMEPLAY_ACTIVE.with(|it| it.get()) || !is_connected() {
        None
    } else {
        CURSOR.with(|it| Some(*it.borrow()))
    }
}

pub fn push_pending(frame: GamepadFrame) {
    PENDING.with(|it| {
        let mut guard = it.borrow_mut();
        guard.key_delta += frame.key_delta;
        guard.keys_down += frame.keys_down;
        guard.menu_pressed |= frame.menu_pressed;
        guard.touches.extend(frame.touches);
    });
}

pub fn take_pending() -> GamepadFrame {
    PENDING.with(|it| std::mem::take(&mut *it.borrow_mut()))
}

pub fn last_frame() -> GamepadFrame {
    LAST_FRAME.with(|it| it.borrow().clone())
}

pub fn poll_global() {
    let dt = macroquad::prelude::get_frame_time();
    GAMEPAD.with(|it| {
        let mut guard = it.borrow_mut();
        let (frame, nav) = guard.poll(dt);
        let raw = guard.get_raw_state();
        LAST_FRAME.with(|f| { *f.borrow_mut() = frame.clone(); });
        push_pending(frame);
        LAST_NAV_INPUT.with(|n| { *n.borrow_mut() = nav; });
        LAST_RAW_STATE.with(|r| { *r.borrow_mut() = raw; });
    });
}

pub fn nav_input_static() -> NavInput {
    LAST_NAV_INPUT.with(|it| *it.borrow())
}

/// Get right stick Y for scrolling (positive = scroll down in UI coords)
pub fn right_stick_scroll() -> f32 {
    LAST_NAV_INPUT.with(|it| it.borrow().right_stick.y)
}

pub fn raw_state_static() -> GamepadRawState {
    LAST_RAW_STATE.with(|it| *it.borrow())
}

/// Whether any gamepad is currently connected.
pub fn is_connected() -> bool {
    LAST_RAW_STATE.with(|it| it.borrow().connected)
}

pub fn get_nav_state() -> NavState {
    NAV_STATE.with(|it| it.borrow().clone())
}

pub fn update_nav(targets: &[FocusTarget], input: NavInput, dt: f32) -> NavState {
    NAV_STATE.with(|it| {
        let mut state = it.borrow_mut();
        state.update(targets, input, dt);
        state.clone()
    })
}

pub fn reset_nav_focus() {
    NAV_STATE.with(|it| { it.borrow_mut().focus_index = 0; });
}

pub fn push_nav_touch(pos: Vec2) {
    let touch = Touch { id: u64::MAX - 3000, phase: TouchPhase::Started, position: pos, time: f64::NEG_INFINITY };
    push_pending(GamepadFrame {
        key_delta: 0, keys_down: 0, menu_pressed: false,
        touches: vec![touch, Touch { id: u64::MAX - 3000, phase: TouchPhase::Ended, position: pos, time: f64::NEG_INFINITY }],
    });
}

pub fn push_nav_back() { BACK_PRESSED.with(|it| { *it.borrow_mut() = true; }); }
pub fn push_nav_multilang() { MULTILANG_PRESSED.with(|it| { *it.borrow_mut() = true; }); }

thread_local! {
    static BACK_PRESSED: RefCell<bool> = const { RefCell::new(false) };
    static MULTILANG_PRESSED: RefCell<bool> = const { RefCell::new(false) };
}

pub fn take_back_pressed() -> bool { BACK_PRESSED.with(|it| std::mem::take(&mut *it.borrow_mut())) }
pub fn take_multilang_pressed() -> bool { MULTILANG_PRESSED.with(|it| std::mem::take(&mut *it.borrow_mut())) }
