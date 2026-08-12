# context-handoff.md — argocd-applicationsets-gitops

Purpose: reference GitOps structure to park alongside Broadcom's existing
`argocd-app-of-apps/` folder. Demonstrates ApplicationSets + directory-per-env
for both cluster-addons and cluster-apps.

Status: cluster-addons complete; cluster-apps empty; Rust app not yet charted.

---

## 1. Blocking — fix before delivery

### 1.1 Generator paths are one level short
The repo root now has `apps/`, `argocd-apps/`, `docs/`. The ApplicationSet git
generators were written when `eql-argocd-apps/` *was* the root, so they scan:

```yaml
directories:
  - path: cluster-addons/*
```

Nothing matches. Must be:

```yaml
directories:
  - path: argocd-apps/cluster-addons/*
  - path: argocd-apps/cluster-apps/*
```

`spec.source.path: '{{.path.path}}'` then resolves correctly, and the
`valueFiles: env/<env>/values-<env>.yaml` entries stay as-is because Helm value
file paths are relative to the chart directory, not the repo root.

Note the naming impact: `{{.path.basename}}` still yields `cert-manager`,
`kyverno`, etc., so Application names and namespaces are unaffected.

### 1.2 Helm and ArgoCD both own `{{ }}`
`app-sets/` is now a Helm chart whose templates *are* ApplicationSet manifests.
Helm renders `{{ }}` before ArgoCD ever sees the file, so every ApplicationSet
placeholder gets consumed by Helm and fails or renders empty.

Every ArgoCD-side placeholder must be escaped. Two workable styles:

```yaml
# inline escape
name: '{{ printf "{{.cluster}}-{{.path.basename}}" }}'
```

```yaml
# helper-based (preferred — keeps the manifests readable)
# in __helpers.tpl:
{{- define "appset.appName" -}}
{{ `{{.cluster}}-{{.path.basename}}` }}
{{- end -}}
```

Backtick raw strings are the cleanest: Go templates treat backticked content as
a literal, so `{{ `{{.path.basename}}` }}` emits `{{.path.basename}}` untouched.

**Decide deliberately whether the chart earns this cost.** It does buy one real
thing: `repoURL` and the cluster list move into `app-sets/values.yaml`, so
repointing the whole repo at Broadcom's Git org is a one-line change instead of
an edit across both ApplicationSets. Given the repo *will* be renamed, keep it —
but the escaping has to be systematic, and `helm template app-sets/` must be in
CI before anything merges.

Rename `__helpers.tpl` → `_helpers.tpl` (single underscore is the Helm
convention; double still works since Helm skips any `_`-prefixed file, but it
reads as a typo).

### 1.3 Legacy templating syntax
Both ApplicationSets use fasttemplate (`{{cluster}}`, `{{path.basename}}`).
Deprecated. Add to each ApplicationSet spec:

```yaml
spec:
  goTemplate: true
  goTemplateOptions: ["missingkey=error"]
```

and convert every placeholder to dotted form (`{{.cluster}}`,
`{{.path.basename}}`, `{{.values}}`). `missingkey=error` is the point of the
exercise — without it a mistyped key renders an empty string and silently
generates a malformed Application. Shipping deprecated syntax to Broadcom as
"the modern pattern" undercuts the delivery.

---

## 2. Deprecations and inconsistencies in the addon values

### 2.1 Ingress contradicts the Gateway API decision
`argocd`, `goldilocks`, and `vault` prod values all carry
`ingress.enabled: true`, `ingressClassName: nginx`, and
`nginx.ingress.kubernetes.io/*` annotations. There is no `ingress-nginx` addon
in `cluster-addons/`, so these reference a controller this repo never installs.

Target is Cilium Gateway API (Gateway + HTTPRoute). Replace each ingress block
with an HTTPRoute in the wrapper chart's own `templates/`, parented to a shared
Gateway. Keeps the addon subchart untouched and the routing declarative.

### 2.2 cert-manager needs the Gateway API feature gate
Current prod values:

```yaml
extraArgs:
  - --feature-gates=AdditionalCertificateOutputFormats=true
```

For cert-manager to issue certificates from Gateway `listeners[].tls`
annotations it needs `ExperimentalGatewayAPISupport=true` as well. Without it,
the Gateway API migration has no certificate path.

### 2.3 Deprecated control-plane toleration
cert-manager prod values use:

```yaml
tolerations:
  - key: node-role.kubernetes.io/master
```

`node-role.kubernetes.io/master` was removed in Kubernetes 1.25. Use
`node-role.kubernetes.io/control-plane`.

### 2.4 Pinned chart versions are stale
Every `Chart.yaml` dependency pin dates from the original drafting (argo-cd
5.46.7, cert-manager v1.13.2, crossplane 1.14.1, kyverno 3.0.5, keda 2.12.0,
external-secrets 0.9.9, sealed-secrets 2.13.3, cluster-api 1.5.1, cnpg 0.18.1,
kargo 0.3.0, vault 0.25.0, goldilocks 6.7.0).

Re-pin all of them against current upstream before delivery, and check breaking
changes for Crossplane and Kargo specifically — both have had major version
transitions since these pins. I can't verify current versions from here; run
`helm search repo --versions` against each repo, or check upstream release notes.

### 2.5 Crossplane provider installation
`provider.packages: [aws, kubernetes]` in values is the old install path. Current
Crossplane installs providers as `Provider` CRs, and the monolithic AWS provider
was split into per-service family providers. Either drop provider installation
from the chart and manage it as a separate Application, or update to the family
provider names.

