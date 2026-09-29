# zaokaiy — social commerce platform

Sellers open shops and manage products and inventory. They can open their catalog to **sell-staff**: other shops and content creators who resell those products in their own storefront and earn a commission the supplier sets. Ads, a creator content studio, and an AI agent help everyone grow.

| Layer | Stack |
|---|---|
| Backend | Rust · Axum 0.8 · SQLx 0.8 (Postgres) · JWT + Argon2 · Anthropic Messages API |
| Frontend | Nuxt 4 · Vue 3 · Nuxt UI 4 (Vuesax-style theme in `app/app.config.ts`) · Tailwind CSS v4 · Lucide icons · light/dark/system colour mode |
| Database | PostgreSQL 16. Migrations are embedded and run automatically when the API starts. |

## Quick start

### Option A: Docker (everything)
```bash
docker compose up --build
python scripts/seed.py            # optional demo data
```
Open http://localhost:3000. The API runs at http://localhost:8080/api.

### Production (VPS, e.g. Hostinger)

See **[DEPLOY.md](DEPLOY.md)**: `docker-compose.prod.yml` + Caddy (automatic HTTPS on one domain), `.env.prod`, backups.

### Option B: Local dev
```bash
# 1. Postgres
docker compose up -d db

# 2. API
cd backend
cp .env.example .env              # add ANTHROPIC_API_KEY to enable the AI features
cargo run

# 3. Web (new terminal)
cd frontend
npm install
npm run dev                       # http://localhost:3000

# 4. Demo data + tests (new terminal, from the repo root)
python scripts/seed.py
python scripts/smoke_test.py
python scripts/media_test.py
python scripts/review_test.py
python scripts/pos_test.py
python scripts/mock_graph.py &   # fake Meta Graph API for the social test
python scripts/social_test.py    # API started with META_VERIFY_TOKEN=vt META_APP_SECRET=appsecret TIKTOK_CLIENT_SECRET=ttsecret META_GRAPH_URL=http://127.0.0.1:9998
python scripts/logistics_test.py  # couriers, delivery fees, COD ledger (ADMIN_EMAILS=admin@demo.dev)
python scripts/kyb_test.py       # business verification (KYC): documents, review, publish gate
python scripts/vehicle_test.py   # vehicle dealers: search, buyer requests, deposits (ADMIN_EMAILS=admin@demo.dev)
python scripts/oauth_test.py     # social login; API env listed at the top of the script (mock Google/Facebook/WhatsApp)
```
The tests and seed need the API started with `ADMIN_EMAILS=admin@demo.dev` (the default in `.env.example`).
Demo logins (password `password123`): `siam@demo.dev`, `lanna@demo.dev` (suppliers), `bee@demo.dev` (creator/sell-staff), `buyer@demo.dev`, `auto@demo.dev` (vehicle dealer "Vientiane Auto"). Siam Crafts and Vientiane Auto are verified businesses; Lanna Botanics is waiting in the KYC queue, `admin@demo.dev` (platform admin → `/admin`).

## Core concepts

**Shop.** One account can own many shops. `kind` is `seller` (own products) or `creator` (mostly resells).

**Product & inventory.** `stock` changes only in transactions, and every change writes an `inventory_movements` row with a reason: `restock`, `sale`, `adjust`, `return`, `cancel`, or `reserve`/`release` (social orders). There are low-stock alerts per product.

**Sell-staff program:**
1. The supplier turns on `allow_resell` for a product and sets a default `commission_bps` (1000 = 10%).
2. A reseller shop requests a **partnership** with the supplier shop.
3. The supplier approves it and can give that reseller a custom rate.
4. The reseller adds **listings** to its storefront. Shoppers who buy through `/p/:id?via=<reseller_shop_id>` credit the reseller.
5. Checkout locks the product rows, reserves stock, creates one order per supplier, and writes a **commission** row (`pending`).
6. When the order is `completed`, the commission becomes `approved`, and the supplier marks it `paid`. If the order is cancelled, the stock is returned and the commission becomes `void`.
7. There is no self-commission: a reseller who buys through their own storefront earns nothing.

