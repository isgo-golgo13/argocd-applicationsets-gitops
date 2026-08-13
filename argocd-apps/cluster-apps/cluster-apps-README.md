## Deploy the Appp

```yaml
helm template drone-convoy-tracker argocd-apps/cluster-apps/drone-convoy-tracker \
  -f argocd-apps/cluster-apps/drone-convoy-tracker/env/prod/values-prod.yaml \
  --namespace drone-ops
```
