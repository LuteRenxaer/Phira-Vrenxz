use crate::{
    event::{EventHandler, KeyCode, TouchPhase},
    native::egl::{self, LibEgl},
    native::NativeDisplay,
    GraphicsContext,
};
use ohos_hilog_binding::{hilog_error, hilog_fatal, hilog_info};
use ohos_init_binding::canIUse;
use ohos_input_sys::input_manager::*;
use ohos_native_buffer_sys::OH_NativeBuffer_Usage_NATIVEBUFFER_USAGE_CPU_READ;
use ohos_native_window_sys::{
    NativeWindowOperation_GET_USAGE, NativeWindowOperation_SET_USAGE,
    OH_NativeWindow_NativeWindowHandleOpt,
};
use ohos_qos_sys::{OH_QoS_SetThreadQoS, QoS_Level_QOS_USER_INTERACTIVE};
use ohos_sys_opaque_types::{Input_AxisEvent, Input_MouseEvent, Input_TouchEvent};
use ohos_xcomponent_binding::{WindowRaw, XComponent};
use ohos_xcomponent_sys::{
    OH_NativeXComponent, OH_NativeXComponent_ExpectedRateRange, OH_NativeXComponent_GetKeyEvent,
    OH_NativeXComponent_GetKeyEventAction, OH_NativeXComponent_GetKeyEventCode,
    OH_NativeXComponent_RegisterKeyEventCallback, OH_NativeXComponent_SetExpectedFrameRateRange,
};
mod keycodes;
pub use crate::gl::{self, *};
use crate::{OHOS_ENV, OHOS_EXPORTS};
use napi_derive_ohos::napi;
use napi_ohos::{
    bindgen_prelude::*,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
};
use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    sync::mpsc,
    sync::OnceLock,
    thread,
};

static IS_2IN1_DEVICE: AtomicBool = AtomicBool::new(false);
static INTERCEPTOR: AtomicUsize = AtomicUsize::new(0);
static REQUEST_CALLBACK: OnceLock<
    ThreadsafeFunction<String, (), String, napi_ohos::Status, false, false, 1>,
> = OnceLock::new();

#[derive(Debug)]
enum Message {
    SurfaceChanged {
        window: WindowRaw,
        width: i32,
        height: i32,
    },
    SurfaceCreated {
        window: WindowRaw,
    },
    SurfaceDestroyed,
    Touch {
        phase: TouchPhase,
        touch_id: u64,
        x: f32,
        y: f32,
        time: u64,
    },
    Character {
        character: u32,
    },
    KeyDown {
        keycode: KeyCode,
    },
    KeyUp {
        keycode: KeyCode,
    },
    Pause,
    Resume,
    Destroy,
}

unsafe impl Send for Message {}

thread_local! {
    static MESSAGES_TX: RefCell<Option<mpsc::Sender<Message>>> = RefCell::new(None);
}

fn send_message(message: Message) {
    MESSAGES_TX.with(|tx| {
        let mut tx = tx.borrow_mut();
        tx.as_mut().unwrap().send(message).unwrap();
    })
}

struct OHOSDisplay {
    screen_width: f32,
    screen_height: f32,
    fullscreen: bool,
}

impl NativeDisplay for OHOSDisplay {
    fn screen_size(&self) -> (f32, f32) {
        (self.screen_width as _, self.screen_height as _)
    }
    fn dpi_scale(&self) -> f32 {
        1.
    }
    fn high_dpi(&self) -> bool {
        true
    }
    fn order_quit(&mut self) {}
    fn request_quit(&mut self) {}
    fn cancel_quit(&mut self) {}
    fn set_cursor_grab(&mut self, _grab: bool) {}
    fn show_mouse(&mut self, _shown: bool) {}
    fn set_mouse_cursor(&mut self, _cursor: crate::CursorIcon) {}
    fn set_window_size(&mut self, _new_width: u32, _new_height: u32) {}
    fn set_fullscreen(&mut self, _: bool) {}
    fn clipboard_get(&mut self) -> Option<String> {
        None
    }
    fn clipboard_set(&mut self, _: &str) {}

    fn show_keyboard(&mut self, _: bool) {}
    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self as _
    }
}

struct MainThreadState {
    libegl: LibEgl,
    context: GraphicsContext,
    egl_display: egl::EGLDisplay,
    egl_config: egl::EGLConfig,
    egl_context: egl::EGLContext,
    surface: egl::EGLSurface,
    display: OHOSDisplay,
    window: WindowRaw,
    event_handler: Box<dyn EventHandler>,
    quit: bool,
}

