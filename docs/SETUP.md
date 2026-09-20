# Setting up Readloop on your CrossPoint device

Readloop runs two small services on your computer and gives your CrossPoint
e-reader two addresses: one for **browsing/downloading** your Readwise library
(OPDS) and one for **syncing reading progress** (KOSync). This guide covers
pairing the device with them.

> The Readloop app shows these addresses with the exact username/password to
> enter. Click any field in the app to copy it.

## 1. Browse your library (OPDS)

OPDS lets the device browse and download files from Readwise.

**On the device:** Settings › System › OPDS Servers › Add Server

- **URL:** the OPDS address shown in the app (e.g. `http://192.168.1.50:8080/opds`)
- **Username / Password:** as shown in the app

> _Screenshot: OPDS Servers screen_ <!-- TODO -->

Once added, open the catalog to browse Inbox / Later / Shortlist / Feed and
download articles as EPUBs.

## 2. Sync reading progress (KOSync)

KOSync syncs your reading status from/to Readwise (finish an article → it
archives in Readwise and drops off the feed; resume where you left off).

**On the device:** Settings › System › KOReader Sync

- **Server:** the KOSync address shown in the app
- **Sign up** once with the username/password shown in the app, then **log in**.

> _Screenshot: KOReader Sync screen_ <!-- TODO -->

## 3. Access from anywhere (optional)

By default the addresses work only when the device and computer are on the same
Wi-Fi. To reach Readloop away from home, use the app's **Remote access** card
(one click, via Tailscale Funnel — no router setup). It publishes a secure
public `https://…` URL to use on the device instead of the LAN address.

## 4. Mount as a drive (optional)

The **Mount in Finder** card mounts the device over Wi-Fi (WebDAV) so you can
drag files straight onto it.

---

_Screenshots on this page are captured from the CrossPoint firmware. See the
project README for how they're produced._
