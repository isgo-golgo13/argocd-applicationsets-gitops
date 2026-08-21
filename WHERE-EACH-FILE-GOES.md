# Where each file goes

Two different repositories. Copy the contents of each directory over the
matching repo.

## `app-repo/` → your application repository

```
Makefile        drone-convoy-tracker/Makefile
```

Replaces the existing Makefile. Changes:

- **New Docker Hub config**: `DOCKERHUB_USER ?= isgogolgo13`,
  `REGISTRY ?= docker.io/$(DOCKERHUB_USER)`, `PLATFORMS ?= linux/amd64,linux/arm64`.
  Override on the command line, never by editing.
- **New targets**: `login`, `push`, `release`, `image-docker`, `push-multiarch`,
  `images-show`.
- **BUG FIX** — `kind-load` depended on `images`, which is not a target. The
  real one is `image`. Make failed with *"No rule to make target 'images'"*
  every single time it was run.
- **`kind-load` now goes through an archive.** `kind load docker-image` reads
  the Docker daemon and cannot see a Podman-built image even with
  `KIND_EXPERIMENTAL_PROVIDER=podman`. `podman save` + `kind load image-archive`
  works under either runtime.
- **`KIND_CLUSTER ?= drone-ops`** so `kind-load` and `kind-down` can target the
  three-cluster POC:
  `make kind-load KIND_CLUSTER=workload-nonprod`

Nothing else was touched. Every existing target still resolves — verified with
`make -n`.

## `gitops-repo/` → argocd-applicationsets-gitops

```
poc/README.md                                           NEW
argocd-apps/cluster-apps/drone-convoy-tracker/values.yaml   image block only
```

`values.yaml` changes only the `image:` block — registry moves from
`ghcr.io/CHANGEME-ORG` to `docker.io/isgogolgo13`. Everything else is unchanged.

---

## The three-command path

From the application repo:

```bash
make login          # Docker Hub access token, not your password
make release        # build both images, tag, push
```

Then the chart pulls from Docker Hub with no side-loading, which is also how a
real cluster behaves.

On Apple silicon `make release` pushes **arm64 only**. Correct for KinD on the
same Mac, wrong for an amd64 cluster — use `make push-multiarch` for that.
