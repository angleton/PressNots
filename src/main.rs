#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("PressNots only runs on Windows.");
}

#[cfg(target_os = "windows")]
mod windows_app;

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = windows_app::run() {
        eprintln!("PressNots failed: {error}");
        std::process::exit(1);
    }
}
