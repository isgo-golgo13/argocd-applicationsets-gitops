# dronegrid-convoy-leaderdash-tracking-svc-rs
Drone P2P Convoy Leader Status Tracking and Storage (OLTP, OLAP) using Rust, Rust Tokio Async, GraphQL,  Redis, DuckDB, ScyllaDB and Leptos and Leaflet Rust UI Toolkit Crates


*Classification: UNCLASSIFIED // FOR OFFICIAL USE ONLY**

Production-grade military drone convoy tracking system with real-time leaderboard, telemetry persistence, and tactical visualization.

## Architecture

```
┌────────────────────────────────────────────────────────────────────────────────┐
│                              Leptos Frontend (WASM)                            │
│                    Afghanistan Map + D3.js Visualization                       │
└────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│                          Axum + async-graphql API                              │
│                    Queries / Mutations / Subscriptions                         │
└────────────────────────────────────────────────────────────────────────────────┘
                                        │
                                        ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│                       drone-persistence (Repository Pattern)                   │
│                   Cache-Aside Strategy / Write-Through Strategy                │
└────────────────────────────────────────────────────────────────────────────────┘
                          │                              │
                          ▼                              ▼
┌────────────────────────────────────┐    ┌─────────────────────────────────────┐
│           Redis Cache              │    │           ScyllaDB                   │
│   • Leaderboard (ZSET)             │    │   • convoys                         │
│   • Drone State (HASH)             │    │   • drones                          │
│   • Latest Telemetry               │    │   • waypoints (25/drone)            │
│   • Convoy Roster (SET)            │    │   • telemetry (time-series)         │
└────────────────────────────────────┘    │   • engagements                     │
                                          │   • leaderboard                     │
                                          │   • accuracy_counters               │
                                          └─────────────────────────────────────┘
                                                         │
                                                         ▼
                                          ┌─────────────────────────────────────┐
                                          │         DuckDB (OLAP)               │
                                          │    Parquet Export Analytics         │
                                          └─────────────────────────────────────┘
```

## Convoy Tracking (Rust) Leptos Dash UI

![convoy-tracker-dash](docs/drone-convoy-tracker.png)

![convoy-tracker-dash](docs/drone-convoy-tracker-zoom.png)




## Project Structure

```
drone-convoy-tracker/
├── Cargo.toml                    # Workspace manifest
├── schema/
│   ├── cql/
│   │   └── 001_core_schema.cql   # ScyllaDB DDL (keyspace, UDTs, tables, MVs)
│   └── redis/
│       └── cache_schema.md       # Redis key patterns, TTLs, Lua scripts
├── crates/
│   ├── drone-domain/             # Core types, enums, value objects
│   ├── drone-persistence/        # Repository + Strategy pattern
│   │   ├── cache/                # Redis client wrapper
│   │   ├── repository/           # Repository traits + ScyllaDB impl
│   │   └── strategy/             # Read/Write strategies (cache-first, ...)
│   ├── drone-graphql-api/        # Axum + async-graphql service
│   │   ├── schema/               # GraphQL types (enums, inputs, objects)
│   │   ├── resolvers/            # Query, Mutation, Subscription
│   │   ├── loaders/              # DataLoaders for N+1 prevention
│   │   └── context.rs            # Application state / DI
│   ├── drone-frontend/           # Leptos WASM SPA
│   ├── drone-simulator/          # Telemetry + engagement simulation
│   └── drone-analytics/          # DuckDB OLAP queries
├── config/                       # Environment configs
└── docs/                         # Architecture documentation
```

## Crates

| Crate | Purpose |
|-------|---------|
| `drone-domain` | Shared types: `Convoy`, `Drone`, `Waypoint`, `Telemetry`, `Engagement`, `LeaderboardEntry` |
| `drone-persistence` | Repository pattern with pluggable cache strategies (cache-first, write-through, etc.) |
| `drone-graphql-api` | GraphQL API server: leaderboard queries, engagement mutations, real-time subscriptions |
| `drone-frontend` | Leptos + Charming visualization: Afghanistan map, drone convoy positions, accuracy leaderboard |
| `drone-simulator` | Mock telemetry generator: 25 waypoints per drone, random engagements |
| `drone-analytics` | DuckDB OLAP: Parquet export from ScyllaDB, mission analytics |

## Data Model

### Core Entities

- **Convoy**: Mission-level grouping of drones with AOR, ROE, commanding unit
- **Drone**: Individual platform with callsign, position, fuel, accuracy stats
- **Waypoint**: 25 waypoints per drone defining the mission route
- **Telemetry**: Time-series position/sensor data (hourly partitioned, 30-day TTL)
- **Engagement**: Weapon employment record with target, result, BDA
- **LeaderboardEntry**: Pre-computed accuracy ranking

