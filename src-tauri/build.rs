//! Generates the Tauri context (config, icons, permissions) at build time.

fn main() {
    tauri_build::build();
    // Unit-test executables carry no manifest, and Tauri's menu code needs Common Controls v6 to
    // load. The app executable already embeds one, so it opts out of this one.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
        println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
    }
}
