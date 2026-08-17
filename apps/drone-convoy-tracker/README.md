##  DoD Attack Drone Convoy Tracking System (Rust)

The provided Rust application is a full-stack DoD attack drone convoy tracking system using a WebAssembly native Rust Letpos frontend reading realtime WebSocket drone status telemetry, GraphQL drone leader tracking on the server-side using Redis RW cache cluster and ScyllaDB NoSQL DB (ScyllaDB is 100% rewrite of Cassandra DB NoSQL in C++).


See the following screenshots of systems frontend.

Screenshot 1
![drone-svc-ui-1](docs/drone-convoy-1.png)

Screenshot 2
![drone-svc-ui-2](docs/drone-convoy-2.png)

Screenshot 3
![drone-svc-ui-3](docs/drone-convoy-3.png)

Screenshot 4 
![drone-svc-ui-4](docs/drone-convoy-4.png)

Screenshot 5 
![drone-svc-ui-5](docs/drone-convoy-5.png)


## Project Structure

```shell
drone-convoy-attack-tracker-leptos-rs
├── Cargo.lock
├── Cargo.toml
├── Makefile
├── README.md
├── assets
│   └── images
│       ├── drone.svg              # airframe: map markers (red) + drone cards (green)
│       ├── drone-favicon.svg      # same airframe, HUD-green defaults, browser tab
│       ├── explosion.svg          # impact bursts on the map
│       └── target-streak.svg      # hit-streak roundel on the leaderboard
├── config
│   └── app.toml
├── containers
│   ├── Containerfile.api
│   ├── Containerfile.frontend
│   ├── nginx.conf
│   ├── podman-compose.dev.yml
│   ├── podman-compose.yml
│   └── prometheus.yml
├── crates
│   ├── drone-analytics
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── engine.rs
│   │       ├── error.rs
│   │       ├── lib.rs
│   │       ├── queries.rs
│   │       └── reports.rs
│   ├── drone-domain
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── lib.rs
│   │       └── theaters.rs        # the ONE theater/route table (sim flies it, UI draws it)
│   ├── drone-frontend
│   │   ├── Cargo.toml
│   │   ├── Trunk.toml
│   │   ├── index.html
│   │   ├── src
│   │   │   ├── components
│   │   │   │   ├── charts.rs
│   │   │   │   ├── drone_card.rs
│   │   │   │   ├── engagement_feed.rs
│   │   │   │   ├── footer.rs
│   │   │   │   ├── header.rs
│   │   │   │   ├── leaderboard.rs
│   │   │   │   ├── map.rs
│   │   │   │   ├── mod.rs
│   │   │   │   ├── regions.rs         # re-exports drone_domain::theaters
│   │   │   │   └── tactical_select.rs # web-native HUD dropdown (the THEATER selector)
│   │   │   ├── lib.rs
│   │   │   ├── main.rs
│   │   │   ├── services
│   │   │   │   ├── api.rs
│   │   │   │   ├── mod.rs
│   │   │   │   └── websocket.rs
│   │   │   └── state
│   │   │       └── mod.rs
│   │   └── style
│   │       └── main.css
│   ├── drone-graphql-api
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── config.rs
│   │       ├── context.rs
│   │       ├── error.rs
│   │       ├── lib.rs
│   │       ├── loaders
│   │       │   └── mod.rs
│   │       ├── main.rs
│   │       ├── resolvers
│   │       │   ├── mod.rs
│   │       │   ├── mutation.rs
│   │       │   ├── query.rs
│   │       │   └── subscription.rs
│   │       └── schema
│   │           ├── enums.rs
│   │           ├── inputs.rs
│   │           ├── mod.rs
│   │           └── objects.rs
│   ├── drone-persistence
│   │   ├── Cargo.toml
│   │   └── src
│   │       ├── cache
│   │       │   ├── mod.rs
│   │       │   └── redis_client.rs
│   │       ├── error.rs
│   │       ├── lib.rs
│   │       ├── repository
│   │       │   ├── mod.rs
│   │       │   └── scylla_impl.rs
│   │       └── strategy
│   │           ├── mod.rs
│   │           ├── read_strategy.rs
│   │           └── write_strategy.rs
│   └── drone-simulator
│       ├── Cargo.toml
│       └── src
│           ├── convoy.rs
│           ├── engagement.rs
│           ├── flight.rs
│           ├── lib.rs
│           ├── main.rs            # runs as the CONVOY SERVICE under `make serve`
│           └── telemetry.rs
├── deploy
│   ├── cloudflare
│   │   └── cloudflare-workers
│   │       └── wrangler.jsonc     # dashboard as Workers Static Assets
│   ├── cluster
│   │   ├── README-setup.md        # step-by-step KinD walkthrough
│   │   ├── kind-bootstrap.sh      # Gateway API CRDs → Cilium → cert-manager → ESO → KEDA/VPA/scylla-operator
│   │   ├── kind-config.yaml       # 3 control-plane + 3 workers
│   │   └── kind-expose.sh
│   ├── fly.io
│   │   └── fly.prod.toml
│   ├── kubernetes
│   │   └── drone-convoy-attack-tracker   # Helm chart (Cilium Gateway API, cert-manager, ESO, KEDA/VPA/HPA)
│   │       ├── Chart.yaml
│   │       ├── templates
│   │       │   ├── _helpers.tpl
│   │       │   ├── app.yaml
│   │       │   ├── app-config.yaml
│   │       │   ├── app-metrics.yaml
│   │       │   ├── app-scaling.yaml
│   │       │   ├── app-secrets.yaml
│   │       │   ├── app-service.yaml
│   │       │   └── app-storage.yaml
│   │       ├── values-nonprod.yaml
│   │       ├── values-prod.yaml
│   │       └── values.yaml
│   └── railway
│       ├── railway.nonprod.toml
│       └── railway.prod.toml
├── docs
│   ├── drone-convoy-1.png
│   ├── drone-convoy-2.png
│   ├── drone-convoy-3.png
│   ├── drone-convoy-4.png
│   └── drone-convoy-5.png
└── schema
    └── cql
        ├── 000_keyspace_dev.cql
        ├── 000_keyspace_prod.cql
        ├── 001_core_schema.cql
        └── 002_waypoint_columns.cql
```



