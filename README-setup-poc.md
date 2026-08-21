# README-setup-poc.md

**Three KinD clusters, one ArgoCD hub, two workload spokes, and a Rust
full-stack application deployed entirely by GitOps.**

Written to be followed literally. Every command is copy-pasteable, every wait
is explained, and every step that commonly fails has a "if this went wrong"
box. Where there are two ways to do something — the ArgoCD **web UI** and the
**`argocd` CLI** — both are given.

---

## What you are building

```
                    ┌─────────────────────────────┐
                    │   KinD cluster 1            │
                    │   gitops-hub                │
                    │                             │
                    │   ArgoCD                    │
                    │   AppProjects               │
                    │   ApplicationSets           │
                    │                             │
                    │   NO application workloads  │
                    └──────────┬──────────────────┘
                               │
                 reaches out   │   (spokes never call in)
                    ┌──────────┴──────────┐
                    ▼                     ▼
      ┌───────────────────────┐ ┌───────────────────────┐
      │ KinD cluster 2        │ │ KinD cluster 3        │
      │ workload-nonprod      │ │ workload-prod         │
      │                       │ │                       │
      │ NO ArgoCD             │ │ NO ArgoCD             │
      │                       │ │                       │
      │ Cilium + Gateway API  │ │ Cilium + Gateway API  │
      │ cert-manager          │ │ cert-manager          │
      │ external-secrets      │ │ external-secrets      │
      │ kyverno               │ │ kyverno               │
      │ keda                  │ │ keda                  │
      │ scylla-operator       │ │ scylla-operator       │
      │ platform-gateway      │ │ platform-gateway      │
      │                       │ │                       │
      │ drone-convoy-tracker  │ │ drone-convoy-tracker  │
      └───────────────────────┘ └───────────────────────┘
```

The spokes do not know GitOps exists. They have no ArgoCD, no agent, no
webhook back to the hub. The hub holds a kubeconfig for each and applies
manifests. That is **hub and spoke**, and it is why a spoke can be destroyed
and rebuilt from nothing in minutes.

### The four objects, and who creates what

| Object | Created by | Answers |
|---|---|---|
| **AppProject** | you, once, via Helm | *What is any Application allowed to do?* |
| **ApplicationSet** | you, once, via Helm | *Which Applications should exist?* |
| **Application** | the ApplicationSet controller, automatically | *What desired state gets reconciled?* |
| the workloads | the Application, on a spoke | the actual Deployments, Services, CRs |

You will hand-write **zero** Application manifests. Watching ~14 of them appear
from two ApplicationSets is the first "oh" moment of the demo.

---

## Before you start

| Tool | Check | Install |
|---|---|---|
| Docker | `docker version` | Docker Desktop, Colima or Podman-in-Docker mode |
| kind | `kind version` | `brew install kind` |
| kubectl | `kubectl version --client` | `brew install kubectl` |
| helm | `helm version` | `brew install helm` |
| argocd CLI | `argocd version --client` | `brew install argocd` |
| jq | `jq --version` | `brew install jq` |

**Resources.** Three KinD clusters with Cilium and ScyllaDB want roughly
**8 CPUs and 12 GB of RAM** given to Docker. On Docker Desktop:
Settings → Resources. If you have less, run one spoke instead of two — the
architecture is identical, you just lose the per-environment AppProject
demonstration in step 12.

**A fork you can push to.** ArgoCD clones the repository over the network; it
cannot read your laptop's filesystem. Fork this repo, push it, and note the
URL. Everywhere below that says `CHANGEME-ORG`, use your fork.

```bash
export GITOPS_REPO="https://github.com/YOUR-ORG/argocd-applicationsets-gitops"
```

---

## Step 1 — Point the repo at your fork

Three files carry the repository URL. They must agree, or every Application is
rejected by its project.

