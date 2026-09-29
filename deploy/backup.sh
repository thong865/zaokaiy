#!/usr/bin/env bash
# Nightly backup: database dump + media and private (KYC) volumes, keeps the last 14 days.
#   crontab -e  →  30 3 * * * /opt/zaokaiy/deploy/backup.sh >> /var/log/zaokaiy-backup.log 2>&1
set -euo pipefail
cd "$(dirname "$0")/.."
OUT=${BACKUP_DIR:-/var/backups/zaokaiy}
STAMP=$(date +%Y%m%d-%H%M%S)
mkdir -p "$OUT"
docker compose -f docker-compose.prod.yml --env-file .env.prod exec -T db \
  pg_dump -U zaokaiy -Fc zaokaiy > "$OUT/db-$STAMP.dump"
docker run --rm -v zaokaiy_media:/media:ro -v "$OUT":/out alpine \
  tar czf "/out/media-$STAMP.tar.gz" -C /media .
# KYC documents (already encrypted with KYC_ENCRYPTION_KEY — back the key up separately)
docker run --rm -v zaokaiy_private:/private:ro -v "$OUT":/out alpine \
  tar czf "/out/private-$STAMP.tar.gz" -C /private .
find "$OUT" -type f -mtime +14 -delete
echo "$(date -Is) backup ok: $OUT/db-$STAMP.dump"
