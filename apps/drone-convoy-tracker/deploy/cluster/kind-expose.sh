#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# deploy/cluster/kind-expose.sh -- make the chart's Cilium Gateway reachable
# from the Mac. Cilium renders each Gateway as a LoadBalancer Service that
# stays <pending> on KinD; convert it to NodePort on the ports kind-config.yaml
# maps to the host, and pin the Cilium gateway envoy to the ingress-ready node.
#
# Why not MetalLB: it would announce an IP inside the container VM's network,
# invisible to the Mac. Port mapping is the honest local answer.
# ---------------------------------------------------------------------------
set -euo pipefail
NS=${1:-drone-ops}
GW=${2:-drone-gateway}
SVC="cilium-gateway-$GW"
echo "▶ waiting for Service $NS/$SVC"
until kubectl -n "$NS" get svc "$SVC" >/dev/null 2>&1; do sleep 2; done
kubectl -n "$NS" patch svc "$SVC" --type merge -p '{
  "spec": {
    "type": "NodePort",
    "ports": [
      {"name":"port-80",  "port":80,  "targetPort":80,  "nodePort":30080, "protocol":"TCP"},
      {"name":"port-443", "port":443, "targetPort":443, "nodePort":30443, "protocol":"TCP"}
    ]
  }}'
# Cilium's per-Gateway envoy runs as a Deployment; pin it to the mapped node.
kubectl -n "$NS" patch deploy "cilium-gateway-$GW" --type merge -p \
  '{"spec":{"template":{"spec":{"nodeSelector":{"ingress-ready":"true"},"tolerations":[{"key":"node-role.kubernetes.io/control-plane","operator":"Exists","effect":"NoSchedule"}]}}}}' 2>/dev/null || true
echo "✓ Gateway exposed:  https://drone.localtest.me  (self-signed cert: accept the warning)"
echo "  Playground:       https://drone.localtest.me/graphql"