**Categories.** There are two trees, and each can be up to 3 levels deep (category › sub-category › sub-sub-category):
- *Marketplace categories* are managed by admins at `/admin/categories`. Every product needs one before it can be published. Shoppers browse by them, and a category page also shows everything in its sub-categories. A starter set is created by the migration. Admins can hide a category, or reassign its products before deleting it.
- *Shop categories* are each seller's own category tree for their storefront, managed at `/dashboard/categories`. Sellers can add, rename, reorder, move under another category, hide and delete them. They appear as a sidebar filter on `/s/:slug`.

**Product review (moderation).** Sellers' products go through `not_submitted → pending → approved | rejected`. Shoppers only see products that are both `active` and `approved`; this applies to the catalog, storefronts, checkout, ads, the creator feed and resale.
- Publishing (status `active`, or `POST /products/:id/submit`) sends a product for review. Admins approve or reject it at `/admin`, one at a time or in bulk, and a rejection must include a reason the seller can see.
- If a live product's name, description, marketplace category or gallery changes, it goes back to review (`REVIEW_ON_EDIT`). Price, stock and commission changes stay live.
- Trusted shops (`auto_approve`, set at `/admin/shops`) skip the queue. `PRODUCT_REVIEW=off` turns moderation off completely.
- Every step is recorded in `product_reviews`. Admins are the emails listed in `ADMIN_EMAILS`; they are promoted at startup and at registration, and the role is re-checked against the database on every admin request.

**Social selling (comment → order).** Customers order by commenting a product's short code (`CF A01 x2`) on a Facebook post or live, a TikTok live, or by sending it on WhatsApp or Messenger.
- The parser understands CF/F/เอา/รับ/สั่ง, quantities (`x2`, `=2`, `2 ชิ้น`), Thai, Lao and full-width digits, several codes in one comment, and codes written without a space (`CFA01`). Questions like "A01 ราคาเท่าไหร่?" are ignored.
- Each customer gets one open order; new claims are added to it. Stock is reserved first come, first served, up to a per-customer limit. Items that run out get a sold-out reply.
- The auto-reply (a private reply plus a public acknowledgement on Facebook, or a WhatsApp message) includes a checkout link `/c/:token`. The customer confirms their address and reports their payment reference; no account is needed.
- The seller marks the order paid → shipped (with a tracking number) → delivered.
- Orders that aren't confirmed or paid expire after `hold_hours`, and a background job returns their stock.
- Live board: start or end a live session, see product codes with claimed and remaining counts, and watch comments arrive about every 2 seconds. A built-in simulator lets you test without connecting any account.
- Webhooks:
  - `/api/webhooks/meta`: verify token handshake, and `X-Hub-Signature-256` checked with `META_APP_SECRET`.
  - `/api/webhooks/tiktok`: `TikTok-Signature` with a 5-minute window against replays.
  - `/api/webhooks/ingest/:channel_id`: HMAC signed with the channel's secret, for n8n, Make, Zapier, LINE bots and similar. The reply text and checkout link come back in the response.
- Every webhook message is stored once, so platform retries never reserve stock twice.
- Access tokens can be written through the API but are never returned.
- Limitations: TikTok gives LIVE comments only to approved partners, and there is no public API for replying to them. WhatsApp replies must be sent within the 24-hour customer service window.

