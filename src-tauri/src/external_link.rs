//! Opens an address from the interface in the system browser.

/// Characters a link may contain. The address is handed to the browser launcher as one argument,
/// so `&` and friends are safe; `|`, `^`, quotes, spaces, backticks and angle brackets stay out.
/// The terminal link pattern in the interface uses the same set.
fn is_safe(url: &str) -> bool {
    url.len() <= 2_048
        && (url.starts_with("https://") || url.starts_with("http://"))
        && url.chars().all(|character| {
            character.is_ascii_alphanumeric() || "/:._-?=#%~+&@,;!()*[]$".contains(character)
        })
}

/// Opens an `https` address in the default browser. Used for links the interface shows, such as
/// a pull request or the log of a failed check.
#[tauri::command]
pub async fn open_external_link(url: String) -> Result<(), String> {
    if !is_safe(&url) {
        return Err("Only plain http and https addresses can be opened.".to_owned());
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
        assert!(is_safe(
            "https://example.com/oauth/authorize?response_type=code&client_id=a"
        ));
        assert!(is_safe("http://example.com"));
        for refused in [
            "ftp://github.com/o/r",
            "file:///C:/Windows/System32/calc.exe",
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
