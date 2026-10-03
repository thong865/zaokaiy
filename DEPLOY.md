# Deploy zaokaiy to a Hostinger VPS with Docker

One server runs everything: Postgres, the Rust API, the Nuxt web app and Caddy. Caddy gets a free Let's Encrypt certificate and serves your domain over HTTPS:

| URL | Goes to |
|---|---|
| `https://DOMAIN/` | Nuxt web app |
| `https://DOMAIN/api/*` | Rust API, including webhooks `/api/webhooks/meta` and `/api/webhooks/tiktok` |
| `https://DOMAIN/media/*` | uploaded images and videos |

Files: `docker-compose.prod.yml`, `deploy/Caddyfile`, `.env.prod.example` and `deploy/backup.sh`.

> Hostinger **web / cloud hosting cannot run Docker**. You need a **VPS** (KVM plan).

## 1. Get the VPS

- **Plan:** KVM 2 (2 vCPU, 8 GB RAM) is comfortable. KVM 1 (4 GB) is enough: nothing is compiled on the server.
- **OS template:** in hPanel choose **Ubuntu 24.04 with Docker** (or plain Ubuntu 24.04 and install Docker in step 4).
- **Server location:** pick the one closest to your customers (e.g. Asia) for faster pages.

## 2. Point your domain at the VPS

In hPanel → **Domains → DNS / Nameservers** (or at your registrar), add:

| Type | Name | Value |
|---|---|---|
| A | `@` (or a subdomain like `shop`) | your VPS IPv4 |
| A | `www` (optional) | your VPS IPv4 |

Wait until `ping DOMAIN` shows the VPS IP. The certificate can only be issued once DNS points at the server.

## 3. Open the firewall

hPanel → **VPS → Security → Firewall**: allow **22** (SSH), **80** and **443** (TCP), plus **443/UDP** for HTTP/3. Don't open 5432, 8080 or 3000; those stay private inside Docker.

## 4. Prepare the server

```bash
ssh root@YOUR_VPS_IP

# Docker (skip if you chose the Docker template)
curl -fsSL https://get.docker.com | sh

# 2 GB swap (images are built off the server, so the VPS no longer needs RAM for compiling Rust)
fallocate -l 2G /swapfile && chmod 600 /swapfile && mkswap /swapfile && swapon /swapfile
echo '/swapfile none swap sw 0 0' >> /etc/fstab
```

## 5. Prepare the deploy folder (no source code on the server)

The server never gets the source code. It holds only the files needed to **run** prebuilt images:

```
/var/serv/zaokaiy/
├── docker-compose.prod.yml
├── .env.prod                 # your secrets (step 6)
├── .env.prod.example
└── deploy/
    ├── Caddyfile
    ├── deploy.sh             # starts a release, health check, rollback
    ├── backup.sh
    └── .release.env          # which images are running (written by deploy.sh)
```

The images contain only compiled output: `zaokaiy-api` is the Rust binary (+ ffmpeg) on Debian slim, `zaokaiy-web`
is Nuxt's `.output` on Node Alpine. They are built either by GitHub Actions (step 10) or on your PC with
`deploy/ship.ps1`, never on the VPS.

Upload the runtime files once from your PC (PowerShell, in `D:\app\zaokaiy`):

```powershell
.\deploy\ship.ps1 -Server root@YOUR_VPS_IP -FilesOnly
```

## 6. Configure

```bash
cd /var/serv/zaokaiy
cp .env.prod.example .env.prod
sed -i "s/^POSTGRES_PASSWORD=.*/POSTGRES_PASSWORD=$(openssl rand -hex 24)/" .env.prod
sed -i "s/^JWT_SECRET=.*/JWT_SECRET=$(openssl rand -hex 32)/" .env.prod
sed -i "s|^KYC_ENCRYPTION_KEY=.*|KYC_ENCRYPTION_KEY=$(openssl rand -base64 32)|" .env.prod
sed -i "s|^COD_RISK_PEPPER=.*|COD_RISK_PEPPER=$(openssl rand -base64 32)|" .env.prod
nano .env.prod      # set DOMAIN, TUNNEL_TOKEN, ADMIN_EMAILS, and optionally the AI / social keys
chmod 600 .env.prod
```

