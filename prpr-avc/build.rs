use std::path::Path;

fn main() {
    let libs_dir = std::env::var("PRPR_AVC_LIBS").unwrap_or_else(|_| format!("{}/static-lib", std::env::var("CARGO_MANIFEST_DIR").unwrap()));
    let libs_path = Path::new(&libs_dir).join(std::env::var("TARGET").unwrap());
    let libs_path = libs_path.display();
    println!("cargo:rustc-link-search={libs_path}");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rustc-link-lib=bcrypt");
        println!("cargo:rustc-link-lib=ole32");
        println!("cargo:rustc-link-lib=user32");
    }

    // The bundled FFmpeg builds are configured with `--disable-autodetect`
    // and `--disable-videotoolbox`, so they only need the C runtime and math
    // library.  Android also needs libatomic for the AArch64 atomics emitted
    // by the NEON code paths.
    match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        Ok("android") => {
            println!("cargo:rustc-link-lib=m");
            println!("cargo:rustc-link-lib=atomic");
        }
        Ok("linux") | Ok("ohos") => {
            println!("cargo:rustc-link-lib=m");
        }
        _ => {}
    }

    println!("cargo:rerun-if-changed={libs_path}");
}