```bash
cd argocd-applicationsets-gitops

# macOS uses BSD sed; this form works on both BSD and GNU.
for f in argocd-apps/app-sets/values.yaml \
         poc/values-app-sets-kind.yaml \
         poc/values-app-projects-kind.yaml \
         argocd-apps/app-projects/values.yaml; do
  sed "s|https://github.com/CHANGEME-ORG|${GITOPS_REPO%/*}|g" "$f" > "$f.tmp" && mv "$f.tmp" "$f"
done

grep -rn "CHANGEME-ORG" argocd-apps poc || echo "OK: no placeholders left"
```

Commit and push. **ArgoCD reads Git, not your working copy** — an uncommitted
change is invisible to it. This trips everyone up at least once.

```bash
git add -A && git commit -m "poc: point at my fork" && git push
```

---

## Step 2 — Create the three clusters

```bash
kind create cluster --config poc/kind/hub.yaml
kind create cluster --config poc/kind/spoke-nonprod.yaml
kind create cluster --config poc/kind/spoke-prod.yaml

kind get clusters
```

Expect `gitops-hub`, `workload-nonprod`, `workload-prod`.

> **The spokes will show NotReady. That is correct.**
> ```
> kubectl --context kind-workload-nonprod get nodes
> NAME                             STATUS     ROLES
> workload-nonprod-control-plane   NotReady   control-plane
> ```
> The spoke configs set `disableDefaultCNI: true`, so no CNI is installed and
> the kubelet correctly reports the node as not ready. Cilium fixes it in step
> 3. **Do not debug this.** The hub, which keeps its default CNI, will be Ready
> immediately.

---

## Step 3 — Install Gateway API CRDs, then Cilium, on both spokes

Order matters. Cilium only enables its Gateway API support if the CRDs already
exist when it starts. Install Cilium first and you get a working CNI with no
`cilium` GatewayClass, and no error explaining why.

```bash
helm repo add cilium https://helm.cilium.io/
helm repo update

for env in nonprod prod; do
  CTX="kind-workload-${env}"
  echo "=== ${env} ==="

  # 1. Gateway API CRDs FIRST
  kubectl --context "$CTX" apply -f \
    https://github.com/kubernetes-sigs/gateway-api/releases/download/v1.2.1/standard-install.yaml

  # 2. The API server address as seen from INSIDE the cluster's own containers
  API_IP=$(docker inspect "workload-${env}-control-plane" \
    --format '{{ (index .NetworkSettings.Networks "kind").IPAddress }}')

  # 3. Cilium, replacing kube-proxy (the KinD config set kubeProxyMode: none)
  # Pinned. Cilium's flags move between minors, and an unpinned install is how
  # a tutorial that worked last month stops working mid-screenshare. Check
  # `helm search repo cilium/cilium --versions` and the Cilium KinD guide if you
  # raise it.
  helm --kube-context "$CTX" upgrade --install cilium cilium/cilium \
    --version 1.16.5 \
    --namespace kube-system \
    --set kubeProxyReplacement=true \
    --set k8sServiceHost="${API_IP}" \
    --set k8sServicePort=6443 \
    --set gatewayAPI.enabled=true \
    --set l2announcements.enabled=true \
    --set externalIPs.enabled=true \
    --wait --timeout 5m
done
```

Verify both spokes:

```bash
for env in nonprod prod; do
  echo "=== ${env} ==="
  kubectl --context "kind-workload-${env}" get nodes
  kubectl --context "kind-workload-${env}" get gatewayclass
done
```

You want `Ready` nodes and:

```
NAME     CONTROLLER                     ACCEPTED
cilium   io.cilium/gateway-controller   True
```

> **If `get gatewayclass` returns nothing**, Cilium started before the CRDs
> existed. Fix by restarting the operator so it re-detects them:
> ```bash
> kubectl --context kind-workload-nonprod -n kube-system rollout restart deploy/cilium-operator
> ```

---

## Step 4 — Install ArgoCD on the hub, and only the hub

