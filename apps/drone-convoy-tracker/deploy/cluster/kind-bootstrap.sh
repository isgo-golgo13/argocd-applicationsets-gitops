#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# deploy/cluster/kind-bootstrap.sh -- create the drone-ops KinD cluster and
# install every platform prerequisite the chart consumes, in dependency order.
#
#   1. KinD cluster (3 cp + 3 workers, no CNI)
#   2. Gateway API CRDs            <- MUST precede Cilium or gatewayAPI.enabled
#                                     is silently ignored
#   3. Cilium (kube-proxy replacement + Gateway API)
#   4. cert-manager (+ ExperimentalGatewayAPISupport) + self-signed ClusterIssuer
#   5. External Secrets Operator + `fake` ClusterSecretStore
#   6. KEDA, VPA, scylla-operator, metrics-server
#   7. Cilium Gateway -> NodePort 30080/30443 on the ingress-ready node
#
# Idempotent: re-running upgrades in place. Requires: kind, kubectl, helm,
# podman or docker (KIND_EXPERIMENTAL_PROVIDER=podman for Podman).
# ---------------------------------------------------------------------------
set -euo pipefail
CLUSTER=drone-ops
GATEWAY_API_VERSION=v1.2.1
CILIUM_VERSION=1.17.4
CERT_MANAGER_VERSION=v1.17.2
ESO_VERSION=0.16.1
KEDA_VERSION=2.17.1
SCYLLA_OPERATOR_VERSION=v1.16.0
here="$(cd "$(dirname "$0")" && pwd)"

log(){ printf '\033[0;32m▶ %s\033[0m\n' "$*"; }

# ---- 1. cluster --------------------------------------------------------------
if ! kind get clusters | grep -qx "$CLUSTER"; then
  log "Creating KinD cluster '$CLUSTER' (3 control-plane + 3 workers)"
  kind create cluster --config "$here/kind-config.yaml" --wait 0s
else
  log "Cluster '$CLUSTER' exists -- reconciling addons"
fi
kubectl config use-context "kind-$CLUSTER" >/dev/null

# ---- 2. Gateway API CRDs (BEFORE Cilium) --------------------------------------
log "Gateway API CRDs $GATEWAY_API_VERSION"
kubectl apply -f "https://github.com/kubernetes-sigs/gateway-api/releases/download/$GATEWAY_API_VERSION/standard-install.yaml"
# TLSRoute etc. live in the experimental channel; not needed for HTTPRoute.

# ---- 3. Cilium ---------------------------------------------------------------
log "Cilium $CILIUM_VERSION (kube-proxy replacement, Gateway API)"
helm repo add cilium https://helm.cilium.io >/dev/null 2>&1 || true
helm repo update >/dev/null
API_SERVER=$(kubectl get nodes -l node-role.kubernetes.io/control-plane -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}')
helm upgrade --install cilium cilium/cilium --version "$CILIUM_VERSION" -n kube-system \
  --set kubeProxyReplacement=true \
  --set k8sServiceHost="$API_SERVER" --set k8sServicePort=6443 \
  --set gatewayAPI.enabled=true \
  --set gatewayAPI.enableAlpn=true \
  --set gatewayAPI.hostNetwork.enabled=false \
  --set ipam.mode=kubernetes \
  --set hubble.enabled=true --set hubble.relay.enabled=true \
  --set operator.replicas=1 \
  --wait
kubectl -n kube-system rollout status ds/cilium --timeout=300s

# ---- 4. cert-manager -----------------------------------------------------------
log "cert-manager $CERT_MANAGER_VERSION (+ Gateway API support)"
helm repo add jetstack https://charts.jetstack.io >/dev/null 2>&1 || true
helm upgrade --install cert-manager jetstack/cert-manager --version "$CERT_MANAGER_VERSION" \
  -n cert-manager --create-namespace \
  --set crds.enabled=true \
  --set config.apiVersion=controller.config.cert-manager.io/v1alpha1 \
  --set config.kind=ControllerConfiguration \
  --set config.enableGatewayAPI=true \
  --wait
log "self-signed ClusterIssuer (Let's Encrypt cannot reach a KinD cluster)"
kubectl apply -f - <<'YAML'
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata: { name: selfsigned-cluster-issuer }
spec: { selfSigned: {} }
YAML

# ---- 5. External Secrets Operator ---------------------------------------------
log "External Secrets Operator $ESO_VERSION + fake ClusterSecretStore"
helm repo add external-secrets https://charts.external-secrets.io >/dev/null 2>&1 || true
helm upgrade --install external-secrets external-secrets/external-secrets --version "$ESO_VERSION" \
  -n external-secrets --create-namespace --set installCRDs=true --wait
kubectl apply -f - <<'YAML'
# `fake` provider: the chart's ExternalSecret is exercised unchanged; the
# values are dev credentials (Scylla auth is off in developerMode anyway).
apiVersion: external-secrets.io/v1
kind: ClusterSecretStore
metadata: { name: drone-secret-store }
spec:
  provider:
    fake:
      data:
        - key: drone-convoy-attack-tracker
          value: '{"scylla_username":"cassandra","scylla_password":"cassandra"}'
YAML

# ---- 6. KEDA, VPA, scylla-operator, metrics-server ----------------------------
log "KEDA $KEDA_VERSION"
helm repo add kedacore https://kedacore.github.io/charts >/dev/null 2>&1 || true
helm upgrade --install keda kedacore/keda --version "$KEDA_VERSION" -n keda --create-namespace --wait

log "VPA (recommender/updater/admission) via Fairwinds chart"
helm repo add fairwinds-stable https://charts.fairwinds.com/stable >/dev/null 2>&1 || true
helm upgrade --install vpa fairwinds-stable/vpa -n vpa --create-namespace --wait

log "metrics-server (HPA needs it; KinD kubelets use self-signed certs)"
helm repo add metrics-server https://kubernetes-sigs.github.io/metrics-server/ >/dev/null 2>&1 || true
helm upgrade --install metrics-server metrics-server/metrics-server -n kube-system \
  --set 'args={--kubelet-insecure-tls}' --wait

log "scylla-operator $SCYLLA_OPERATOR_VERSION"
helm repo add scylla https://scylla-operator-charts.storage.googleapis.com/stable >/dev/null 2>&1 || true
helm upgrade --install scylla-operator scylla/scylla-operator --version "$SCYLLA_OPERATOR_VERSION" \
  -n scylla-operator --create-namespace --wait

# ---- 7. Expose the Cilium Gateway on the port-mapped node ----------------------
# Cilium creates a LoadBalancer Service per Gateway; on KinD it stays Pending
# (no LB provider). We don't need one: patch it to NodePort on 30080/30443,
# which kind-config.yaml maps to host :80/:443. Runs after the chart creates
# the Gateway, so it lives in `make kind-expose` -- see the Makefile.
log "Bootstrap complete. Next: make kind-deploy && make kind-expose"
kubectl get nodes -o wide
