#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# poc/register-spokes.sh
#
# Registers the two KinD spokes with the hub's ArgoCD and rewrites the two POC
# values files with the real Docker network addresses.
#
# This exists because the address a spoke is reachable at FROM THE HUB is not
# the address in your kubeconfig. kubectl talks to a spoke via 127.0.0.1:<random
# host port>; inside the hub container, 127.0.0.1 is the hub. The hub reaches a
# spoke over the shared container network instead.
#
# The address is read from the spoke's own Kubernetes node object, NOT from
# `docker inspect`. The node InternalIP IS the container's address on that
# network, and reading it through kubectl works identically whether KinD is
# backed by Docker or Podman -- which matters here, because this project builds
# with Podman.
#
# Getting this wrong is the single most common way this POC fails, and it fails
# confusingly: `argocd cluster add` appears to succeed and every Application
# then reports a connection error.
#
#   ./poc/register-spokes.sh
#
# Prerequisites: kind, kubectl, argocd CLI, and an argocd login against the hub
# (README-setup-poc.md step 5). No container runtime binary needed.
# ---------------------------------------------------------------------------
set -euo pipefail

SPOKES=("nonprod" "prod")
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

command -v argocd  >/dev/null || { echo "argocd CLI not found"; exit 1; }
command -v kubectl >/dev/null || { echo "kubectl not found"; exit 1; }

echo "==> hub context: kind-gitops-hub"
kubectl config use-context kind-gitops-hub >/dev/null

declare -A ADDR
for env in "${SPOKES[@]}"; do
  spoke_ctx="kind-workload-${env}"

  if ! kubectl config get-contexts "$spoke_ctx" >/dev/null 2>&1; then
    echo "!! no kubeconfig context ${spoke_ctx} -- create the spoke first:"
    echo "   kind create cluster --config poc/kind/spoke-${env}.yaml"
    exit 1
  fi

  # The node's InternalIP is its address on the shared container network, which
  # is exactly what the hub can reach. Provider-agnostic: works the same under
  # Docker and Podman, and needs neither binary.
  ip="$(kubectl --context "$spoke_ctx" get nodes \
        -o jsonpath='{.items[0].status.addresses[?(@.type=="InternalIP")].address}' 2>/dev/null)"
  [[ -n "$ip" ]] || {
    echo "!! could not read the InternalIP of ${spoke_ctx}'s first node."
    echo "   Is the spoke Ready? Nodes stay NotReady until Cilium is installed,"
    echo "   but the InternalIP is set well before that."
    exit 1
  }
  ADDR[$env]="https://${ip}:6443"
  echo "==> ${env}: ${ADDR[$env]}"

  # --cluster-endpoint direct makes argocd store the container address instead
  # of the kubeconfig's 127.0.0.1. Without it the hub cannot reach the spoke.
  argocd cluster add "kind-workload-${env}" \
    --name "${env}" \
    --cluster-endpoint direct \
    --upsert \
    --yes
done

echo
echo "==> rewriting POC values with the real endpoints"
for env in "${SPOKES[@]}"; do
  up="$(echo "$env" | tr '[:lower:]' '[:upper:]')"
  for f in "${REPO_ROOT}/poc/values-app-sets-kind.yaml" \
           "${REPO_ROOT}/poc/values-app-projects-kind.yaml"; do
    # BSD and GNU sed disagree about -i; write through a temp file instead.
    sed "s|https://REPLACE-ME-${up}-IP:6443|${ADDR[$env]}|g" "$f" > "$f.tmp"
    mv "$f.tmp" "$f"
  done
done

echo
echo "==> registered clusters"
argocd cluster list

cat <<'NEXT'

Next:
  1. COMMIT AND PUSH the two updated poc/values-*.yaml files if your ArgoCD
     reads them from Git. If you install them with `helm -f` from a local
     checkout, no push is needed -- but the repo.url inside them must still
     point at a repository ArgoCD can clone.
  2. helm upgrade --install app-projects argocd-apps/app-projects -n argocd \
       -f poc/values-app-projects-kind.yaml
  3. helm upgrade --install app-sets argocd-apps/app-sets -n argocd \
       -f poc/values-app-sets-kind.yaml

Order matters: projects before app-sets.
NEXT
