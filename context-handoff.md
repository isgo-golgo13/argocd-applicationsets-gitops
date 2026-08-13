# context-handoff.md — argocd-applicationsets-gitops

**Updated 2026-08-13.** Supersedes the previous handoff entirely; every item in
that document has been either fixed or restated below.

## What this repository is

A production-grade **reference** showing how ArgoCD ApplicationSets replace the
App-of-Apps pattern. It is parked as a sibling folder to Broadcom's existing
`argocd-app-of-apps/`, handed over as a zip (no write access to their repo), and
pushed by their principal architect.

It is a **guide**, not a shipping product. Broadcom is the audience — not the
healthcare end client. Client-specific deltas belong in a README appendix, never
in the architecture.

The repository has three corners, and the whole argument depends on all three
being real:

```
apps/drone-convoy-tracker/                        the application (Rust + Containerfiles)
argocd-apps/cluster-apps/drone-convoy-tracker/    its Helm chart
argocd-apps/app-sets/templates/cluster-apps.yaml  the ApplicationSet that globs it up
```

Drop a chart directory into `cluster-apps/` and four Applications appear, one per
environment, with no per-app `Application.yaml` and no edit to any shared file.
That is the demo. Do not break it.

---

# PART 1 — THE DASHBOARD DOES NOT BREATHE

**This is the top priority and the reason for this handoff.**

The Leptos dashboard renders beautifully and is almost entirely inert. The drones
fly (client-side animation), the leaderboard polls a real query, and every other
number on screen is frozen. The owner is selling this as a $100 Gumroad
Rust/Leptos course, so "looks live" is not good enough — a buyer will open
`lib.rs` on about page three.

## 1.1 What is actually live today

Exactly one path reaches storage end to end:

```
drone-simulator  --recordEngagement-->  mutation.rs
                 -->  ScyllaLeaderboardRepository::update_entry()  -->  ScyllaDB
                 -->  leaderboard query  -->  frontend poll (2s)  -->  leaderboard panel
```

Everything else is seeded in the frontend by `seed_static_panels()` in
`crates/drone-frontend/src/lib.rs`, or returned as hardcoded literals by stub
resolvers.

## 1.2 Why nothing else can be live

`crates/drone-graphql-api/src/context.rs` — `ApiContext` carries **only**
`leaderboard_repo`, plus the Scylla client, the cache, and five broadcast senders.
The resolvers therefore have no handle on any other repository, regardless of what
exists in `drone-persistence`.

**Twelve stubs in `resolvers/query.rs`** (lines ~76, 122, 154, 206, 210, 229, 267,
312, 370, 391, 412, 443) and **eight in `resolvers/mutation.rs`** (~49, 133, 178,
237, 283, 318, 355, 377). The ones that matter:

| Resolver | Current behaviour |
|---|---|
| `drones` | returns ONE hardcoded `REAPER-01` built inline, with a fresh `Uuid::new_v4()` each call |
| `engagements` | returns an empty `Connection` |
| `convoyStats` | `average_fuel_pct: 75.0` and `airborne_count: entries.len()` as literals |
| `latestTelemetry`, `telemetryHistory` | `// TODO`, empty |
| `waypoints` | `// TODO`, empty |
| `activeConvoys` | one hardcoded convoy, id `550e8400-e29b-41d4-a716-446655440000` |
| `convoy` | `// TODO` |

## 1.3 The repository layer is half-built — know which half

`crates/drone-persistence/src/repository/scylla_impl.rs`. **Write paths are real;
most read paths are not.** Do not assume "the repos exist" means "the repos work".

| Method | State |
|---|---|
| `ScyllaLeaderboardRepository::get_leaderboard` | **REAL** — full SELECT and row parsing |
| `ScyllaLeaderboardRepository::update_entry` | **REAL** — UPDATE plus counter maths |
| `ScyllaLeaderboardRepository::get_drone_entry` | partial — falls through to `Ok(None)` |
| `ScyllaEngagementRepository::record` | **REAL** — INSERT |
| `ScyllaEngagementRepository::get_by_drone` | **STUB** — `Ok(Vec::new())`, args underscored |
| `ScyllaTelemetryRepository::record` | **REAL** — INSERT |
| `ScyllaTelemetryRepository::get_latest` | **STUB** — `Ok(None)` |
| `ScyllaConvoyRepository::create` | **REAL** |
| `ScyllaConvoyRepository::get` | **STUB** — `Ok(None)` |
| `ScyllaWaypointRepository::get_waypoints` | **STUB** — `Ok(Vec::new())` |