```bash
kubectl config use-context kind-gitops-hub

helm repo add argo https://argoproj.github.io/argo-helm
helm repo update

helm upgrade --install argocd argo/argo-cd \
  --namespace argocd --create-namespace \
  --set configs.params."server\.insecure"=true \
  --set applicationSet.enabled=true \
  --set-string applicationSet.extraArgs[0]="--enable-policy-override" \
  --wait --timeout 10m
```

Two flags worth understanding, because you will be asked:

- **`server.insecure=true`** — terminates TLS at the browser instead of inside
  the pod, so a port-forward works without certificate warnings. Never in
  production.
- **`--enable-policy-override`** — without it, the per-ApplicationSet
  `applicationsSync: create-update` setting in `cluster-addons.yaml` is
  **silently ignored**. The addon layer would lose its deletion protection and
  nothing would tell you. This is a genuine footgun in ArgoCD.

```bash
kubectl -n argocd get pods
```

Wait until all are Running.

---

## Step 5 — Log in to ArgoCD

Get the initial admin password:

```bash
argocd admin initial-password -n argocd
```

Start a port-forward and leave it running in its own terminal:

```bash
kubectl -n argocd port-forward svc/argocd-server 8080:80
```

**UI:** open <http://localhost:8080>, user `admin`, password from above.

**CLI:** in another terminal —

```bash
argocd login localhost:8080 \
  --username admin \
  --password "$(argocd admin initial-password -n argocd | head -1)" \
  --insecure
```

---

## Step 6 — Register the two spokes with the hub

**This is the step that fails most often, and it fails confusingly.**

Your kubeconfig reaches a spoke at `https://127.0.0.1:<random-port>`. That
address is meaningless inside the hub's container, where `127.0.0.1` is the
hub itself. The hub reaches a spoke over the shared `kind` Docker network.

If you register the kubeconfig address, `argocd cluster add` **appears to
succeed** and every Application later reports a connection timeout.

The script handles it and rewrites the POC values files with the real
addresses:

```bash
./poc/register-spokes.sh
```

<details>
<summary>What it does, if you would rather run it by hand</summary>

```bash
for env in nonprod prod; do
  IP=$(docker inspect "workload-${env}-control-plane" \
       --format '{{ (index .NetworkSettings.Networks "kind").IPAddress }}')
  echo "${env} -> https://${IP}:6443"

  argocd cluster add "kind-workload-${env}" \
    --name "${env}" \
    --cluster-endpoint direct \
    --upsert --yes
done
```

`--cluster-endpoint direct` is the flag that makes ArgoCD store the container
address rather than the kubeconfig's.
</details>

Verify:

```bash
argocd cluster list
```

```
SERVER                      NAME        STATUS      MESSAGE
https://10.89.0.4:6443      nonprod     Successful
https://10.89.0.6:6443      prod        Successful
https://kubernetes.default.svc  in-cluster  Successful
```

**Status must be `Successful`.** If it says `Failed`, the address is wrong —
re-run the script.

**UI equivalent:** Settings → Clusters shows the same list. Registration itself
has no UI; it needs a kubeconfig, so it is CLI or a Secret. `cluster-secrets/`
in this repo is the declarative version for real estates.

Now commit the rewritten values:

```bash
git add poc/values-*.yaml && git commit -m "poc: real spoke endpoints" && git push
```

---

## Step 7 — Install the AppProjects. Before anything else.

```bash
helm upgrade --install app-projects argocd-apps/app-projects \
  --namespace argocd \
  -f poc/values-app-projects-kind.yaml
```

```bash
kubectl -n argocd get appprojects
```

```
NAME                     AGE
cluster-addons-nonprod   5s
cluster-addons-prod      5s
cluster-apps-nonprod     5s
cluster-apps-prod        5s
default                  20m
platform-bootstrap       5s
```

