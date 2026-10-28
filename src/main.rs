#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    meh::win::app::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("meh is Windows-only");
    std::process::exit(1);
}