**Languages (English / ລາວ).** The whole UI is available in English and Lao. The EN/ລາວ switch in the header saves the choice in the `zk_lang` cookie; on a first visit the browser language decides, and English is the fallback.
- Strings live in `frontend/i18n/locales/{en,lo}/<namespace>.json`. Conventions and the Lao glossary are in `frontend/i18n/GLOSSARY.md`. `node scripts/i18n-check.mjs` (in `frontend/`) checks that both languages have the same keys and that every `$t()` key exists.
- The frontend sends `Accept-Language` with every request. For `lo`, the API returns its error messages in Lao (`backend/src/i18n.rs`, with a test that fails if any error message lacks a translation).
- Lao dates are formatted in `utils/format.ts` rather than with `Intl`, because Chrome has no Lao locale data. Numbers use Latin digits in both languages.
- Printed documents follow the shop currency: LAK shops print Lao with English (amounts in Lao words), THB shops print Thai with English, others English. Shops can choose THB, LAK or USD.
- Social selling understands Lao comments (`ເອົາ A01 2`, `ຈອງ B03`); Lao trigger words are in the defaults, and the settings page has Lao, Thai and English reply templates.
- Set `NUXT_PUBLIC_I18N_BASE_URL` to the public site URL in production.

**Sign-in.** Email or phone + password, or **Continue with Google / Facebook / WhatsApp** (only the methods configured on the server are shown).
- Google and Facebook use the OAuth authorization-code flow run by the API (state + PKCE for Google, `appsecret_proof` for Facebook). After the callback the browser gets a one-time code that `/auth/exchange` swaps for the session token, so tokens never appear in URLs.
- WhatsApp sends a 6-digit code through a WhatsApp Cloud API *authentication* template. Codes are hashed, expire after 10 minutes, allow 5 attempts, and are rate-limited per number (1 per minute, 5 per hour).
- Linking: a Google login with a verified email joins the account with that email. A Facebook email that matches an existing account is refused with a prompt to sign in and connect Facebook from **/account**, so nobody can take over an account through Facebook. On **/account** people connect or disconnect methods and set a password; the last usable method can't be removed.

**Delivery & cash on delivery (Laos).** Couriers: Anousith Express, HAL Express, Mixay Express and "shop delivery". Admins can add more at **/admin/carriers**, each with an optional tracking-page template using `{tracking}`.
- These couriers have no public booking API. The seller drops the parcel at the courier and types the tracking number; the system records courier, tracking and COD.
- **/dashboard/shipping:** per courier, the seller sets the fee, who pays it, home delivery with its fee, free shipping over an amount, and COD with a COD fee. The fee can be added at checkout, paid by the customer to the courier at pickup (ປາຍທາງ), or free.
- **Checkout:** the buyer picks courier, branch pickup or home delivery, and Pay now or COD, both in the cart and on the comment-order checkout `/c/:token`. `orders.grand_total_cents` = items + shipping (when charged at checkout) + COD fee = `cod_amount_cents` for COD.
- COD orders ship without online payment. **/print/label/{order|social}/:id** prints an A6 label with the COD amount.
- **/dashboard/cod** is the ledger: `pending` → `collected` (courier has the cash; order completes) → `remitted` (courier paid the shop, with transfer reference). `returned` = refused parcel: order cancelled, stock back (movement reason `return`).

**Business verification (corporate KYC).** Shops are either *individual* or *registered business* (`shops.entity_type`, chosen at onboarding or in Shop settings). Business shops verify their company at **/dashboard/verification** before they can publish products (`KYB_REQUIRED_TO_PUBLISH`, default on; drafts are always allowed). Verified shops show a **Verified business** badge on their storefront and product pages.
- **What the seller provides:** company type (Lao enterprise forms: sole enterprise, sole/limited/public company, partnership, state enterprise, cooperative, foreign branch), registered names (EN/Lao), enterprise registration number and date, tax ID, country, province, address, business activity and contacts. They also add the people behind it: one legal representative, the directors, and the beneficial owners (≥25%) with ownership %, ID type/number, nationality, date of birth and PEP flag. Plus the payout bank account and documents.
- **Documents:** the registration certificate, tax certificate and representative ID are required. Limited, public and sole companies and partnerships also need a shareholder register. An authorisation letter is needed when the representative is not a director. Business licence, articles, proof of address, bank proof and other documents are optional. Files are JPG/PNG/WebP/PDF up to 10 MB, checked by content, with an optional expiry date; expired or rejected documents don't count. A live checklist shows what's missing.
- **Privacy & security:**
  - ID and bank account numbers are encrypted with AES-256-GCM (`KYC_ENCRYPTION_KEY`) and shown to the owner only as the last 4 characters.
  - Documents are encrypted files in **private storage**: `PRIVATE_DIR`, a separate volume that is never served, or `KYC_S3_BUCKET` / the `private/` prefix on S3. They are only readable through `GET /kyb/documents/:id/file` by the owner or an admin, with `no-store` and a sandbox CSP.
  - Every admin view of a verification or a document is written to the audit log (`kyb_events`).
  - **Back up `KYC_ENCRYPTION_KEY` separately: without it the documents can't be read.**
