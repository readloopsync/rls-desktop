//! Readloop desktop shell: a tray app that supervises the two Node sidecars
//! (news2reader = delivery, crosspoint-sync = archive), persists config, and
//! serves the popover UI with LAN URLs + QR codes for pairing the X3.

mod autolink;
mod config;
mod net;
mod services;
mod tailscale;
mod webdav;

use config::AppConfig;
use serde::Serialize;
use services::{ServiceManager, ServiceSpec};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, State, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;

/// Logical service names (also the sidecar subdir names in a release bundle).
const SVC_OPDS: &str = "news2reader";
const SVC_KOSYNC: &str = "crosspoint-sync";

/// Filesystem locations resolved once at startup.
struct Paths {
    node: PathBuf,
    opds_dir: PathBuf,
    kosync_dir: PathBuf,
    data_dir: PathBuf,
    log_dir: PathBuf,
}

struct AppState {
    config: Mutex<AppConfig>,
    config_dir: PathBuf,
    paths: Paths,
    services: ServiceManager,
}

/// Everything the popover UI renders in one shot.
#[derive(Serialize)]
struct AppView {
    configured: bool,
    has_token: bool,
    lan_ip: String,
    opds_url: String,
    kosync_url: String,
    opds_user: String,
    opds_pass: String,
    kosync_user: String,
    kosync_pass: String,
    opds_running: bool,
    kosync_running: bool,
    x3_host: String,
    /// Device LAN IP the sync server last saw (empty if unknown) — for the mount.
    detected_device_ip: String,
    autostart_services: bool,
    /// OS launch-at-login for the app itself.
    launch_at_login: bool,
    tailscale: tailscale::TsInfo,
}

/// Ask the local sync server for the last device IP it saw (shared-secret).
fn detect_device_ip(kosync_port: u16, secret: &str) -> String {
    if secret.is_empty() {
        return String::new();
    }
    let url = format!("http://127.0.0.1:{kosync_port}/readloop/last-client");
    match ureq::get(&url)
        .set("x-readloop-seed", secret)
        .timeout(std::time::Duration::from_millis(700))
        .call()
    {
        Ok(r) => r
            .into_json::<serde_json::Value>()
            .ok()
            .and_then(|v| v.get("ip").and_then(|s| s.as_str()).map(String::from))
            .unwrap_or_default(),
        Err(_) => String::new(),
    }
}

// ---- path resolution -------------------------------------------------------

