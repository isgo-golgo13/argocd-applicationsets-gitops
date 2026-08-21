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
# spoke over the shared `kind` Docker network instead.
#
# Getting this wrong is the single most common way this POC fails, and it fails
# confusingly: `argocd cluster add` appears to succeed and every Application
# then reports a connection error.
#
#   ./poc/register-spokes.sh
#
# Prerequisites: kind, kubectl, docker, argocd CLI, and an argocd login against
# the hub (README-setup-poc.md step 5).
# ---------------------------------------------------------------------------
set -euo pipefail

SPOKES=("nonprod" "prod")
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

command -v argocd >/dev/null || { echo "argocd CLI not found"; exit 1; }
command -v docker >/dev/null || { echo "docker not found"; exit 1; }

echo "==> hub context: kind-gitops-hub"
kubectl config use-context kind-gitops-hub >/dev/null

declare -A ADDR
for env in "${SPOKES[@]}"; do
  container="workload-${env}-control-plane"

  if ! docker inspect "$container" >/dev/null 2>&1; then
    echo "!! $container not running -- create the spoke first:"
    echo "   kind create cluster --config poc/kind/spoke-${env}.yaml"
    exit 1
  fi

  # The address on the shared kind Docker network, which is what the hub can
  # reach. Not the kubeconfig address.
  ip="$(docker inspect "$container" \
        --format '{{ (index .NetworkSettings.Networks "kind").IPAddress }}')"
  [[ -n "$ip" ]] || { echo "!! could not read the kind-network IP of $container"; exit 1; }
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