- **Workflow:** draft → submitted (locked) → approved | changes requested | rejected; approved → revoked.
  - Admins work the queue at **/admin/kyb**. It shows risk flags: PEP, a registration number used by another account, a foreign representative or owner, documents expiring within 30 days, and under 25% ownership declared.
  - The admin detail page shows full numbers behind a "show" toggle and previews documents in the page. Admins accept or reject each document with a reason, then approve, request changes, reject or revoke with a note. Approval sets a review date (`KYB_REVIEW_MONTHS`, default 12).
  - A verified shop that edits its details stays verified while the update is reviewed. Revoking removes the badge and takes the shop's products off sale.
  - A business shop in review or verified can't switch itself back to individual.

**Vehicle sellers (cars, motorbikes, trucks).** A shop with `vertical = vehicle` ("Vehicle dealer" in onboarding or Shop settings) gets a showroom storefront at `/s/:slug`, and every vehicle listing is also browsable at **/vehicles**.
- A listing is a normal product plus a spec sheet (`vehicle_specs`): type, condition, make (canonical spelling, e.g. `toyota` → `Toyota`), model, variant, year, mileage, fuel, transmission, body type, drive, engine cc, power, seats, doors, colour, previous owners, registration and plate province, location, features, warranty. Plate number and VIN are private; the public page shows only the last 4 VIN characters. The product form shows a "Vehicle details" section (on by default for vehicle dealers) and offers "2020 Toyota Hilux Revo 2.4 E" as the title.
- **Showroom:** tabs per type with counts; filters for make → model, year range, price range, max mileage, condition, fuel, transmission and body type (with counts), text search and sorting, all in the URL so filtered links can be shared. `GET /vehicles?shop=<slug>` returns items, total and facets.
- **Listing page:** key facts, spec table, features, Call and WhatsApp buttons (pre-filled message with the link), a flat-rate loan calculator (down payment, term, rate), and a request form: **ask a question, book a test drive (date & time), make an offer, reserve (deposit), ask about finance**. No account is needed; Lao numbers like `020 5555 1234` are accepted. Repeats within 10 minutes are merged and each phone number can send 5 requests per hour.
- **Selling terms per vehicle:** reservation deposit, negotiable, finance available, and "allow buying online". By default vehicles are not in the cart; checkout refuses vehicles that aren't buy-online or aren't available.
- **Sale status:** `available → reserved → sold`. Recording a reservation deposit on a request reserves the vehicle for 7 days (one reservation at a time; it lapses automatically). Sold vehicles stay listed for 7 days with a "Sold" badge, sort last and take no new requests. Selling the last unit online marks it sold; cancelling that order makes it available again.
- **/dashboard/leads** ("Buyer requests", shown for vehicle dealers): filter by status and type, call or WhatsApp the buyer, set the appointment and private notes, move through `new → contacted → scheduled → won | lost`, record or undo the deposit, and mark the vehicle sold or available.
- Brokers: a sell-staff storefront that resells a vehicle (normal partnership + listing) shows it in its showroom, and buyers who arrive through it contact the broker.
- Marketplace categories *Vehicles › Cars / Motorbikes / Trucks & Vans / Parts & Accessories* are created by migration `0010_vehicles.sql`.

