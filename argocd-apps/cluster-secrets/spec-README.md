## Cluster Secrets for ArgoCD

This directory stores all the registered Kubernetes spoke cluster secrets for ArgoCD control-plane to provision
cluster-addons and cluster apps using their `kubeconfig` contexts.

Place all the sealed-*.yaml (Sealed Secrets) for all four env spoke clusters in this directory.
## Bootstrap ordering — read before registering the first cluster

There is a deliberate chicken-and-egg here, and it has a standard answer.

The list generator in `app-sets/values.yaml` names four spoke clusters. For
ArgoCD to deploy to any of them, a cluster-registration Secret
(`argocd.argoproj.io/secret-type: cluster`) must exist on the hub. But the two
controllers that could deliver that Secret from Git — External Secrets
Operator and sealed-secrets — are themselves cluster-addons, deployed *by*
ArgoCD, *to* clusters it can already reach.

The resolution, in order:

1. **Hub bootstrap (manual, once).** The hub's own registration ("in-cluster")
   exists implicitly. Install ArgoCD on the hub (`helm install` of this repo's
   `cluster-addons/argocd` wrapper, or the org's bootstrap tooling), then apply
   the `app-sets` chart. Addons reconcile onto the hub, including
   sealed-secrets and ESO.
2. **First spoke secrets (SealedSecret, in Git).** With the sealed-secrets
   controller now running on the hub, spoke registration Secrets can live in
   this directory as `sealed-*.yaml` — encrypted, committable, applied by the
   hub's ArgoCD. This is exactly the bootstrap-only scope sealed-secrets is
   kept for (see `cluster-addons/sealed-secrets/values.yaml`).
3. **Steady state (ESO).** Everything after bootstrap flows Vault → ESO →
   Secret. Rotating a spoke credential is a Vault write, not a re-seal.

Post-delivery refinement: once this directory is populated, replace the
hardcoded cluster list with the **cluster generator** using label selectors
(`env: prod`) — adding a cluster then becomes a Secret, not a repo edit.

## Finishing the pattern, concretely — per spoke

The machinery is in place: the `cluster-secrets` Application
(`app-sets/templates/cluster-secrets.yaml`, wave -3) syncs every
`sealed-*.yaml` in this directory to the hub's `argocd` namespace, where the
sealed-secrets controller unseals it into a live cluster registration. This
directory ships with only `cluster-registration.template.yaml` — filling one
slot per spoke is a four-step loop:

1. **On the spoke** — create the ServiceAccount ArgoCD will act as:

   ```shell
   kubectl create serviceaccount argocd-manager -n kube-system
   kubectl create clusterrolebinding argocd-manager \
     --clusterrole=cluster-admin --serviceaccount=kube-system:argocd-manager
   kubectl apply -f - <<'YAML'
   apiVersion: v1
   kind: Secret
   metadata:
     name: argocd-manager-token
     namespace: kube-system
     annotations:
       kubernetes.io/service-account.name: argocd-manager
   type: kubernetes.io/service-account-token
   YAML
   kubectl -n kube-system get secret argocd-manager-token \
     -o jsonpath='{.data.token}' | base64 -d      # -> bearerToken
   kubectl -n kube-system get secret argocd-manager-token \
     -o jsonpath='{.data.ca\.crt}'                # -> caData (already base64)
   ```

   (`cluster-admin` is the blunt default; scope the ClusterRole down to what
   the addons and apps actually deploy once the platform settles.)

2. **Locally** — copy the template, fill name/server/env label/token/CA. The
   `server` URL must byte-match that cluster's entry in
   `app-sets/values.yaml`.

3. **Seal against the HUB's controller** (public cert, safe to fetch and even
   commit):

   ```shell
   kubeseal --fetch-cert \
     --controller-name sealed-secrets-controller \
     --controller-namespace kube-system > pub-sealed-secrets.pem
   kubeseal --format yaml --cert pub-sealed-secrets.pem \
     < cluster-nonprod.filled.yaml > sealed-nonprod.yaml
   rm cluster-nonprod.filled.yaml            # the unencrypted copy never lands in Git
   ```

4. **Commit `sealed-nonprod.yaml`.** The Application syncs it, the controller
   unseals it, the list generator's destination resolves, and every
   ApplicationSet-generated Application for that env goes Healthy. Repeat per
   spoke. Deregistering is the mirror image: `git rm` the sealed file
   (`prune: true` removes the Secret).

Credential rotation note: re-sealing is only the bootstrap-era path. Once ESO
is the steady state (see ordering above), spoke credentials can graduate to
Vault → ExternalSecret like every other secret, leaving sealed-secrets with
nothing but the first-boot artifacts — which is exactly its stated scope.
