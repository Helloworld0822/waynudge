#!/usr/bin/env bash
set -euo pipefail
script=$(cd "$(dirname "$0")" && pwd)/install-binary.sh
sandbox=$(mktemp -d)
trap 'rm -rf "$sandbox"' EXIT
mkdir "$sandbox/bin"
export WAYNUDGE_HEALTH_TOKEN=sandbox-only-token
export TEST_TARGET="$sandbox/waynudge" TEST_CALLS="$sandbox/calls"
printf 'old\n' > "$TEST_TARGET"
chmod +x "$TEST_TARGET"
printf 'new\n' > "$sandbox/candidate"
cat > "$sandbox/bin/systemctl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$*" >> "$TEST_CALLS"
if [[ ${FAIL_RESTART:-false} == true && $* == *restart* && $(cat "$TEST_TARGET") == new ]]; then exit 1; fi
MOCK
cat > "$sandbox/bin/curl" <<'MOCK'
#!/usr/bin/env bash
set -euo pipefail
[[ $* == *'--header @-'* ]]
IFS= read -r header
[[ $header == "Authorization: Bearer $WAYNUDGE_HEALTH_TOKEN" ]]
[[ $(cat "$TEST_TARGET") != bad ]]
MOCK
printf '#!/usr/bin/env bash\nexit 0\n' > "$sandbox/bin/sleep"
chmod +x "$sandbox/bin/"*
export PATH="$sandbox/bin:$PATH"
args=("$sandbox/candidate" "$TEST_TARGET" waynudge-test.service http://127.0.0.1:8080/health)
"$script" "${args[@]}"
[[ $(cat "$TEST_TARGET") == old && ! -e $TEST_TARGET.previous && ! -e $TEST_CALLS && ! -e $TEST_TARGET.deploy.lock ]]
echo 'PASS dry-run: no binary, backup, lock or service mutation'
"$script" --apply "${args[@]}"
[[ $(cat "$TEST_TARGET") == new && $(cat "$TEST_TARGET.previous") == old ]]
echo 'PASS install: candidate installed, prior binary backed up, auth header received on stdin'
"$script" --apply --rollback "${args[@]}"
[[ $(cat "$TEST_TARGET") == old && $(cat "$TEST_TARGET.previous") == new ]]
echo 'PASS explicit rollback: previous binary restored'
printf 'bad\n' > "$sandbox/candidate"
if "$script" --apply "${args[@]}"; then echo 'Unhealthy deployment unexpectedly passed' >&2; exit 1; fi
[[ $(cat "$TEST_TARGET") == old ]]
echo 'PASS health failure: command fails and previous binary restored'
printf 'new\n' > "$sandbox/candidate"
if FAIL_RESTART=true "$script" --apply "${args[@]}"; then echo 'Failed restart unexpectedly passed' >&2; exit 1; fi
[[ $(cat "$TEST_TARGET") == old ]]
echo 'PASS restart failure: command fails and previous binary restored'
ln -s "$sandbox/candidate" "$sandbox/link"
if "$script" --apply "$sandbox/link" "${args[@]:1}"; then echo 'Symlink unexpectedly accepted' >&2; exit 1; fi
[[ $(cat "$TEST_TARGET") == old ]]
echo 'PASS invalid candidate: symlink rejected without target mutation'