## The Architecture of the DoD Attack Drone Tracking Service

The system is five Rust crates behind one workspace, arranged so that every
layer owns exactly one concern and the data flows in a single direction:

```
drone-simulator ──GraphQL mutations──▶ drone-graphql-api ──▶ drone-persistence ──▶ ScyllaDB
  (convoy service)   telemetry/state/        │                      │
        ▲            engagements             │                      └──▶ Redis (leaderboard read cache)
        │                                    ▼
        │  reads tasking order      drone-frontend (Leptos/WASM)
        │  (convoy.aorName)         2s GraphQL polling + /graphql/ws subscriptions
        │                                    │
        └────── convoy record ◀── retaskConvoy ── THEATER selector (the UI is the commander)
```

**The UI is the commander.** The dashboard never controls a process. Its
THEATER selector issues a *tasking order* — the `retaskConvoy` mutation writes
the theater onto the convoy record (`aor_name` = theater slug, `aor_center` =
theater centre; both pre-existing columns, so no schema change). The simulator
runs as a long-lived **convoy service** under `make serve`: it flies sorties
back to back, reads the record between sorties and every few ticks mid-sortie,
and on a change re-flies from the new theater's IP. Every dashboard observing
the record sees the same switch. This is exactly the seam a live ground
station plugs into later — the UI writes to the system of record, whatever
flies the drones reads it — so nothing about the dashboard changes when the
simulator is replaced.

**One route table, two consumers.** `drone-domain/src/theaters.rs` holds the
six theaters (Afghanistan/Kandahar — the default — Syria, Libya, Pakistan,
Iran, Iraq), each with centre, AOR ring and sortie route. The simulator flies
those exact waypoints and posts real positions; the frontend draws those exact
waypoints as pins. Because both read one array, the airframes on the map, the
GPS readout on each drone card, and the database all agree — in every theater.

