#!/usr/bin/env bash
# Start a release built by GitHub Actions (.github/workflows/deploy.yml) — or roll back.
#
#   deploy/deploy.sh <api-image> <web-image>   pull these images, restart, wait for /api/health
#   deploy/deploy.sh rollback                  go back to the release that ran before the last deploy
#
# The images of the running release are kept in deploy/.release.env (API_IMAGE / WEB_IMAGE), so
# `docker compose ... --env-file deploy/.release.env` keeps using them for later manual commands.
# If the new release isn't healthy within HEALTH_TIMEOUT seconds, the previous one is restored.
set -euo pipefail
cd "$(dirname "$0")/.."

RELEASE=deploy/.release.env
PREV=deploy/.release.prev.env
HEALTH_URL=${HEALTH_URL:-http://127.0.0.1:8081/api/health}
HEALTH_TIMEOUT=${HEALTH_TIMEOUT:-180} # first start runs database migrations
touch "$RELEASE"
compose() { docker compose -f docker-compose.prod.yml --env-file .env.prod --env-file "$RELEASE" "$@"; }

start() {
  compose pull api web
  compose up -d --no-build --remove-orphans
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

if [[ "${1:-}" == "rollback" ]]; then
  [[ -s "$PREV" ]] || { echo "no previous release to roll back to" >&2; exit 1; }
  cp "$RELEASE" "$RELEASE.tmp" && cp "$PREV" "$RELEASE" && mv "$RELEASE.tmp" "$PREV"
  echo "rolling back to: $(tr '\n' ' ' < "$RELEASE")"
  start
  healthy && echo "rollback ok" || { echo "rollback started but the health check fails" >&2; exit 1; }
  exit 0
fi

[[ $# -eq 2 ]] || { echo "usage: $0 <api-image> <web-image> | rollback" >&2; exit 2; }
cp "$RELEASE" "$PREV"
printf 'API_IMAGE=%s\nWEB_IMAGE=%s\n' "$1" "$2" > "$RELEASE"
echo "deploying: $1 $2"
start

if healthy; then
  echo "$(date -Is) deploy ok: $1 $2"
  docker image prune -f > /dev/null
  exit 0
fi

echo "new release is not healthy — recent api logs:" >&2
compose logs --tail=80 api >&2 || true
if [[ -s "$PREV" ]]; then
  echo "restoring the previous release" >&2
  cp "$PREV" "$RELEASE"
  start
fi
exit 1