impl MainThreadState {
    unsafe fn destroy_surface(&mut self) {
        (self.libegl.eglMakeCurrent.unwrap())(
            self.egl_display,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        (self.libegl.eglDestroySurface.unwrap())(self.egl_display, self.surface);
        self.surface = std::ptr::null_mut();
    }

    unsafe fn update_surface(&mut self, window: WindowRaw) {
        self.window = window;
        if !self.surface.is_null() {
            self.destroy_surface();
        }
        self.surface = (self.libegl.eglCreateWindowSurface.unwrap())(
            self.egl_display,
            self.egl_config,
            window.0 as _,
            std::ptr::null_mut(),
        );

        if self.surface.is_null() {
            let error = (self.libegl.eglGetError.unwrap())();
            hilog_fatal!(format!(
                "Failed to create EGL window surface, EGL error: {}",
                error
            ));
            return;
        }
        let res = (self.libegl.eglMakeCurrent.unwrap())(
            self.egl_display,
            self.surface,
            self.surface,
            self.egl_context,
        );

        if res == 0 {
            let error = (self.libegl.eglGetError.unwrap())();
            hilog_fatal!(format!(
                "Failed to make EGL context current, EGL error: {}",
                error
            ));
        }
        if let Some(egl_swap_interval) = self.libegl.eglSwapInterval {
            egl_swap_interval(self.egl_display, 0);
        }
    }

    fn process_message(&mut self, msg: Message) {
        match msg {
            Message::SurfaceCreated { window } => unsafe {
                self.update_surface(window);
            },
            Message::SurfaceDestroyed => unsafe {
                self.destroy_surface();
            },
            Message::SurfaceChanged {
                window,
                width,
                height,
            } => {
                unsafe {
                    self.update_surface(window);
                }

                self.display.screen_width = width as _;
                self.display.screen_height = height as _;
                self.event_handler.resize_event(
                    self.context.with_display(&mut self.display),
                    width as _,
                    height as _,
                );
            }
            Message::Touch {
                phase,
                touch_id,
                x,
                y,
                time,
            } => {
                self.event_handler.touch_event(
                    self.context.with_display(&mut self.display),
                    phase,
                    touch_id,
                    x,
                    y,
                    time as f64 / 1000.,
                );
            }

            Message::Character { character } => {
                if let Some(character) = char::from_u32(character) {
                    self.event_handler.char_event(
                        self.context.with_display(&mut self.display),
                        character,
                        Default::default(),
                        false,
                    );
                }
            }
            Message::KeyDown { keycode } => {
                self.event_handler.key_down_event(
                    self.context.with_display(&mut self.display),
                    keycode,
                    Default::default(),
                    false,
                );
            }
            Message::KeyUp { keycode } => {
                self.event_handler.key_up_event(
                    self.context.with_display(&mut self.display),
                    keycode,
                    Default::default(),
                );
            }
            Message::Pause => self
                .event_handler
                .window_minimized_event(self.context.with_display(&mut self.display)),
            Message::Resume => self
                .event_handler
                .window_restored_event(self.context.with_display(&mut self.display)),
            Message::Destroy => {
                self.quit = true;
            }
        }
    }

    fn frame(&mut self) {
        self.event_handler
            .update(self.context.with_display(&mut self.display));

        if !self.surface.is_null() {
            self.event_handler
                .draw(self.context.with_display(&mut self.display));

            unsafe {
                (self.libegl.eglSwapBuffers.unwrap())(self.egl_display, self.surface);
            }
        }
    }
}
fn register_xcomponent_callbacks(xcomponent: &XComponent) -> napi_ohos::Result<()> {
    let native_xcomponent = xcomponent.raw();
    let res = unsafe {
        OH_NativeXComponent_RegisterKeyEventCallback(native_xcomponent, Some(on_dispatch_key_event))
    };
    if res != 0 {
        hilog_error!("Failed to register key event callbacks");
    } else {
        hilog_info!("Registered key event callbacks successfully");
    }

    Ok(())
}

fn set_display_sync(xcomponent: &XComponent) -> bool {
    let native_xcomponent = xcomponent.raw();
    let mut expected_rate_range = OH_NativeXComponent_ExpectedRateRange {
        min: 110,
        max: 120,
        expected: 120,
    };
    let res = unsafe {
        OH_NativeXComponent_SetExpectedFrameRateRange(native_xcomponent, &mut expected_rate_range)
    };
    hilog_info!("Set display sync: {}", res);
    res == 0
}
pub unsafe extern "C" fn on_dispatch_key_event(
    xcomponent: *mut OH_NativeXComponent,
    window: *mut std::os::raw::c_void,
) {
    let mut event = std::ptr::null_mut();
    let ret = OH_NativeXComponent_GetKeyEvent(xcomponent, &mut event);
    assert!(ret == 0, "Get key event failed");

    let mut action = 0;
    let ret = OH_NativeXComponent_GetKeyEventAction(event, &mut action);
    assert!(ret == 0, "Get key event action failed");

    let mut code = ohos_input_sys::key_code::Input_KeyCode::KEYCODE_FN;
    let ret = OH_NativeXComponent_GetKeyEventCode(event, &mut std::mem::transmute(code));
    assert!(ret == 0, "Get key event code failed");

    let keycode = keycodes::translate_keycode(code);
    match action {
        0 => send_message(Message::KeyDown { keycode }),
        1 => send_message(Message::KeyUp { keycode }),
        _ => (),
    }
}

pub unsafe fn run<F>(conf: crate::conf::Conf, f: F)
where
    F: 'static + FnOnce(&mut crate::Context) -> Box<dyn EventHandler>,
{
    use std::panic;
    panic::set_hook(Box::new(|info| hilog_fatal!(info)));
    let env = OHOS_ENV.as_ref().expect("OHOS_ENV is not initialized");
    let exports = OHOS_EXPORTS
        .as_ref()
        .expect("OHOS_EXPORTS is not initialized");
    let xcomponent = XComponent::init(*env, *exports).expect("Failed to initialize XComponent");
    let _ = register_xcomponent_callbacks(&xcomponent);
    set_display_sync(&xcomponent);
    save_2in1_device();
    struct SendHack<F>(F);
    unsafe impl<F> Send for SendHack<F> {}
    let f = SendHack(f);

    let (tx, rx) = mpsc::channel();

    let tx2 = tx.clone();
    MESSAGES_TX.with(move |messages_tx| *messages_tx.borrow_mut() = Some(tx2));
    thread::spawn(move || {
        //set thread QoS to USER INTERACTIVE
        unsafe {
            let ret = OH_QoS_SetThreadQoS(QoS_Level_QOS_USER_INTERACTIVE);
            if ret < 0 {
                hilog_error!(format!("Failed to set thread QoS, ret: {}", ret));
            } else {
                hilog_info!("Thread QoS set to USER_INTERACTIVE");
            }
        }
        let mut libegl = LibEgl::try_load().expect("Cant load LibEGL");
        // skip all the messages until android will be able to actually open a window
        //
        // sometimes before launching an app android will show a permission dialog
        // it is important to create GL context only after a first SurfaceChanged
        let window = 'a: loop {
            match rx.try_recv() {
                Ok(Message::SurfaceCreated { window }) => {
                    break 'a window;
                }
                _ => {}
            }
        };
        let (screen_width, screen_height) = 'a: loop {
            match rx.try_recv() {
                Ok(Message::SurfaceChanged {
                    window: _,
                    width,
                    height,
                }) => {
                    break 'a (width as f32, height as f32);
                }
                _ => {}
            }
        };
        let (egl_context, egl_config, egl_display) = crate::native::egl::create_egl_context(
            &mut libegl,
            std::ptr::null_mut(), /* EGL_DEFAULT_DISPLAY */
            true,                 // force set rgba 8888 for ohos
        )
        .expect("Cant create EGL context");

        assert!(!egl_display.is_null());
        assert!(!egl_config.is_null());

        crate::native::gl::load_gl_funcs(|proc| {
            let name = std::ffi::CString::new(proc).unwrap();
            (libegl.eglGetProcAddress.unwrap())(name.as_ptr() as _)
        });

        let surface = (libegl.eglCreateWindowSurface.unwrap())(
            egl_display,
            egl_config,
            window.0 as _,
            std::ptr::null_mut(),
        );

        if (libegl.eglMakeCurrent.unwrap())(egl_display, surface, surface, egl_context) == 0 {
            panic!();
        }

        let mut context = GraphicsContext::new(gl::is_gl2());

        let mut display = OHOSDisplay {
            screen_width,
            screen_height,
            fullscreen: conf.fullscreen,
        };
        let event_handler = f.0(context.with_display(&mut display));
        let mut s = MainThreadState {
            libegl,
            egl_display,
            egl_config,
            egl_context,
            surface,
            context,
            display,
            window,
            event_handler,
            quit: false,
        };

        unsafe {
            s.update_surface(window);
            if s.surface.is_null() {
                hilog_fatal!("Failed to create initial EGL surface");
                return;
            }
        }

        while !s.quit {
            // process all the messages from the main thread
            while let Ok(msg) = rx.try_recv() {
                s.process_message(msg);
            }

            s.frame();

            thread::yield_now();
        }

        (s.libegl.eglMakeCurrent.unwrap())(
            s.egl_display,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        );
        (s.libegl.eglDestroySurface.unwrap())(s.egl_display, s.surface);
        (s.libegl.eglDestroyContext.unwrap())(s.egl_display, s.egl_context);
        (s.libegl.eglTerminate.unwrap())(s.egl_display);
    });

    xcomponent.on_surface_created(|xcomponent, win: WindowRaw| {
        unsafe {
            let mut usage: u64 = 0;
            let ret = OH_NativeWindow_NativeWindowHandleOpt(
                win.0 as _,
                NativeWindowOperation_GET_USAGE as i32,
                &mut usage as *mut u64,
            );
            if ret != 0 {
                usage &= !(OH_NativeBuffer_Usage_NATIVEBUFFER_USAGE_CPU_READ as u64);
                let ret2 = OH_NativeWindow_NativeWindowHandleOpt(
                    win.0 as _,
                    NativeWindowOperation_SET_USAGE as i32,
                    usage,
                );
                if ret2 != 0 {
                    hilog_error!("Failed to set buffer usage, ret: {}", ret2);
                }
            } else {
                hilog_error!("Failed to get buffer usage, ret: {}", ret);
            }
        }
        send_message(Message::SurfaceCreated { window: win });
        let sz = xcomponent.size(win)?;
        let width = sz.width as i32;
        let height = sz.height as i32;
        send_message(Message::SurfaceChanged {
            window: win,
            width,
            height,
        });
        Ok(())
    });

    xcomponent.on_surface_changed(|xcomponent, win| {
        let sz = xcomponent.size(win)?;
        let width = sz.width as i32;
        let height = sz.height as i32;
        send_message(Message::SurfaceChanged {
            window: win,
            width,
            height,
        });
        Ok(())
    });

    xcomponent.on_surface_destroyed(|_xcomponent, _win| {
        send_message(Message::SurfaceDestroyed);
        Ok(())
    });
    xcomponent.on_touch_event(|_xcomponent, _win, data| {
        if INTERCEPTOR.load(Ordering::Acquire) != 0 {
            return Ok(());
        }
        for i in 0..data.num_points {
            let touch_point = &data.touch_points[i as usize];
            let phase = match touch_point.event_type {
                ohos_xcomponent_binding::TouchEvent::Down => TouchPhase::Started,
                ohos_xcomponent_binding::TouchEvent::Up => TouchPhase::Ended,
                ohos_xcomponent_binding::TouchEvent::Move => TouchPhase::Moved,
                ohos_xcomponent_binding::TouchEvent::Cancel => TouchPhase::Cancelled,
                _ => TouchPhase::Cancelled, // Default to cancelled for unknown events
            };
            send_message(Message::Touch {
                phase,
                touch_id: touch_point.id as u64,
                x: touch_point.x,
                y: touch_point.y,
                time: (touch_point.timestamp / 1_000_000) as u64,
            });
        }
        Ok(())
    });
    //if !set_interceptor_state(true) {
    //    hilog_info!("Interceptor registration failed, using on_touch_event fallback.");
    //}

    let _ = xcomponent.register_callback();
    let _ = xcomponent.on_frame_callback(|_, _, _| Ok(()));
}