> **Why this comes before the ApplicationSets.**
> ArgoCD refuses to sync an Application whose project does not exist. Install
> the ApplicationSets first and every generated Application sits in
> *"Application referencing project cluster-apps-nonprod which does not
> exist"*.
>
> These projects used to live inside the `argocd` addon chart — deployed *by*
> an Application that referenced them. That is a deadlock that cannot resolve
> on retry, because the only thing that would create the project is the thing
> being blocked. It works on a cluster where the projects already exist from an
> earlier install, which is exactly why it survives to the first clean
> bootstrap. **A KinD cluster is nothing but clean bootstraps**, which is one
> reason a POC like this earns its keep.

**Look at what a project actually enforces** — this is worth showing on screen:

```bash
kubectl -n argocd get appproject cluster-apps-prod -o yaml | \
  yq '.spec | {sourceRepos, destinations, clusterResourceWhitelist, namespaceResourceBlacklist}'
```

```yaml
sourceRepos:
  - https://github.com/YOUR-ORG/*     # only your repo
destinations:
  - namespace: '*-*'                   # only <cluster>-<app> namespaces
    server: https://10.89.0.6:6443     # ONLY the prod spoke
clusterResourceWhitelist: []           # NO cluster-scoped resources at all
namespaceResourceBlacklist:
  - kind: ResourceQuota                # apps may not set their own quota
  - kind: LimitRange
```

**UI:** Settings → Projects → `cluster-apps-prod`.

---

## Step 8 — Install the ApplicationSets

```bash
helm upgrade --install app-sets argocd-apps/app-sets \
  --namespace argocd \
  -f poc/values-app-sets-kind.yaml
```

```bash
kubectl -n argocd get applicationsets
```

Two: `cluster-addons` and `cluster-apps`. Now watch Applications appear that
nobody wrote:

```bash
kubectl -n argocd get applications -w
```

Within seconds:

```
NAME                            SYNC     HEALTH
nonprod-cert-manager            Synced   Healthy
nonprod-external-secrets        Synced   Healthy
nonprod-keda                    Synced   Healthy
nonprod-kyverno                 Synced   Healthy
nonprod-platform-gateway        Synced   Healthy
nonprod-scylla-operator         Synced   Healthy
nonprod-drone-convoy-tracker    Synced   Healthy
prod-cert-manager               Synced   Healthy
... and the same seven for prod
```

**Fourteen Applications from two ApplicationSets and one values file.**

**UI:** Applications view. Filter by the `env` label to split nonprod from
prod. Click any Application → **App Details → Manifest** to see the generated
YAML, and note `spec.project` naming the per-environment project.

### How the generation actually works

Worth pausing on during a screenshare. `cluster-addons.yaml` uses a **matrix
generator** — a cross-product of two generators:

```yaml
- matrix:
    generators:
      - list:                  # the clusters, from values.yaml
          elements:
            - cluster: nonprod
              url: https://10.89.0.4:6443
            - cluster: prod
              url: https://10.89.0.6:6443
      - git:                   # the chart directories, discovered in Git
          directories:
            - path: argocd-apps/cluster-addons/*
```

2 clusters × 7 addon directories = 14 Applications, named
`{{.cluster}}-{{.path.basename}}`.

**This is the drop-in property.** Add `argocd-apps/cluster-apps/my-service/`
with a `Chart.yaml`, commit, push — and two more Applications appear. No shared
file is edited, no Application is written, no platform ticket is raised. Do
this live; it lands better than any slide.

---

## Step 9 — Watch the sync waves order the platform

Addons do not all land at once. `values.yaml` assigns sync waves, and ArgoCD
processes them in order:

| Wave | Addons | Why |
|---|---|---|
| −2 | cert-manager, external-secrets | provide CRDs others need |
| −1 | kyverno, keda, scylla-operator | **scylla-operator needs cert-manager** — cert-manager issues the cert for its admission webhook |
| 0 | platform-gateway | the shared Gateway, after cert-manager can issue for it |
| 10 | drone-convoy-tracker | the app, after the whole platform |

