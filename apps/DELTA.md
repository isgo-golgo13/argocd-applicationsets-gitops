# Delta since argocd-applicationsets-gitops-p2.zip

Only changed files are included. Overlay onto your working copy.

| File | Change |
|---|---|
| `apps/drone-convoy-tracker/Makefile` | New front door: `serve` / `stop` / `logs` added, `help` trimmed to 8 verbs, `help-all` lists everything. No existing target removed. |
| `apps/drone-convoy-tracker/containers/Containerfile.frontend` | Fixed three build blockers: `rust:1.83` -> `ARG RUST_VERSION=1.89` (edition 2024 needs >= 1.85); `COPY docker/nginx.conf` -> `containers/nginx.conf` (stale after the docker/ -> containers/ rename); `nginx:alpine` -> `docker.io/library/nginx:1.27-alpine` (fully qualified + pinned).

Nothing else changed. No deletions this round.

## Run it

    cd apps/drone-convoy-tracker
    make serve

Dashboard http://localhost:3000 - GraphQL http://localhost:8080/graphql - Grafana http://localhost:3001
