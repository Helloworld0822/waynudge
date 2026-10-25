#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "Usage: $0 [--apply] [--rollback] CANDIDATE TARGET USER_SERVICE HEALTH_URL" >&2
  exit 2
}
apply=false
rollback=false
while [[ ${1:-} == --* ]]; do
  case $1 in
    --apply) apply=true ;;
    --rollback) rollback=true ;;
    *) usage ;;
  esac
  shift
done
[[ $# == 4 ]] || usage
candidate=$1 target=$2 service=$3 health_url=$4
[[ $target == /* && $service != -* && $service == *.service ]] || usage
[[ $health_url == http://127.0.0.1:*/* || $health_url == http://localhost:*/* ]] || usage
if $rollback; then candidate=$target.previous; fi
[[ -f $candidate && ! -L $candidate && -s $candidate ]] || { echo 'Candidate must be a nonempty regular file' >&2; exit 2; }
[[ -f $target && ! -L $target && -x $target ]] || { echo 'Target must be an existing executable regular file' >&2; exit 2; }
[[ ! -L $target.previous ]] || { echo 'Backup cannot be a symlink' >&2; exit 2; }
if ! $apply; then
  printf 'DRY RUN: install %s at %s; restart user service %s; check %s\n' "$candidate" "$target" "$service" "$health_url"
  exit 0
fi
: "${WAYNUDGE_HEALTH_TOKEN:?Set WAYNUDGE_HEALTH_TOKEN for authenticated health checks}"
[[ $WAYNUDGE_HEALTH_TOKEN != *$'\n'* && $WAYNUDGE_HEALTH_TOKEN != *$'\r'* ]] || { echo 'Health token contains a newline' >&2; exit 2; }
exec 9>"$target.deploy.lock"
flock -n 9 || { echo 'Another deployment is active' >&2; exit 1; }
stage=$(mktemp "${target}.stage.XXXXXX")
backup=$(mktemp "${target}.backup.XXXXXX")
changed=false
healthy() {
  local attempt
  for attempt in {1..10}; do
    if systemctl --user is-active --quiet "$service" && printf 'Authorization: Bearer %s\n' "$WAYNUDGE_HEALTH_TOKEN" | curl --header @- --fail --silent --max-time 2 "$health_url" >/dev/null; then return 0; fi
    sleep 1
  done
  return 1
}
cleanup() {
  local result=$?
  trap - EXIT INT TERM
  if $changed && (( result != 0 )); then
    echo 'Deployment failed; restoring previous binary' >&2
    if mv -f -- "$backup" "$target" && systemctl --user restart "$service" && healthy; then
      echo 'ROLLBACK_OK' >&2
    else
      echo 'ROLLBACK_FAILED: operator intervention required' >&2
    fi
  fi
  rm -f -- "$stage" "$backup"
  exit "$result"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
install -m 755 -- "$candidate" "$stage"
cp -p -- "$target" "$backup"
# Copy before replacing so rollback input remains available until staged.
cp -p -- "$backup" "$target.previous"
mv -f -- "$stage" "$target"
changed=true
systemctl --user restart "$service"
healthy
changed=false
echo 'DEPLOY_OK'