**POS (counter selling).** The till is at `/pos`:
- Scan a barcode, search, or tap products. Custom items are supported.
- **Barcode scanning:**
  - USB and Bluetooth scanners work anywhere on the POS screen, with no need to click the search box. A scanner's fast keystrokes are told apart from human typing, and form fields are never hijacked.
  - **Camera scanning** works on phones and tablets: the browser's BarcodeDetector where available, otherwise a bundled ZXing WebAssembly reader (Safari, Firefox, desktop Linux). It is loaded only when the camera opens, works offline, and adds an item once per appearance in view.
  - Type `3*` or `3x` before a code to add 3. Each scan beeps (can be turned off) and highlights the line.
  - `GET /shops/{id}/pos/scan` matches equivalent codes, so a scanner sending UPC-A, EAN-13 or GTIN-14 finds the same product, as does the SKU. Scanner noise such as an AIM prefix or CR/LF is ignored.
  - **Unknown barcode:** the cashier searches for the product and saves the code to it on the spot (it scans next time), or opens "create product" with the code filled in.
- **Barcode labels** (`/dashboard/barcodes`): generate in-store EAN-13 codes (prefix `20`, valid check digit, unique per shop) for products without one, then print labels on a 40×30 or 50×30 mm label printer or an A4 sheet (3×8). Labels show name, price and a vector barcode. The product form can scan a barcode with the camera, validate its check digit, preview it and generate one.
- Discounts: per line or on the whole bill (amount or %). Price overrides per line.
- Split payments across cash, card, transfer, QR/PromptPay and other, with quick cash buttons and change calculation (change can only come from cash).
- Keyboard: F2 = search, F9 = charge, Esc = back.
- The cart is saved in the browser, so a refresh doesn't lose a sale.
- Stock is reduced in the same transaction, and every movement is logged with the receipt number.
- VAT uses the shop's settings (default 7%; prices can include or exclude VAT), and the rate is saved on each sale, so later setting changes never alter issued documents.
- Receipt and tax-invoice numbers run in sequence per shop with no gaps (`R000001`, `INV000001`; prefixes are configurable).
- Voiding a sale returns the stock and keeps its number.

**Printing.** Print pages open in a hidden frame and print straight away, so the POS stays on screen; they can also be opened directly to preview or save as PDF.
- `/print/receipt/:id?w=80|58`: thermal receipt. It becomes an abbreviated tax invoice (ใบกำกับภาษีอย่างย่อ) when the shop has a tax ID.
- `/print/invoice/:id[?copy=1]`: A4 full tax invoice (ใบกำกับภาษี / TAX INVOICE) with original/copy versions, buyer tax ID and branch, the amount in Thai baht text and English, and signature lines.
- `/print/order/:id?type=invoice|packing`: invoice or packing slip for online orders.
- `/print/z-report?shop=&date=`: end-of-day report with takings by payment method, cash net of change, VAT, voids and top items.
- Business and tax details are edited in Shop settings.

**Media library.** Each shop has its own library of images and videos. You upload a file once and can reuse it in product galleries, creator posts and ads.
- Uploads are checked by their actual file content, not the file name. Photos are rotated using their camera orientation, shrunk to at most 2000 px, re-encoded (which strips GPS and other camera data) and given a 480 px thumbnail. Animated GIFs are kept as they are.
- Videos (MP4, WebM, MOV) get a poster frame when `ffmpeg` is installed.
- A product gallery holds up to 15 items and can be reordered; the first item is the cover. `products.images` is kept in sync with the gallery for catalog listings.
- Deleting an item that is still used is blocked (409) unless you pass `force`, which removes it from every product, post and ad.
- Storage is either `local` (disk, served at `/media`) or `s3` (AWS, MinIO, R2). See `backend/.env.example`.
- **Fast images:** each upload gets responsive **WebP** versions (320/640/960/1280/1920 px, never upscaled, quality 82), a ~0.5 KB blurred placeholder and a dominant colour. A JPEG (or PNG with transparency) fallback is kept for zoom and sharing. `products.cover` stores the first gallery image with its versions, so catalog cards download a ~20-60 KB file instead of the full-size original.
- The frontend `AppImage` component renders `<picture>` + `srcset`/`sizes`, native lazy loading, `decoding=async`, width/height (no layout shift) and the blurred placeholder. The product page's main image loads eagerly with `fetchpriority=high`.
- **Minimum resolution:** `MEDIA_MIN_IMAGE_EDGE` (default 500 px on the shortest side) is checked in the browser before upload and enforced by the API. Tiles under 1000 px get a "Low res" badge; 1200×1200+ is recommended.
- Older uploads get their versions from a background job at API start; admins can also run `POST /admin/media/backfill`. Image processing is limited to one job per CPU core, and files in one upload are processed in parallel.

