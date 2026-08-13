# drone-convoy-tracker

Drone P2P convoy leader status tracking and storage (OLTP + OLAP) in Rust — Tokio,
GraphQL, Redis, ScyllaDB, DuckDB, with a Leptos + Leaflet WASM tactical dashboard.

**Classification: UNCLASSIFIED // FOR OFFICIAL USE ONLY**

---

## Quick start

Three commands, two terminals. Everything runs **natively** — only ScyllaDB and
Redis run in containers.

```bash
make build           # touch sweep, then compile backend + frontend
make serve           # starts ScyllaDB + Redis, then API and dashboard
make run-simulator   # second terminal, once the API logs "Starting HTTP server"
```

| | |
|---|---|
| Dashboard | http://localhost:3000 |
| GraphQL playground | http://localhost:8080/graphql |

`make serve` depends on `deps-up`, so you do not start the databases yourself.
First run pulls the ScyllaDB image and waits up to three minutes for it to accept
CQL, then applies the dev schema automatically.

When you are done:

```bash
make stop            # stop the containers, keep the data
make deps-clean      # remove the containers and their data
```

### Prerequisites

- **Rust ≥ 1.85** — the workspace is `edition = "2024"` and no earlier toolchain
  can parse it.
- **Podman** with a running machine. On macOS every container needs a Linux VM:
  ```bash
  podman machine init --cpus 4 --memory 8192 --disk-size 60
  podman machine start
  ```
  Size it at `init` time; the default machine is too small for ScyllaDB.
- **Trunk** and the WASM target for the frontend. `make setup` installs both, or:
  ```bash
  rustup target add wasm32-unknown-unknown
  cargo install trunk
  ```

`podman-compose` is **not** required for `make serve`. It is only needed for
`make stack-up`, the everything-in-containers path.

---

## Make targets

`make help` prints the front door — the eight verbs you need day to day.
`make help-all` lists everything.

### Front door

| Target | What it does |
|---|---|
| `make serve` | ScyllaDB + Redis, then API (`cargo run --release`) and `trunk serve` in parallel. Ctrl-C stops both. |
| `make stop` | Stop ScyllaDB and Redis. Data preserved. |
| `make logs` | Follow ScyllaDB logs. |
| `make build` | Restamp mtimes, then compile backend and frontend. |
| `make dev` | Same as `serve` but with hot reload. |
| `make test` | Workspace tests. |
| `make lint` | `rustfmt --check` plus clippy. |
| `make clean` | Remove build artifacts. |
| `make touch` | Restamp source mtimes — `build` runs this for you. |

### Dependency containers

| Target | What it does |
|---|---|
| `make deps-up` | Start ScyllaDB and Redis via plain `podman run`, wait for CQL, apply the dev schema. Reuses existing containers. |
| `make stop` | Stop both. Volumes and data survive. |
| `make deps-clean` | `podman rm -f` both. **Destroys the data.** |

`deps-up` deliberately uses `podman run` rather than compose: the app runs
natively, so there is no reason to make `podman-compose` a prerequisite for local
development.

```
scylla   docker.io/scylladb/scylla:6.2   :9042   --smp 2 --memory 2G --developer-mode 1
redis    docker.io/library/redis:7-alpine :6379   --appendonly yes --maxmemory 256mb
```

Images are fully qualified because Podman has no implicit registry — a bare
`redis:7-alpine` either prompts or fails under a locked `registries.conf`.

### Database

| Target | What it does |
|---|---|
| `make db-init` | Apply the dev schema (alias for `db-init-dev`). |
| `make db-init-dev` | Keyspace with `SimpleStrategy` + core schema. |
| `make db-init-prod` | Keyspace with `NetworkTopologyStrategy`. Datacenters must exist. |
| `make db-reset` | **Drops** and recreates the `drone_ops` keyspace. |
| `make db-status` | `DESCRIBE KEYSPACES`. |
| `make db-shell` | `cqlsh` inside the Scylla container. |
| `make db-shell-ops` | `cqlsh -k drone_ops`. |
| `make redis-cli` | `redis-cli` inside the Redis container. |

### Build and run

