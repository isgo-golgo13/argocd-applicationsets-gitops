# Changes in this revision

Applied to the pure-Kubernetes GitOps reference. Targets KinD first, then
EKS/GKE. No vendor-specific content anywhere — `hack/validate.py` section 7
enforces that mechanically.

## New

| Path | What |
|---|---|
| `argocd-apps/app-projects/` | The AppProject governance layer. Hub-side Helm chart, installed before `app-sets`. |
| `argocd-apps/cluster-addons/platform-gateway/` | Owns the shared Gateway every HTTPRoute attaches to. |
| `poc/kind/{hub,spoke-nonprod,spoke-prod}.yaml` | Three KinD cluster configs. |
| `poc/values-app-sets-kind.yaml` | POC overlay: two spokes, addon subset. |
| `poc/values-app-projects-kind.yaml` | POC overlay: two environments. |
| `poc/register-spokes.sh` | Registers spokes at the address the hub can actually reach. |
| `hack/validate.py` | Static validation; 8 sections. |
| `README-setup-poc.md` | The step-by-step tutorial. |

## Bugs fixed

**1. AppProject bootstrap deadlock.** The projects lived in
`cluster-addons/argocd/templates/projects/`, deployed by an Application that
referenced them. ArgoCD refuses to sync an Application whose project does not
exist, and the project was created by that very Application. It cannot resolve
on retry. It works on a cluster where the projects already exist from an
earlier install — which is why it would have survived until the first clean
bootstrap. Fixed by promoting them to their own chart, installed first.

**2. The shared Gateway had no owner.** `cluster-addons/argocd` referenced
`platform-gateway` in `gateway-system`; `cluster-apps/drone-convoy-tracker`
referenced `cilium-gateway` in the same namespace. **No chart created either.**
Gateway API fails quietly here: the HTTPRoute applies cleanly, reports a parent
that does not exist, and the app is simply unreachable — with a green sync
status. Fixed by adding the `platform-gateway` addon and pointing both
consumers at it.

**3. Literal brace directory.** `cluster-addons/scylla-operator/env/{nonprod,preprod,uat,prod}`
— shell brace expansion that never expanded. Removed.

**4. `developerMode: false` was hardcoded in the ScyllaCluster template.**
Correct on real hardware and required for a supported ScyllaDB production
deployment, but impossible on KinD: local-path hands out an overlayfs
directory, not XFS, so Scylla refuses to start. Now a value, defaulting to
`false`; `nonprod`, `preprod` and `uat` set it `true`. The `prod` overlay
deliberately keeps `false` — see the developer-mode box in
`README-setup-poc.md` step 10 for the KinD accommodation.

**5. The app attached to an `https` listener with no certificate path.**
Changed to `http`, which is what a laptop demo uses. The prod overlay of
`platform-gateway` enables TLS with a cert-manager Certificate.

## Upgrades

**Per-environment AppProjects** (`perEnvironment: true`). Renders
`cluster-addons-<env>` and `cluster-apps-<env>`, each permitting exactly one
destination server, plus `platform-bootstrap` for hub-side Applications. An
Application generated for nonprod cannot be synced into prod. Set to `false` to
restore the previous single-project behaviour without editing templates.

**`addonExclude`.** Names addon directories that stay in Git but are not
deployed. The POC uses it to keep the `argocd` addon off the spokes — without
it a spoke becomes a second control plane and the hub/spoke demonstration
collapses.

**Retry policy** on every generated Application. Sync waves order Applications
relative to each other; they cannot order objects inside one Application. Retry
covers that, and it is what makes a first bootstrap against a brand-new cluster
reliable rather than a coin flip.

**`cluster-secrets` moved to `platform-bootstrap`.** Its destination is the hub,
not a workload cluster, so a per-environment addons project was the wrong home.

## Not changed

`scylla-operator` stays — the Rust application's persistence layer needs it.
All four environment overlays (nonprod/preprod/uat/prod) stay in the charts;
the POC registers two clusters and the other overlays simply go unused, which
is what `ignoreMissingValueFiles` is for.

## Validation

```
python3 hack/validate.py
```

All 8 sections pass. **Not** checked: Go template rendering. Run
`helm template argocd-apps/app-projects -f poc/values-app-projects-kind.yaml`
and the same for `app-sets` before the first live run — those two charts carry
the newest templating.