**Ads.** CPC campaigns on your own or resold products. `/ads/serve` records impressions. `/ads/:id/click` charges the CPC and ends the campaign automatically when the budget runs out.

**Creator content.** Posts, captions, video scripts, descriptions and ad copy. Published posts appear in `/feed` with shoppable links that credit the creator's shop.

**AI:**
- `POST /shops/:id/ai/generate` writes content for a product in any tone or language, and can save it as a draft.
- `POST /shops/:id/ai/agent` is a tool-using assistant. It can read stats, products, low stock, orders and sell-staff requests, and it can create **drafts** of content and ad campaigns. It never publishes anything or spends money by itself.
- If `ANTHROPIC_API_KEY` is not set, both endpoints fall back to offline templates, so the platform still works.

## API map (`/api`)
```
auth         POST /auth/register · POST /auth/login (email or +phone) · GET /auth/me · GET /auth/providers
delivery     GET /carriers · GET /shipping/options?shops= · GET /shops/{id}/shipping · PUT /shops/{id}/shipping/{carrier}
             GET /shops/{id}/cod · POST /shops/{id}/cod/update · GET|POST /admin/carriers · PATCH /admin/carriers/{code}
social login POST /auth/oauth/{google|facebook}/start · GET /auth/oauth/{p}/callback · POST /auth/exchange
             POST /auth/whatsapp/send · POST /auth/whatsapp/verify · GET /auth/identities · DELETE /auth/identities/{p} · POST /auth/password
vehicles     GET /vehicles?shop=&type=&make=&model=&year_min=&year_max=&price_min=&price_max=&km_max=&fuel=&transmission=&condition=&body=&q=&sort=
             POST /catalog/products/:id/leads · GET /shops/:id/leads · PATCH /leads/:id
             GET|PUT /products/:id/vehicle · POST /products/:id/vehicle/status
catalog      GET /catalog/products · GET /catalog/products/:id?via= · GET /catalog/categories
             GET /storefront/:slug · GET /feed/contents · GET /ads/serve · POST /ads/:id/click
shops        GET /me/shops · POST /shops · PATCH /shops/:id {entity_type…} · GET /shops/:id/stats
kyb          GET|PUT /shops/:id/kyb · POST /shops/:id/kyb/submit · POST /shops/:id/kyb/documents (multipart kind, file, expires_on)
             DELETE /kyb/documents/:id · GET /kyb/documents/:id/file
             GET /admin/kyb?status=&q= · GET /admin/kyb/:shop_id · POST /admin/kyb/:shop_id/decision {action, note, documents}
products     GET|POST /shops/:id/products · GET|PATCH|DELETE /products/:id
inventory    POST /products/:id/stock · GET /shops/:id/inventory/movements · GET /shops/:id/inventory/low-stock
sell-staff   GET /marketplace/products · GET|POST /shops/:id/partnerships
             POST /partnerships/:id/decision · POST /partnerships/:id/revoke
             GET|POST /shops/:id/listings · DELETE /listings/:id
commissions  GET /shops/:id/commissions · GET /shops/:id/commissions/payable · POST /commissions/:id/pay
orders       POST /orders/checkout · GET /orders · GET /orders/:id · POST /orders/:id/pay · POST /orders/:id/cancel
             GET /shops/:id/sales?role=supplier|seller · POST /shops/:id/orders/:order_id/status
ads          GET|POST /shops/:id/ads · PATCH /ads/:id
content      GET|POST /shops/:id/contents · PATCH|DELETE /contents/:id
media        GET /shops/:id/media · POST /shops/:id/media/upload[?product_id=] (multipart `file`)
             POST /shops/:id/media/url · POST /shops/:id/media/delete · GET|PATCH|DELETE /media/:id[?force=true]
             GET|PUT /products/:id/media   (PUT {asset_ids:[…]} = full ordered gallery)
categories   GET /categories · GET|POST /shops/:id/categories · PATCH|DELETE /categories/:id[?reassign_to=] · POST /categories/reorder
review       POST /products/:id/submit · GET /products/:id/reviews
admin        GET /admin/overview · GET /admin/products[?review_status=] · GET /admin/products/:id
             POST /admin/products/:id/review {action: approve|reject, note} · POST /admin/products/review (bulk)
             GET /admin/shops · PATCH /admin/shops/:id {auto_approve} · GET|POST /admin/categories
pos          GET /shops/:id/pos/products?q= · GET|POST /shops/:id/pos/sales · GET /shops/:id/pos/summary?date=&tz=
             GET /pos/sales/:id · POST /pos/sales/:id/void {reason} · POST /pos/sales/:id/invoice {customer…}
             GET /orders/:id/document  (printable online order)
social       GET|PATCH /shops/:id/social/settings · GET|POST /shops/:id/social/channels · PATCH|DELETE /social/channels/:id
             GET|POST /shops/:id/social/sessions · POST /social/sessions/:id/end · GET /shops/:id/social/feed|board
             POST /shops/:id/social/simulate · POST /shops/:id/social/assign-codes · GET /shops/:id/social/orders
             GET /social/orders/:id · POST /social/orders/:id/status · POST /social/orders/:id/items
             GET /public/social-orders/:token · POST …/confirm · POST …/payment
webhooks     GET|POST /webhooks/meta · POST /webhooks/tiktok · POST /webhooks/ingest/:channel_id
ai           GET /ai/status · POST /shops/:id/ai/generate · POST /shops/:id/ai/agent
```

