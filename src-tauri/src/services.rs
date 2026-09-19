//! Spawns and supervises the two Node sidecars (news2reader = delivery,
//! crosspoint-sync = archive) as child processes. Both are plain localhost HTTP
//! servers; we inject config as env and keep handles so we can stop them.

use std::collections::HashMap;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

/// Everything needed to launch one Node service.
#[derive(Clone)]
pub struct ServiceSpec {
    pub name: String,
    /// Absolute path to the `node` runtime.
    pub node: PathBuf,
    /// Entry script, e.g. `dist/server.js`.
    pub entry: PathBuf,
    /// Working directory (the service's own package root).
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    /// Where to tee stdout+stderr (rotated by truncation on each start).
    pub log: PathBuf,
    /// Bound port (kept for a future health probe).
    #[allow(dead_code)]
    pub port: u16,
}

#[derive(Default)]
pub struct ServiceManager {
    children: Mutex<HashMap<String, Child>>,
}

impl ServiceManager {
    pub fn new() -> Self {
        ServiceManager {
            children: Mutex::new(HashMap::new()),
        }
    }

    /// Launch one service if it isn't already running. Missing runtime/entry is
    /// reported as an error rather than a panic so the UI can surface it.
    pub fn start(&self, spec: &ServiceSpec) -> Result<(), String> {
        let mut kids = self.children.lock().map_err(|e| e.to_string())?;
        if let Some(child) = kids.get_mut(&spec.name) {
            if matches!(child.try_wait(), Ok(None)) {
                return Ok(()); // already running
            }
            kids.remove(&spec.name); // exited; fall through to restart
        }

        if !spec.node.exists() {
            return Err(format!("node runtime not found at {}", spec.node.display()));
        }
        if !spec.entry.exists() {
            return Err(format!("service entry not found at {}", spec.entry.display()));
        }
        if let Some(parent) = spec.log.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let out = File::create(&spec.log).map_err(|e| e.to_string())?;
        let err = out.try_clone().map_err(|e| e.to_string())?;

        let child = Command::new(&spec.node)
            .arg(&spec.entry)
            .current_dir(&spec.cwd)
            .envs(spec.env.iter().map(|(k, v)| (k.clone(), v.clone())))
            .stdout(Stdio::from(out))
            .stderr(Stdio::from(err))
            .stdin(Stdio::null())
            .spawn()
            .map_err(|e| format!("failed to spawn {}: {}", spec.name, e))?;

        kids.insert(spec.name.clone(), child);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn stop(&self, name: &str) {
        if let Ok(mut kids) = self.children.lock() {
            if let Some(mut child) = kids.remove(name) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    pub fn stop_all(&self) {
        if let Ok(mut kids) = self.children.lock() {
            for (_, mut child) in kids.drain() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }

    /// Per-service liveness (process still running). Reaps exited children.
    pub fn status(&self, names: &[&str]) -> HashMap<String, bool> {
        let mut out = HashMap::new();
        if let Ok(mut kids) = self.children.lock() {
            for name in names {
                let running = match kids.get_mut(*name) {
                    Some(child) => matches!(child.try_wait(), Ok(None)),
                    None => false,
                };
                out.insert(name.to_string(), running);
            }
        }
        out
    }
}
