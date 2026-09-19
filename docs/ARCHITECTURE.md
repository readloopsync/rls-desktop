# rls-desktop — architecture & packaging plan

Goal: a **cross-platform menu-bar/tray app** that runs Readloop's full loop (delivery + archive) for non-technical users — no Docker, no Terminal. macOS / Windows / Linux.

## Shell
- **Tauri v2**, menu-bar/tray pattern (scaffold from `ahkohd/tauri-macos-menubar-app-example`, `v2-popover`). Native tray + a small WebView popover for the UI. Rust shell manages the child processes, tray, autostart.

## The two bundled services (spawned as sidecars/children)
1. **news2reader** (delivery: Readwise → OPDS → EPUB). Needs a **Node runtime** and **`sharp`** (native module, for webp→JPEG transcode). Ships with our epub-gen TOC patch.
2. **crosspoint-sync** (archive: KOSync server + our `readwise-reader` connector). Needs **Node ≥22.13** (uses built-in `node:sqlite`).

Both are plain HTTP servers on localhost; the app spawns them, injects config via env, and monitors them.

## Packaging (the crux)
We **cannot** ship a single compiled binary:
- `sharp` is a native `.node` — per-OS/arch prebuilt binaries.
- `node:sqlite` requires a real **Node ≥22.13** runtime (Bun/pkg won't provide it).

So each platform bundle contains:
- a pinned **Node ≥22.13** runtime for that OS/arch,
- each service's **built `dist/` + `node_modules`** installed with the **node-modules linker** (not Yarn PnP) and the **platform-correct `sharp`** binary,
- launched as `node dist/…`.

Tauri ships these as **resources / `externalBin` sidecars**; the Rust side spawns `node` against the bundled app dir. Cross-platform means producing the right `sharp` per target (sharp's `@img/sharp-<platform>` packages / `--cpu`/`--os` install), and the matching Node runtime per target.

**De-risk order (Rust-independent, do first):** prove a standalone bundle of each service (bundled Node + node_modules + native deps) runs on macOS, then replicate per platform.

✅ **Validated on macOS (2026-09-19):** news2reader built with `nodeLinker: node-modules` (keeping the epub-gen patch) runs as a bare `node dist/server.js` — no Yarn/PnP — serving OPDS and building a **transcoded grayscale JPEG** (so `sharp`'s native binary resolves from `node_modules/@img/sharp-darwin-arm64`). crosspoint-sync already runs as plain `node dist/index.js` (npm + built-in `node:sqlite`). So the sidecar model is confirmed; per-platform work is just fetching the right `sharp` + Node runtime per target (CI).

## Config / state (managed by the app)
- Readwise **token**, service **ports**, **TOKEN_ENC_KEY** (for crosspoint-sync credential encryption) — stored in the OS app-data dir; passed as env to the sidecars.
- On first run / token entry: **auto-link** the `readwise-reader` connector (write the encrypted credential via crosspoint-sync's own code) so the user never does manual wiring.
- Services run with our validated defaults: `READWISE_IMAGES=transcode`, `OPDS_PROVIDERS=readwise`, `FINISHED_THRESHOLD≈0.95`, `REGISTRATION_DISABLED=1` (single local user).

## Networking scope
- **LAN-first** (device + computer on the same Wi-Fi). The app shows the machine's **LAN IP** OPDS/KOSync URLs + QR codes.
- Remote access (Tailscale Funnel etc.) is **out of scope** for the app — that's a homelab/self-host concern, handled by the Docker/launchd path in the main repo.

## UI (popover)
- Readwise token field (masked) + "connected" state.
- **OPDS URL** + QR (delivery) and **KOSync URL** + QR (archive), with the username/password to enter on the device.
- Start/stop toggle + per-service status (green/red).
- Start-at-login toggle.

## Build & distribution
- **CI (GitHub Actions + `tauri-apps/tauri-action`)** matrix: macOS (arm64/x64), Windows (x64), Linux (x64). Local dev builds macOS only.
- Signing/notarization deferred: document the unsigned "run anyway" path first; **Homebrew Cask** (mac), **winget/MSI** (win), **AppImage/deb** (linux). Developer ID / Authenticode later if it gets traction.

## Open questions
- Does bundling **two** Node runtimes bloat the app unacceptably? (Could share one Node runtime between both services — likely yes, one runtime, two `node` invocations.)
- KOSync user provisioning: the device registers the kosync user on first sync (Sign Up) — does the app need to pre-provision, or is the device-side Sign Up fine? (Fine, but the app should show clear steps.)
- Windows/Linux `node:sqlite` + `sharp` bundling parity (test in CI).
