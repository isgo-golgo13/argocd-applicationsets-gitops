# Delta since drone-convoy-tracker-delta-p4.zip

All NEW files. Nothing changed, nothing deleted.

    argocd-apps/cluster-apps/drone-convoy-tracker/
    ├── Chart.yaml
    ├── values.yaml                     # common base
    ├── env/{nonprod,preprod,uat,prod}/values-<env>.yaml
    └── templates/
        ├── _helpers.tpl
        ├── app.yaml                    # API + frontend Deployments, SA, PDB
        ├── app-service.yaml            # Services + Cilium Gateway + HTTPRoute
        ├── app-config.yaml             # ConfigMap
        ├── app-storage.yaml            # ScyllaCluster CR
        ├── app-secrets.yaml            # ExternalSecret + cert-manager Certificate
        ├── app-scaling.yaml            # HPA + VPA (recommender) + KEDA
        └── app-metrics.yaml            # scrape Service

Screaming-architecture naming throughout. No Ingress anywhere.

## Verify before the meeting

    helm template argocd-apps/cluster-apps/drone-convoy-tracker \
      -f argocd-apps/cluster-apps/drone-convoy-tracker/env/prod/values-prod.yaml

Then confirm the ApplicationSet picks it up:

    helm template argocd-apps/app-sets/

The cluster-apps generator globs argocd-apps/cluster-apps/*, so this directory
now produces nonprod/preprod/uat/prod-drone-convoy-tracker Applications with no
edit to any shared file. That is the demo.
