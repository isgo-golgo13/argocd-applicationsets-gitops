# KinD Cluster Setup — Step by Step

This gets you a real 6-node Kubernetes cluster on your Mac with the drone tracker running behind HTTPS at **https://drone.localtest.me**. Every step says what to type, what you should see, and what to do if you don't see it. Follow it top to bottom — don't skip.

**Time:** ~30 minutes the first time (mostly waiting for downloads). **Disk:** ~10 GB. **RAM:** give your container VM at least 8 GB (see Step 1).

---

## Step 0 — What you are building

Six containers on your Mac pretend to be six Kubernetes machines: 3 control-plane, 3 workers. Inside them we install the platform pieces (Cilium networking with Gateway API, cert-manager, secrets, autoscalers, ScyllaDB operator), then install the drone tracker on top. Your browser reaches it on ports 80/443, forwarded into the first control-plane node.

```
Your Mac ──:443──▶ [control-plane-1] ──▶ Cilium Gateway ──▶ dashboard + API pods
                   [control-plane-2]                        ScyllaDB (3 pods → 1 on KinD)
                   [control-plane-3]                        Redis
                   [worker-1..3]
```

---

## Step 1 — Install the tools (one time)

Open Terminal and run these one at a time.

```shell
brew install kind kubectl helm
```

You also need a container engine. **Pick ONE:**

**Option A — Podman** (what this repo uses everywhere else):
```shell
brew install podman
podman machine init --cpus 4 --memory 8192 --disk-size 40
podman machine start
export KIND_EXPERIMENTAL_PROVIDER=podman     # put this line in ~/.zshrc too
```

**Option B — Docker Desktop:** install it, open it, then in *Settings → Resources* set **8 GB memory, 4 CPUs**. Nothing else to do.

**Check it worked:**
```shell
kind version && kubectl version --client && helm version
```
You should see three version lines and no errors. If any command says "not found", re-run its `brew install`.

> **Why 8 GB?** ScyllaDB is a C++ database that takes memory seriously, and there are six nodes. Less than 8 GB and pods will sit in `Pending` forever.

---

## Step 2 — Go to the project

```shell
cd /path/to/drone-convoy-attack-tracker-leptos-rs
```

Every command below is run from this directory. Check you're in the right place:
```shell
ls deploy/cluster
```
You should see: `kind-bootstrap.sh  kind-config.yaml  kind-expose.sh  README-setup.md`

---

## Step 3 — Create the cluster and install the platform

This is the big one. It creates the 6 nodes and installs everything the app depends on, **in the right order** (order matters — the script knows it).

```shell
make kind-up
```

**What you'll see:** green `▶` lines walking through: Creating KinD cluster → Gateway API CRDs → Cilium → cert-manager → External Secrets → KEDA → VPA → metrics-server → scylla-operator → `Bootstrap complete`. Then a table of 6 nodes all saying `Ready`.

**How long:** 8–15 minutes. Cilium alone can take 3–4 minutes to roll out. Go get coffee. Do NOT Ctrl-C it.

**If it fails:** just run `make kind-up` again. The script is idempotent — it picks up where it left off and never breaks what already worked.

**Check it worked:**
```shell
kubectl get nodes
```
Six lines, all `Ready`. If any say `NotReady`, wait 60 seconds and check again (Cilium is still starting).

---

## Step 4 — Build the app images and put them in the cluster

The cluster can't pull from your Mac's image registry, so we build the two images and copy them into the nodes.

```shell
make kind-load
```

**What you'll see:** two container builds (API and frontend — the first build compiles all the Rust in release mode, ~5–10 min), then `Image: "..." with ID "..." not yet present on node ... loading` for each of the six nodes.

**Check it worked:** the last lines say the images loaded onto all nodes with no errors.

---

## Step 5 — Install the drone tracker

```shell
make kind-deploy
```

**What you'll see:** Helm creates the `drone-ops` namespace and installs the chart, then waits (`--wait`, up to 15 min) for everything to be healthy. **ScyllaDB is the slow part** — the operator has to create the database pod and it takes 2–4 minutes to bootstrap. Then a schema job runs. Then you'll see:

```
✓ Gateway exposed:  https://drone.localtest.me  (self-signed cert: accept the warning)
  Playground:       https://drone.localtest.me/graphql
```