| Target | What it does |
|---|---|
| `make all` | Backend and frontend release builds. |
| `make build-backend` / `build-frontend` | One side only. |
| `make build-api` / `build-simulator` | A single package. |
| `make build-debug` / `make quick` | Debug profile, faster iteration. |
| `make run-api` / `run-api-release` | API alone. |
| `make run-simulator` / `run-simulator-release` | Simulator alone. |
| `make dev-backend` / `dev-frontend` / `dev-db` / `dev-stop` | Individual dev pieces. |

### Containers and packaging

| Target | What it does |
|---|---|
| `make image` | Build both images with Podman. |
| `make image-api` / `image-frontend` | One image. Honours `RUST_VERSION`. |
| `make stack-up` / `stack-down` | Everything in containers. **Needs `podman-compose`.** |
| `make package` / `make zip` | Distribution artifacts. |

### Quality

`make fmt`, `make fmt-check`, `make clippy`, `make audit`, `make test-unit`,
`make test-integration`, `make docs`, `make docs-open`, `make ci`, `make ci-full`.

---

## Why `make build` touches every file

`build` runs `touch` first, restamping mtimes across `Cargo.toml`, `Cargo.lock`,
`crates/`, `config/`, `schema/` and `assets/`.

Unzipping an archive or checking out files restores the **archive's** timestamps.
A file changed five minutes ago can land on disk older than the artifact built
from its previous version — and Cargo and Trunk both decide staleness from
mtimes, so the change compiles away to nothing and you stare at stale output.

The cost: every source becomes newer than every artifact, so `build` is always a
full rebuild, never incremental. Right trade after dropping in changes, wrong one
for a tight edit loop:

```bash
TOUCH=0 make build     # skip the sweep
```

`make dev` never touches — a full rebuild per keystroke would defeat it.
`containers/` is excluded, because Podman keys its `COPY` cache on content, so
restamping there only busts layer cache for nothing.

---

## Configuration

| Variable | Default | Notes |
|---|---|---|
| `SERVER_ADDR` | `0.0.0.0:8080` | Malformed values warn and fall back rather than panicking. |
| `RUST_LOG` | `info,drone_graphql_api=debug` | |
| `SCYLLA_HOSTS` | `localhost:9042` | |
| `SCYLLA_KEYSPACE` | `drone_ops` | |
| `REDIS_URL` | `redis://localhost:6379` | |
| `DRONE_CONVOY_ID` | `550e8400-e29b-41d4-a716-446655440000` | Simulator target convoy. |
| `RUST_VERSION` | `1.89` | Container build arg. Must be ≥ 1.85. |

`DRONE_CONVOY_ID` matters: the API's `activeConvoys` resolver currently returns
that same well-known UUID, so the dashboard, the simulator's writes and the
leaderboard reads all agree on one convoy. A random id there means the simulator
writes to a convoy the UI never asks about — both processes look healthy and every
panel stays empty.

---

## Current state — read before filing a bug

The dashboard is **not fully live yet.** One path reaches storage end to end:

```
simulator --recordEngagement--> API --> ScyllaLeaderboardRepository --> ScyllaDB
                                    --> leaderboard query --> dashboard (2s poll)
```

Live: the accuracy leaderboard, and the drone convoy animating along the sortie
route (client-side).

**Not live yet:** convoy status, telemetry chart, engagement feed, drone cards.
Those resolvers are stubs — `drones` returns one hardcoded aircraft, `engagements`
returns empty, `convoyStats` has literals. `ApiContext` carries only
`leaderboard_repo`, and most repository **read** paths in `drone-persistence` are
unimplemented even though the write paths and every CQL table are real.

GraphQL subscriptions are disabled: the `/graphql/ws` route is commented out in
`drone-graphql-api/src/lib.rs`, so the dashboard polls rather than being pushed to.

See `context-handoff.md` in the repository root for the full inventory and the
ordered plan to finish it.

---

## Architecture

