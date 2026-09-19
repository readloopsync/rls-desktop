//! Mount the X3's WebDAV share as an OS volume. The X3 (ESP32-C3) cannot present
//! as USB mass storage, but CrossPoint serves WebDAV over Wi-Fi, so we mount
//! that. macOS is implemented; Windows/Linux return actionable guidance.

use std::path::PathBuf;
use std::process::Command;

/// Build the WebDAV base URL for the device. `host` may be an IP or hostname;
/// `port`/`path` are configurable because CrossPoint's exact WebDAV endpoint is
/// confirmed per firmware build.
pub fn dav_url(host: &str, port: u16, path: &str) -> String {
    let path = path.trim_start_matches('/');
    if port == 80 {
        format!("http://{host}/{path}")
    } else {
        format!("http://{host}:{port}/{path}")
    }
}

/// Mount the share and return the local mount point. On success the caller can
/// reveal it in the file manager.
#[cfg(target_os = "macos")]
pub fn mount(host: &str, port: u16, path: &str) -> Result<String, String> {
    if host.trim().is_empty() {
        return Err("no X3 address set — connect the device first".into());
    }
    let url = dav_url(host, port, path);
    let mount_point = PathBuf::from(format!("/Volumes/Readloop-X3"));
    // mount_webdav needs an existing empty directory.
    let _ = std::fs::create_dir_all(&mount_point);

    let status = Command::new("mount_webdav")
        .arg("-S") // suppress interactive auth prompt (fail instead of hanging)
        .arg("-v")
        .arg("Readloop-X3")
        .arg(&url)
        .arg(&mount_point)
        .status()
        .map_err(|e| format!("mount_webdav failed to run: {e}"))?;

    if status.success() {
        Ok(mount_point.to_string_lossy().to_string())
    } else {
        Err(format!(
            "could not mount {url}. Confirm the X3 is on Wi-Fi and its WebDAV server is on."
        ))
    }
}

#[cfg(target_os = "windows")]
pub fn mount(host: &str, port: u16, path: &str) -> Result<String, String> {
    if host.trim().is_empty() {
        return Err("no X3 address set — connect the device first".into());
    }
    let url = dav_url(host, port, path).replace('/', "\\");
    let status = Command::new("net")
        .args(["use", "*", &url, "/persistent:no"])
        .status()
        .map_err(|e| format!("net use failed to run: {e}"))?;
    if status.success() {
        Ok("mapped network drive".into())
    } else {
        Err("could not map the X3 as a network drive (is the WebClient service running?)".into())
    }
}

#[cfg(target_os = "linux")]
pub fn mount(host: &str, port: u16, path: &str) -> Result<String, String> {
    if host.trim().is_empty() {
        return Err("no X3 address set — connect the device first".into());
    }
    let url = format!("dav{}", &dav_url(host, port, path)[4..]); // http -> davhttp? use gio scheme
    let dav = format!("dav://{}", dav_url(host, port, path).trim_start_matches("http://"));
    let _ = url;
    let status = Command::new("gio")
        .args(["mount", &dav])
        .status()
        .map_err(|e| format!("gio mount failed to run (install gvfs, or use davfs2): {e}"))?;
    if status.success() {
        Ok(dav)
    } else {
        Err("could not mount via gio (try davfs2)".into())
    }
}

/// Reveal a path in the platform file manager.
pub fn reveal(path: &str) {
    #[cfg(target_os = "macos")]
    let _ = Command::new("open").arg(path).status();
    #[cfg(target_os = "windows")]
    let _ = Command::new("explorer").arg(path).status();
    #[cfg(target_os = "linux")]
    let _ = Command::new("xdg-open").arg(path).status();
}
