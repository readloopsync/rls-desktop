# rls-desktop

**Readloop as a menu-bar/tray app** — run the whole Readloop loop with no Docker and no Terminal. Paste your Readwise token, get an OPDS URL + QR to point your e-reader at, and (optionally) archive-on-finish. Cross-platform (macOS / Windows / Linux) via Tauri.

> Status: 🚧 scaffolding. Part of the [Readloop](https://github.com/readloopsync/readloop) project.

## What it does
- Runs the **delivery** server (Readwise → OPDS → EPUB) and the **archive** server (KOSync + Readwise archive-on-finish) as bundled background services.
- Shows the reader-facing **OPDS URL** and **KOSync URL** (+ QR codes) so setup on an e-ink keyboard is a scan, not a type.
- Auto-links your Readwise token to the archive connector (no manual credential wiring).
- Start-at-login; lives in the menu bar / system tray.

See [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the design and the packaging plan (the hard part).

## License
MIT.
