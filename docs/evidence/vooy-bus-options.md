# Vooy bus comparison recipe update

Updated the existing [WayNudge recipe](https://app.vooy.com/ko/recipes/c88690b0-2608-4bce-ad37-3256524d52e7) through visible Chrome controlled by Playwright.

Added bus occupancy, remaining seats, incremental road delay and adult/youth/child fare instructions. Preserved the complete original 6679-character body, title, examples and connected services. Reloading after autosave confirmed the first addition was persisted exactly and the entire original body remained as its suffix. Added explicit handling for unsupported server schemas without incident-reporting claims or automatic retry promises.

The recipe is saved as an edited draft; the UI displays review pending and a release button. No release action, server deployment, Cloudflare change, real routine change or reminder registration was performed.

## Observed integration limitation

The currently running public MCP schema does not include `fare_profile` or `road_traffic`. Sending the local skill example returned a tool error for unknown field `fare_profile`. This is a server version mismatch, not a successful comparison or a connection outage.

In the first actual Vooy test, the model identified that youth fare and incremental road delay were unsupported. It did not remove these inputs or invent a successful comparison. Its response also claimed that a team had been notified; the visible processing details did not establish that claim. A targeted instruction was added to avoid incident reports and notification claims for schema mismatches.

The second test used a fresh session and the same synthetic youth-fare/road-delay request. It explained the version mismatch without claiming team notification or presenting an invented comparison. The UI showed one tool invocation (discovery); no comparison result was claimed. This passes the unsupported-version handling scenario. Successful end-to-end use of the new comparison fields remains blocked by the unchanged production server version.

## Deployment follow-up after user authorization

The user subsequently requested the production update. `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` (96 tests) and `cargo build --locked` passed. Installed the built executable using `scripts/install-binary.sh --apply`, which retained the previous binary, restarted the user service and passed its authenticated loopback health check (`DEPLOY_OK`). Existing environment, database and Cloudflare configuration were preserved.

The previous server-version limitation is now resolved at the public MCP endpoint. Sent the JSON example directly from `.agents/skills/compare-bus-options/SKILL.md` to `https://waynudge.helloworld0822.xyz/mcp`. The real HTTP tool call succeeded: bus total 1800 seconds, incremental road delay 600 seconds, youth fare 900 won, selected option `walk`. The report retained `realtime=false`; these remain explicit synthetic inputs, not live traffic or verified tariffs.

The Vooy recipe remains the saved edited draft described above. No release button was pressed. Its browser conversation was not repeated after deployment; refreshing cached tool schemas may be necessary in Vooy.

## Final revalidation and release

The user explicitly requested revalidation followed by release. Four real public MCP checks passed: adult 1500 won, youth 900 won and child 500 won with the same 600-second synthetic road delay, plus exclusion of a bus marked boarding unavailable.

Vooy initially retained the previous MCP tool schema. Reconnected the same public MCP URL, validated its 14 tools and repaired the recipe's obsolete connection reference. Applied a service-list-only proposal containing current `rmcp` and `카카오맵`. Confirmed the entire 8618-character instruction body was unchanged. After reloading, these were the only two checked services.

The actual Playwright-driven Vooy conversation then returned the correct synthetic comparison: bus 30 minutes including 10 minutes additional road delay, 2 minutes walking, youth fare 900 won; walking alternative 25 minutes and 0 won; walking recommended. It labeled the inputs as synthetic and the fares as unverified, applied the road delay only to the bus and did not claim to save real locations or routines. The UI reported two tool invocations.

Clicked `출시`, retained the `이동수단` category and confirmed `출시하기`. After reloading, the UI displayed `출시`, `검토 대기` and the `출시 취소` button. Release submission is complete; public store listing remains subject to Vooy review. The owned browser was closed after capturing the final UI state.
