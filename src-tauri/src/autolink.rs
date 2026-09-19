//! Auto-link the Readwise Reader connector so archive-on-finish works with no
//! manual wiring. Runs against the bundled crosspoint-sync over loopback:
//!   1. ensure a kosync user exists (the device uses the same account),
//!   2. PUT the connector credential (crosspoint-sync validates the token and
//!      stores it encrypted under TOKEN_ENC_KEY).
//! Everything is idempotent and best-effort — failures are logged, never fatal.

use std::thread;
use std::time::Duration;

/// Fire-and-forget: wait for crosspoint-sync to boot, then ensure the user and
/// link the connector. Safe to call on every service start (idempotent).
pub fn spawn(kosync_port: u16, user: String, pass: String, token: String) {
    thread::spawn(move || {
        let base = format!("http://127.0.0.1:{kosync_port}");
        if !wait_healthy(&base) {
            eprintln!("[readloop] autolink: crosspoint-sync did not come up; skipping");
            return;
        }
        if let Err(e) = ensure_user(&base, &user, &pass) {
            eprintln!("[readloop] autolink: ensure_user failed: {e}");
        }
        match link_connector(&base, &user, &pass, &token) {
            Ok(()) => eprintln!("[readloop] autolink: readwise-reader connector linked"),
            Err(e) => eprintln!("[readloop] autolink: link failed: {e}"),
        }
    });
}

fn wait_healthy(base: &str) -> bool {
    let url = format!("{base}/healthz");
    for _ in 0..30 {
        if ureq::get(&url).timeout(Duration::from_secs(2)).call().is_ok() {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }
    false
}

/// Stock kosync convention: x-auth-key is MD5(password).
fn md5_key(pass: &str) -> String {
    format!("{:x}", md5::compute(pass.as_bytes()))
}

fn ensure_user(base: &str, user: &str, pass: &str) -> Result<(), String> {
    let url = format!("{base}/users/create");
    let res = ureq::post(&url).send_json(serde_json::json!({
        "username": user,
        "password": pass,
    }));
    match res {
        Ok(_) => Ok(()),
        // 402 = username already registered — that's the happy path on restart.
        Err(ureq::Error::Status(402, _)) => Ok(()),
        Err(ureq::Error::Status(code, r)) => {
            Err(format!("create {code}: {}", r.into_string().unwrap_or_default()))
        }
        Err(e) => Err(e.to_string()),
    }
}

fn link_connector(base: &str, user: &str, pass: &str, token: &str) -> Result<(), String> {
    let url = format!("{base}/api/v1/connectors/readwise-reader");
    let res = ureq::put(&url)
        .set("x-auth-user", user)
        .set("x-auth-key", &md5_key(pass))
        .send_json(serde_json::json!({ "credential": { "token": token } }));
    match res {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(code, r)) => {
            Err(format!("link {code}: {}", r.into_string().unwrap_or_default()))
        }
        Err(e) => Err(e.to_string()),
    }
}
