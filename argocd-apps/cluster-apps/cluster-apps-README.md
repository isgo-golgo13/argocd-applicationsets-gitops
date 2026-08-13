## Deploy the DoD Convoy Attack Dron Trackiong App

The full Rust application is up the directory chain in `apps/`.

```yaml
helm template drone-convoy-tracker argocd-apps/cluster-apps/drone-convoy-tracker \
  -f argocd-apps/cluster-apps/drone-convoy-tracker/env/prod/values-prod.yaml \
  --namespace drone-ops
```