Every stub carries the same comment: *"TODO: Implement full parsing of complex
X type"*. The CQL tables all exist in `schema/cql/001_core_schema.cql`: `convoys`,
`drones`, `waypoints`, `telemetry`, `engagements`, `engagements_by_drone`,
`accuracy_counters`, `leaderboard`, `command_log`, `alerts`.

So the work is **row deserialisation**, not schema design. That is the single
largest ticket in this repo.

## 1.4 The simulator only writes engagements

`crates/drone-simulator/src/main.rs` calls `convoy.generate_telemetry()` every
tick, logs the count, and **throws it away**. There is no `post_telemetry`, and no
drone-state call either.

So even after the read paths are implemented, `telemetry` and `drones` return
empty until the simulator posts them. Both mutations exist in the schema already:
`recordTelemetry` and `updateDroneState` (themselves stubs, mutation.rs ~283 and
~237).

## 1.5 Recommended order of work

Sequential. Each step is independently verifiable in the GraphQL playground at
`http://localhost:8080/graphql`, which is the fastest way to tell a backend
problem from a frontend one.

1. **Widen `ApiContext`.** Add `engagement_repo`, `telemetry_repo`, `convoy_repo`,
   `waypoint_repo` alongside `leaderboard_repo`, and construct them in
   `ApiContextBuilder`. Nothing else can proceed without this.
2. **Implement the read-path stubs in `scylla_impl.rs`** — row parsing for
   `Engagement`, `Telemetry`, `Convoy`, `Waypoint`. Copy the pattern from
   `get_leaderboard`, which already does this correctly for `LeaderboardEntry`.
3. **Implement `recordTelemetry` and `updateDroneState`** in `mutation.rs` against
   the now-available repos, and **have the simulator post telemetry and drone
   state** each tick, not just engagements.
4. **Point the query resolvers at the repos**: `drones`, `engagements`,
   `convoyStats`, `latestTelemetry`, `telemetryHistory`, `waypoints`, `convoy`,
   `activeConvoys`. Delete every inline hardcoded struct as you go.
5. **Frontend**: extend `start_live_feed()` in `lib.rs` to poll the new queries and
   drive `state.drones`, `state.engagements` and the stats panels. Then delete
   `seed_static_panels()` entirely — while it exists, a regression looks like
   working software.
6. **Swap polling for subscriptions.** See 1.6.
7. **Feed the map from real waypoints.** `map.rs` has a `ROUTE` constant of 14
   hardcoded points. Once `waypoints` returns rows, replace the constant with the
   query result; the animation code needs no other change.

## 1.6 The WebSocket route is commented out

`crates/drone-graphql-api/src/lib.rs:122`:

```rust
// TODO: WebSocket subscriptions disabled until async-graphql-axum supports axum 0.8
// .route("/graphql/ws", any(GraphQLSubscription::new(schema)))
```

Meanwhile `main.rs` still logs *"WebSocket subscriptions at ws://…/graphql/ws"*,
and the frontend's `services/websocket.rs` (`use_websocket`, never called)
connects there and gets a 404.

The subscription resolvers are already wired to the broadcast channels in
`ApiContext`, and were changed to return `async_graphql::Result<impl Stream<...>>`
so a missing context propagates with `?` instead of panicking the task.
**Re-check the axum / async-graphql-axum version constraint** before assuming this
is still blocked — the comment may predate a release that fixes it.

## 1.7 Frontend polish, low effort

- **An empty leaderboard collapses to a bare header.** When the poll returns
  `entries: []` the panel renders nothing at all. Add an explicit empty state
  ("NO ENGAGEMENTS RECORDED") so an empty result is distinguishable from a broken
  one — this has already caused confusion once.
- **`ws_connected` takes ~20s to flip on load**, because `start_live_feed()`
  awaits `fetch_active_convoys()` before the first leaderboard poll.
- **Hardcoded API URL.** `services/api.rs` has
  `const API_URL: &str = "http://localhost:8080/graphql"`, and `websocket.rs` the
  matching `ws://`. This **cannot work in-cluster**. Move to same-origin relative
  paths (`/graphql`, `/graphql/ws`); the Helm chart's HTTPRoute already routes UI
  and API through one hostname for exactly this reason, which also removes CORS.
- **ENGAGE button.** Requested, not built. `record_engagement` exists in
  `services/api.rs` with zero callers and the mutation works. Wire a control to it
  and play `assets/images/explosion.svg` as a temporary Leaflet marker for ~1.5s
  (self-animating SMIL: add it, wait, remove it).