Get `TUNNEL_TOKEN` from Cloudflare Zero Trust → Networks → Tunnels → your tunnel → install command. Keep `KYC_ENCRYPTION_KEY` and `COD_RISK_PEPPER` backed up; changing either can invalidate protected production data. `DOMAIN` is the bare host name, e.g. `shop.example.com`. The app, API, CORS, checkout links and media URLs are all derived from it.

## 7. First release

Pick one way to build and ship (both described in step 10):

```powershell
# A) from your PC, no GitHub needed: builds locally, uploads only the images
.\deploy\ship.ps1 -Server root@YOUR_VPS_IP
```

or **B)** push to `master` with the GitHub Actions secrets set up (step 10); the workflow builds the images and
uploads only the runtime files.

The first local build takes about 5–15 minutes, mostly compiling Rust. Database migrations run automatically when
the API starts.

Tip: add an alias to save typing:

```bash
echo "alias zk='docker compose -f /var/serv/zaokaiy/docker-compose.prod.yml --env-file /var/serv/zaokaiy/.env.prod --env-file /var/serv/zaokaiy/deploy/.release.env'" >> ~/.bashrc && source ~/.bashrc
zk ps
```

## 8. Check it

```bash
zk ps                       # all 4 services "running"
curl https://DOMAIN/api/health   # {"ok":true}
zk logs -f caddy            # certificate issued? ("certificate obtained successfully")
zk logs -f api
```

Open `https://DOMAIN`, **register with the email you put in `ADMIN_EMAILS`**. That account gets `/admin` (product review and categories). Don't run `scripts/seed.py` on production; it creates demo shops.

## 9. Social selling webhooks

In the Meta and TikTok developer consoles, use:

- Meta (Facebook comments, Messenger, WhatsApp): callback `https://DOMAIN/api/webhooks/meta`, verify token = `META_VERIFY_TOKEN`, app secret = `META_APP_SECRET`
- TikTok: `https://DOMAIN/api/webhooks/tiktok`, client secret = `TIKTOK_CLIENT_SECRET`

Then connect each page or number in the dashboard under **Social selling → Channels**. After changing `.env.prod`, run `zk up -d`.

## 9b. Social login (Google, Facebook, WhatsApp)

Each button appears only when its keys are set in `.env.prod`. After editing, run `zk up -d`.

