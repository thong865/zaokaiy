# zaokaiy POS — desktop app (Windows · macOS)

An offline-first till for zaokaiy shops. It keeps selling, printing receipts and opening the cash
drawer when the internet is down, and uploads every sale automatically when the connection is back.

| Layer | Stack |
|---|---|
| Shell | [Tauri 2](https://tauri.app) — native Rust app, system WebView (WebView2 on Windows, WebKit on macOS), small installer |
| Core (Rust, `src-tauri/`) | SQLite (bundled `rusqlite`) · sync engine (tokio + reqwest/rustls) · ESC/POS printing (TCP, Windows spooler, CUPS) |
| UI (`app/`) | Nuxt 4 SPA · Nuxt UI 4 · Tailwind v4 — same theme as the web app; fonts and icons bundled for offline use |
| Server | module `pos_sync` of the zaokaiy API (`backend/src/modules/pos_sync`, migration `0016_pos_sync.sql`) |

## How it works

```
 ┌──────────── desktop app ─────────────┐                     ┌──────── zaokaiy API ────────┐
 │  UI (Nuxt)  ──invoke──▶  Rust core    │  push sales/voids   │  pos_sync module             │
 │                          │  SQLite    │ ──────────────────▶ │  idempotent by sale id       │
 │   till · sales · print   │  outbox    │  pull catalogue     │  xid8 change cursor          │
 │                          │  catalogue │ ◀────────────────── │  tombstones for deletions    │
 └──────────────────────────┴────────────┘   device token      └──────────────────────────────┘
```

* **Local first.** Every sale is written to SQLite in one transaction with its final receipt
  number, then queued. The till never waits for the network.
* **Receipt numbers.** Each terminal has a code (`T01`, `T02` …) and its own gap-free series:
  `<receipt prefix><code>-000001` (e.g. `RT01-000001`). A receipt printed offline is final.
* **Sync loop** (Rust, background): every 30 s when online, every 10 s when offline, and
  immediately after each sale. It pushes up to 100 sales + voids per request, then pulls products
  changed since the last cursor. Each pushed item gets its own result (`ok`, `duplicate`,
  `rejected`), so one bad sale never blocks the queue; rejected ones appear under *Sales → Needs attention* with a *Try again* button.
* **Stock.** The till shows *server stock − sales not uploaded yet*. A sale made offline is always
  accepted by the server; if its stock ran out meanwhile, stock stops at 0 and the missing units are
  recorded on the sale (`stock_shortfall`) and shown on the terminal.
* **Totals** are computed in Rust with exactly the server's rules (VAT incl./excl., round half up,
  change only from cash); the server re-checks them.
* **Catalogue pull** uses PostgreSQL transaction ids (`xid8`) as the cursor, so a product changed by
  a long transaction that commits late is never missed. The first pull is a full snapshot that only
  replaces the local catalogue once its last page has arrived.
* **Security.** Pairing uses a device-authorisation flow (no password, captcha or 2FA on the till).
  The device token (`zkd_…`) lives only in the Rust side's database, never in page JavaScript; the
  server stores only its hash. Owners remove terminals at **/dashboard/pos-devices**. A terminal acts
  as the owner/cashier who approved it and loses access when that person leaves the shop.

## Pairing a terminal

1. Install and open the app, enter the server address (e.g. `https://your-domain.com/api`) and a name.
2. The app shows a code like `K7Q4-9MZD` and an **Open approval page** button (`/pos-pair?code=…`).
3. Sign in on the web as the owner or a cashier, pick the shop, press **Approve**.
4. The app downloads the products and is ready. From then on it works offline.

## Printing

Settings → Receipt printer:

* **Network printer (IP)** — raw TCP to `host:9100` (most Ethernet/Wi‑Fi thermal printers).
* **Installed printer (USB)** — the printer's name in Windows (spooler, RAW mode) or macOS (CUPS `lp -o raw`).
* **System print dialog** — any printer, via the OS dialog.

Receipts are drawn on a canvas and sent as a bitmap (`GS v 0`), so **Lao and Thai print correctly**
on any ESC/POS printer (they have no Lao code page). 58 mm and 80 mm paper, auto-cut, cash-drawer
kick on cash sales, day summary print. Barcode scanners work as keyboards (USB/Bluetooth) anywhere on
the till; `3*<code>` adds three.

## Keyboard

`F2` search · `F9` charge · `F8` new bill · `F7` next bill · `Enter` completes payment / starts the next sale · `Esc` back.

## Develop

Prerequisites: Node 22, Rust (stable), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/)
(Windows: WebView2 + MSVC build tools; macOS: Xcode command line tools).

```bash
cd pos-desktop
npm install
npm run app:dev            # desktop app with hot reload (Nuxt on :3100)
npm run dev                # UI only in a browser, with an in-memory demo instead of Rust
cd src-tauri && cargo test # money, local database, printer tests
```

End-to-end test against a running API (pairs, sells offline, syncs back):

```bash
# API started with ADMIN_EMAILS=admin@demo.dev and demo data (python scripts/seed.py)
cd pos-desktop/src-tauri && ZK_API=http://localhost:8080/api cargo test --test offline_sync
python scripts/pos_sync_test.py   # server side: pairing, pull cursors, push, voids, revocation
```

## Build installers

```bash
cd pos-desktop
NUXT_PUBLIC_DEFAULT_API_BASE=https://your-domain.com/api npm run app:build
```

* Windows → `src-tauri/target/release/bundle/nsis/*.exe` and `msi/*.msi` (build on Windows).
* macOS → `src-tauri/target/release/bundle/dmg/*.dmg` (build on a Mac; add
  `-- --target universal-apple-darwin` for Intel + Apple silicon).

CI: `.github/workflows/pos-desktop.yml` builds both on GitHub Actions. Push a tag `pos-v0.1.0` to
get a draft release with the installers. Set the repository variable
`NUXT_PUBLIC_DEFAULT_API_BASE` to pre-fill the server address.

**Code signing (recommended for shops):** without it Windows SmartScreen and macOS Gatekeeper warn on
first launch. macOS: add the `APPLE_*` secrets used by the workflow (Developer ID certificate +
notarisation). Windows: add a code-signing certificate per the
[Tauri Windows signing guide](https://tauri.app/distribute/sign/windows/).

## Data on the computer

`pos.sqlite` in the app data folder (`%APPDATA%\com.zaokaiy.pos\` on Windows,
`~/Library/Application Support/com.zaokaiy.pos/` on macOS): products, categories, sales (the upload
queue), parked bills and settings. WAL mode with `synchronous=FULL`, so a power cut never loses a
completed sale. Disconnecting from a shop is refused while sales are still waiting for upload.

## Not in this version

* Several cashiers on one terminal with PIN switching (a terminal sells as the person who paired it).
* Full A4 tax invoices from the terminal (issue them on the web from *POS sales* after upload).
* Product photos offline (tiles show a letter when there is no connection).
* Auto-update (can be added with `tauri-plugin-updater` once installers are signed).
