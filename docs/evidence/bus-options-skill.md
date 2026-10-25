# Bus options skill verification

Added `.agents/skills/compare-bus-options/SKILL.md` and linked it from the existing recipe skill. The new skill covers caller-supplied bus occupancy, road delay and adult/youth/child fares without changing running services or Vooy.

## Executed verification

Built with `cargo build --locked`. Extracted the JSON code block directly from the new SKILL.md, started `target/debug/waynudge` on an ephemeral loopback port with an in-memory SQLite database, mock provider, test-only credentials and public MCP. Sent actual HTTP JSON-RPC requests using Python's standard-library HTTP client. The isolated process was terminated after verification.

| Case | Observed result |
| --- | --- |
| MCP tool discovery | `compare_travel_options` present |
| Verbatim youth example | Bus 1800 seconds, 600 seconds additional road delay, 900 won; arrival 08:30 +09:00; 25-minute walk selected |
| Adult quote | 1500 won |
| Child quote | 500 won |
| Missing youth quote with adult `cost_won=1500` | Youth cost null |
| Fresh `boarding_unavailable`, zero seats | Bus excluded; walk remains |
| Expired unavailable-boarding observation | Bus retained; observation not applied; warning returned |
| Negative additional road seconds | MCP error returned |

All eight checks passed. Successful reports retained `source=simulation`, `realtime=false`, `validated_prediction=false`; the example fare retained `fare_verified=false`. The repository example link was resolved to an existing file.

The bundled `skill-creator/scripts/quick_validate.py` reported `Skill is valid!` for both the new skill and the linked recipe skill (run through `uv run --with pyyaml`). `cargo test --locked --test options` passed all five integration tests. `git diff --check` passed.

This verifies the skill example and described server behavior, not automatic model skill selection or an end-to-end Vooy chat. No live API, real fare validation, production restart, deployment, location record, or reminder was involved.