## 1.8 Bugs already found and fixed — do not reintroduce

Each of these cost hours, and every one presented as something other than its
cause.

- **GraphQL errors arrive in a 200 OK body.** The simulator checked only
  `response.status().is_success()`, so every rejected mutation logged a cheerful
  HIT line and wrote nothing. Now parses `errors[]`. **Any new HTTP caller must do
  the same.**
- **Enum drift.** Simulator `TargetType` had `Artillery`/`Aircraft`; the schema has
  `AIR_DEFENSE`/`SUPPLY`. Silent coercion failures. async-graphql serialises
  variants as SCREAMING_SNAKE (`Agm114Hellfire` → `AGM114_HELLFIRE`).
- **Convoy id mismatch.** `activeConvoys` returns a fixed UUID; the simulator
  generated a random one. Both processes healthy, every panel empty. The simulator
  now pins to `ConvoySimulator::DEMO_CONVOY_ID`, overridable via
  `DRONE_CONVOY_ID`. **When `activeConvoys` becomes real, keep them agreeing.**
- **GraphQL variables not camelCased.** `fetch_leaderboard`'s `Variables` struct
  lacked `#[serde(rename_all = "camelCase")]` while the query declared `$convoyId`.
  Rejected on every poll, which read as a connectivity failure. Check this on every
  new query.
- **CSS class collision.** `main.css:245` defines `.drone-marker` (green card
  badge). Map markers use `.drone-air-marker`; keep them separate.
- **`var()` in SVG presentation attributes is unreliable.** Custom properties are
  substituted on CSS declarations. `assets/images/*.svg` use hex attributes with a
  `<style>` block re-theming via classes.
- **Container builds:** base was `rust:1.83` against `edition = "2024"` (needs
  ≥1.85), and the API Containerfile copied `drone-graphql-api` when the `[[bin]]`
  is `drone-api`. Both fixed; `ARG RUST_VERSION` is now parameterised.
- **`-C lto` via RUSTFLAGS is incompatible with `-C embed-bitcode=no`**, which
  Cargo passes for proc-macros — it fails every proc-macro dependency at once.
  Codegen settings live in `[profile.release]` in the root `Cargo.toml`, never in
  RUSTFLAGS.
- **ScyllaDB has no `/docker-entrypoint-initdb.d` hook** — that is a Postgres/MySQL
  convention. Schema is applied explicitly by `deps-up` and by the `scylla-init`
  compose service.

## 1.9 How to run

```
make build          # touch sweep + full build
make serve          # API + trunk dev server NATIVELY; podman only for Scylla/Redis
make run-simulator  # second terminal
```

Dashboard `http://localhost:3000`, playground `http://localhost:8080/graphql`.
`make stack-up` is the all-in-containers path and needs podman-compose.

`make build` runs `touch` first, because overlaying a zip restores the archive's
mtimes and Cargo then skips rebuilding files that look older than their artifacts.
`TOUCH=0` opts out.

---

# PART 2 — ARGOCD: P4 AND P5

Both corners of the ApplicationSet story are built and **unverified by Helm** —
there was no Helm binary in the authoring environment. Run `helm template` on both
charts before anything else.

```
helm template argocd-apps/app-sets/
helm template argocd-apps/cluster-apps/drone-convoy-tracker \
  -f argocd-apps/cluster-apps/drone-convoy-tracker/env/prod/values-prod.yaml
```

For `app-sets`, confirm the rendered `cluster-addons` ApplicationSet has **three**
generators and that `{{.path.basename}}`-style placeholders survive verbatim into
the output. That escaping is the one thing reading cannot prove.

## 2.0 Already done

- `app-sets/` rewritten: generator paths corrected to repo-root-relative,
  `goTemplate: true` + `goTemplateOptions: ["missingkey=error"]`, all placeholders
  dotted, `repo.url` driven from `values.yaml`, ArgoCD-side escaping confined to
  `templates/_helpers.tpl` behind backticks.
- **Sync waves without losing the glob.** Each wave group is its own matrix
  generator scoped to explicit directories and stamped via the git generator's
  `values.wave`. A catch-all globs `cluster-addons/*` with `exclude: true` on
  everything already claimed, so a new addon needs no edit and sorts into
  `addonDefaultWave`. Wave -2: cert-manager, external-secrets, sealed-secrets.
  Wave -1: kyverno, crossplane, keda, scylla-operator. Catch-all: argocd, cast-ai,
  external-dns, goldilocks, kargo, vault. Apps: wave 10.
