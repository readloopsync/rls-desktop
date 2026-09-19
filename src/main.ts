import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

interface AppView {
  configured: boolean;
  has_token: boolean;
  lan_ip: string;
  opds_url: string;
  kosync_url: string;
  opds_user: string;
  opds_pass: string;
  kosync_user: string;
  kosync_pass: string;
  opds_qr_svg: string;
  kosync_qr_svg: string;
  opds_running: boolean;
  kosync_running: boolean;
  x3_host: string;
  autostart_services: boolean;
}

const $ = <T extends HTMLElement = HTMLElement>(id: string) =>
  document.getElementById(id) as T;

function setText(id: string, value: string) {
  const el = document.getElementById(id);
  if (el) el.textContent = value;
}

let refreshTimer: number | undefined;

async function refresh() {
  let view: AppView;
  try {
    view = await invoke<AppView>("get_view");
  } catch (e) {
    console.error("get_view failed", e);
    return;
  }

  // Setup vs dashboard.
  $("setup").classList.toggle("hidden", view.has_token);
  $("dash").classList.toggle("hidden", !view.has_token);

  // Status dots.
  $("dot-opds").classList.toggle("on", view.opds_running);
  $("dot-kosync").classList.toggle("on", view.kosync_running);

  // Run toggle.
  const anyRunning = view.opds_running || view.kosync_running;
  setText("run-toggle", anyRunning ? "Stop" : "Start");
  $("run-toggle").dataset.running = String(anyRunning);
  setText(
    "run-label",
    anyRunning
      ? `running on ${view.lan_ip}`
      : view.configured
        ? "stopped"
        : "add a token to start",
  );

  // Delivery card.
  $("qr-opds").innerHTML = view.opds_qr_svg;
  setText("url-opds", view.opds_url);
  setText("opds-user", view.opds_user);
  setText("opds-pass", view.opds_pass);

  // Sync card.
  $("qr-kosync").innerHTML = view.kosync_qr_svg;
  setText("url-kosync", view.kosync_url);
  setText("kosync-user", view.kosync_user);
  setText("kosync-pass", view.kosync_pass);

  // X3 + autostart.
  ($("x3-host") as HTMLInputElement).value = view.x3_host;
  ($("autostart") as HTMLInputElement).checked = view.autostart_services;
}

async function saveToken() {
  const input = $("token-input") as HTMLInputElement;
  const token = input.value.trim();
  setText("setup-error", "");
  if (!token) {
    setText("setup-error", "Paste your Readwise token first.");
    return;
  }
  try {
    await invoke("set_token", { token });
    await invoke("start_services");
    input.value = "";
    await refresh();
  } catch (e) {
    setText("setup-error", String(e));
  }
}

async function toggleRun() {
  const running = $("run-toggle").dataset.running === "true";
  try {
    await invoke(running ? "stop_services" : "start_services");
  } catch (e) {
    setText("run-label", String(e));
  }
  await refresh();
}

async function mountX3() {
  const host = ($("x3-host") as HTMLInputElement).value.trim();
  setText("mount-msg", "Mounting…");
  try {
    await invoke("set_x3_host", { host });
    const point = await invoke<string>("mount_x3");
    setText("mount-msg", `Mounted at ${point}`);
  } catch (e) {
    setText("mount-msg", String(e));
  }
}

window.addEventListener("DOMContentLoaded", () => {
  $("token-save").addEventListener("click", saveToken);
  $("run-toggle").addEventListener("click", toggleRun);
  $("x3-mount").addEventListener("click", mountX3);

  $("token-link").addEventListener("click", (e) => {
    e.preventDefault();
    openUrl("https://readwise.io/access_token");
  });

  $("change-token").addEventListener("click", () => {
    $("setup").classList.remove("hidden");
    $("dash").classList.add("hidden");
  });

  $("autostart").addEventListener("change", (e) => {
    invoke("set_autostart", { on: (e.target as HTMLInputElement).checked });
  });

  $("x3-host").addEventListener("change", (e) => {
    invoke("set_x3_host", { host: (e.target as HTMLInputElement).value.trim() });
  });

  refresh();
  refreshTimer = window.setInterval(refresh, 3000);
  window.addEventListener("beforeunload", () => clearInterval(refreshTimer));
});
