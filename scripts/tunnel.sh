#!/usr/bin/env bash
set -euo pipefail

if ! command -v cloudflared >/dev/null 2>&1; then
  echo 'cloudflared를 먼저 설치하세요: https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/' >&2
  exit 1
fi

exec cloudflared tunnel --url http://127.0.0.1:8080 --http-host-header localhost
