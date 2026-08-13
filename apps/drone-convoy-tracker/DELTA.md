# Delta — dashboard goes live (Stage 2 of 2)

Overlay on top of `drone-convoy-tracker-delta-server-live.zip` (Stage 1).
This delta rewires the Leptos frontend onto the now-real GraphQL queries.
**`seed_static_panels()` is deleted.** Nothing on screen is faked and nothing
is stale: every panel — leaderboard, drone cards, convoy status, telemetry
chart, engagement feed, footer counters, and the map's airframes — is driven
by a 2-second poll against ScyllaDB-backed resolvers. An empty panel now means
an empty table; start the simulator.

Nothing deleted at file level. Overlay over the repo root.

## Apply

    unzip -o drone-convoy-tracker-delta-frontend-live.zip -d <repo-root>
    make build
    make serve            # deps + API + trunk
    make run-simulator    # second terminal

Load `http://localhost:3000`. Expected behavior, in order:

1. **ONLINE within ~2s of page load.** The dashboard selects the well-known
   demo convoy id synchronously at startup, so the first leaderboard poll
   fires on the first tick. The old flow awaited an `activeConvoys` round
   trip before polling anything — that await was the 20–205s OFFLINE you saw
   on every refresh. `activeConvoys` is still fetched in the background and
   overrides the selection if a different convoy is live.
2. **Leaderboard shows "NO ENGAGEMENTS RECORDED"** on a fresh database —
   an explicit empty state, so empty is distinguishable from broken.
3. **Within ~3s of the simulator's bootstrap**: drone cards fill, convoy
   status counts real assets/fuel, airframes join the map one by one (watch
   the console: "map: airframe joined — ALPHA-01"), and the leaderboard and
   engagement feed populate as shots land.
4. **The telemetry chart starts drawing after two polls** (~4s): rolling
   convoy-average altitude and fuel, one point per poll, 30-point window.

## Files

### Rewritten
    apps/drone-convoy-tracker/crates/drone-frontend/src/lib.rs

### Edited
    apps/drone-convoy-tracker/crates/drone-frontend/src/state/mod.rs           (+telemetry_series, +TelemetryPoint)
    apps/drone-convoy-tracker/crates/drone-frontend/src/services/api.rs        (+fetch_drones, +fetch_engagements; fetch_active_convoys now checks errors[])
    apps/drone-convoy-tracker/crates/drone-frontend/src/components/charts.rs   (reactive series; render-once-then-update)
    apps/drone-convoy-tracker/crates/drone-frontend/src/components/leaderboard.rs (empty state)
    apps/drone-convoy-tracker/crates/drone-frontend/src/components/map.rs      (incremental airframe markers)

## What changed and why

- **lib.rs** — `seed_static_panels()` deleted outright; while it existed, a
  regression looked like working software. `start_live_feed()` now: sets the
  demo convoy immediately, fetches `activeConvoys` in the background, and
  runs one 2s tick that polls `leaderboard` + `drones` + `engagements`.
  `ws_connected` (the ONLINE pill) reflects the leaderboard poll succeeding —
  an honest link indicator. Each drones poll also appends one convoy-average
  point to `telemetry_series` (capped at 30). Engagement rows are stamped
  with the shooter's current leaderboard accuracy before display, so the feed
  reads like the subscription event would.
- **api.rs** — `fetch_drones` and `fetch_engagements` follow the exact
  camelCase-`Variables` pattern of `fetch_leaderboard` (the missing
  `#[serde(rename_all = "camelCase")]` once cost a day). `DroneStatus`
  deserializes straight off the GraphQL enum wire form via its
  SCREAMING_SNAKE_CASE serde rename. `fetch_active_convoys` now checks
  `errors[]` before `data` — same lesson as the simulator: GraphQL rejections
  arrive in a 200 OK body.
- **charts.rs** — the hardcoded sample vectors are gone. The Effect reads
  `state.telemetry_series` (reactive: re-runs per appended point), renders
  once, and pushes subsequent frames through `WasmRenderer::update` on the
  kept `Echarts` handle — calling `render()` per tick would echarts-init the
  same DOM node repeatedly and stack instances. X-axis is poll time (HH:MM:SS)
  rather than fictional WP labels. The LIVE badge appears when the series is
  actually flowing.
- **map.rs** — markers were created ONCE at mount from a state snapshot; with
  the seeds gone that snapshot is empty and the map would show zero drones
  forever. Markers are now created incrementally: a 1s sync interval adds an
  airframe for every callsign it hasn't seen (inserted in callsign order so
  formation slots are deterministic regardless of join order) and refreshes
  existing popups so FUEL/ACC track live values. The flight loop animates the
  shared marker list. Route-following animation is unchanged.
- **leaderboard.rs** — explicit "NO ENGAGEMENTS RECORDED" empty state; a bare
  header has been misread as a bug once already.

## Deliberately unchanged (known, not forgotten)

- **`API_URL` stays `http://localhost:8080/graphql`.** Correct for
  `make serve` local dev; wrong in-cluster. The move to same-origin relative
  paths (`/graphql`, `/graphql/ws` behind the chart's HTTPRoute) is its own
  small change and belongs with the container/cluster work, not this delta.
- **The map still flies the client-side `ROUTE` constant.** The `waypoints`
  query is real server-side now; swapping the constant for the query result
  is a follow-up that touches only `map.rs` and nothing else.
- **Polling, not subscriptions.** `/graphql/ws` is mounted server-side
  (Stage 1); the frontend's `use_websocket` exists with zero callers. The
  poll is the fallback either way, so the swap is additive.
- **ENGAGE button** — `record_engagement` in `api.rs` still has zero callers;
  wiring a control plus the explosion SVG is the fun follow-up, on the
  wishlist, untouched here.