**Check it worked:**
```shell
make kind-status
```
You want to see the Gateway with `PROGRAMMED: True`, the HTTPRoute, a Certificate with `READY: True`, an ExternalSecret `SecretSynced`, the ScyllaCluster, an HPA, a VPA, and pods all `Running`.

---

## Step 6 — Open it

In your browser go to: **https://drone.localtest.me**

**You WILL get a certificate warning.** That's expected — Let's Encrypt can't issue certificates for a cluster the internet can't reach, so this cluster uses a self-signed one. Click *Advanced → Proceed* (Chrome) or *Show Details → visit this website* (Safari). You only do this once.

You should see the dark green tactical HUD with **ONLINE** in the top right. The panels will be empty until you fly a mission (Step 7).

The GraphQL playground is at **https://drone.localtest.me/graphql**.

> `localtest.me` is a public DNS name that always points at `127.0.0.1`. That's why there's nothing to add to `/etc/hosts`.

---

## Step 7 — Fly a mission

The simulator runs on your Mac and posts to the API through the Gateway. Two settings: where the API is, and permission to accept the self-signed certificate:

```shell
DRONE_API_URL=https://drone.localtest.me/graphql DRONE_INSECURE_TLS=1 make run-simulator
```

Watch the dashboard: within seconds the four ALPHA drones appear, fuel drains, waypoints advance, airframes fly the map. After ~75 seconds (25% mission progress) engagements start and the leaderboard and feed light up. The mission lasts 5 minutes.

`DRONE_INSECURE_TLS=1` only exists for this self-signed local case — the simulator prints a loud warning when it's on. Never set it against a real environment.

---

## Everyday commands

| I want to… | Type |
|---|---|
| See what's running | `make kind-status` |
| See all pods | `kubectl -n drone-ops get pods` |
| Read the API logs | `kubectl -n drone-ops logs deploy/drone-convoy-attack-tracker-api -f` |
| Redeploy after a code change | `make kind-load && make kind-deploy` |
| Wipe the app but keep the cluster | `helm uninstall drone -n drone-ops` |
| **Delete the whole cluster** | `make kind-down` |
| Start over from scratch | `make kind-down && make kind-up` |

---

## When something's wrong

**A pod is `Pending` forever.**
Almost always memory. `kubectl -n drone-ops describe pod <name>` — look at the bottom for `Insufficient memory`. Give the container VM more RAM (Step 1) and `make kind-down && make kind-up`.

**Browser says "connection refused" on drone.localtest.me.**
The Gateway isn't exposed yet. Run `bash deploy/cluster/kind-expose.sh` by hand and read what it prints. Then `kubectl -n drone-ops get svc` — you want `cilium-gateway-drone-gateway` of type `NodePort` on 30080/30443.

**Certificate is `READY: False`.**
`kubectl -n drone-ops describe certificate` — the bottom "Events" say why. Usually cert-manager is still starting; wait a minute. If it mentions the ClusterIssuer, run `kubectl get clusterissuer` — `selfsigned-cluster-issuer` must exist (Step 3 creates it; re-run `make kind-up`).

**ScyllaCluster never becomes ready.**
`kubectl -n drone-ops get pods -l app=scylla` then `kubectl -n drone-ops logs <scylla-pod> -c scylla`. On a laptop the fix is nearly always more RAM. The nonprod values already run Scylla in `developerMode` with 1 member for exactly this reason.

**`make kind-up` says the cluster already exists but nodes are gone.**
`kind delete cluster --name drone-ops` then `make kind-up`.

**Podman: `kind create` fails immediately.**
You forgot `export KIND_EXPERIMENTAL_PROVIDER=podman`. Set it and retry.

**I want to see everything at once.**
```shell
kubectl get all -A | grep -v Running | grep -v Completed
```
Anything that prints is something not healthy.

---

## What the files in this directory are

- **`kind-config.yaml`** — the cluster shape: 3 control-plane + 3 workers, no default networking (Cilium provides it), and the port forwards (Mac :80/:443 → node 30080/30443) that make the browser work.
- **`kind-bootstrap.sh`** — installs every platform piece in dependency order. Safe to run repeatedly.
- **`kind-expose.sh`** — makes the app's Gateway reachable from the Mac. `make kind-deploy` runs it for you.

You never need to edit these to get running. Change `values-nonprod.yaml` in `deploy/kubernetes/drone-convoy-attack-tracker/` if you want different replica counts or resources.
