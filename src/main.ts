import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

interface TsInfo {
  installed: boolean;
  running: boolean;
  dns_name: string;
  funnel_on: boolean;
  public_opds_url: string;
  public_kosync_url: string;
}

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
  opds_running: boolean;
  kosync_running: boolean;
  x3_host: string;
  autostart_services: boolean;
  launch_at_login: boolean;
  tailscale: TsInfo;
}

const $ = <T extends HTMLElement = HTMLElement>(id: string) =>
  document.getElementById(id) as T;

function setText(id: string, value: string) {
  const el = document.getElementById(id);
  if (el) el.textContent = value;
}

let toastTimer: number | undefined;
function toast(msg: string) {
  const t = $("toast");
  t.textContent = msg;
  t.classList.remove("hidden");
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => t.classList.add("hidden"), 1200);
}

async function copyText(text: string) {
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
    toast("Copied");
  } catch {
    toast("Select and copy");
  }
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

  $("setup").classList.toggle("hidden", view.has_token);
  $("dash").classList.toggle("hidden", !view.has_token);

  $("dot-opds").classList.toggle("on", view.opds_running);
  $("dot-kosync").classList.toggle("on", view.kosync_running);

  const anyRunning = view.opds_running || view.kosync_running;
  setText("run-toggle", anyRunning ? "Stop" : "Start");
  $("run-toggle").dataset.running = String(anyRunning);
  setText("run-label", anyRunning ? `Running on ${view.lan_ip}` : "Not running");

  setText("url-opds", view.opds_url);
  setText("opds-user", `user: ${view.opds_user}`);
  setText("opds-pass", `pass: ${view.opds_pass}`);
  setText("url-kosync", view.kosync_url);
  setText("kosync-user", `user: ${view.kosync_user}`);
  setText("kosync-pass", `pass: ${view.kosync_pass}`);

  renderRemote(view.tailscale);

  ($("x3-host") as HTMLInputElement).value = view.x3_host;
  ($("autostart") as HTMLInputElement).checked = view.autostart_services;
  ($("launch-login") as HTMLInputElement).checked = view.launch_at_login;
}

function renderRemote(ts: TsInfo) {
  const body = $("remote-body");

  if (!ts.installed) {
    body.innerHTML = `
      <p class="muted">Your addresses above only work on the same Wi-Fi. For access
      anywhere, Readloop can publish a secure URL with Tailscale (free).</p>
      <button class="primary" data-open="https://tailscale.com/download">Get Tailscale →</button>`;
    return;
  }
  if (!ts.running) {
    body.innerHTML = `
      <p class="muted">Tailscale is installed but not signed in. Open Tailscale,
      sign in, then reopen this panel.</p>
      <button data-open="https://login.tailscale.com/start">Sign in to Tailscale →</button>`;
    return;
  }
  if (ts.funnel_on) {
    body.innerHTML = `
      <p class="muted">Public URLs — use these on your CrossPoint device to reach Readloop from anywhere:</p>
      <div class="field" data-copy>${ts.public_opds_url}</div>
      <div class="field" data-copy>${ts.public_kosync_url}</div>
      <button id="remote-off">Turn off remote access</button>`;
    $("remote-off").addEventListener("click", async () => {
      try { await invoke("disable_remote"); toast("Remote access off"); } catch (e) { toast(String(e)); }
      refresh();
    });
    return;
  }
  body.innerHTML = `
    <p class="muted">Publish a secure public URL (via Tailscale Funnel) so CrossPoint
    works away from home — no router setup.</p>`;
  body.innerHTML += `
    <button class="primary" id="remote-on">Enable remote access</button>
    <p id="remote-msg" class="muted"></p>`;
  $("remote-on").addEventListener("click", async () => {
    setText("remote-msg", "Enabling…");
    try {
      await invoke<string>("enable_remote");
      toast("Remote access on");
    } catch (e) {
      setText("remote-msg", String(e));
      return;
    }
    refresh();
  });
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
  $("setup-guide").addEventListener("click", () => {
    openUrl("https://github.com/readloopsync/rls-desktop/blob/main/docs/SETUP.md");
  });
  $("change-token").addEventListener("click", () => {
    $("setup").classList.remove("hidden");
    $("dash").classList.add("hidden");
  });
  $("autostart").addEventListener("change", (e) => {
    invoke("set_autostart", { on: (e.target as HTMLInputElement).checked });
  });
  $("launch-login").addEventListener("change", async (e) => {
    const on = (e.target as HTMLInputElement).checked;
    try {
      await invoke("set_launch_at_login", { on });
    } catch (err) {
      toast(String(err));
      (e.target as HTMLInputElement).checked = !on; // revert on failure
    }
  });
  $("x3-host").addEventListener("change", (e) => {
    invoke("set_x3_host", { host: (e.target as HTMLInputElement).value.trim() });
  });

  // Delegated handlers: copy-on-click fields and external-link buttons.
  document.addEventListener("click", (e) => {
    const el = (e.target as HTMLElement).closest<HTMLElement>("[data-copy]");
    if (el) {
      const v = el.textContent ?? "";
      copyText(v.replace(/^(user|pass):\s*/, ""));
      return;
    }
    const link = (e.target as HTMLElement).closest<HTMLElement>("[data-open]");
    if (link) openUrl(link.dataset.open!);
  });

  refresh();
  refreshTimer = window.setInterval(refresh, 3000);
  window.addEventListener("beforeunload", () => clearInterval(refreshTimer));
});
