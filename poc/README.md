# poc/README.md — creating the three KinD clusters

**Just the clusters.** This gets you three empty KinD clusters, correctly
networked, ready for ArgoCD. Everything after that — installing ArgoCD,
AppProjects, ApplicationSets, deploying the app — is `../README-setup-poc.md`.

Split deliberately: cluster creation is where most of the fiddly, machine-
specific problems live. Get through this once and the rest is the same on any
machine.

Assumes nothing beyond `kind` being installed.

---

## What you end up with

| KinD cluster | kubectl context | Role | Ports on your Mac |
|---|---|---|---|
| `gitops-hub` | `kind-gitops-hub` | ArgoCD. No workloads. | 8080 |
| `workload-nonprod` | `kind-workload-nonprod` | A managed spoke. No ArgoCD. | 8081, 8443 |
| `workload-prod` | `kind-workload-prod` | A managed spoke. No ArgoCD. | 8082, 8444 |

Three separate Kubernetes clusters, all running as containers on your laptop,
all on one shared container network so the hub can reach the spokes.

---

## Step 0 — Podman, and telling kind to use it

Skip if you use Docker; everything below works either way, but the project
builds with Podman and mixing runtimes means your images land in a store kind
cannot read.

```bash
export KIND_EXPERIMENTAL_PROVIDER=podman
echo 'export KIND_EXPERIMENTAL_PROVIDER=podman' >> ~/.zshrc
```

**Without this, `kind create cluster` looks for Docker and fails.** It is the
single most common first-run problem.

On macOS, Podman runs inside a VM, and **the VM's limits are what matter** —
not your Mac's. Three clusters need a generous one:

```bash
podman machine stop
podman machine rm
podman machine init --cpus 10 --memory 20480 --disk-size 100
podman machine start

podman machine inspect | grep -iE "cpus|memory|diskSize"
```

The **100 GB disk** matters more than you would expect. The default is smaller,
and five KinD nodes plus Cilium, ScyllaDB and your images will fill it. Running
out shows up as pods stuck in `ImagePullBackOff` with no obvious cause.

Check it is alive:

```bash
podman info | head -20
kind version
```

---

## Step 1 — Create the three clusters

Run these one at a time. Each takes 60–90 seconds.

```bash
cd argocd-applicationsets-gitops

kind create cluster --config poc/kind/hub.yaml
kind create cluster --config poc/kind/spoke-nonprod.yaml
kind create cluster --config poc/kind/spoke-prod.yaml
```

Confirm:

```bash
kind get clusters
```

```
gitops-hub
workload-nonprod
workload-prod
```

```bash
kubectl config get-contexts
```

Three contexts, prefixed `kind-`. That prefix is kind's doing — the cluster is
`workload-prod`, the context is `kind-workload-prod`. Mixing them up is a
common early mistake.

---

## Step 2 — Understand what you are looking at

```bash
kubectl --context kind-gitops-hub get nodes
```

```
NAME                       STATUS   ROLES           AGE   VERSION
gitops-hub-control-plane   Ready    control-plane   2m    v1.31.x
```

Now a spoke:

```bash
kubectl --context kind-workload-nonprod get nodes
```

```
NAME                             STATUS     ROLES           AGE
workload-nonprod-control-plane   NotReady   control-plane   2m
workload-nonprod-worker          NotReady   <none>          2m
```

### `NotReady` is correct. Do not debug it.

The spoke configs set `disableDefaultCNI: true`, so no network plugin is
installed and the kubelet correctly reports the node as not ready. It stays that
way until you install Cilium (step 3 of `../README-setup-poc.md`).

The hub keeps its default CNI — nothing on it uses Gateway API — so it is Ready
immediately. That difference between hub and spokes is intentional, and it is
worth noticing: the control plane does not have to look like the clusters it
manages.

Two other things the spoke configs set, both required by Cilium:

- `kubeProxyMode: none` — Cilium replaces kube-proxy, so Cilium must then be
  installed with `kubeProxyReplacement=true` or nothing resolves a Service.
- Two nodes instead of one — enough to show pods spreading across nodes.

---

## Step 3 — The addresses the hub will use

This is the step that breaks first runs, so it is worth doing now while nothing
else is in the way.

Your kubeconfig reaches a spoke at `https://127.0.0.1:<random-port>`. **That
address is useless to the hub**, because inside the hub's own container
`127.0.0.1` means the hub. The hub reaches spokes over the shared container
network instead.

Get the real addresses:

```bash
for env in nonprod prod; do
  echo -n "${env}: "
  kubectl --context "kind-workload-${env}" get nodes \
    -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}'
  echo
done
```

```
nonprod: 10.89.0.4
prod: 10.89.0.6
```

Those are the addresses that go into the values files. Read from the node
object rather than `podman inspect` or `docker inspect`, so the same command
works under either runtime.

**You do not have to copy these by hand.** `poc/register-spokes.sh` reads them,
registers both spokes with ArgoCD, and rewrites both values files for you. It
runs in step 6 of the main tutorial, after ArgoCD exists.

---

## Step 4 — What the two values files do

The clusters you just made are empty. These two files tell ArgoCD what to put
on them, and they must agree with each other.

