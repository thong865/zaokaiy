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
echo "alias zk='docker compose -f /opt/zaokaiy/docker-compose.prod.yml --env-file /opt/zaokaiy/.env.prod'" >> ~/.bashrc && source ~/.bashrc
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

## 10. Updating

```bash
cd /opt/zaokaiy
git pull                     # or upload a new zaokaiy.tgz as in step 5
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

Docker Manager (hPanel → VPS → Docker Manager) deploys compose projects from a URL or pasted YAML. Its documentation doesn't say whether it builds from source (the `build:` sections here compile the Rust API and Nuxt app), so the SSH steps above are the reliable route. To use Docker Manager or GitHub Actions later, build the two images in CI, push them to a registry (e.g. GHCR), and replace `build:` with `image:` in `docker-compose.prod.yml`.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Browser shows a certificate error / Caddy logs `challenge failed` | DNS doesn't point at the VPS yet, or ports 80/443 are closed in the Hostinger firewall. Fix, then `zk restart caddy`. |
| `502 Bad Gateway` right after start | API or web still starting; check `zk logs api web`. |
| Build killed / `signal 9` during `cargo build` | Out of memory; add swap (step 4). |
| Uploads fail for large videos | Raise `MEDIA_MAX_VIDEO_MB` in `.env.prod`, then `zk up -d`. |
| `/admin` shows "forbidden" | The account's email must be in `ADMIN_EMAILS`; restart the API after changing it. |
| Lao text shows as boxes | Fonts load from Google Fonts in the browser; check that the visitor's network allows `fonts.googleapis.com`. |