### 2.6 Cluster API is not a Helm install
CAPI's sanctioned install path is `clusterctl`, which is imperative and doesn't
fit GitOps. The correct ArgoCD-native answer is the **cluster-api-operator**
Helm chart (`kubernetes-sigs/cluster-api-operator`), which takes
`CoreProvider` / `InfrastructureProvider` CRs declaratively. Repoint the
`cluster-api` addon at that chart.

### 2.7 Three secret systems
`sealed-secrets`, `external-secrets`, and `vault` are all present. Pick one
primary. For a regulated target audience the defensible pair is Vault as the
backing store plus External Secrets Operator as the in-cluster sync; drop
sealed-secrets, or keep it explicitly scoped to bootstrap-only secrets that must
exist before ESO is running, and say so in the README.

---

## 3. Structural gaps

### 3.1 No sync waves
CRD-providing addons must land before anything that consumes their CRDs.
Nothing in the tree expresses ordering. Add
`argocd.argoproj.io/sync-wave` annotations via the ApplicationSet template, e.g.
wave -2 for cert-manager / ESO / sealed-secrets, -1 for kyverno / crossplane /
cluster-api-operator, 0 for the rest, and a positive wave for all cluster-apps.

A `wave` key per addon in `app-sets/values.yaml` keyed by directory name is the
cleanest way to drive this without a second generator.

### 3.2 ApplicationSet deletion cascades
Deleting an ApplicationSet deletes every Application it generated and prunes
their resources. For prod addons that is the entire platform. Set on both
ApplicationSets:

```yaml
spec:
  syncPolicy:
    applicationsSync: create-update      # never delete generated Apps
    preserveResourcesOnDeletion: true
```

Worth calling out explicitly in the Broadcom-facing README — it's the objection
an app-of-apps holdout will raise first.

### 3.3 cluster-secrets/ is spec-only
The directory holds only `spec-README.md`. Cluster registration secrets
(`argocd.argoproj.io/secret-type: cluster`) need to actually exist for the
list/cluster generator destinations to resolve.

Bootstrap ordering problem worth documenting: ESO and sealed-secrets are
themselves addons managed by the ApplicationSet, so the very first cluster
secret can't be encrypted by a controller that isn't running yet. Standard
resolution is one manually-applied bootstrap secret for the hub, everything
after that via ESO.

### 3.4 List generator vs cluster generator
The four environments are hardcoded as list-generator elements with cluster
URLs. Once `cluster-secrets/` is populated, switching to the **cluster
generator** with label selectors (`env: prod`) means adding a cluster is a
secret, not a repo edit. Given Cluster API and Crossplane are both in the addon
set, that's the coherent direction — but it's a post-delivery refinement, not a
blocker.

### 3.5 cluster-apps/ is empty
`cluster-apps/` contains only its README. The ApplicationSet exists and
generates nothing. This is the visible hole in the delivery — Broadcom sees a
complete addons story and no application story.

---

## 4. Rust app — `apps/drone-colony-dstar`

Not yet charted, and the workspace layout needs correcting first.

**Layout problems:**
- `crates/app.rs` and `crates/main.rs` sit loose at the `crates/` level.
  A Cargo workspace's `crates/` directory should contain only member crate
  directories, each with its own `Cargo.toml`. The binary entrypoint belongs in
  its own member, e.g. `crates/drone-colony-dstar/src/main.rs`.
- Each member crate uses `src/mod.rs` as its root. That's wrong — `mod.rs` is
  for submodule directories. A library crate root is `src/lib.rs`.
  `drone_pathfinding/src/mod.rs` should be `src/lib.rs` with
  `pub mod dstar;` inside.
- `drone_data_storage/src/spanner_schema.sql` is fine where it is but should be
  reachable via `include_str!` or moved to a `migrations/` dir if it's applied
  at runtime.

**Chart to build (screaming-architecture naming, per the agreed convention):**

```
argocd-apps/cluster-apps/drone-colony-dstar/
├── Chart.yaml
├── env/
│   ├── nonprod/values-nonprod.yaml
│   ├── preprod/values-preprod.yaml
│   ├── uat/values-uat.yaml
│   └── prod/values-prod.yaml
└── templates/
    ├── app.yaml              # Deployment
    ├── app-service.yaml      # Service + Gateway + HTTPRoute (Cilium)
    ├── app-config.yaml       # ConfigMap
    ├── app-storage.yaml      # PVC / StorageClass
    └── app-credentials.yaml  # cert-manager Issuer + Certificate, ExternalSecret
```

Unlike the addons, this is a first-party chart with real templates rather than a
wrapper around an upstream dependency — so `Chart.yaml` has no `dependencies`
block.

Open questions to settle before writing it:
- Spanner means a GCP dependency and workload identity. What authenticates the
  pod — Workload Identity Federation, or a service account key via ESO?
- Is this a long-running service (Deployment + HTTPRoute) or a batch/sortie
  runner (Job/CronJob)? The Gateway API surface only makes sense for the former.
- Does it need persistent storage at all? If not, drop `app-storage.yaml` rather
  than shipping an empty template — the naming convention is only useful if a
  present file means a present concern.

---

## 5. Delivery framing for Broadcom

Drop as a sibling folder to `argocd-app-of-apps/`. Suggested name:
`argocd-applicationsets/`.

Include a top-level README that makes the comparison explicit rather than
implied — the existing `docs/*.png` diagrams already carry most of it:
- `argocd-app-of-apps-v-appsets.png` — the pattern contrast
- `appsets-structure.png` — the directory-per-env layout
- the three `argocd-*-cp-cluster-*.png` — hub-and-spoke topology options

Lead the README with the three things app-of-apps can't do cleanly: no
per-child `Application.yaml` to maintain, environment promotion by directory
rather than by branch, and RBAC boundaries via AppProject rather than a single
`default` project.