```bash
kubectl -n argocd get applications \
  -o custom-columns=NAME:.metadata.name,WAVE:'.metadata.annotations.argocd\.argoproj\.io/sync-wave',SYNC:.status.sync.status \
  --sort-by='.metadata.annotations.argocd\.argoproj\.io/sync-wave'
```

**UI:** each Application's details page shows the sync-wave annotation.

The scylla-operator case is the one to narrate: without waves, its webhook
Deployment starts before cert-manager exists, the webhook has no certificate,
it fails closed, and every ScyllaCluster CR is rejected. Waves are not
cosmetic.

---

## Step 10 — Watch the Rust app come up

```bash
kubectl --context kind-workload-nonprod -n nonprod-drone-convoy-tracker get pods -w
```

Order of appearance:

1. `scylla-*` — the ScyllaCluster CR, reconciled by the operator into a
   StatefulSet. **Slowest thing here; give it 3–5 minutes.**
2. `redis-*` — cache
3. `drone-convoy-tracker-api-*` — the Rust GraphQL API
4. `drone-convoy-tracker-frontend-*` — the Leptos WASM frontend

```bash
kubectl --context kind-workload-nonprod -n nonprod-drone-convoy-tracker \
  get scyllacluster,pods,svc,httproute
```

> ### KinD requires ScyllaDB developer mode — read before running prod
>
> `developerMode: false` is correct on real hardware and required for a
> supported ScyllaDB production deployment. It enforces preflight checks: an
> XFS filesystem, tuned disk IO, dedicated CPUs.
>
> **KinD satisfies none of them.** The local-path provisioner hands out an
> overlayfs directory, so with developer mode off Scylla refuses to start and
> the pod sits in `CrashLoopBackOff` with a filesystem complaint that reads
> like a bug and is not one.
>
> The `nonprod`, `preprod` and `uat` overlays already set
> `scylla.developerMode: true` — legitimate for non-production. **The `prod`
> overlay deliberately does not**, because shipping a production default of
> `true` is exactly the mistake this repository should not teach.
>
> So on KinD, the prod spoke's Scylla will not start until you say so
> explicitly:
>
> ```bash
> cat >> argocd-apps/cluster-apps/drone-convoy-tracker/env/prod/values-prod.yaml <<'EOF'
>
> # KinD ACCOMMODATION ONLY -- revert before this overlay reaches real hardware.
> # KinD has no XFS and no tuned disk, so ScyllaDB cannot run in production mode.
> scylla:
>   developerMode: true
> EOF
> git add -A && git commit -m "poc: scylla developer mode for KinD prod spoke" && git push
> ```
>
> If you would rather keep the prod overlay honest, skip this and run the app on
> the nonprod spoke only. **Step 12, the AppProject demonstration, works either
> way** — it shows an Application being *rejected*, which does not require the
> prod workload to be healthy.

> **If the API pods are in `CrashLoopBackOff`**, they almost certainly started
> before Scylla was accepting connections. Check the logs:
> ```bash
> kubectl --context kind-workload-nonprod -n nonprod-drone-convoy-tracker \
>   logs -l app.kubernetes.io/component=api --tail=50
> ```
> A connection-refused loop resolves itself once Scylla is up. If it persists
> beyond five minutes, confirm the ScyllaCluster reports members ready.

> **If the images cannot be pulled**, the chart points at
> `ghcr.io/CHANGEME-ORG/...`. Either push images to your registry and update
> `image.registry`, or build locally and side-load:
> ```bash
> kind load docker-image drone-convoy-tracker-api:dev --name workload-nonprod
> ```
> then set `image.registry` and `image.api.tag` in
> `cluster-apps/drone-convoy-tracker/env/nonprod/values-nonprod.yaml`.

---

## Step 11 — Reach the app through the Cilium Gateway

Confirm the shared Gateway was programmed:

```bash
kubectl --context kind-workload-nonprod -n gateway-system get gateway platform-gateway
kubectl --context kind-workload-nonprod -n nonprod-drone-convoy-tracker get httproute
```

