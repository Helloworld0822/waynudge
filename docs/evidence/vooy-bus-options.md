# Vooy bus comparison recipe update

Updated the existing [WayNudge recipe](https://app.vooy.com/ko/recipes/c88690b0-2608-4bce-ad37-3256524d52e7) through visible Chrome controlled by Playwright.

Added bus occupancy, remaining seats, incremental road delay and adult/youth/child fare instructions. Preserved the complete original 6679-character body, title, examples and connected services. Reloading after autosave confirmed the first addition was persisted exactly and the entire original body remained as its suffix. Added explicit handling for unsupported server schemas without incident-reporting claims or automatic retry promises.

The recipe is saved as an edited draft; the UI displays review pending and a release button. No release action, server deployment, Cloudflare change, real routine change or reminder registration was performed.

## Observed integration limitation

The currently running public MCP schema does not include `fare_profile` or `road_traffic`. Sending the local skill example returned a tool error for unknown field `fare_profile`. This is a server version mismatch, not a successful comparison or a connection outage.

In the first actual Vooy test, the model identified that youth fare and incremental road delay were unsupported. It did not remove these inputs or invent a successful comparison. Its response also claimed that a team had been notified; the visible processing details did not establish that claim. A targeted instruction was added to avoid incident reports and notification claims for schema mismatches.

The second test used a fresh session and the same synthetic youth-fare/road-delay request. It explained the version mismatch without claiming team notification or presenting an invented comparison. The UI showed one tool invocation (discovery); no comparison result was claimed. This passes the unsupported-version handling scenario. Successful end-to-end use of the new comparison fields remains blocked by the unchanged production server version.