- **Asymmetric deletion posture.** cluster-addons carries
  `applicationsSync: create-update`, `preserveResourcesOnDeletion: true` and the
  `resources-finalizer.argocd.argoproj.io` finalizer; cluster-apps has normal
  semantics so removing a chart removes the Application. **Note:**
  per-ApplicationSet `applicationsSync` is ignored unless the controller runs with
  `--enable-policy-override`, set in `cluster-addons/argocd/values.yaml`. Without
  it the setting looks configured and silently does nothing.
- AppProjects fixed: `sourceRepos` was `https://github.com/eql/*`, which would have
  rejected every generated Application; now driven from values. `cluster-apps` has
  `clusterResourceWhitelist: []`.
- Addon list: `cnpg` → `scylla-operator`; `cluster-api` and `kured` deleted;
  `external-dns` added (source `gateway-httproute`, provider deliberately unset).
- `cluster-apps/drone-convoy-tracker/` chart built with the full
  screaming-architecture set: `app.yaml`, `app-service.yaml` (Cilium Gateway +
  HTTPRoute), `app-config.yaml`, `app-storage.yaml` (ScyllaCluster CR),
  `app-secrets.yaml` (ExternalSecret + cert-manager Certificate),
  `app-scaling.yaml` (HPA + VPA recommender + KEDA), `app-metrics.yaml`.

## 2.1 P4 — addon values hygiene

**The repo currently argues with itself. This is what a careful reader catches.**

1. **Ingress contradicts the Gateway API decision.** `argocd`, `goldilocks` and
   `vault` prod values all carry `ingress.enabled: true`,
   `ingressClassName: nginx` and `nginx.ingress.kubernetes.io/*` annotations —
   pointing at a controller this repo never installs. Replace each with an
   HTTPRoute in that wrapper chart's own `templates/`, parented to a shared
   Gateway. **Highest-priority P4 item.**
2. **cert-manager feature gate.** Prod sets only
   `AdditionalCertificateOutputFormats=true`. Issuing certificates from Gateway
   `listeners[].tls` also needs `ExperimentalGatewayAPISupport=true`, or the
   Gateway migration has no certificate path.
3. **Dead toleration.** cert-manager tolerates `node-role.kubernetes.io/master`,
   removed in Kubernetes 1.25. Use `control-plane`.
4. **Stale chart pins, all of them.** argo-cd 5.46.7 (current ~10.x),
   cert-manager v1.13.2, crossplane 1.14.1, kyverno 3.0.5, keda 2.12.0,
   external-secrets 0.9.9, sealed-secrets 2.13.3, kargo 0.3.0, vault 0.25.0,
   goldilocks 6.7.0. Read release notes for **Crossplane and Kargo** specifically —
   both have had major transitions. Re-pin scylla-operator (1.21.0) and
   external-dns (1.21.1) against `helm search repo --versions` as well.
5. **argo-cd values migration**, which falls out of item 4:
   `server.config` → `configs.cm`, `server.rbacConfig` → `configs.rbac`. Then add
   an RBAC policy mapping a platform group to the `cluster-addons` project and an
   app-team group to `cluster-apps`, with `policy.default: role:readonly`. Keep the
   IdP connector a **commented example** — a reference must not hardcode anyone's
   identity provider. Do NOT hand-roll `argocd-rbac-cm` in wrapper templates; the
   subchart creates it and you will end up with two resources of one name.
6. **Crossplane provider install.** `provider.packages: [aws, kubernetes]` is the
   old path. Current Crossplane uses `Provider` CRs, and the monolithic AWS
   provider split into per-service families.
7. **Secrets story.** Decision made: Vault as backing store, External Secrets
   Operator as in-cluster sync, sealed-secrets kept **explicitly scoped to
   bootstrap-only** secrets that must exist before ESO runs. Implement it and state
   it in the README — three overlapping secret systems with no stated primary is
   the first thing a reviewer asks about.
8. **Brand sweep.** `eql.cloud` hostnames and `eql-org` still appear in addon env
   values; only the AppProject `sourceRepos` was fixed. Separately, `CHANGEME-ORG`
   appears in `app-sets/values.yaml` (`repo.url`) and
   `cluster-addons/argocd/values.yaml` (`projects.sourceRepos`) — **these two must
   agree**, or Applications generate and then refuse to sync with a project
   permission error, which is a confusing failure to debug.
9. **`cast-ai`** is still an addon: third-party SaaS agent needing an API key,
   cost-optimisation rather than platform. Candidate for the same treatment as
   cluster-api and kured.