```
┌────────────────────────────────────────────────────────────────────────────────┐
│                          Leptos Frontend (WASM)                                │
│                     Leaflet map + Charming visualisation                        │
└────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│                          Axum + async-graphql API                              │
│                     Queries / Mutations / Subscriptions                        │
└────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│                     drone-persistence (Repository pattern)                     │
│                  Cache-aside strategy / write-through strategy                 │
└────────────────────────────────────────────────────────────────────────────────┘
                          │                              │
                          ▼                              ▼
┌────────────────────────────────────┐    ┌─────────────────────────────────────┐
│           Redis cache              │    │            ScyllaDB                 │
│   • Leaderboard (ZSET)             │    │   • convoys                         │
│   • Drone state (HASH)             │    │   • drones                          │
│   • Latest telemetry               │    │   • waypoints (25/drone)            │
│   • Convoy roster (SET)            │    │   • telemetry (time-series)         │
└────────────────────────────────────┘    │   • engagements                     │
                                          │   • leaderboard                     │
                                          │   • accuracy_counters               │
                                          └─────────────────────────────────────┘
                                                         │
                                                         ▼
                                          ┌─────────────────────────────────────┐
                                          │            DuckDB (OLAP)            │
                                          │       Parquet export analytics      │
                                          └─────────────────────────────────────┘
```

![convoy-tracker-dash](docs/drone-convoy-tracker.png)

![convoy-tracker-dash](docs/drone-convoy-tracker-zoom.png)

---

## Project structure

```
drone-convoy-tracker/
├── Cargo.toml                    # workspace manifest + [profile.release]
├── Makefile
├── assets/images/                # drone.svg, explosion.svg (map markers)
├── config/app.toml
├── containers/
│   ├── Containerfile.api
│   ├── Containerfile.frontend
│   ├── podman-compose.yml        # full stack
│   ├── podman-compose.dev.yml    # databases only
│   ├── nginx.conf
│   └── prometheus.yml
├── schema/cql/
│   ├── 000_keyspace_dev.cql
│   ├── 000_keyspace_prod.cql
│   └── 001_core_schema.cql
└── crates/
    ├── drone-domain/             # core types, enums, value objects
    ├── drone-persistence/        # repository + strategy pattern
    │   ├── cache/                # Redis client wrapper
    │   ├── repository/           # ScyllaDB implementations
    │   └── strategy/             # read/write strategies
    ├── drone-graphql-api/        # Axum + async-graphql service
    │   ├── schema/               # GraphQL enums, inputs, objects
    │   ├── resolvers/            # query, mutation, subscription
    │   ├── loaders/              # DataLoaders, N+1 prevention
    │   └── context.rs            # application state / DI
    ├── drone-frontend/           # Leptos WASM SPA
    ├── drone-simulator/          # telemetry + engagement simulation
    └── drone-analytics/          # DuckDB OLAP queries
```

> `assets/images/` at the project root holds the map marker SVGs, compiled into
> the WASM bundle via `include_str!`. It is distinct from
> `crates/drone-frontend/assets/`, which is what Trunk bundles.

### Crates

| Crate | Purpose |
|---|---|
| `drone-domain` | Shared types: `Convoy`, `Drone`, `Waypoint`, `Telemetry`, `Engagement`, `LeaderboardEntry` |
| `drone-persistence` | Repository pattern with pluggable cache strategies |
| `drone-graphql-api` | GraphQL server: leaderboard queries, engagement mutations, subscriptions |
| `drone-frontend` | Leptos + Leaflet: tactical map, convoy positions, accuracy leaderboard |
| `drone-simulator` | Telemetry generator: 25 waypoints per drone, random engagements |
| `drone-analytics` | DuckDB OLAP: Parquet export from ScyllaDB, mission analytics |

The binary produced by `drone-graphql-api` is named **`drone-api`**, not
`drone-graphql-api`.

---

## Data model

- **Convoy** — mission-level grouping of drones with AOR, ROE, commanding unit
- **Drone** — platform with callsign, position, fuel, accuracy stats
- **Waypoint** — 25 per drone, defining the mission route
- **Telemetry** — time-series position/sensor data, hourly partitioned, 30-day TTL
- **Engagement** — weapon employment record with target, result, BDA
- **LeaderboardEntry** — pre-computed accuracy ranking

```sql
-- Convoy: single partition per convoy
PRIMARY KEY (convoy_id)

-- Drones: partitioned by convoy for co-location
PRIMARY KEY (convoy_id, drone_id)

-- Telemetry: time-bucketed for bounded partition growth
PRIMARY KEY ((drone_id, time_bucket), recorded_at)

-- Leaderboard: sorted by accuracy for fast top-N
PRIMARY KEY (convoy_id, accuracy_pct, drone_id)
```

