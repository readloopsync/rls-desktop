# rls-desktop

**Readloop as a menu-bar/tray app** — run the whole Readloop loop with no Docker and no Terminal. Paste your Readwise token, get an OPDS URL + QR to point your e-reader at, and (optionally) archive-on-finish. Cross-platform (macOS / Windows / Linux) via Tauri.

> Status: 🚧 scaffolding. Part of the [Readloop](https://github.com/readloopsync/readloop) project.

## What it does
- Runs the **delivery** server (Readwise → OPDS → EPUB) and the **archive** server (KOSync + Readwise archive-on-finish) as bundled background services.
- Shows the reader-facing **OPDS URL** and **KOSync URL** (+ QR codes) so setup on an e-ink keyboard is a scan, not a type.
- Auto-links your Readwise token to the archive connector (no manual credential wiring).
- Start-at-login; lives in the menu bar / system tray.

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the design and the packaging plan (the hard part).

## Running in dev

The Rust shell supervises the two Node services as child processes. In dev it
resolves them from the sibling repos via env vars (a release build bundles them
under the app resource dir instead). Build each service's `dist/` first, then:

```bash
# from rls-desktop/
export RLS_NODE=/path/to/node          # Node >=22.13 (node:sqlite + sharp)
export RLS_OPDS_DIR="$PWD/../news2reader"
export RLS_KOSYNC_DIR="$PWD/../crosspoint-sync"
npm install
npm run tauri dev
```

The app writes its config (Readwise token, generated ports/passwords, encryption
key) to the OS app-config dir and the crosspoint-sync SQLite db + service logs to
the app-data / app-log dirs — nothing lands in the repo.

## License
MIT.