## Project layout
```
backend/
  migrations/0001_init.sql     schema
  src/main.rs                  bootstrap, CORS, migrations
  src/auth.rs                  JWT, Argon2, AuthUser extractor
  src/ai.rs                    content generation + agent tool loop
  src/media.rs · storage.rs    image processing · local/S3 storage
  src/routes/*.rs              one module per domain
frontend/app/
  pages/                       storefront, product, cart, orders, feed, dashboard/*
  components/ composables/     UI kit, useApi/useAuth/useCart/useShop
scripts/seed.py · smoke_test.py
```

## Production checklist / next steps
- **POS:** add a cash-drawer kick and ESC/POS raw printing through a local print bridge if you need silent printing. Credit notes for returns against a tax invoice. Shifts per cashier and staff accounts.
- **Payments:** `POST /orders/:id/pay` is a mock. Replace it with a PSP webhook (PromptPay/Omise/Stripe) and settle commissions through payouts.
- **Media:** use a CDN in front of `MEDIA_PUBLIC_URL`, move video transcoding (HLS) to a background worker, and add per-shop storage quotas.
- Set a strong `JWT_SECRET`, use HTTPS, and add rate limiting (for example `tower_governor`) on auth and ad-click endpoints.
- Ad-click fraud protection (dedupe per visitor/IP), plus search with Postgres FTS or Meilisearch.
- Shipping integrations (Kerry, Flash, Thailand Post), plus PDPA consent and data-export tooling.
"# zaokaiy" 
