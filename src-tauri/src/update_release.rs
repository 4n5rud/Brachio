//! Opens the project's published update in the system browser.

#[tauri::command]
pub async fn open_update_release() -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        let mut command = {
            use std::os::windows::process::CommandExt;

            let mut command = std::process::Command::new("cmd.exe");
            command
                .creation_flags(0x0800_0000)
                .args(["/d", "/c", "start", ""]);
            command
        };
        #[cfg(target_os = "macos")]
        let mut command = std::process::Command::new("open");
        #[cfg(not(any(windows, target_os = "macos")))]
        let mut command = std::process::Command::new("xdg-open");
        let status = command
            .arg("https://github.com/4n5rud/Brachio/releases/latest")
            .status()
            .map_err(|error| error.to_string())?;
        if !status.success() {
            return Err(format!("Could not open the release page ({status})."));
        }
        Ok(())
    })
    .await
    .map_err(|error| error.to_string())?
}
