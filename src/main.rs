#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    // TODO(plan step 6): call `meh::win::app::run()` once the Win32 layer lands.
}

#[cfg(not(windows))]
fn main() {
    eprintln!("meh is Windows-only");
    std::process::exit(1);
}