**Transports.** Three distinct transports, each chosen for what it carries:

- **GraphQL over HTTP (axum + async-graphql)** is the command-and-query plane.
  The simulator posts `recordTelemetry`, `updateDroneState` and
  `recordEngagement` mutations every tick and reads `convoy { aorName }` for
  its tasking; the dashboard polls `leaderboard`, `drones` and `engagements`
  on a 2-second cadence and issues `retaskConvoy` from the THEATER selector. Every HTTP caller in the
  codebase parses the GraphQL `errors[]` array rather than trusting the HTTP
  status — GraphQL rejections arrive in a 200 OK body, and a caller that only
  checks status logs success while writing nothing.
- **GraphQL subscriptions over WebSocket** are mounted at `/graphql/ws`
  (graphql-ws protocol), wired to tokio broadcast channels inside the API.
  The dashboard currently ships on polling with subscriptions as the upgrade
  path; both transports read identical types, so the swap is additive.
- **CQL native protocol** between the API and ScyllaDB via the `scylla` driver
  (shard-aware, prepared statements throughout).

**Enum wire contract.** All digit-bearing GraphQL enum variants
(`MQ9_REAPER`, `AGM114_HELLFIRE`, ...) carry explicit `#[graphql(name)]` pins.
async-graphql's SCREAMING_SNAKE_CASE rename runs through the Inflector crate,
which treats digits as uppercase and would register `MQ_9_REAPER` — silently
rejecting every value the clients send. The pins remove the rename engine from
the contract; do not trust automatic renames for variants containing digits.

**ScyllaDB schema design.** ScyllaDB is queried by partition, so the schema
is laid out per read path rather than normalized (`schema/cql/`):

- `convoys`, `drones` — row-per-entity state, drones partitioned by convoy.
  `drones` also denormalizes engagement counters so the drone-card ACC readout
  is one partition read.
- `waypoints` — partitioned by drone, clustered by sequence number: a route is
  one ordered partition scan.
- `telemetry` — time-series partitioned by `(drone_id, hour_bucket)` with
  time-descending clustering, so "latest N" and "last hour" are single-
  partition reads and partitions cannot grow unbounded.
- `engagements` and `engagements_by_drone` — the same event dual-written in
  one logged batch, because "per convoy" and "per drone" are different
  partition keys and ScyllaDB does not do secondary-index reads cheaply.
- `leaderboard` — partitioned by convoy, clustered by `accuracy_pct DESC`, so
  the ranking IS the storage order. Because the ordering column is a
  clustering key it cannot be UPDATEd in place: a score change is a
  DELETE + INSERT of the row inside a single-partition logged batch (atomic
  at partition scope).
- Custom UDTs (`coordinates`, `target_info`) mirror Rust structs derived with
  `SerializeValue`/`DeserializeValue`, field names matched to the CQL types.

**Redis RW cache.** The leaderboard read path is cached in Redis with a short
TTL; every leaderboard write invalidates the key, so the dashboard's 2-second
poll amortizes to cache hits between engagements while never serving a stale
ranking after one. Cache failures degrade to ScyllaDB reads — Redis is an
accelerator, never a source of truth.

**Determinism as a feature.** The simulator derives drone UUIDs as UUIDv5 of
the callsign (`ALPHA-01` is the same UUID on every machine, every run), and
pins its convoy to a well-known demo UUID (`DRONE_CONVOY_ID` overrides).
Restarts therefore overwrite state instead of accumulating ghost fleets, and
every GraphQL example in this README is reproducible verbatim in the
playground.

**Resilience.** The simulator probes `{ health }` until the API is up before
its bootstrap, and drone identity rides on every per-tick
`updateDroneState` (an idempotent read-merge-write upsert) — a lost
registration self-heals within one tick.

**Server-anchored map.** The airframes fly the positions the API reports, not
a client-side route: each 2 s poll delivers a fix per drone, and the map's
flight loop interpolates from the previous fix to the latest over one poll
interval (clamped — it never extrapolates past ground truth) so a marker sits
exactly on the database position at every poll boundary and glides between.
The same smoothed fix drives the `GPS` readout on each drone card. Impact
bursts render in a dedicated Leaflet pane *below* the airframes, fired from
the shooting drone's live position when a new engagement lands in the feed.