### `poc/values-app-projects-kind.yaml` — the guardrails

Installed **first**, on the hub. Creates the AppProjects: the policy objects
that decide what any Application is allowed to do.

```yaml
environments:
  - name: nonprod
    server: https://REPLACE-ME-NONPROD-IP:6443   # filled in by register-spokes.sh
  - name: prod
    server: https://REPLACE-ME-PROD-IP:6443

perEnvironment: true
```

`perEnvironment: true` creates a separate project per environment, each
permitting exactly **one** destination cluster. An Application generated for
nonprod then physically cannot deploy to prod — ArgoCD rejects it.

### `poc/values-app-sets-kind.yaml` — what gets deployed

Installed **second**, on the hub. Creates the ApplicationSets that generate the
Applications.

```yaml
clusters:
  - name: nonprod
    url: https://REPLACE-ME-NONPROD-IP:6443       # same values as above
    valuesFile: values-nonprod.yaml
  - name: prod
    url: https://REPLACE-ME-PROD-IP:6443
    valuesFile: values-prod.yaml

addonExclude:
  - argocd          # ArgoCD lives on the HUB, never on a spoke
  - sealed-secrets
  - crossplane
  ...
```

Two things to understand:

**The URLs must match byte for byte** between the two files. ArgoCD compares
them as text — `https://10.89.0.4:6443` and `https://10.89.0.4:6443/` are two
different destinations as far as an AppProject is concerned, and the mismatch
presents as every Application refusing to sync.

**`addonExclude` keeps `argocd` off the spokes.** Without it, the ApplicationSet
installs ArgoCD on every spoke, each spoke becomes its own control plane, and
the whole hub-and-spoke idea evaporates. The other exclusions are just to keep
the laptop demo quick — delete a line to bring one back.

### Order matters

Projects first, then ApplicationSets. ArgoCD refuses to sync an Application
whose project does not exist, and that failure does not resolve on its own.

---

## Step 5 — Sanity check before moving on

```bash
kind get clusters | wc -l          # 3
kubectl --context kind-gitops-hub get nodes        # Ready
kubectl --context kind-workload-nonprod get nodes  # NotReady (correct)
kubectl --context kind-workload-prod get nodes     # NotReady (correct)
podman ps | wc -l                  # 5 node containers + header
```

If all four look right, continue with **`../README-setup-poc.md` step 3**
(Gateway API CRDs and Cilium). That document picks up exactly here.

---

## Starting over

Cheap and often the fastest fix:

```bash
kind delete cluster --name workload-prod
kind delete cluster --name workload-nonprod
kind delete cluster --name gitops-hub

git checkout poc/values-app-sets-kind.yaml poc/values-app-projects-kind.yaml
```

The `git checkout` restores the `REPLACE-ME` placeholders, since
`register-spokes.sh` rewrites those files with addresses that will not survive a
rebuild. New clusters get new IPs.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `ERROR: failed to create cluster: ... docker` | provider not set | `export KIND_EXPERIMENTAL_PROVIDER=podman` |
| Cluster creation hangs or the machine crawls | Podman VM too small | `podman machine init --cpus 10 --memory 20480 --disk-size 100` |
| `node(s) already exist for a cluster with the name` | previous run not deleted | `kind delete cluster --name <name>` |
| Spoke nodes `NotReady` | no CNI installed yet | Expected. Cilium comes next |
| Port already allocated | 8080–8444 in use | Stop the other process, or edit `extraPortMappings` in `poc/kind/*.yaml` |
| `kubectl` talks to the wrong cluster | context confusion | Always pass `--context kind-<cluster>` |
| Node IP lookup returns nothing | cluster still starting | Wait 30s; InternalIP is set before the node is Ready |

Useful:

```bash
kind get clusters
kubectl config get-contexts
podman ps --format '{{.Names}}\t{{.Status}}'
kind export logs /tmp/kind-logs --name workload-nonprod
```

---

## Where your app images come from

The chart in `argocd-apps/cluster-apps/drone-convoy-tracker/` defaults to
`docker.io/isgogolgo13`, so once you have pushed images the spokes pull them
straight from Docker Hub — no side-loading needed, and it behaves the same way a
real cluster will.

From the **application** repository:

```bash
make login          # Docker Hub, use an access token not your password
make release        # build both images, tag, push
```

Then point the chart at the tag `make push` prints, in each environment overlay:

```yaml
# argocd-apps/cluster-apps/drone-convoy-tracker/env/nonprod/values-nonprod.yaml
image:
  registry: docker.io/isgogolgo13
  api:
    repository: drone-convoy-tracker-api
    tag: "0.1.0-abc1234"
  frontend:
    repository: drone-convoy-tracker-frontend
    tag: "0.1.0-abc1234"
```

Pulling from Docker Hub is the better path for this POC — it exercises the same
registry mechanics the real deployment will use. Side-loading with
`make kind-load KIND_CLUSTER=workload-nonprod` is there for working offline or
iterating without a push.

**Architecture note:** on Apple silicon `make release` pushes **arm64 only**,
which is fine for KinD on the same Mac and wrong for an amd64 cluster. Use
`make push-multiarch` when the images need to run somewhere else.
