fn main() {
    // macOS: the small Objective-C helper for sensors and monitor brightness (src/macos/hw.m)
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/macos/hw.m");
        println!("cargo:rerun-if-changed=src/macos/appvolume.m");
        println!("cargo:rerun-if-changed=src/macos/listen.m");
        // built for macOS 12 like the app: what is newer (process taps, 14.2) is weak-linked and
        // checked with @available at run time
        cc::Build::new().file("src/macos/hw.m").file("src/macos/appvolume.m").file("src/macos/listen.m").flag("-fobjc-arc").flag("-mmacosx-version-min=12.0").compile("hubdeck_mac_hw");
        for framework in ["IOKit", "CoreFoundation", "CoreGraphics", "CoreAudio", "AppKit", "Foundation"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        // @available calls __isPlatformVersionAtLeast from clang's own runtime, which rustc does not
        // link: a release build failed with an undefined symbol (session 9). Link it from the clang
        // that built the helper.
        let compiler = cc::Build::new().get_compiler();
        if let Ok(out) = std::process::Command::new(compiler.path()).arg("--print-runtime-dir").output() {
            let dir = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if std::path::Path::new(&dir).join("libclang_rt.osx.a").exists() {
                println!("cargo:rustc-link-search=native={dir}");
                println!("cargo:rustc-link-lib=static=clang_rt.osx");
            }
        }
    }
    // the icons go into the .exe here: tauri_build does not ask to run again when they change, so an
    // incremental build kept the old icon on the desktop and in Explorer (session 40, after the F3 icon)
    for icon in ["icons/icon.ico", "icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns"] {
        println!("cargo:rerun-if-changed={icon}");
    }
    tauri_build::build()
}
