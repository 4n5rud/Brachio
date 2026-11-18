//! Brachio terminal runtime and its local client.

#[cfg(any(windows, test))]
mod endpoint;
#[cfg(any(windows, test))]
mod output;
#[cfg(any(windows, test))]
mod protocol;
#[cfg(any(windows, test))]
mod screen;
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