The Gateway should be `PROGRAMMED=True` and the HTTPRoute should show it as an
accepted parent.

> **If the HTTPRoute shows no parents**, it attached to a Gateway that does not
> exist. That was a real bug in this repo: the app referenced `cilium-gateway`
> and the argocd addon referenced `platform-gateway`, and **no chart created
> either**. Gateway API fails quietly here — the route applies cleanly and the
> app is simply unreachable. Both now point at the Gateway that
> `cluster-addons/platform-gateway` owns.

Cilium creates a LoadBalancer Service for the Gateway. KinD has no cloud
provider, so it stays `<pending>` unless you give it one. Two options:

**Option A — port-forward (simplest, works everywhere):**

```bash
kubectl --context kind-workload-nonprod -n gateway-system \
  port-forward svc/cilium-gateway-platform-gateway 8081:80
```

Then, because the Gateway routes on hostname:

```bash
curl -H "Host: drone-convoy-tracker.nonprod.example.internal" http://localhost:8081/
```

For the browser, add to `/etc/hosts`:

```
127.0.0.1  drone-convoy-tracker.nonprod.example.internal
```

and open <http://drone-convoy-tracker.nonprod.example.internal:8081>.

**Option B — cloud-provider-kind (real LoadBalancer IPs):**

```bash
go install sigs.k8s.io/cloud-provider-kind@latest
sudo cloud-provider-kind &
kubectl --context kind-workload-nonprod -n gateway-system get svc
```

The Service gets an external IP reachable from the host.

You should see the tactical dashboard: the map, the leaderboard, the telemetry
chart and the engagement feed.

---

## Step 12 — The demonstration that justifies AppProject

This is the part worth rehearsing. Everything so far shows GitOps working.
**This shows what stops it doing the wrong thing.**

Break it deliberately — point the nonprod cluster entry at the **prod** server:

```bash
PROD_URL=$(argocd cluster list -o json | jq -r '.[] | select(.name=="prod") | .server')

sed "s|url: https://.*6443|url: ${PROD_URL}|" poc/values-app-sets-kind.yaml \
  > /tmp/broken.yaml
# only change the nonprod entry, leaving prod alone
python3 - <<'EOF'
import yaml
d = yaml.safe_load(open('poc/values-app-sets-kind.yaml'))
prod = next(c['url'] for c in d['clusters'] if c['name'] == 'prod')
for c in d['clusters']:
    if c['name'] == 'nonprod':
        c['url'] = prod          # deliberately wrong
yaml.safe_dump(d, open('/tmp/broken.yaml', 'w'), sort_keys=False)
EOF

helm upgrade app-sets argocd-apps/app-sets -n argocd -f /tmp/broken.yaml
```

Now look:

```bash
kubectl -n argocd get application nonprod-drone-convoy-tracker \
  -o jsonpath='{.status.conditions}' | jq
```

```
"type": "InvalidSpecError",
"message": "application destination server ... is not permitted in project
            cluster-apps-nonprod"
```

**The Application was created and refused to sync.** nonprod's manifests did
not reach the prod cluster. The generator was wrong; the control plane caught
it.

**UI:** the Application shows a red condition banner with the same message.

Now show the counterfactual. Set `perEnvironment: false` in
`poc/values-app-projects-kind.yaml`, reinstall both charts, and repeat. With a
single shared project spanning every cluster, **the same mistake succeeds** and
nonprod's manifests deploy into prod.

That contrast is the entire argument for the AppProject layer, and it takes
ninety seconds to show.

Restore afterwards:

```bash
helm upgrade app-sets argocd-apps/app-sets -n argocd -f poc/values-app-sets-kind.yaml
```

---

## Step 13 — Show self-healing and drift correction

Delete something on a spoke by hand and watch it come back:

```bash
kubectl --context kind-workload-nonprod -n nonprod-drone-convoy-tracker \
  delete deploy drone-convoy-tracker-api

# within ~3 minutes, or force it:
argocd app sync nonprod-drone-convoy-tracker
kubectl --context kind-workload-nonprod -n nonprod-drone-convoy-tracker get deploy
```