---

## GraphQL

Playground at http://localhost:8080/graphql. A browser GET serves the IDE; the
frontend POSTs to the same path.

It is also the fastest way to tell a backend problem from a frontend one: if a
query returns rows here but the dashboard shows nothing, the bug is in the client.

```graphql
query {
  leaderboard(convoyId: "550e8400-e29b-41d4-a716-446655440000", limit: 10) {
    convoyCallsign
    totalDrones
    averageAccuracy
    entries {
      rank
      callsign
      accuracyPct
      totalEngagements
      successfulHits
      currentStreak
    }
  }
}

mutation {
  recordEngagement(input: {
    convoyId: "550e8400-e29b-41d4-a716-446655440000"
    droneId: "660e8400-e29b-41d4-a716-446655440001"
    hit: true
    weaponType: AGM114_HELLFIRE
    targetType: VEHICLE
  }) {
    success
    newRank
    rankChange
    newAccuracyPct
  }
}
```

Two things that bite when writing new clients:

**GraphQL reports errors in a 200 OK body.** Checking only the HTTP status makes
every rejected mutation look like a success. Parse `errors[]`.

**Variables must be camelCase.** A `Variables` struct without
`#[serde(rename_all = "camelCase")]` sends `convoy_id` while the query declares
`$convoyId`, and the server rejects it — which reads as a connectivity failure.

Enum values serialise as SCREAMING_SNAKE: `Agm114Hellfire` → `AGM114_HELLFIRE`.

---

## Troubleshooting

**`podman-compose: No such file or directory`** — only `make stack-up` needs it.
Use `make serve`, or `brew install podman-compose`.

**ScyllaDB never comes up** — `podman logs scylla`. Usually the podman machine is
undersized; stop it and `podman machine set --cpus 4 --memory 8192`.

**Dashboard shows OFFLINE** — the leaderboard poll is failing. The browser console
logs `leaderboard poll failed:` with the server's actual GraphQL error.

**Leaderboard is empty but the app says ONLINE** — correct behaviour when no
engagements have been recorded for the convoy. Start the simulator and confirm its
first log line reads `Convoy ID: 550e8400-…`.

**Changes do not appear after unzipping a delta** — run `make build`, not just
`make serve`. See the mtime section above.

**`-C embed-bitcode=no` and `-C lto` are incompatible** — codegen settings belong
in `[profile.release]` in the root `Cargo.toml`, never in `RUSTFLAGS`. Cargo passes
`-C embed-bitcode=no` for proc-macros, and forcing LTO globally fails every one of
them at once.

---

## Roadmap

**Phase 1 — data layer** ✅ ScyllaDB CQL schema with UDTs and time-bucketed
partitions · Redis cache schema with TTL strategy · domain types · repository +
strategy pattern

**Phase 2 — persistence** 🔶 ScyllaDB repositories (writes done, most reads
unimplemented) · Redis cache client · read strategies · write strategies

**Phase 3 — GraphQL API** 🔶 schema, leaderboard queries and engagement mutations
done · most other resolvers still stubs · subscriptions written but route disabled
· DataLoaders

**Phase 4 — frontend** 🔶 Leptos WASM SPA · Leaflet map with animated convoy and
red waypoint pins · leaderboard HUD on live data · tactical dark theme · remaining
panels still seeded

**Phase 5 — analytics** ⬜ DuckDB crate · ScyllaDB → Parquet export · mission
analytics · engagement pattern analysis

---

## License

MIT
# ArgoCD ApplicationSets GitOps


## The Architecture of ArgoCD ApplicationSets vs App-of-Apps


### Advantages of ArgoCD ApplicationSets


### The Delivery Patterns of ArgoCD ApplicationSets 



## The Application (Rust Drone Convoy Tracking System)  


## Adoption of ArgoCD ApplicationSets to SuperVisor and VKS 





## The ArgoCD ApplicationSet Architecture

### The ArgoCD Cluster Addons (How it Works)

### The ArgoCD Cluster Apps (How it Works)


### The ArgoCD Cluster Secrets 


## ArgoCD Feature Promotion w/ Kargo



