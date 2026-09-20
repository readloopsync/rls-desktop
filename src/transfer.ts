import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";

interface WebdavStatus {
  reachable: boolean;
  writable: boolean;
  error: string;
}

const $ = <T extends HTMLElement = HTMLElement>(id: string) =>
  document.getElementById(id) as T;

let connected = false;
let checking = false;

function host(): string {
  return ($("host") as HTMLInputElement).value.trim();
}

/** Resize the window to fit its content (height only; width stays fixed). */
async function fitWindow() {
  // Wait two frames so freshly-inserted rows are laid out before measuring.
  await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(null))));
  const content = document.getElementById("tapp")!.getBoundingClientRect().height;
  const h = Math.min(Math.max(Math.ceil(content) + 10, 260), 760);
  try {
    await getCurrentWindow().setSize(new LogicalSize(440, h));
  } catch {
    /* ignore */
  }
}

function setStatus(state: "checking" | "ok" | "bad", text: string) {
  $("status").dataset.state = state;
  $("status-text").textContent = text;
  $("instructions").classList.toggle("hidden", state !== "bad");
  $("drop").classList.toggle("disabled", state !== "ok");
}

let lastOk: boolean | null = null;

/** Apply status; only resize the window when the connected state flips (so
 * background polls don't cause jitter). */
function update(state: "ok" | "bad", text: string) {
  setStatus(state, text);
  const ok = state === "ok";
  if (ok !== lastOk) {
    lastOk = ok;
    fitWindow();
  }
}

/** `silent` background polls skip the "Checking…" flash. */
async function check(silent = false) {
  if (checking) return;
  const h = host();
  if (!h) {
    connected = false;
    update("bad", "Enter the device's address");
    return;
  }
  checking = true;
  if (!silent) setStatus("checking", "Checking…");
  try {
    const s = await invoke<WebdavStatus>("webdav_check", { host: h });
    connected = s.writable;
    update(s.writable ? "ok" : "bad", s.writable ? `Connected to ${h}` : (s.error || "Not reachable"));
  } catch (e) {
    connected = false;
    update("bad", String(e));
  } finally {
    checking = false;
  }
}

function addRow(name: string): HTMLElement {
  $("uploads-wrap").classList.remove("hidden");
  const row = document.createElement("div");
  row.className = "urow";
  row.innerHTML = `<span class="uname"></span><span class="ustate">uploading…</span>`;
  (row.querySelector(".uname") as HTMLElement).textContent = name;
  $("uploads").prepend(row);
  return row;
}

function baseName(p: string): string {
  return p.split(/[\\/]/).pop() || p;
}

async function uploadPaths(paths: string[]) {
  if (!connected) {
    await check();
    if (!connected) return;
  }
  for (const path of paths) {
    const row = addRow(baseName(path));
    const state = row.querySelector(".ustate") as HTMLElement;
    await fitWindow();
    try {
      await invoke("send_file", { host: host(), path });
      state.textContent = "sent ✓";
      state.className = "ustate ok";
    } catch (e) {
      state.textContent = String(e);
      state.className = "ustate bad";
    }
  }
  await fitWindow();
}

window.addEventListener("DOMContentLoaded", async () => {
  // Seed the address from the app's saved host / auto-detected device IP.
  try {
    const view = await invoke<{ x3_host: string; detected_device_ip: string }>("get_view");
    ($("host") as HTMLInputElement).value = view.x3_host || view.detected_device_ip || "";
  } catch {
    /* ignore */
  }

  $("host").addEventListener("change", (e) => {
    const h = (e.target as HTMLInputElement).value.trim();
    invoke("set_x3_host", { host: h }).catch(() => {});
    check();
  });

  $("recheck").addEventListener("click", () => check());

  $("find-ip").addEventListener("click", (e) => {
    e.preventDefault();
    openUrl("https://github.com/readloopsync/rls-desktop/blob/main/docs/FIND-IP.md");
  });

  $("choose").addEventListener("click", async () => {
    const sel = await openDialog({ multiple: true });
    if (!sel) return;
    await uploadPaths(Array.isArray(sel) ? sel : [sel]);
  });

  // Native drag-and-drop of files onto the window (Tauri gives real paths).
  await getCurrentWebview().onDragDropEvent((event) => {
    const p = event.payload;
    if (p.type === "over" || p.type === "enter") {
      $("drop").classList.add("hover");
    } else if (p.type === "leave") {
      $("drop").classList.remove("hover");
    } else if (p.type === "drop") {
      $("drop").classList.remove("hover");
      if (p.paths && p.paths.length) uploadPaths(p.paths);
    }
  });

  await check();

  // Poll every 4s while the window is open so it notices File Transfer being
  // turned on OR off (silent = no "Checking…" flash).
  window.setInterval(() => {
    if (!checking) check(true);
  }, 4000);
});
