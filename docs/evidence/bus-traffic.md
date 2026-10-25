# Bus traffic and age-specific fare comparison

`compare_travel_options` accepts optional fields on both its HTTP and MCP inputs. Existing inputs keep their duration and fare behavior. This is caller-driven simulation: `realtime=false`, `validated_prediction=false`; neither provenance text nor timestamps mean the server verified live traffic or fares.

## Input contract

- Request `require_bus_seat` (default false): Bus/Mixed candidates need a fresh positive `remaining_seats`. Missing or stale seat evidence cannot satisfy this preference.
- Request `avoid_crowded_bus` (default false): a fresh `crowded` observation excludes the candidate. There is no invented crowding time penalty or probability.
- Bus/Mixed option `bus_observation`: `{observation:{source,observed_at,valid_for_seconds},crowding,remaining_seats}`. Source is nonblank caller provenance, timestamp includes an offset, validity is 1–3600 seconds, optional seats are 0–200. Crowding is `available`, `crowded`, or `boarding_unavailable`. The last means boarding capacity is unavailable and excludes the option without inventing a next-bus wait. Zero seats alone still permits standing. Unavailable boarding and positive seats conflict and are rejected.
- Bus/Mixed option `road_traffic`: `{kind:"observed",observation:{source,observed_at,valid_for_seconds},additional_seconds}` or `{kind:"synthetic",additional_seconds:600}`. The single tagged value prevents simultaneous observed and synthetic delay on the same option. Additional seconds are 0–86400, **strictly incremental over `total_minutes`**. Traffic already included by the route provider must not be submitted again. No automatic duplicate detection is possible without the caller identifying the baseline correctly.
- Observations apply only when `depart_at` is within the inclusive observation validity window. Missing departure, future observation or expiry ignores the observation and emits a warning. Synthetic delay needs no observation timestamp. Applied data is returned in `applied_bus_observation`, `applied_road_traffic`, and `road_delay_seconds`; ignored data is null/zero. Subway/Walk reject bus and road fields.
- Request `fare_profile`: `adult` (default), `youth`, `child`. Option `fares_won`: `{source,adult,youth,child}`, with optional amounts 0–1000000 won and nonblank source. The selected amount appears in candidate `cost_won`, with `fare_profile`, `fare_source`, and `fare_verified=false`. Legacy `cost_won` is an adult fallback only. Missing youth/child amounts produce null and a warning; no percentage discount, age, or eligibility is inferred. Walking is free only when an explicit zero quote is supplied for that profile. This is per comparison request; saved routines and `get_commute_brief` do not adopt the profile.

Synthetic example: `examples/bus-traffic.json` explicitly supplies a 600-second additional bus delay and separately labeled adult/youth/child fixture quotes. `examples/travel-options.json` retains the original baseline. These are not real tariffs or observed delays. Road delay changes total duration, arrival, target feasibility, ordering, and the existing switching threshold. Fare breaks otherwise equal comparisons under the existing ordering.

## Recorded checks

| Scenario | Invocation | Binary observable | Artifact |
|---|---|---|---|
| Fresh observed road congestion changes choice, no minute rounding/double addition | `cargo test options --lib` | 20-minute bus + 301 seconds = 1501 seconds; 25-minute walk selected | `.omo/evidence/bus-traffic/unit-tests.log` |
| Unavailable bus boarding, zero seats versus standing, crowd avoidance | same | Unavailable/crowd-avoided bus excluded; zero seats keeps bus unless seat required | same |
| Stale, future and undated observations | same | Baseline bus 1200 seconds selected; applied observations absent | same |
| Invalid seats, delay, timestamp, fare and non-bus traffic | same | Comparison returns error | same |
| Adult/youth/child quotes and missing reduced fares | same | Explicit 1700/900/500 quote selected; absent youth/child remains null | same |
| Legacy comparison behavior | `cargo test --test options` | Existing three scenarios pass unchanged | `.omo/evidence/bus-traffic/mcp-tests.log` |
| Real public MCP router, synthetic delay and child fare | same | HTTP 200; high-congestion choice changes from bus to subway; bus 3240 seconds, arrival 08:54, target false, fare 400, synthetic tag | same |

Focused results: 16 library comparison tests and 4 integration tests passed. Automatic LSP feedback timed out; compiler/test execution provided diagnostics. Loop status discovery failed with an executable permission error, so evidence uses `.omo/evidence/` as fallback. No production runtime, actual traffic APIs, tariff verification, saved routine, deployment, or real alarm was exercised.