#[napi]
pub fn set_interceptor_state(state: bool) -> bool {
    if state {
        if INTERCEPTOR.load(Ordering::Acquire) != 0 {
            hilog_info!("Interceptor already registered");
            call_request_callback(r#"{"action":"interceptor_state","state":true}"#.to_string());
            return true;
        }
        let callback = Box::new(Input_InterceptorEventCallback {
            mouseCallback: Some(mouse_event_callback),
            touchCallback: Some(touch_event_callback),
            axisCallback: Some(axis_event_callback),
        });
        let callback = Box::into_raw(callback);
        unsafe {
            let ret = OH_Input_AddInputEventInterceptor(callback, std::ptr::null_mut());
            if ret.is_err() {
                hilog_info!("add input Event Interceptor failed , ret: {:?}", ret);
                call_request_callback(
                    r#"{"action":"interceptor_state","state":false}"#.to_string(),
                );
                return false;
            }
        }
        INTERCEPTOR.store(callback as usize, Ordering::Release);
        hilog_info!("Interceptor registered successfully");
        call_request_callback(r#"{"action":"interceptor_state","state":true}"#.to_string());
    } else {
        let prev = INTERCEPTOR.swap(0, Ordering::AcqRel);
        if prev != 0 {
            unsafe {
                let _ = OH_Input_RemoveInputEventInterceptor();
            }
            unsafe {
                Box::from_raw(prev as *mut Input_InterceptorEventCallback);
            }
            hilog_info!("Interceptor removed, falling back to on_touch_event");
        }
        call_request_callback(r#"{"action":"interceptor_state","state":false}"#.to_string());
    }
    return true;
}

#[no_mangle]
unsafe extern "C" fn touch_event_callback(touch_event: *const Input_TouchEvent) {
    if INTERCEPTOR.load(Ordering::Acquire) == 0 {
        return;
    }
    if !touch_event.is_null() {
        let action = OH_Input_GetTouchEventAction(touch_event);
        let x = OH_Input_GetTouchEventDisplayX(touch_event);
        let y = OH_Input_GetTouchEventDisplayY(touch_event);
        let finger_id = OH_Input_GetTouchEventFingerId(touch_event);
        let action_time = OH_Input_GetTouchEventActionTime(touch_event);
        let phase = match action {
            1 => TouchPhase::Started,
            2 => TouchPhase::Moved,
            3 => TouchPhase::Ended,
            _ => TouchPhase::Cancelled,
        };
        send_message(Message::Touch {
            phase,
            touch_id: finger_id as u64,
            x: x as f32,
            y: y as f32,
            time: (action_time / 1000) as u64,
        });
    }
}

unsafe extern "C" fn mouse_event_callback(mouse_event: *const Input_MouseEvent) {
    if INTERCEPTOR.load(Ordering::Acquire) == 0 {
        return;
    }
    // in fact we disable mouse support for normal devices
    // according to Huawei AGC, we must send the mouse event when the device is '2in1'
    if !IS_2IN1_DEVICE.load(Ordering::Relaxed) {
        return;
    }
    if !mouse_event.is_null() {
        let action = Input_MouseEventAction(OH_Input_GetMouseEventAction(mouse_event) as u32);
        let x = OH_Input_GetMouseEventDisplayX(mouse_event);
        let y = OH_Input_GetMouseEventDisplayY(mouse_event);
        let action_time = OH_Input_GetMouseEventActionTime(mouse_event);
        let phase = match action {
            Input_MouseEventAction::MOUSE_ACTION_BUTTON_DOWN => TouchPhase::Started,
            Input_MouseEventAction::MOUSE_ACTION_MOVE => TouchPhase::Moved,
            Input_MouseEventAction::MOUSE_ACTION_BUTTON_UP => TouchPhase::Ended,
            _ => TouchPhase::Cancelled,
        };
        send_message(Message::Touch {
            phase,
            touch_id: 0,
            x: x as f32,
            y: y as f32,
            time: (action_time / 1000) as u64,
        });
    }
}

unsafe extern "C" fn axis_event_callback(_axis_event: *const Input_AxisEvent) {}

pub fn load_file<F: Fn(crate::fs::Response) + 'static>(path: &str, on_loaded: F) {
    let response = load_file_sync(path);
    on_loaded(response);
}

fn load_file_sync(path: &str) -> crate::fs::Response {
    let full_path = format!("/data/storage/el1/bundle/entry/resources/resfile/{}", path);
    match std::fs::read(&full_path) {
        Ok(data) => Ok(data),
        Err(e) => {
            hilog_error!(format!(
                "load_file_sync: failed to load file: {} - error: {:?}",
                full_path, e
            ));
            Err(e.into())
        }
    }
}

#[napi]
pub fn register_arkts_callback(callback: Function<String, ()>) -> napi_ohos::Result<()> {
    let tsfn_builder = callback.build_threadsafe_function();
    let tsfn = tsfn_builder.max_queue_size::<1>().build()?;
    let _ = REQUEST_CALLBACK.set(tsfn);
    Ok(())
}

pub fn call_request_callback(value: String) {
    if let Some(tsfn) = REQUEST_CALLBACK.get() {
        let status = tsfn.call(value, ThreadsafeFunctionCallMode::NonBlocking);
        if !matches!(status, napi_ohos::Status::Ok) {
            hilog_error!(format!("Failed to call ArkTS callback: {:?}", status));
        }
    } else {
        hilog_error!("REQUEST_CALLBACK not initialized");
    }
}

pub fn save_2in1_device() {
    // ref:https://developer.huawei.com/consumer/cn/doc/harmonyos-references/js-apis-huksexternalcrypto
    // this Capability only shows true in 2in1 device.
    let is_2in1 = canIUse("SystemCapability.Security.Huks.CryptoExtension");
    IS_2IN1_DEVICE.store(is_2in1, Ordering::Relaxed);
}
