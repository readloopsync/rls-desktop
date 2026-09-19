//! Persisted app configuration. Stored as JSON in the OS app-config dir and
//! injected as environment into the two Node sidecars at spawn time.

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// User-facing + generated settings. Secrets (Readwise token, enc key, kosync
/// password) live here too — the file sits in the per-user app-config dir.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Readwise Reader API token (from readwise.io/access_token).
    #[serde(default)]
    pub readwise_token: String,

    /// Ports for the two local services.
    #[serde(default = "default_opds_port")]
    pub opds_port: u16,
    #[serde(default = "default_kosync_port")]
    pub kosync_port: u16,

    /// Basic-auth guarding the OPDS feed (device enters these).
    #[serde(default = "default_user")]
    pub opds_user: String,
    #[serde(default = "gen_password")]
    pub opds_pass: String,

    /// KOSync account the device signs up / logs in with.
    #[serde(default = "default_user")]
    pub kosync_user: String,
    #[serde(default = "gen_password")]
    pub kosync_pass: String,

    /// Encrypts crosspoint-sync's stored connector credentials at rest.
    #[serde(default = "gen_enc_key")]
    pub token_enc_key: String,

    /// Shared secret the OPDS server uses to pre-seed reading positions into
    /// crosspoint-sync at download time (READLOOP_SEED_SECRET on both).
    #[serde(default = "gen_password")]
    pub seed_secret: String,

    /// Last known LAN address of the X3 (for OPDS reach + WebDAV mount).
    #[serde(default)]
    pub x3_host: String,

    /// Start the services when the app launches.
    #[serde(default = "default_true")]
    pub autostart_services: bool,
}

fn default_opds_port() -> u16 {
    8080
}
fn default_kosync_port() -> u16 {
    7200
}
fn default_user() -> String {
    "readloop".into()
}
fn default_true() -> bool {
    true
}

/// URL-safe random string, no ambiguous chars, good enough for a LAN password.
fn gen_password() -> String {
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
    let mut rng = rand::thread_rng();
    (0..16).map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char).collect()
}

/// 64 hex chars = 32 bytes, the form crosspoint-sync's TOKEN_ENC_KEY expects.
fn gen_enc_key() -> String {
    let mut rng = rand::thread_rng();
    (0..64).map(|_| format!("{:x}", rng.gen_range(0..16u8))).collect()
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            readwise_token: String::new(),
            opds_port: default_opds_port(),
            kosync_port: default_kosync_port(),
            opds_user: default_user(),
            opds_pass: gen_password(),
            kosync_user: default_user(),
            kosync_pass: gen_password(),
            token_enc_key: gen_enc_key(),
            seed_secret: gen_password(),
            x3_host: String::new(),
            autostart_services: true,
        }
    }
}

impl AppConfig {
    fn path(dir: &PathBuf) -> PathBuf {
        dir.join("config.json")
    }

    /// Load from the config dir, creating (and persisting) defaults on first run
    /// or if the file is missing/corrupt.
    pub fn load(config_dir: &PathBuf) -> AppConfig {
        let path = Self::path(config_dir);
        match fs::read_to_string(&path) {
            Ok(s) => match serde_json::from_str::<AppConfig>(&s) {
                Ok(cfg) => cfg,
                Err(_) => {
                    let cfg = AppConfig::default();
                    let _ = cfg.save(config_dir);
                    cfg
                }
            },
            Err(_) => {
                let cfg = AppConfig::default();
                let _ = cfg.save(config_dir);
                cfg
            }
        }
    }

    pub fn save(&self, config_dir: &PathBuf) -> std::io::Result<()> {
        fs::create_dir_all(config_dir)?;
        let json = serde_json::to_string_pretty(self).unwrap_or_default();
        fs::write(Self::path(config_dir), json)
    }

    /// True once the user has entered a Readwise token — the gate for starting
    /// delivery.
    pub fn is_configured(&self) -> bool {
        !self.readwise_token.trim().is_empty()
    }
}
