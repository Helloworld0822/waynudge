#!/usr/bin/env bash
set -euo pipefail
umask 077
cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

if [[ -e .env || -L .env ]]; then
  if [[ ! -f .env || -L .env || ! -O .env ]]; then
    echo '.env는 현재 사용자가 소유한 일반 파일이어야 합니다.' >&2
    exit 1
  fi
  mode=$(stat -c '%a' .env)
  if (( (8#$mode & 077) != 0 )); then
    echo 'chmod 600 .env 실행 후 다시 시작하세요.' >&2
    exit 1
  fi
  # .env는 셸 코드이므로 직접 작성하고 신뢰하는 로컬 파일만 사용합니다.
  set -a
  source ./.env
  set +a
fi

: "${WAYNUDGE_USERS:?사용자별 고유 토큰을 담은 WAYNUDGE_USERS JSON을 설정하세요.}"
exec cargo run --locked
