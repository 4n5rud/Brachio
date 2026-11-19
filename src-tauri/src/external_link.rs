//! Opens an address from the interface in the system browser.

/// Characters a link may contain. Anything a shell treats specially (`&`, `|`, `^`, quotes,
/// spaces, backticks) is left out, so an address can never be read as more than one argument.
fn is_safe(url: &str) -> bool {
    url.len() <= 2_048
        && url.starts_with("https://")
        && url
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "/:._-?=#%~+".contains(character))
}

/// Opens an `https` address in the default browser. Used for links the interface shows, such as
/// a pull request or the log of a failed check.
#[tauri::command]
pub async fn open_external_link(url: String) -> Result<(), String> {
    if !is_safe(&url) {
        return Err("Only plain https addresses can be opened.".to_owned());
    }
    open_in_browser(url).await
}

/// Hands an address the app already trusts to the system browser, as a single argument.
pub(crate) async fn open_in_browser(url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(windows)]
        let mut command = {
            use std::os::windows::process::CommandExt;

            // The file protocol handler takes the address as one argument; `cmd /c start` would
            // read it as a command line.
            let mut command = std::process::Command::new("rundll32");
            command
                .creation_flags(0x0800_0000)
                .arg("url.dll,FileProtocolHandler");
            command
        };
        #[cfg(target_os = "macos")]
        let mut command = std::process::Command::new("open");
        #[cfg(not(any(windows, target_os = "macos")))]
        let mut command = std::process::Command::new("xdg-open");
        let status = command
            .arg(&url)
            .status()
            .map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Could not open the browser ({status})."))
        }
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::is_safe;

    #[test]
    fn plain_https_addresses_pass_and_anything_a_shell_would_read_is_refused() {
        assert!(is_safe(
            "https://github.com/4n5rud/Brachio/actions/runs/44337487778/job/1"
        ));
        assert!(is_safe("https://github.com/o/r/pull/7?x=1#issuecomment-2"));
        for refused in [
            "http://github.com/o/r",
            "file:///C:/Windows/System32/calc.exe",
            "https://github.com/o/r&calc",
            "https://github.com/o/r | calc",
            "https://github.com/o/r\"",
            "https://github.com/o/r^",
            "javascript:alert(1)",
            "",
        ] {
            assert!(!is_safe(refused), "{refused}");
        }
    }
}
