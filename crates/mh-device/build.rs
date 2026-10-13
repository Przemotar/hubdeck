// macOS: compile the small Objective-C helper that re-enumerates a module (src/reenumerate_macos.m).
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/reenumerate_macos.m");
        cc::Build::new().file("src/reenumerate_macos.m").flag("-fobjc-arc").compile("mh_reenumerate");
        println!("cargo:rustc-link-lib=framework=IOUSBHost");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=IOKit");
    }
}
