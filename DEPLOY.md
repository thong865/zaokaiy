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

- **Plan:** KVM 2 (2 vCPU, 8 GB RAM) is comfortable. KVM 1 (4 GB) works, but add swap (step 4) because compiling the Rust API needs about 3 GB of RAM.
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

# 4 GB swap, needed on KVM 1 for the Rust build
fallocate -l 4G /swapfile && chmod 600 /swapfile && mkswap /swapfile && swapon /swapfile
echo '/swapfile none swap sw 0 0' >> /etc/fstab
```

## 5. Upload the code

**Option A — Git (recommended).** Push the project to a private GitHub repo, add the server's key as a *deploy key*, then:

```bash
git clone git@github.com:YOU/zaokaiy.git /opt/zaokaiy
```

**Option B — copy from Windows** (PowerShell, from `D:\app`):

```powershell
tar --exclude=node_modules --exclude=target --exclude=.nuxt --exclude=.output --exclude=media --exclude=.env --exclude=*.tar.gz -czf zaokaiy.tgz zaokaiy
scp zaokaiy.tgz root@YOUR_VPS_IP:/opt/
ssh root@YOUR_VPS_IP "cd /opt && tar xzf zaokaiy.tgz && rm zaokaiy.tgz"
```

## 6. Configure

```bash
cd /opt/zaokaiy
cp .env.prod.example .env.prod
sed -i "s/^POSTGRES_PASSWORD=.*/POSTGRES_PASSWORD=$(openssl rand -hex 24)/" .env.prod
sed -i "s/^JWT_SECRET=.*/JWT_SECRET=$(openssl rand -hex 32)/" .env.prod
nano .env.prod      # set DOMAIN, ADMIN_EMAILS, and optionally the AI / social keys
chmod 600 .env.prod
```

`DOMAIN` is the bare host name, e.g. `shop.example.com`. The app, API, CORS, checkout links and media URLs are all derived from it.

## 7. Build and start

```bash
docker compose -f docker-compose.prod.yml --env-file .env.prod up -d --build
```

The first build takes about 5–15 minutes, mostly compiling Rust. Database migrations run automatically when the API starts.

Tip: add an alias to save typing:

```bash
touch /opt/zaokaiy/deploy/.release.env   # image tags of the running release (written by GitHub Actions deploys)
echo "alias zk='docker compose -f /opt/zaokaiy/docker-compose.prod.yml --env-file /opt/zaokaiy/.env.prod --env-file /opt/zaokaiy/deploy/.release.env'" >> ~/.bashrc && source ~/.bashrc
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

### Automatic: GitHub Actions (recommended)

Every push to `master` runs [.github/workflows/deploy.yml](.github/workflows/deploy.yml):

1. **check** — Rust unit tests and the Nuxt typecheck.
2. **build** — builds the API and web images in GitHub (not on the VPS) and pushes them to
   `ghcr.io/<owner>/zaokaiy-api` and `zaokaiy-web`, tagged with the commit SHA.
3. **deploy** — over SSH: fast-forwards the checkout in `/opt/zaokaiy` to that commit, pulls the two images and
   runs `deploy/deploy.sh`, which restarts the stack and waits for `/api/health`. If the new release isn't
   healthy within 3 minutes, the previous release is started again and the run fails.

Changes only under `pos-desktop/`, `docs/`, `scripts/` or `*.md` don't deploy. Run it by hand from
*Actions → Deploy → Run workflow* (tick *Skip tests* for an emergency fix).

One-time setup:

1. The server has the repo cloned at `/opt/zaokaiy` (step 5, option A) and `.env.prod` filled in (step 6).
2. A key GitHub Actions can log in with:
   ```bash
   ssh-keygen -t ed25519 -N '' -f gh-deploy -C github-actions   # on your PC
   ssh-copy-id -i gh-deploy.pub root@YOUR_VPS_IP
   ```
3. GitHub → repo → *Settings → Secrets and variables → Actions*:
   - secrets: `SERVER_HOST` (VPS IP), `SERVER_USER` (`root`), `SERVER_SSH_KEY` (the contents of `gh-deploy`),
     optional `SERVER_PORT` if SSH isn't on 22
   - variables (optional): `DEPLOY_PATH` if the checkout isn't `/opt/zaokaiy`, `SITE_URL` (`https://DOMAIN`)
     for the link on each deploy
4. Optional: *Settings → Environments → production → Required reviewers* to approve each deploy before it runs.

The images are private packages; each deploy logs the server in to ghcr.io with the run's own token and logs
out afterwards, so the server needs no GitHub token of its own.

Roll back to the release before the last deploy:

```bash
cd /opt/zaokaiy && bash deploy/deploy.sh rollback
```

Database migrations are not undone by a rollback — keep migrations backward compatible (add columns, don't
rename or drop them in the same release that stops using them), and restore a backup (step 11) if you must.

### Manual: build on the server

```bash
cd /opt/zaokaiy
git pull                     # or upload a new zaokaiy.tgz as in step 5
: > deploy/.release.env      # forget CI images, so compose builds from source
zk up -d --build             # rebuilds only what changed; migrations run on start
docker image prune -f        # free disk space from old images
```

## 11. Backups

```bash
chmod +x /opt/zaokaiy/deploy/backup.sh
crontab -e
# add:
30 3 * * * /opt/zaokaiy/deploy/backup.sh >> /var/log/zaokaiy-backup.log 2>&1
```

This keeps 14 days of database dumps and media archives in `/var/backups/zaokaiy`. Copy them off the server too (Hostinger's weekly VPS snapshots are a good second layer).

Restore a database dump:

```bash
zk exec -T db pg_restore -U zaokaiy -d zaokaiy --clean --if-exists < /var/backups/zaokaiy/db-YYYYMMDD-HHMMSS.dump
```

## About Hostinger's Docker Manager

Docker Manager (hPanel → VPS → Docker Manager) deploys compose projects from a URL or pasted YAML. Its documentation doesn't say whether it builds from source (the `build:` sections here compile the Rust API and Nuxt app), so the SSH steps above are the reliable route. The GitHub Actions deploy (step 10) already pushes ready-made images to GHCR (`ghcr.io/<owner>/zaokaiy-api:<sha>` and `zaokaiy-web`), which Docker Manager could also pull.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Browser shows a certificate error / Caddy logs `challenge failed` | DNS doesn't point at the VPS yet, or ports 80/443 are closed in the Hostinger firewall. Fix, then `zk restart caddy`. |
| `502 Bad Gateway` right after start | API or web still starting; check `zk logs api web`. |
| Build killed / `signal 9` during `cargo build` | Out of memory; add swap (step 4). |
| Uploads fail for large videos | Raise `MEDIA_MAX_VIDEO_MB` in `.env.prod`, then `zk up -d`. |
| `/admin` shows "forbidden" | The account's email must be in `ADMIN_EMAILS`; restart the API after changing it. |
| Lao text shows as boxes | Fonts load from Google Fonts in the browser; check that the visitor's network allows `fonts.googleapis.com`. |