10. **`cluster-secrets/` is spec-only** — it holds just a README. Cluster
    registration secrets (`argocd.argoproj.io/secret-type: cluster`) must exist for
    the list generator destinations to resolve. Document the bootstrap ordering:
    ESO and sealed-secrets are themselves addons, so the first cluster secret
    cannot be encrypted by a controller that is not yet running. Standard answer is
    one manually-applied bootstrap secret for the hub, everything after that
    via ESO.

**Post-delivery refinement, not a blocker:** once `cluster-secrets/` is populated,
swap the hardcoded list generator for the **cluster generator** with label
selectors (`env: prod`), so adding a cluster becomes a Secret rather than a repo
edit.

## 2.2 P5 — the README

For a guide this is arguably the highest-value artifact. Lead with the three things
App-of-Apps cannot do cleanly:

1. No per-child `Application.yaml` to write or maintain.
2. Environment promotion by **directory**, not by branch.
3. RBAC boundaries via AppProject, not everything in `default`.

Then cover:

- **Blast radius.** Deleting an ApplicationSet deletes every Application it
  generated and prunes their resources. Explain the asymmetric posture (protective
  for addons, normal for apps) and the `--enable-policy-override` dependency. This
  is the first objection an App-of-Apps holdout raises — answer it before it is
  asked.
- **The ScyllaDB point.** Upstream ScyllaDB documentation states Helm only creates
  CRDs on first install and never updates them, and recommends their GitOps
  manifest path over Helm for that reason. ArgoCD renders with `--include-crds` and
  re-applies every sync, so the ApplicationSet path closes a gap the vendor
  documents as a Helm limitation. A vendor describing the exact problem your
  delivery solves is the strongest paragraph available.
- **Worked drop-in examples.** `external-dns` was added with no edit to any shared
  file (catch-all glob). `drone-convoy-tracker` generates four Applications from
  one directory. Show both.
- **The wave example.** cert-manager (wave -2) must reconcile before
  scylla-operator (wave -1) because it issues the cert for the operator's webhook
  server. Concrete beats abstract.
- The existing `docs/*.png` diagrams already carry most of the comparison:
  `argocd-app-of-apps-v-appsets.png`, `appsets-structure.png`, and the three
  `argocd-*-cp-cluster-*.png` hub-and-spoke topologies.
- A clearly separated appendix for VKS/VCF-specific configuration, skippable
  without losing the architecture.

## 2.3 Verification — KinD on Apple Silicon

Planned, not yet done. Prerequisites, none of them one-liners:

- Create the cluster with the default CNI **disabled**, then install Cilium with
  `gatewayAPI.enabled=true`.
- Install the **Gateway API CRDs** before Cilium.
- `cloud-provider-kind` or MetalLB — KinD has no LoadBalancer, so the Gateway never
  gets an address without one.
- The chart's ScyllaCluster will not come up as written: `developerMode: false`
  assumes tuned nodes, and even the nonprod overlay's single 4Gi member is heavy
  for a KinD node. **Add a `scylla.developerMode` value** and drop nonprod
  resources — five minutes of work that should be decided, not discovered at 4pm.

A KinD run proves ApplicationSet mechanics, chart rendering and sync-wave ordering.
It will not prove the Vault/ESO path without standing up Vault; skip that.

---

# PART 3 — CONVENTIONS

- **Deliverable zips carry only the delta** — files changed since the previous zip,
  plus a `DELTA.md` naming what changed and why. Never re-ship unchanged files.
  Deletions must be called out explicitly in prose; an overlay cannot remove files.
- **Make targets stay a small front door** of bare verbs. The eight in `make help`
  are the interface; everything else is machinery under `make help-all`.
- **Rust standards:** no bare `unwrap()`/`expect()` in non-test code, `map_err` and
  `?` on `Result` returns, `thiserror` per domain crate with `anyhow` at the binary,
  lifetimes over `Arc<Mutex<T>>`, no reflexive `clone()`. The workspace already
  sets `clippy::pedantic` + `nursery` and `unsafe_code = "forbid"`, and there is no
  `Arc<Mutex<T>>` anywhere in the tree — keep it that way. Roughly 38 `unwrap()`s
  remain: mostly tests, WASM JS-interop in `map.rs`, and four
  `Normal::new(...).unwrap()` in the simulator marked `TODO(unwrap-sweep)` —
  infallible by construction but still fallible constructors.
- **Screaming architecture** for chart templates: a file's presence means that
  concern is present. Do not ship an empty `app-storage.yaml`.
- Nothing secret is templated into this repository — only references to where
  values live.
