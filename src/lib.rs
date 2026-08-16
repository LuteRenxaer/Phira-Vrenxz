#[cfg(target_os = "android")]
use android_activity::AndroidApp;

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(_app: AndroidApp) {
    use android_logger::Config;
    use log::LevelFilter;

    android_logger::init_once(
        Config::default()
            .with_max_level(LevelFilter::Info)
            .with_tag("phirLie"),
    );

    log::info!("PhirLie starting");
    PhirLie::quad_main();
}

#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_quad_1native_QuadNative_preprocessInput(
    _: *mut std::ffi::c_void,
    _: *const std::ffi::c_void,
    #[allow(dead_code)] _motion_event: ndk_sys::AInputEvent,
    #[allow(dead_code)] _f: f32,
    #[allow(dead_code)] _f2: f32,
    #[allow(dead_code)] _z: u8,
    #[allow(dead_code)] _z2: u8,
) {
}
