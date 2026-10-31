// macOS: compile the small Objective-C helpers that re-enumerate a module (src/reenumerate_macos.m)
// and write pictures several reports at a time (src/pictures_macos.m).
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/reenumerate_macos.m");
        cc::Build::new().file("src/reenumerate_macos.m").flag("-fobjc-arc").compile("mh_reenumerate");
        // pictures several reports at a time (src/pictures.rs); without ARC, its dispatch objects
        // sit in a C struct
        println!("cargo:rerun-if-changed=src/pictures_macos.m");
        cc::Build::new().file("src/pictures_macos.m").flag("-fno-objc-arc").compile("mh_pictures");
        println!("cargo:rustc-link-lib=framework=IOUSBHost");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=IOKit");
    }
}
