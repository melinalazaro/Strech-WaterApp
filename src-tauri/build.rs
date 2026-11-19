fn main() {
    println!("cargo:rerun-if-changed=native/power.m");
    println!("cargo:rerun-if-changed=native/audio.m");
    println!("cargo:rerun-if-changed=assets/cling.wav");
    println!("cargo:rerun-if-changed=native/login.m");
    let mut native = cc::Build::new();
    native
        .file("native/power.m")
        .file("native/audio.m")
        .file("native/reminder.m")
        .file("native/reminder_frames.m")
        .flag("-fobjc-arc")
        .flag("-fblocks")
        .flag("-mmacosx-version-min=12.0");
    if std::env::var_os("CARGO_FEATURE_TEST_PROFILE").is_none() {
        native.file("native/login.m");
        println!("cargo:rustc-link-lib=framework=ServiceManagement");
    }
    for file in [
        "native/reminder.m",
        "native/reminder_frames.m",
        "native/reminder_frames.h",
    ] {
        println!("cargo:rerun-if-changed={file}");
    }
    for framework in ["CoreGraphics", "ImageIO", "QuartzCore"] {
        println!("cargo:rustc-link-lib=framework={framework}");
    }
    native.compile("sw_platform");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=Foundation");
    tauri_build::build();
}
