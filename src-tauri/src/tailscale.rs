//! Optional remote access via Tailscale Funnel. The X3 can't run a VPN, but
//! Funnel gives a stable public HTTPS URL that proxies to the local services —
//! no router config, works through CGNAT. We detect Tailscale; if it's missing
//! the UI links out to install it. We use a dedicated funnel port so we never
//! disturb any funnel the user already runs on another port.

use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;

/// Funnel is only allowed on 443, 8443, or 10000. We take 443 (URLs then need
/// no port suffix); a user's other funnels typically live on 10000.
pub const FUNNEL_PORT: u16 = 443;

#[derive(Serialize, Default, Clone)]
pub struct TsInfo {
    /// The `tailscale` binary is present.
    pub installed: bool,
    /// Backend is Running (logged in + connected).
    pub running: bool,
    /// This node's MagicDNS name (no trailing dot), e.g. "box.tailXXduc.ts.net".
    pub dns_name: String,
    /// Our funnel paths are currently live.
    pub funnel_on: bool,
    /// Public URLs when funnel is on (empty otherwise).
    pub public_opds_url: String,
    pub public_kosync_url: String,
}

fn binary() -> Option<PathBuf> {
    let candidates = [
        "/opt/homebrew/bin/tailscale",
        "/usr/local/bin/tailscale",
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
        "/usr/bin/tailscale",
        "C:\\Program Files\\Tailscale\\tailscale.exe",
    ];
    for c in candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    // Fall back to PATH.
    if let Ok(out) = Command::new("which").arg("tailscale").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return Some(PathBuf::from(s));
            }
        }
    }
    None
}

/// Public base URL (443 omits the port; other ports include it).
fn base(dns: &str) -> String {
    if FUNNEL_PORT == 443 {
        format!("https://{dns}")
    } else {
        format!("https://{dns}:{FUNNEL_PORT}")
    }
}

/// Current Tailscale state. `kosync_port` is used to recognise *our* funnel
/// mapping in the funnel-status output without matching the user's other ones.
pub fn info(kosync_port: u16) -> TsInfo {
    let mut i = TsInfo::default();
    let bin = match binary() {
        Some(b) => b,
        None => return i,
    };
    i.installed = true;

    if let Ok(out) = Command::new(&bin).args(["status", "--json"]).output() {
        if out.status.success() {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                i.running = v.get("BackendState").and_then(|s| s.as_str()) == Some("Running");
                if let Some(dns) = v
                    .get("Self")
                    .and_then(|s| s.get("DNSName"))
                    .and_then(|s| s.as_str())
                {
                    i.dns_name = dns.trim_end_matches('.').to_string();
                }
            }
        }
    }

    // Recognise our funnel by the kosync target in the funnel-status text.
    if let Ok(out) = Command::new(&bin).args(["funnel", "status"]).output() {
        let txt = String::from_utf8_lossy(&out.stdout);
        i.funnel_on = txt.contains(&format!("127.0.0.1:{kosync_port}"));
    }

    if i.funnel_on && !i.dns_name.is_empty() {
        i.public_opds_url = format!("{}/opds", base(&i.dns_name));
        i.public_kosync_url = format!("{}/sync", base(&i.dns_name));
    }
    i
}

fn run(bin: &PathBuf, args: &[&str]) -> Result<(), String> {
    let out = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Turn on Funnel: mount the delivery service at the root of our port and the
/// sync service at `/sync` (mirrors the proven two-path layout). Returns the
/// public OPDS URL.
pub fn enable(opds_port: u16, kosync_port: u16) -> Result<String, String> {
    let bin = binary().ok_or("Tailscale is not installed")?;
    let port = format!("--https={FUNNEL_PORT}");
    // Delivery at root; sync under /sync. --bg keeps it running after we return.
    run(&bin, &["funnel", "--bg", &port, &format!("http://127.0.0.1:{opds_port}")])?;
    run(&bin, &["funnel", "--bg", &port, "--set-path=/sync", &format!("http://127.0.0.1:{kosync_port}")])?;

    let i = info(kosync_port);
    if i.dns_name.is_empty() {
        return Err("Tailscale is not logged in".into());
    }
    Ok(format!("{}/opds", base(&i.dns_name)))
}

/// Turn off *our* funnel port only, leaving any other serve/funnel config
/// (e.g. a self-host funnel on another port) untouched.
pub fn disable() -> Result<(), String> {
    let bin = binary().ok_or("Tailscale is not installed")?;
    let port = format!("--https={FUNNEL_PORT}");
    let _ = run(&bin, &["funnel", &port, "off"]);
    let _ = run(&bin, &["serve", &port, "off"]);
    Ok(())
}
