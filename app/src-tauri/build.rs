fn main() {
    // macOS: the small Objective-C helper for sensors and monitor brightness (src/macos/hw.m)
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/macos/hw.m");
        println!("cargo:rerun-if-changed=src/macos/appvolume.m");
        // built for macOS 12 like the app: what is newer (process taps, 14.2) is weak-linked and
        // checked with @available at run time
        cc::Build::new().file("src/macos/hw.m").file("src/macos/appvolume.m").flag("-fobjc-arc").flag("-mmacosx-version-min=12.0").compile("hubdeck_mac_hw");
        for framework in ["IOKit", "CoreFoundation", "CoreGraphics", "CoreAudio", "AppKit", "Foundation"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    tauri_build::build()
}