## Build Prerequisites 

Native toolchain (the default `make serve` path runs the API, the frontend
and the convoy service natively; containers are used only for the databases):

| Requirement | Version | Why |
|---|---|---|
| Rust toolchain (rustup) | **1.85+** (edition 2024) | workspace edition; older toolchains fail immediately |
| `wasm32-unknown-unknown` target | — | Leptos frontend (`rustup target add wasm32-unknown-unknown`) |
| Trunk | 0.21+ | WASM build + dev server (`cargo install trunk`) |
| Podman | 4.x+ | runs ScyllaDB and Redis (Docker works identically) |
| podman-compose | 1.x | only for the all-in-containers `make stack-up` path |
| GNU Make | any | the front door; every workflow is a `make` verb |
| ~6 GB free RAM | — | ScyllaDB is a C++ database that takes its memory seriously |

No Node, no npm, no Python: the frontend toolchain is entirely Rust.
ScyllaDB schema application is explicit (`make` targets drive `cqlsh` inside
the container) — ScyllaDB has no `/docker-entrypoint-initdb.d` convention, so
do not expect mounted init scripts to run.


## Building the Project

```shell
make build          # touch sweep + full workspace build (API, frontend, simulator)
make serve          # starts ScyllaDB+Redis (podman), applies schema, then runs — natively,
                    #   in parallel — the API (release), the Trunk dev server, and the
                    #   CONVOY SERVICE (the simulator, flying sorties back to back)
```

That is the whole operator workflow. Open the dashboard and use the **THEATER**
selector: pick a theater and the convoy is retasked to it — a small amber
"RETASKING — CONVOY EN ROUTE" row shows under the AOR label for a few seconds,
then the airframes appear on that theater's pins. No second command, no flags.

Dashboard at `http://localhost:3000`, GraphQL playground at
`http://localhost:8080/graphql`. First database boot takes ~60s; subsequent
runs reuse the running containers. `make db-reset` drops the keyspace inside
the running container (seconds) for a clean demo; `make stop` stops the
databases; `make help` lists the full front door.

Optional knobs (none needed for the normal flow):

- `THEATER=<slug>` seeds the *first* sortie when the convoy record carries no
  tasking yet (default `afghanistan`); after that the selector is in charge.
- `SIM=0 make serve` runs API + frontend without the convoy service.
- `make run-simulator [THEATER=syria]` flies one manual sortie and exits —
  a dev tool, not part of the operator flow.

Two Make behaviors worth knowing rather than discovering:

- `make build` runs a `touch` sweep first, because overlaying delivery zips
  restores archive mtimes and Cargo would otherwise skip "older" files.
  `TOUCH=0` opts out.
- `make serve` builds API, frontend and service in parallel; the service's
  `wait_for_api` probe exists precisely so a race against the API's release
  build cannot lose the bootstrap.

Each sortie runs ~5 minutes; the service starts the next one automatically.
Engagements fire only between 25% and 75% of sortie progress — an empty
leaderboard in the first ~75 seconds of a sortie is correct behavior, not a
defect. At sortie end, the service prints its FINAL LEADERBOARD; the
playground `leaderboard` query must match it exactly, which is the end-to-end
integrity check for the whole pipeline.

Container images (`containers/Containerfile.api`, `.frontend`) build with a
parameterised `ARG RUST_VERSION` (≥1.85) for the `make stack-up` path.
Kubernetes: `deploy/cluster/README-setup.md` walks a KinD cluster (Cilium
Gateway API, cert-manager, ESO, KEDA/VPA, scylla-operator) end to end;
`make kind-up && make kind-load && make kind-deploy` → `https://drone.localtest.me`.


## References 

- Rust Leptos 
https://leptos.dev/

- ScyllaDB
https://www.scylladb.com/

- Rust Crates Registry
https://crates.io/

- Rust Site
https://rust-lang.org/