`selfHeal: true` in the ApplicationSet template is what does this. Git is the
source of truth; a `kubectl edit` on a spoke is a temporary opinion.

**Show the asymmetry too.** The addon ApplicationSet sets
`preserveResourcesOnDeletion: true` and `applicationsSync: create-update`, so
deleting the ApplicationSet does **not** tear the platform out from under the
workloads. The apps ApplicationSet keeps normal semantics — remove a chart
directory and the workload goes. That asymmetry is a deliberate answer to the
first objection an app-of-apps holdout raises: *"an ApplicationSet is one
`kubectl delete` away from destroying everything."*

---

## Step 14 — Tear down

```bash
kind delete cluster --name workload-prod
kind delete cluster --name workload-nonprod
kind delete cluster --name gitops-hub
```

Rebuilding takes about fifteen minutes. That the spokes are disposable *is* the
demonstration.

---

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| Applications say *"project ... does not exist"* | app-sets installed before app-projects | Run step 7, then re-sync |
| *"application repo ... is not permitted in project"* | `sourceRepos` does not match `repo.url` | Make them agree; step 1 |
| `argocd cluster list` shows `Failed` | Registered the kubeconfig address, not the Docker one | Re-run `poc/register-spokes.sh` |
| Applications stuck `OutOfSync`, nothing on the spoke | Hub cannot reach the spoke | Same as above |
| Spoke nodes `NotReady` | No CNI yet | Expected before step 3 |
| No `cilium` GatewayClass | Cilium started before the Gateway API CRDs | `rollout restart deploy/cilium-operator` |
| HTTPRoute has no parents | Gateway missing or name mismatch | `kubectl -n gateway-system get gateway` |
| ScyllaCluster never becomes ready | scylla-operator webhook has no cert | Check cert-manager is Healthy (wave −2) |
| Scylla pods `CrashLoopBackOff` with a filesystem or IO complaint | `developerMode: false` on KinD | See the developer-mode box in step 10 |
| Rust app images cannot be pulled | `image.registry` still points at a placeholder | Build and `kind load docker-image`, or push to your registry |
| Gateway Service stuck `<pending>` | KinD has no LoadBalancer | Port-forward, or `cloud-provider-kind` |
| Git changes have no effect | Not committed and pushed | ArgoCD reads Git, not your working copy |

Useful throughout:

```bash
argocd app list
argocd app get nonprod-drone-convoy-tracker
argocd app logs nonprod-drone-convoy-tracker
kubectl -n argocd logs deploy/argocd-applicationset-controller --tail=100
kubectl -n argocd logs deploy/argocd-application-controller --tail=100
```

---

## Suggested screenshare running order

Roughly 45 minutes with discussion.

| # | Show | The point |
|---|---|---|
| 1 | The three clusters, `argocd cluster list` | Hub and spoke: one control plane, disposable workload clusters |
| 2 | `kubectl -n argocd get appprojects` | The governance envelope exists before anything is deployed |
| 3 | `cluster-apps-prod` YAML | What a project actually enforces: repo, destination, resource kinds |
| 4 | `get applicationsets` then `get applications` | Two objects generate fourteen. Nobody wrote them |
| 5 | An Application's manifest in the UI | Where the generator's values landed, and `spec.project` |
| 6 | Sync waves, and the scylla-operator/cert-manager dependency | Ordering is a platform concern, expressed declaratively |
| 7 | The Rust app in a browser, both environments | It is a real application, not a hello-world |
| 8 | Add a chart directory, commit, push | The drop-in property — the pitch, live |
| 9 | **Step 12, the AppProject rejection** | The generator was wrong and the control plane caught it |
| 10 | Delete a Deployment, watch it return | Self-healing, and the deletion asymmetry between layers |

Step 9 is the one people remember. Rehearse it.
