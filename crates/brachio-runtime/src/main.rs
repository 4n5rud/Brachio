//! Brachio terminal runtime and its local client.

#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    windows::run()
}

#[cfg(not(windows))]
fn main() -> std::process::ExitCode {
    eprintln!("The independent terminal runtime currently supports Windows.");
    std::process::ExitCode::FAILURE
}