**Google**
1. [Google Cloud Console](https://console.cloud.google.com/) → APIs & Services → **OAuth consent screen**: app name, support email, your domain; scopes `openid`, `email`, `profile`. Publish the app (move it out of "Testing") so anyone can sign in.
2. **Credentials → Create credentials → OAuth client ID → Web application**.
   - Authorized redirect URI: `https://DOMAIN/api/auth/oauth/google/callback`
3. Put the client ID and secret in `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET`.

**Facebook**
1. [Meta for Developers](https://developers.facebook.com/apps) → your app (it can be the same app used for social selling) → add **Facebook Login**.
2. Facebook Login → Settings → **Valid OAuth Redirect URIs**: `https://DOMAIN/api/auth/oauth/facebook/callback`
3. Permissions: `public_profile` and `email`. Switch the app to **Live** mode.
4. App settings → Basic: copy the App ID and App secret to `FACEBOOK_APP_ID` / `FACEBOOK_APP_SECRET`.

**WhatsApp**
1. You need a WhatsApp Business Account with a phone number on the **WhatsApp Cloud API** (the number customers will receive codes from).
2. WhatsApp Manager → Message templates → **Create template → Authentication**, with a *Copy code* button. Name it `zaokaiy_login` (or set `WHATSAPP_OTP_TEMPLATE`), language English (`en_US`). Optionally add a Lao version and set `WHATSAPP_OTP_LANG_LO` to its language code.
3. Create a **System user** token with `whatsapp_business_messaging` permission → `WHATSAPP_TOKEN`; the number's **Phone number ID** → `WHATSAPP_PHONE_NUMBER_ID`.
4. Meta charges per authentication message. The API limits each number to 1 code per minute and 5 per hour.

Never set `OTP_DEV_ECHO=true` in production; it returns the code in the API response.

## 9c. Human check (Cloudflare Turnstile) and two-step verification

**Turnstile** protects log in, sign up and WhatsApp code requests from bots. It is off until both keys are set.
1. Cloudflare dashboard → **Turnstile → Add widget**. Hostname: your `DOMAIN` (add `localhost` for local testing). Widget mode: **Managed**.
2. Copy the **Site key** to `TURNSTILE_SITE_KEY` and the **Secret key** to `TURNSTILE_SECRET_KEY` in `.env.prod`, then `zk up -d`.
3. For local testing you can use Cloudflare's test keys: site `1x00000000000000000000AA`, secret `1x0000000000000000000000000000000AA` (always pass).

**Two-step verification (2FA)** needs no setup. Users turn it on from **Account → Two-step verification** with any authenticator app. It then applies to every sign-in method (password, Google, Facebook, WhatsApp). The TOTP secrets are encrypted with `KYC_ENCRYPTION_KEY`. If you change that key, everyone with 2FA on is locked out, so keep it backed up.

A user who loses both their phone and their recovery codes can be reset by an admin in the database:
```bash
zk exec db psql -U zaokaiy -c "UPDATE users SET totp_secret=NULL, totp_enabled_at=NULL, totp_last_step=NULL WHERE email='user@example.com'; DELETE FROM user_recovery_codes WHERE user_id=(SELECT id FROM users WHERE email='user@example.com');"
```

## 10. Updating

Both ways build the images **off the server** and send only images + the five runtime files. Each release is
tagged (commit SHA), `deploy/deploy.sh` waits for `/api/health`, and if the new release isn't healthy within
3 minutes the previous one is started again. The last 3 releases are kept on the server for rollback.

### A) From your PC: `deploy/ship.ps1` (no GitHub, no registry)

Needs Docker Desktop and the built-in Windows OpenSSH (`ssh`/`scp`). From `D:\app\zaokaiy`:

```powershell
$env:ZK_SERVER = 'root@YOUR_VPS_IP'      # optional: ZK_PATH (default /var/serv/zaokaiy), ZK_PORT, ZK_SSH_KEY
.\deploy\ship.ps1                        # build -> docker save -> scp -> docker load -> deploy.sh
.\deploy\ship.ps1 -Rollback              # previous release
```

What it does:

1. `docker build --platform linux/amd64` of `backend` and `frontend`, tagged `zaokaiy-api:<git-sha>` /
   `zaokaiy-web:<git-sha>` (`-dirty-<time>` if `backend/` or `frontend/` has uncommitted changes).
2. Uploads `docker-compose.prod.yml`, `.env.prod.example` and `deploy/{Caddyfile,deploy.sh,backup.sh}`.
3. `docker save` → `scp -C` → `docker load` on the server (the whole images each time, a few hundred MB;
   GitHub Actions only transfers changed layers).
4. `bash deploy/deploy.sh zaokaiy-api:<tag> zaokaiy-web:<tag>`.

Other switches: `-FilesOnly` (config files only), `-SkipBuild -Tag <tag>` (re-ship images you already built).

### B) Automatic: GitHub Actions

Every push to `master` runs [.github/workflows/deploy.yml](.github/workflows/deploy.yml):

1. **check** — Rust unit tests and the Nuxt typecheck.
2. **build** — builds the API and web images in GitHub and pushes them to
   `ghcr.io/<owner>/zaokaiy-api` and `zaokaiy-web`, tagged with the commit SHA.
3. **deploy** — sparse-checks out only the runtime files, copies them to the server with scp (no git checkout on
   the server), then over SSH logs in to ghcr.io with the run's token, pulls the two images and runs
   `deploy/deploy.sh`.

Changes only under `pos-desktop/`, `docs/`, `scripts/` or `*.md` don't deploy. Run it by hand from
*Actions → Deploy → Run workflow* (tick *Skip tests* for an emergency fix).

One-time setup:

1. The server has `/var/serv/zaokaiy` with `.env.prod` filled in (steps 5–6).
2. A key GitHub Actions can log in with:
   ```bash
   ssh-keygen -t ed25519 -N '' -f gh-deploy -C github-actions   # on your PC
   ssh-copy-id -i gh-deploy.pub root@YOUR_VPS_IP
   ```
3. GitHub → repo → *Settings → Secrets and variables → Actions*:
   - secrets: `SERVER_HOST` (VPS IP), `SERVER_USER` (`root`), `SERVER_SSH_KEY` (the contents of `gh-deploy`),
     optional `SERVER_SSH_PASSPHRASE`, optional `SERVER_PORT` if SSH isn't on 22
   - variables (optional): `DEPLOY_PATH` if not `/var/serv/zaokaiy`, `SITE_URL` (`https://DOMAIN`)
     for the link on each deploy
4. Optional: *Settings → Environments → production → Required reviewers* to approve each deploy before it runs.

The images are private packages; each deploy logs the server in to ghcr.io with the run's own token and logs
out afterwards, so the server needs no GitHub token of its own.

### Rollback

```bash
cd /var/serv/zaokaiy && bash deploy/deploy.sh rollback      # or .\deploy\ship.ps1 -Rollback
```

Database migrations are not undone by a rollback — keep migrations backward compatible (add columns, don't
rename or drop them in the same release that stops using them), and restore a backup (step 11) if you must.

### Moving an existing server off source code

If the server still has the old git checkout (or a copied source tree), deploy once with A or B, check
`zk ps` / `curl -fsS http://127.0.0.1:8081/api/health`, then remove everything that isn't a runtime file.
Volumes (`zaokaiy_pgdata`, `zaokaiy_media`, ...) are named by the compose project, not the folder, so data is safe.

```bash
cd /var/serv/zaokaiy
find . -mindepth 1 -maxdepth 1 ! -name .env.prod ! -name .env.prod.example ! -name docker-compose.prod.yml ! -name deploy -exec rm -rf {} +
find deploy -mindepth 1 ! -name Caddyfile ! -name deploy.sh ! -name backup.sh ! -name '.release*.env' ! -name .caddyfile.sha256 -exec rm -rf {} +
docker image rm zaokaiy-api zaokaiy-web 2>/dev/null   # old images built on the server
docker builder prune -af                                # build cache holds copies of the source
```

## 11. Backups

```bash
chmod +x /var/serv/zaokaiy/deploy/backup.sh
crontab -e
# add:
30 3 * * * /var/serv/zaokaiy/deploy/backup.sh >> /var/log/zaokaiy-backup.log 2>&1
```

This keeps 14 days of database dumps and media archives in `/var/backups/zaokaiy`. Copy them off the server too (Hostinger's weekly VPS snapshots are a good second layer).

Restore a database dump:

```bash
zk exec -T db pg_restore -U zaokaiy -d zaokaiy --clean --if-exists < /var/backups/zaokaiy/db-YYYYMMDD-HHMMSS.dump
```

## About Hostinger's Docker Manager

Docker Manager (hPanel → VPS → Docker Manager) deploys compose projects from a URL or pasted YAML. `docker-compose.prod.yml` has no `build:` sections; it only runs prebuilt images. The GitHub Actions deploy (step 10) pushes them to GHCR (`ghcr.io/<owner>/zaokaiy-api:<sha>` and `zaokaiy-web`), which Docker Manager could also pull if you set `API_IMAGE` / `WEB_IMAGE`.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Browser shows a certificate error / Caddy logs `challenge failed` | DNS doesn't point at the VPS yet, or ports 80/443 are closed in the Hostinger firewall. Fix, then `zk restart caddy`. |
| `502 Bad Gateway` right after start | API or web still starting; check `zk logs api web`. |
| Build killed / `signal 9` during `cargo build` (on your PC) | Give Docker Desktop more memory (Settings → Resources, 6 GB+). |
| `API_IMAGE not set` from `docker compose` | Add `--env-file deploy/.release.env` (the `zk` alias does), or run `deploy/deploy.sh`. |
| `ship.ps1`: `scp`/`ssh` not found | Windows Settings → Optional features → add *OpenSSH Client*. |
| Uploads fail for large videos | Raise `MEDIA_MAX_VIDEO_MB` in `.env.prod`, then `zk up -d`. |
| `/admin` shows "forbidden" | The account's email must be in `ADMIN_EMAILS`; restart the API after changing it. |
| Lao text shows as boxes | Fonts load from Google Fonts in the browser; check that the visitor's network allows `fonts.googleapis.com`. |