### Partition Strategy

```sql
-- Convoy: Single partition per convoy
PRIMARY KEY (convoy_id)

-- Drones: Partitioned by convoy for co-location
PRIMARY KEY (convoy_id, drone_id)

-- Telemetry: Time-bucketed for bounded partition growth
PRIMARY KEY ((drone_id, time_bucket), recorded_at)

-- Leaderboard: Sorted by accuracy for fast top-N queries
PRIMARY KEY (convoy_id, accuracy_pct, drone_id)
```

## Getting Started

### Prerequisites

- Rust 1.75+ (edition 2024)
- ScyllaDB 5.x or Cassandra 4.x
- Redis 7.x

### Run ScyllaDB Schema

```bash
cqlsh -f schema/cql/001_core_schema.cql
```

### Environment Variables

```bash
# Server
SERVER_ADDR=0.0.0.0:8080
LOG_LEVEL=info

# ScyllaDB
SCYLLA_HOSTS=127.0.0.1:9042
SCYLLA_KEYSPACE=drone_ops
SCYLLA_USERNAME=cassandra
SCYLLA_PASSWORD=cassandra

# Redis
REDIS_URL=redis://127.0.0.1:6379
REDIS_POOL_SIZE=10
```

### Build & Run

```bash
# Build all crates
cargo build --release

# Run API server
cargo run --release -p drone-graphql-api

# Run with tracing
RUST_LOG=drone_graphql_api=debug cargo run -p drone-graphql-api
```

### GraphQL Playground

Navigate to `http://localhost:8080/graphql`

```graphql
# Get leaderboard
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

# Record engagement
mutation {
  recordEngagement(input: {
    convoyId: "550e8400-e29b-41d4-a716-446655440000"
    droneId: "660e8400-e29b-41d4-a716-446655440001"
    hit: true
    weaponType: AGM114_HELLFIRE
  }) {
    success
    newRank
    rankChange
    newAccuracyPct
  }
}

# Subscribe to engagement events
subscription {
  engagementEvents(convoyId: "550e8400-e29b-41d4-a716-446655440000") {
    droneId
    callsign
    hit
    weaponType
    newAccuracyPct
    timestamp
  }
}
```


## Quick Start (Non-Linux)


1. Extract and enter
```shell
cd ~/drone-convoy-tracker
```
2. Build backend crates first (no WASM needed)
```shell
cargo build --workspace --exclude drone-frontend
```

3. If that succeeds, build frontend (needs wasm target + trunk)
```shell
rustup target add wasm32-unknown-unknown
cargo install trunk
cd crates/drone-frontend && trunk build
```






1. Start databases only
```shell
cd docker
docker compose -f docker-compose.dev.yml up -d
```

2. Wait ~60-90s for ScyllaDB to be ready, then check:
```shell
docker logs scylla 2>&1 | tail -5
```

3. Initialize schema
```shell
docker exec -it scylla cqlsh -f /schema/001_core_schema.cql
```

4. Run API (in project root)
```shell
cargo run --package drone-graphql-api
```

5. Run frontend (in another terminal)
```shell
cd crates/drone-frontend
trunk serve --open
```




## Development Roadmap

### Phase 1 (Current): Data Layer
- [x] ScyllaDB CQL schema with UDTs, time-bucketed partitions
- [x] Redis cache schema with TTL strategy
- [x] Domain types crate
- [x] Repository pattern with strategy pattern

### Phase 2: Persistence Layer
- [x] ScyllaDB repository implementations
- [x] Redis cache client
- [x] Read strategies (cache-first, read-through)
- [x] Write strategies (write-through, write-around)

### Phase 3: GraphQL API
- [x] async-graphql schema (Query, Mutation, Subscription)
- [x] Leaderboard queries with filtering
- [x] Engagement mutations with accuracy tracking
- [x] Real-time subscriptions
- [x] DataLoaders for N+1 prevention

### Phase 4: Frontend
- [ ] Leptos WASM SPA setup
- [ ] Charming (D3.js equivalent) Afghanistan map
- [ ] Real-time drone position visualization
- [ ] Leaderboard HUD component
- [ ] Tactical dark theme CSS

### Phase 5: Analytics
- [ ] DuckDB analytics crate
- [ ] ScyllaDB → Parquet export pipeline
- [ ] Mission analytics queries
- [ ] Engagement pattern analysis

## License

MIT