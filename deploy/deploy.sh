#!/usr/bin/env bash
# Start a prebuilt release — or roll back. The server holds no source code, only:
#   docker-compose.prod.yml  .env.prod  deploy/{Caddyfile,deploy.sh,backup.sh}
#
#   deploy/deploy.sh <api-image> <web-image>   start these images, wait for /api/health
#   deploy/deploy.sh rollback                  go back to the release that ran before the last deploy
#
# Images come from GitHub Actions (ghcr.io/..., pulled here) or from deploy/ship.ps1 on your PC
# (docker-loaded onto the server before this runs, so nothing is pulled).
# The running images are kept in deploy/.release.env (API_IMAGE / WEB_IMAGE), so
# `docker compose ... --env-file deploy/.release.env` keeps using them for later manual commands.
# If the new release isn't healthy within HEALTH_TIMEOUT seconds, the previous one is restored.
set -euo pipefail
cd "$(dirname "$0")/.."

RELEASE=deploy/.release.env
PREV=deploy/.release.prev.env
CADDY_SUM=deploy/.caddyfile.sha256
HEALTH_URL=${HEALTH_URL:-http://127.0.0.1:8081/api/health}
HEALTH_TIMEOUT=${HEALTH_TIMEOUT:-180} # first start runs database migrations
KEEP_RELEASES=${KEEP_RELEASES:-3}     # image tags kept on disk for rollback

[[ -f .env.prod ]] || { echo "missing $(pwd)/.env.prod — copy .env.prod.example and fill it in (DEPLOY.md step 6)" >&2; exit 1; }
touch "$RELEASE"
compose() { docker compose -f docker-compose.prod.yml --env-file .env.prod --env-file "$RELEASE" "$@"; }
release_images() { sed -n 's/^\(API\|WEB\)_IMAGE=//p' "$@" 2>/dev/null || true; }

start() {
  # validate required .env.prod values before downloading images
  compose config --quiet
  # pull only what isn't on the server yet (tags are commit SHAs, so a present tag is the right one)
  local img
  for img in $(release_images "$RELEASE"); do
    docker image inspect "$img" > /dev/null 2>&1 || docker pull "$img"
  done
  compose up -d --no-build --remove-orphans
  # Caddyfile is bind-mounted as a file; an uploaded copy is a new inode the container can't see
  local sum; sum=$(sha256sum deploy/Caddyfile | cut -d' ' -f1)
  if [[ "$(cat "$CADDY_SUM" 2>/dev/null)" != "$sum" ]]; then
    compose up -d --no-deps --force-recreate caddy
    echo "$sum" > "$CADDY_SUM"
  fi
}

healthy() {
  local waited=0
  until curl -fsS --max-time 5 "$HEALTH_URL" > /dev/null 2>&1; do
    if (( waited >= HEALTH_TIMEOUT )); then return 1; fi
    sleep 5
    waited=$((waited + 5))
  done
  # the web app must answer too (SSR through Caddy)
  curl -fsS --max-time 10 -o /dev/null "${HEALTH_URL%/api/health}/"
}

# delete old zaokaiy-api / zaokaiy-web tags, keeping the current, the previous and the newest few
prune_releases() {
  local keep; keep=$(release_images "$RELEASE" "$PREV")
  local repo
  for repo in $(docker image ls --format '{{.Repository}}' | grep -E '(^|/)zaokaiy-(api|web)$' | sort -u); do
    docker image ls "$repo" --format '{{.Repository}}:{{.Tag}}' | tail -n +$((KEEP_RELEASES + 1)) |
      while read -r img; do grep -qxF "$img" <<< "$keep" || docker image rm "$img" > /dev/null 2>&1 || true; done
  done
  docker image prune -f > /dev/null
}

if [[ "${1:-}" == "rollback" ]]; then
  [[ -s "$PREV" ]] || { echo "no previous release to roll back to" >&2; exit 1; }
  cp "$RELEASE" "$RELEASE.tmp" && cp "$PREV" "$RELEASE" && mv "$RELEASE.tmp" "$PREV"
  echo "rolling back to: $(tr '\n' ' ' < "$RELEASE")"
  start
  healthy && echo "rollback ok" || { echo "rollback started but the health check fails" >&2; exit 1; }
  exit 0
fi

[[ $# -eq 2 ]] || { echo "usage: $0 <api-image> <web-image> | rollback" >&2; exit 2; }
touch "$PREV" && cp "$PREV" "$PREV.bak"
cp "$RELEASE" "$PREV"
printf 'API_IMAGE=%s\nWEB_IMAGE=%s\n' "$1" "$2" > "$RELEASE"
echo "deploying: $1 $2"
start

if healthy; then
  echo "$(date -Is) deploy ok: $1 $2"
  rm -f "$PREV.bak"
  prune_releases
  exit 0
fi

echo "new release is not healthy — recent api logs:" >&2
compose logs --tail=80 api >&2 || true
if [[ -s "$PREV" ]]; then
  echo "restoring the previous release" >&2
  cp "$PREV" "$RELEASE"
  mv "$PREV.bak" "$PREV" # the failed release never ran; keep the older rollback target
  start
fi
exit 1
