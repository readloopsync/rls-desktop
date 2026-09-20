//! Send files to the CrossPoint device over WebDAV. The device's WebDAV server
//! (up while "File Transfer" is active on the device) accepts real writes via
//! PUT, even though its LOCK support is faked — so native OS mounts come up
//! read-only. We therefore upload with PUT directly, which is reliable and
//! identical on macOS/Windows/Linux.

use serde::Serialize;
use std::path::Path;
use std::time::Duration;

fn base(host: &str) -> String {
    let host = host.trim().trim_end_matches('/');
    // Accept a bare host/IP or a full URL.
    if host.starts_with("http://") || host.starts_with("https://") {
        host.to_string()
    } else {
        format!("http://{host}")
    }
}

#[derive(Serialize)]
pub struct WebdavStatus {
    /// The device answered at all.
    pub reachable: bool,
    /// It's a WebDAV server that accepts uploads (PUT).
    pub writable: bool,
    pub error: String,
}

/// Probe the device: is its WebDAV server up and accepting writes? Used when the
/// transfer window opens so we can guide the user into File Transfer mode.
pub fn check(host: &str) -> WebdavStatus {
    let mut s = WebdavStatus { reachable: false, writable: false, error: String::new() };
    if host.trim().is_empty() {
        s.error = "Enter the device's address".into();
        return s;
    }
    let url = format!("{}/", base(host));
    match ureq::request("OPTIONS", &url).timeout(Duration::from_secs(4)).call() {
        Ok(resp) => {
            s.reachable = true;
            let allow = resp.header("Allow").unwrap_or("").to_uppercase();
            let dav = resp.header("DAV").unwrap_or("");
            s.writable = allow.contains("PUT") || !dav.is_empty();
            if !s.writable {
                s.error = "Reachable, but not a WebDAV server".into();
            }
        }
        // A status error still means something answered on that port.
        Err(ureq::Error::Status(code, _)) => {
            s.reachable = true;
            s.error = format!("HTTP {code}");
        }
        Err(_) => {
            s.error = "No response — is File Transfer on and on the same Wi-Fi?".into();
        }
    }
    s
}

/// PUT one local file to the device's WebDAV root, keyed by its filename.
pub fn upload(host: &str, file_path: &str) -> Result<(), String> {
    let p = Path::new(file_path);
    let name = p
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("bad filename")?;
    let url = format!("{}/{}", base(host), urlencoding::encode(name));
    let bytes = std::fs::read(p).map_err(|e| format!("read {name}: {e}"))?;
    match ureq::put(&url)
        .timeout(Duration::from_secs(180))
        .send_bytes(&bytes)
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(code, _)) => Err(format!("device rejected upload (HTTP {code})")),
        Err(e) => Err(e.to_string()),
    }
}
