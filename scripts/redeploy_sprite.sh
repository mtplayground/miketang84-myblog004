#!/usr/bin/env bash
set -euo pipefail

SPRITE=${SPRITE:-/root/.local/bin/sprite}
APP=${APP:-miketang84-myblog004-14e034}
WORKDIR=${WORKDIR:-/workspace}

cd "$WORKDIR"

tar -C "$WORKDIR" -cf - \
  Cargo.toml Cargo.lock .cargo migrations src static templates content \
  | "$SPRITE" exec -s "$APP" -- bash -lc 'cd /opt/app && tar -xf -'

"$SPRITE" exec -s "$APP" -- bash -lc 'cd /opt/app && cargo build --release'
"$SPRITE" exec -s "$APP" -- bash -lc 'sudo sh -c ": > /.sprite/logs/services/app.log"'
"$SPRITE" api "/sprites/${APP}/services/app/stop" -X POST -L >/dev/null || true
sleep 2
"$SPRITE" api "/sprites/${APP}/services/app/start" -X POST -L >/dev/null

DEPLOY_URL=$("$SPRITE" api "/sprites/${APP}" 2>/dev/null | grep -oE 'https://[a-z0-9-]+\.sprites\.app' | head -1)
[ -n "$DEPLOY_URL" ]
printf '%s\n' "$DEPLOY_URL" > "$WORKDIR/.deploy_url"
