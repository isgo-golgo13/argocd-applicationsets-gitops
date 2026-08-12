## Cluster Secrets for ArgoCD

This directory stores all the registered Kubernetes spoke cluster secrets for ArgoCD control-plane to provision
cluster-addons and cluster apps using their `kubeconfig` contexts.

Place all the sealed-*.yaml (Sealed Secrets) for all four env spoke clusters in this directory.