//! Embeds the exe icon and version info on Windows builds.
//!
//! `winresource` is a build-dependency only on Windows hosts, so the
//! resource step is compiled only there, and runs only when the target is
//! Windows too. Elsewhere this script does nothing.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/meh.ico");
    #[cfg(windows)]
    windows_resources();
}

#[cfg(windows)]
fn windows_resources() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // FileVersion and ProductVersion default to CARGO_PKG_VERSION.
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/meh.ico")
        .set("ProductName", "Meh")
        .set("FileDescription", "Meh text expander")
        .set("InternalName", "meh")
        .set("OriginalFilename", "meh.exe")
        .set(
            "LegalCopyright",
            "Copyright (c) 2026 Moaaz Mahmoud. MIT License.",
        );
    if let Err(e) = res.compile() {
        panic!("embedding Windows resources failed: {e}");
    }
}