/// Resolve runtime paths. Dev builds point at the sibling repos via env vars;
/// release builds resolve bundled sidecars under the app resource dir.
fn resolve_paths(app: &tauri::AppHandle) -> Paths {
    let resource = app.path().resource_dir().unwrap_or_else(|_| PathBuf::from("."));
    let services_root = resource.join("services");

    let node = std::env::var("RLS_NODE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| services_root.join("node/bin/node"));

    let opds_dir = std::env::var("RLS_OPDS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| services_root.join(SVC_OPDS));

    let kosync_dir = std::env::var("RLS_KOSYNC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| services_root.join(SVC_KOSYNC));

    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
    let log_dir = app.path().app_log_dir().unwrap_or_else(|_| data_dir.join("logs"));

    Paths { node, opds_dir, kosync_dir, data_dir, log_dir }
}

// ---- service specs ---------------------------------------------------------

/// If a service dir is a Yarn PnP install (dev builds against the source repos),
/// return the NODE_OPTIONS needed to run it with plain `node`. Release bundles
/// use the node-modules linker (no .pnp.cjs), so this returns nothing there.
fn pnp_env(dir: &PathBuf) -> Vec<(String, String)> {
    let pnp = dir.join(".pnp.cjs");
    let loader = dir.join(".pnp.loader.mjs");
    if pnp.exists() {
        let mut opts = format!("--require {}", pnp.to_string_lossy());
        if loader.exists() {
            opts.push_str(&format!(" --loader file://{}", loader.to_string_lossy()));
        }
        vec![("NODE_OPTIONS".into(), opts)]
    } else {
        vec![]
    }
}

fn opds_spec(state: &AppState, cfg: &AppConfig) -> ServiceSpec {
    let seed_url = format!("http://127.0.0.1:{}/readloop/seed-progress", cfg.kosync_port);
    let mut env: Vec<(String, String)> = pnp_env(&state.paths.opds_dir);
    env.extend([
        ("PORT".into(), cfg.opds_port.to_string()),
        ("READWISE_TOKEN".into(), cfg.readwise_token.clone()),
        ("OPDS_PROVIDERS".into(), "readwise".into()),
        ("OPDS_AUTH_USER".into(), cfg.opds_user.clone()),
        ("OPDS_AUTH_PASS".into(), cfg.opds_pass.clone()),
        ("READWISE_IMAGES".into(), "transcode".into()),
        ("READWISE_FEED_LIMIT".into(), "40".into()),
        ("READLOOP_SEED_URL".into(), seed_url),
        ("READLOOP_SEED_SECRET".into(), cfg.seed_secret.clone()),
        ("READLOOP_SEED_USER".into(), cfg.kosync_user.clone()),
    ]);
    ServiceSpec {
        name: SVC_OPDS.into(),
        node: state.paths.node.clone(),
        entry: state.paths.opds_dir.join("dist/server.js"),
        cwd: state.paths.opds_dir.clone(),
        log: state.paths.log_dir.join("news2reader.log"),
        port: cfg.opds_port,
        env,
    }
}

fn kosync_spec(state: &AppState, cfg: &AppConfig) -> ServiceSpec {
    let mut env: Vec<(String, String)> = pnp_env(&state.paths.kosync_dir);
    env.extend([
        ("PORT".into(), cfg.kosync_port.to_string()),
        (
            "DATABASE_PATH".into(),
            state.paths.data_dir.join("crosspoint.db").to_string_lossy().to_string(),
        ),
        ("TOKEN_ENC_KEY".into(), cfg.token_enc_key.clone()),
        ("REGISTRATION_DISABLED".into(), "0".into()),
        ("FINISHED_THRESHOLD".into(), "0.95".into()),
        ("READLOOP_SEED_SECRET".into(), cfg.seed_secret.clone()),
    ]);
    ServiceSpec {
        name: SVC_KOSYNC.into(),
        node: state.paths.node.clone(),
        entry: state.paths.kosync_dir.join("dist/index.js"),
        cwd: state.paths.kosync_dir.clone(),
        log: state.paths.log_dir.join("crosspoint-sync.log"),
        port: cfg.kosync_port,
        env,
    }
}

fn start_all(state: &AppState) -> Result<(), String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    if !cfg.is_configured() {
        return Err("Add your Readwise token first.".into());
    }
    // kosync first so its seed endpoint is up before delivery seeds positions.
    state.services.start(&kosync_spec(state, &cfg))?;
    state.services.start(&opds_spec(state, &cfg))?;
    // Once crosspoint-sync is up, link the Readwise Reader connector so
    // archive-on-finish works without the user wiring anything (idempotent).
    autolink::spawn(
        cfg.kosync_port,
        cfg.kosync_user.clone(),
        cfg.kosync_pass.clone(),
        cfg.readwise_token.clone(),
    );
    Ok(())
}

// ---- tauri commands --------------------------------------------------------

#[tauri::command]
async fn get_view(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<AppView, String> {
    // Fast, non-blocking bits on the calling thread (lock is dropped immediately).
    let cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    let launch_at_login = app.autolaunch().is_enabled().unwrap_or(false);
    let ip = net::lan_ip();
    let opds_url = format!("http://{}:{}/opds", ip, cfg.opds_port);
    let kosync_url = format!("http://{}:{}", ip, cfg.kosync_port);
    let status = state.services.status(&[SVC_OPDS, SVC_KOSYNC]);

    // Slow bits (network probe + spawning the tailscale CLI) off the main thread.
    let kport = cfg.kosync_port;
    let secret = cfg.seed_secret.clone();
    let (detected_device_ip, tsinfo) =
        tauri::async_runtime::spawn_blocking(move || {
            (detect_device_ip(kport, &secret), tailscale::info(kport))
        })
        .await
        .map_err(|e| e.to_string())?;

    Ok(AppView {
        configured: cfg.is_configured(),
        has_token: !cfg.readwise_token.trim().is_empty(),
        lan_ip: ip,
        opds_url,
        kosync_url,
        opds_user: cfg.opds_user,
        opds_pass: cfg.opds_pass,
        kosync_user: cfg.kosync_user,
        kosync_pass: cfg.kosync_pass,
        opds_running: *status.get(SVC_OPDS).unwrap_or(&false),
        kosync_running: *status.get(SVC_KOSYNC).unwrap_or(&false),
        detected_device_ip,
        x3_host: cfg.x3_host,
        autostart_services: cfg.autostart_services,
        launch_at_login,
        tailscale: tsinfo,
    })
}

#[tauri::command]
fn set_launch_at_login(app: tauri::AppHandle, on: bool) -> Result<(), String> {
    let m = app.autolaunch();
    if on {
        m.enable().map_err(|e| e.to_string())
    } else {
        m.disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn set_token(state: State<AppState>, token: String) -> Result<(), String> {
    {
        let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
        cfg.readwise_token = token.trim().to_string();
        cfg.save(&state.config_dir).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn set_x3_host(state: State<AppState>, host: String) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.x3_host = host.trim().to_string();
    cfg.save(&state.config_dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_autostart(state: State<AppState>, on: bool) -> Result<(), String> {
    let mut cfg = state.config.lock().map_err(|e| e.to_string())?;
    cfg.autostart_services = on;
    cfg.save(&state.config_dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn start_services(state: State<AppState>) -> Result<(), String> {
    start_all(&state)
}

#[tauri::command]
fn stop_services(state: State<AppState>) -> Result<(), String> {
    state.services.stop_all();
    Ok(())
}

#[tauri::command]
fn enable_remote(state: State<AppState>) -> Result<String, String> {
    let cfg = state.config.lock().map_err(|e| e.to_string())?.clone();
    tailscale::enable(cfg.opds_port, cfg.kosync_port)
}

#[tauri::command]
fn disable_remote() -> Result<(), String> {
    tailscale::disable()
}

// Both run blocking network I/O, so they execute on the blocking pool — never
// the main thread — to keep the UI responsive (no spinner/beachball).
#[tauri::command]
async fn webdav_check(host: String) -> webdav::WebdavStatus {
    tauri::async_runtime::spawn_blocking(move || webdav::check(&host))
        .await
        .unwrap_or(webdav::WebdavStatus {
            reachable: false,
            writable: false,
            error: "check failed".into(),
        })
}

#[tauri::command]
async fn send_file(host: String, path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || webdav::upload(&host, &path))
        .await
        .map_err(|e| e.to_string())?
}

/// Open (or focus) the stay-on-top "Send to CrossPoint" window.
#[tauri::command]
fn open_transfer(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    activate_app();
    if let Some(win) = app.get_webview_window("transfer") {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(&app, "transfer", tauri::WebviewUrl::App("transfer.html".into()))
        .title("Send to CrossPoint")
        .inner_size(440.0, 420.0)
        .min_inner_size(360.0, 260.0)
        .resizable(true)
        .always_on_top(true)
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// ---- app setup -------------------------------------------------------------

/// Bring the app to the foreground (make it the active app) so its window can
/// become key and receive keystrokes. Changing the activation policy alone
/// doesn't activate an accessory app — this does.
#[cfg(target_os = "macos")]
fn activate_app() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    if let Some(mtm) = MainThreadMarker::new() {
        let ns_app = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        ns_app.activateIgnoringOtherApps(true);
    }
}

/// Show and focus the popover. On macOS we become a Regular app and activate so
/// the window can become key (accessory apps can't take keyboard focus
/// otherwise), then drop back to accessory when it hides.
fn show_popover(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    activate_app();
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn hide_popover(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
}

fn toggle_popover(app: &tauri::AppHandle) {
    let visible = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if visible {
        hide_popover(app);
    } else {
        show_popover(app);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // Owned handle so it doesn't hold a borrow across set_activation_policy.
            let handle = app.handle().clone();

            // NOTE: temporarily running as a Regular app (Dock icon visible) to
            // confirm keyboard input works; accessory/no-Dock is restored via
            // NSPanel once verified.

            let paths = resolve_paths(&handle);
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            let cfg = AppConfig::load(&config_dir);
            let autostart = cfg.autostart_services && cfg.is_configured();

            let state = AppState {
                config: Mutex::new(cfg),
                config_dir,
                paths,
                services: ServiceManager::new(),
            };

            if autostart {
                if let Err(e) = start_all(&state) {
                    eprintln!("[readloop] autostart skipped: {e}");
                }
            }
            app.manage(state);

            // Tray menu.
            let open_i = MenuItem::with_id(app, "open", "Open Readloop", true, None::<&str>)?;
            let start_i = MenuItem::with_id(app, "start", "Start services", true, None::<&str>)?;
            let stop_i = MenuItem::with_id(app, "stop", "Stop services", true, None::<&str>)?;
            let sep = PredefinedMenuItem::separator(app)?;
            let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_i, &sep, &start_i, &stop_i, &sep, &quit_i])?;

            // Menu-bar icon: the wordmark's infinity mark, as a template image so
            // macOS tints it to match the menu bar (dark/light) automatically.
            let tray_icon =
                tauri::image::Image::from_bytes(include_bytes!("../icons/tray@2x.png"))?;
            TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .icon_as_template(true)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_popover(app),
                    "start" => {
                        if let Some(state) = app.try_state::<AppState>() {
                            if let Err(e) = start_all(&state) {
                                eprintln!("[readloop] start failed: {e}");
                            }
                        }
                    }
                    "stop" => {
                        if let Some(state) = app.try_state::<AppState>() {
                            state.services.stop_all();
                        }
                    }
                    "quit" => {
                        if let Some(state) = app.try_state::<AppState>() {
                            state.services.stop_all();
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            use tauri_plugin_positioner::{Position, WindowExt};
                            let _ = win.move_window(Position::TrayCenter);
                        }
                        toggle_popover(app);
                    }
                })
                .build(app)?;

            // Hide the popover when it loses focus (menu-bar app feel).
            if let Some(win) = app.get_webview_window("main") {
                let w = win.clone();
                win.on_window_event(move |ev| {
                    if let WindowEvent::Focused(false) = ev {
                        hide_popover(w.app_handle());
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_view,
            set_token,
            set_x3_host,
            set_autostart,
            set_launch_at_login,
            start_services,
            stop_services,
            enable_remote,
            disable_remote,
            webdav_check,
            send_file,
            open_transfer,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Readloop")
        .run(|app, event| {
            // Keep running when the popover is dismissed; only Quit exits.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(state) = app.try_state::<AppState>() {
                    state.services.stop_all();
                }
            }
        });
}
