# ==============================================================================
# DRONE CONVOY TRACKER - Production Build System
# ==============================================================================
# Classification: UNCLASSIFIED // FOR OFFICIAL USE ONLY
#
# Makefile for building, testing, and deploying the complete drone convoy
# tracking system including ScyllaDB backend, GraphQL API, and Leptos frontend.
#
# Usage:
#   make help          - Show available targets
#   make all           - Build everything (backend + frontend)
#   make dev           - Start development environment
#   make prod          - Build for production
#   make image        - Build container images with Podman
# ==============================================================================

SHELL := /bin/bash
.ONESHELL:
.SHELLFLAGS := -eu -o pipefail -c
.DELETE_ON_ERROR:
MAKEFLAGS += --warn-undefined-variables
MAKEFLAGS += --no-builtin-rules

# ------------------------------------------------------------------------------
# Configuration
# ------------------------------------------------------------------------------

PROJECT_NAME := drone-convoy-tracker
RUST_VERSION ?= 1.89   # must be >= 1.85 for edition 2024

# Source trees whose mtimes drive rebuild detection. Deliberately excludes
# containers/ -- Podman keys its COPY cache on content, so touching those files
# only invalidates layer cache for no gain.
TOUCH_PATHS := Cargo.toml Cargo.lock $(wildcard crates) $(wildcard config) $(wildcard schema) $(wildcard assets)

# Set TOUCH=0 to skip the mtime sweep in `make build` (see the touch target).
TOUCH ?= 1
VERSION := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
GIT_SHA := $(shell git rev-parse --short HEAD 2>/dev/null || echo "unknown")
BUILD_TIME := $(shell date -u +"%Y-%m-%dT%H:%M:%SZ")

ROOT_DIR := $(shell pwd)
TARGET_DIR := $(ROOT_DIR)/target
DIST_DIR := $(ROOT_DIR)/dist
FRONTEND_DIR := $(ROOT_DIR)/crates/drone-frontend
SCHEMA_DIR := $(ROOT_DIR)/schema

CARGO := cargo
# Codegen settings (opt-level, lto, codegen-units) live in [profile.release] in
# Cargo.toml -- see the note there. Leave this empty unless you have a reason.
#
# `-C target-cpu=native` is the one flag worth adding by hand, and only for a
# local build: it produces binaries that may not run on another CPU, and it
# makes `cargo build` and `cargo check` disagree on fingerprints, so every
# switch between them recompiles the world.
#   make build RUSTFLAGS_RELEASE="-C target-cpu=native"
RUSTFLAGS_RELEASE ?=
CARGO_BUILD_JOBS := $(shell nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

TRUNK := trunk
WASM_TARGET := wasm32-unknown-unknown

PODMAN := podman
COMPOSE := podman-compose
IMAGE_REGISTRY ?= localhost
IMAGE_TAG := $(VERSION)-$(GIT_SHA)

SCYLLA_HOST := localhost
SCYLLA_PORT := 9042
SCYLLA_KEYSPACE := drone_ops
REDIS_URL := redis://localhost:6379

RED := \033[0;31m
GREEN := \033[0;32m
YELLOW := \033[0;33m
BLUE := \033[0;34m
PURPLE := \033[0;35m
CYAN := \033[0;36m
NC := \033[0m

.DEFAULT_GOAL := help

# ------------------------------------------------------------------------------
# Help
# ------------------------------------------------------------------------------

# ==============================================================================
# FRONT DOOR -- the only targets you need day to day.
#
#   make serve    run API + dashboard NATIVELY (only ScyllaDB/Redis in podman)
#   make stop     stop ScyllaDB and Redis
#   make logs     follow ScyllaDB logs
#   make build    compile everything locally (backend + frontend WASM)
#   make dev      run API + frontend with hot reload against a local DB
#   make test     workspace tests
#   make lint     rustfmt check + clippy
#   make clean    remove build artifacts
#   make touch    restamp source mtimes -- `build` runs this automatically
#
# Why touch: overlaying a zip or checkout preserves the ARCHIVE's mtimes, so a
# file you just changed can land with a timestamp OLDER than the artifact built
# from its previous version. Cargo and Trunk decide staleness from mtimes, so
# the change compiles away to nothing and you stare at stale output. Restamping
# before every build makes that class of bug impossible.
#
# The cost is real: every source becomes newer than every artifact, so `build`
# is always a full rebuild, never incremental. That is the right trade for a
# build you run after dropping in changes, and the wrong one for a tight edit
# loop -- which is why `make dev` does NOT touch. Use TOUCH=0 make build to opt
# out for a single run.
#
# Everything below the front door is machinery those targets call. Run
# `make help-all` to see it.
# ==============================================================================

.PHONY: serve
serve: deps-up
	@printf "$(CYAN)▶ Starting API (release), frontend dev server and the convoy service...$(NC)\n"
	@printf "$(GREEN)  Dashboard          http://localhost:3000$(NC)\n"
	@printf "$(GREEN)  GraphQL playground http://localhost:8080/graphql$(NC)\n"
	@printf "$(GREEN)  Convoy service     flies sorties back-to-back; the dashboard's THEATER selector retasks it$(NC)\n"
	@printf "$(YELLOW)  Ctrl-C stops all three. 'make stop' stops ScyllaDB and Redis. SIM=0 to run without the service.$(NC)\n"
	@echo ""
	@$(MAKE) --no-print-directory -j3 serve-api serve-frontend $(if $(filter 0,$(SIM)),,serve-sim)

# The convoy service: the simulator in --service mode. It waits for the API,
# flies sorties back to back, and obeys tasking orders from the convoy record
# (the dashboard writes them via retaskConvoy). One process, no flags after
# `make serve`. THEATER only seeds the FIRST sortie when the record has no
# tasking yet; the dashboard is the commander from then on.
.PHONY: serve-sim
serve-sim:
	@DRONE_SERVICE=true DRONE_THEATER=$(THEATER) $(CARGO) run --release --package drone-simulator

.PHONY: serve-api
serve-api:
	@SCYLLA_HOSTS=$(SCYLLA_HOST):$(SCYLLA_PORT) \
	 SCYLLA_KEYSPACE=$(SCYLLA_KEYSPACE) \
	 REDIS_URL=$(REDIS_URL) \
	 SERVER_ADDR=0.0.0.0:8080 \
	 RUST_LOG=$${RUST_LOG:-info,drone_graphql_api=debug} \
	 $(CARGO) run --release --package drone-graphql-api

.PHONY: serve-frontend
serve-frontend: wasm-check
	@cd $(FRONTEND_DIR) && $(TRUNK) serve

# ------------------------------------------------------------------------------
# deps -- ScyllaDB and Redis only. Plain `podman run`, deliberately NOT compose:
# the app runs natively, so there is no reason to make podman-compose a
# prerequisite for local development. `make stack-up` is the all-in-containers
# path if you want it.
# ------------------------------------------------------------------------------
.PHONY: deps-up
deps-up:
	@printf "$(CYAN)▶ Starting ScyllaDB and Redis...$(NC)\n"
	@if $(PODMAN) container exists scylla >/dev/null 2>&1; then \
		$(PODMAN) start scylla >/dev/null; \
	else \
		$(PODMAN) run -d --name scylla -p $(SCYLLA_PORT):9042 \
			docker.io/scylladb/scylla:6.2 \
			--smp 2 --memory 2G --overprovisioned 1 --developer-mode 1 >/dev/null; \
	fi
	@if $(PODMAN) container exists redis >/dev/null 2>&1; then \
		$(PODMAN) start redis >/dev/null; \
	else \
		$(PODMAN) run -d --name redis -p 6379:6379 \
			docker.io/library/redis:7-alpine \
			redis-server --appendonly yes --maxmemory 256mb >/dev/null; \
	fi
	@printf "$(YELLOW)  Waiting for ScyllaDB to accept CQL (first boot ~60s)...$(NC)\n"
	@for i in $$(seq 1 60); do \
		if $(PODMAN) exec scylla cqlsh -e "describe keyspaces" >/dev/null 2>&1; then break; fi; \
		sleep 3; \
	done
	@$(PODMAN) exec scylla cqlsh -e "describe keyspaces" >/dev/null 2>&1 || \
		{ printf "$(RED)✗ ScyllaDB never came up. 'podman logs scylla' for why.$(NC)\n"; exit 1; }
	@$(MAKE) --no-print-directory db-init-dev
	@printf "$(GREEN)✓ ScyllaDB and Redis ready$(NC)\n"

.PHONY: stop
stop:
	@printf "$(CYAN)▶ Stopping ScyllaDB and Redis...$(NC)\n"
	@-$(PODMAN) stop scylla redis >/dev/null 2>&1
	@printf "$(GREEN)✓ Stopped (data preserved; 'make deps-clean' to wipe)$(NC)\n"

.PHONY: deps-clean
deps-clean:
	@-$(PODMAN) rm -f scylla redis >/dev/null 2>&1
	@printf "$(GREEN)✓ Dependency containers removed$(NC)\n"

.PHONY: logs
logs:
	@$(PODMAN) logs -f scylla

.PHONY: help
help:
	@printf "\n"
	@printf "$(CYAN)  DRONE CONVOY TRACKER$(NC)\n"
	@printf "\n"
	@printf "  $(BLUE)make serve$(NC)    run API + dashboard natively -> http://localhost:3000\n"
	@printf "  $(BLUE)make stop$(NC)     stop ScyllaDB and Redis\n"
	@printf "  $(BLUE)make logs$(NC)     follow ScyllaDB logs\n"
	@printf "  $(BLUE)make build$(NC)    compile backend + frontend\n"
	@printf "  $(BLUE)make dev$(NC)      same as serve, but with hot reload\n"
	@printf "  $(BLUE)make test$(NC)     workspace tests\n"
	@printf "  $(BLUE)make lint$(NC)     rustfmt check + clippy\n"
	@printf "  $(BLUE)make clean$(NC)    remove build artifacts\n"
	@printf "  $(BLUE)make touch$(NC)    restamp source mtimes (build runs this for you)\n"
	@printf "\n"
	@printf "  $(YELLOW)make stack-up$(NC)  everything in containers instead (needs podman-compose)\n"
	@printf "  $(YELLOW)make kind-up$(NC)   3+3 KinD cluster w/ Cilium Gateway API + all operators\n"
	@printf "  $(YELLOW)make kind-deploy$(NC) helm install the chart into it -> https://drone.localtest.me\n"
	@printf "  $(YELLOW)make kind-down$(NC) delete the KinD cluster\n"
	@printf "  $(YELLOW)make help-all$(NC)  full target list\n"
	@printf "\n"

.PHONY: help-all
help-all:
	@printf "\n$(CYAN)  All targets$(NC)\n\n"
	@grep -E '^[a-z][a-z0-9_-]*:' $(MAKEFILE_LIST) \
		| cut -d: -f1 | sort -u | pr -4 -t -w 80

.PHONY: touch
touch:
	@printf "$(CYAN)▶ Normalising source mtimes...$(NC)\n"
	@find $(TOUCH_PATHS) -type f -not -path '*/target/*' -not -path '*/dist/*' -exec touch {} +
	@printf "$(GREEN)✓ mtimes normalised$(NC)\n"

# ------------------------------------------------------------------------------
# Setup
# ------------------------------------------------------------------------------

.PHONY: setup
setup:
	@printf "$(CYAN)▶ Installing development dependencies...$(NC)\n"
	@rustup show active-toolchain || rustup default stable
	@rustup target add $(WASM_TARGET)
	@cargo install trunk --locked 2>/dev/null || true
	@cargo install wasm-bindgen-cli --locked 2>/dev/null || true
	@cargo install cargo-watch --locked 2>/dev/null || true
	@cargo install cargo-audit --locked 2>/dev/null || true
	@printf "$(GREEN)✓ Setup complete!$(NC)\n"

.PHONY: setup-wasm
setup-wasm:
	@printf "$(CYAN)▶ Setting up WASM environment...$(NC)\n"
	@rustup target add $(WASM_TARGET)
	@cargo install trunk --locked 2>/dev/null || echo "trunk already installed"
	@cargo install wasm-bindgen-cli --locked 2>/dev/null || echo "wasm-bindgen-cli already installed"
	@printf "$(GREEN)✓ WASM environment ready$(NC)\n"
	@echo ""
	@echo "  WASM target: $(WASM_TARGET)"
	@echo "  Trunk:       $$(trunk --version 2>/dev/null || echo 'not found')"
	@echo ""

.PHONY: wasm-check
wasm-check:
	@printf "$(CYAN)▶ Checking WASM environment...$(NC)\n"
	@rustup target list --installed | grep -q $(WASM_TARGET) && \
		echo "$(GREEN)✓ WASM target installed$(NC)" || \
		{ echo "$(RED)✗ WASM target missing - run 'make setup-wasm'$(NC)"; exit 1; }
	@command -v trunk >/dev/null 2>&1 && \
		echo "$(GREEN)✓ trunk: $$(trunk --version)$(NC)" || \
		{ echo "$(RED)✗ trunk not found - run 'make setup-wasm'$(NC)"; exit 1; }
	@command -v wasm-bindgen >/dev/null 2>&1 && \
		echo "$(GREEN)✓ wasm-bindgen: $$(wasm-bindgen --version)$(NC)" || \
		echo "$(YELLOW)⚠ wasm-bindgen not found (optional)$(NC)"

.PHONY: check-deps
check-deps:
	@command -v cargo >/dev/null 2>&1 || { echo "$(RED)✗ cargo not found$(NC)"; exit 1; }
	@command -v trunk >/dev/null 2>&1 || { echo "$(RED)✗ trunk not found - run 'make setup'$(NC)"; exit 1; }
	@rustup target list --installed | grep -q $(WASM_TARGET) || { echo "$(RED)✗ WASM target not installed$(NC)"; exit 1; }
	@printf "$(GREEN)✓ Dependencies OK$(NC)\n"

# ------------------------------------------------------------------------------
# Build
# ------------------------------------------------------------------------------

.PHONY: all
all: build-backend build-frontend
	@printf "$(GREEN)✓ Full build complete!$(NC)\n"

.PHONY: build
build:
ifeq ($(TOUCH),1)
	@$(MAKE) --no-print-directory touch
endif
	@$(MAKE) --no-print-directory all

.PHONY: build-backend
build-backend:
	@printf "$(CYAN)▶ Building backend crates...$(NC)\n"
	@RUSTFLAGS="$(RUSTFLAGS_RELEASE)" $(CARGO) build \
		--release \
		--jobs $(CARGO_BUILD_JOBS) \
		--workspace \
		--exclude drone-frontend
	@printf "$(GREEN)✓ Backend build complete$(NC)\n"

.PHONY: build-frontend
build-frontend: wasm-check
	@printf "$(CYAN)▶ Building frontend (WASM)...$(NC)\n"
	@cd $(FRONTEND_DIR) && $(TRUNK) build --release
	@printf "$(GREEN)✓ Frontend build complete$(NC)\n"
	@echo "  Output: $(FRONTEND_DIR)/dist/"

.PHONY: build-debug
build-debug:
	@printf "$(CYAN)▶ Building (debug mode)...$(NC)\n"
	@$(CARGO) build --workspace
	@printf "$(GREEN)✓ Debug build complete$(NC)\n"

.PHONY: build-api
build-api:
	@printf "$(CYAN)▶ Building GraphQL API...$(NC)\n"
	@RUSTFLAGS="$(RUSTFLAGS_RELEASE)" $(CARGO) build --release --package drone-graphql-api
	@printf "$(GREEN)✓ API: $(TARGET_DIR)/release/drone-graphql-api$(NC)\n"

.PHONY: build-simulator
build-simulator:
	@printf "$(CYAN)▶ Building Drone Simulator...$(NC)\n"
	@RUSTFLAGS="$(RUSTFLAGS_RELEASE)" $(CARGO) build --release --package drone-simulator
	@printf "$(GREEN)✓ Simulator: $(TARGET_DIR)/release/drone-simulator$(NC)\n"

# Quick debug build (excludes frontend - much faster)
.PHONY: quick
quick:
	@printf "$(CYAN)▶ Quick build (backend only, debug)...$(NC)\n"
	@$(CARGO) build --workspace --exclude drone-frontend
	@printf "$(GREEN)✓ Quick build complete$(NC)\n"
	@echo "  API:       $(TARGET_DIR)/debug/drone-api"
	@echo "  Simulator: $(TARGET_DIR)/debug/drone-simulator"

# ------------------------------------------------------------------------------
# Run
# ------------------------------------------------------------------------------

.PHONY: run-api
run-api:
	@printf "$(CYAN)▶ Starting GraphQL API...$(NC)\n"
	@$(CARGO) run --package drone-graphql-api

.PHONY: run-api-release
run-api-release: build-api
	@printf "$(CYAN)▶ Starting GraphQL API (release)...$(NC)\n"
	@$(TARGET_DIR)/release/drone-api

# Theater that SEEDS the convoy service's first sortie when the convoy record
# carries no tasking yet. After that the dashboard's THEATER selector is the
# commander (retaskConvoy) and this value is ignored. Also the theater for a
# manual `make run-simulator` sortie.
THEATER ?= afghanistan

.PHONY: run-simulator
run-simulator:  ## single manual sortie (dev). `make serve` already runs the convoy service.
	@printf "$(CYAN)▶ Starting Drone Simulator — single sortie ($(THEATER))...$(NC)\n"
	@DRONE_THEATER=$(THEATER) $(CARGO) run --package drone-simulator

.PHONY: run-simulator-release
run-simulator-release: build-simulator
	@printf "$(CYAN)▶ Starting Drone Simulator (release)...$(NC)\n"
	@$(TARGET_DIR)/release/drone-simulator

# ------------------------------------------------------------------------------
# Development
# ------------------------------------------------------------------------------

.PHONY: dev
dev: deps-up
	@printf "$(CYAN)▶ Starting development environment...$(NC)\n"
	@printf "$(YELLOW)  Starting API + Frontend in parallel...$(NC)\n"
	@$(MAKE) -j2 dev-backend dev-frontend

.PHONY: dev-backend
dev-backend:
	@printf "$(CYAN)▶ Starting API server (watch mode)...$(NC)\n"
	@cargo watch -x 'run --package drone-graphql-api'

.PHONY: dev-frontend
dev-frontend: wasm-check
	@printf "$(CYAN)▶ Starting frontend dev server...$(NC)\n"
	@cd $(FRONTEND_DIR) && $(TRUNK) serve --open

.PHONY: dev-db
dev-db:
	@printf "$(CYAN)▶ Starting development databases...$(NC)\n"
	@podman-compose -f containers/podman-compose.dev.yml up -d scylla redis
	@printf "$(YELLOW)  Waiting for ScyllaDB...$(NC)\n"
	@sleep 15
	@$(MAKE) db-init
	@printf "$(GREEN)✓ Databases ready$(NC)\n"

.PHONY: dev-stop
dev-stop:
	@podman-compose -f containers/podman-compose.dev.yml down

# ------------------------------------------------------------------------------
# Database
# ------------------------------------------------------------------------------

.PHONY: db-init
db-init: db-init-dev

.PHONY: db-init-dev
db-init-dev:
	@printf "$(CYAN)▶ Initializing ScyllaDB schema (development)...$(NC)\n"
	@$(PODMAN) exec -i scylla cqlsh < $(SCHEMA_DIR)/cql/000_keyspace_dev.cql || \
		{ printf "$(RED)✗ Failed to create keyspace$(NC)\n"; exit 1; }
	@$(PODMAN) exec -i scylla cqlsh < $(SCHEMA_DIR)/cql/001_core_schema.cql || \
		{ printf "$(RED)✗ Failed to create tables$(NC)\n"; exit 1; }
	@$(PODMAN) exec -i scylla cqlsh < $(SCHEMA_DIR)/cql/002_waypoint_columns.cql 2>/dev/null || true
	@printf "$(GREEN)✓ Dev schema initialized$(NC)\n"

.PHONY: db-init-prod
db-init-prod:
	@printf "$(CYAN)▶ Initializing ScyllaDB schema (production)...$(NC)\n"
	@printf "$(YELLOW)⚠ Using NetworkTopologyStrategy - ensure datacenters exist$(NC)\n"
	@cqlsh $(SCYLLA_HOST) $(SCYLLA_PORT) -f $(SCHEMA_DIR)/cql/000_keyspace_prod.cql || \
		{ printf "$(RED)✗ Failed to create keyspace$(NC)\n"; exit 1; }
	@cqlsh $(SCYLLA_HOST) $(SCYLLA_PORT) -f $(SCHEMA_DIR)/cql/001_core_schema.cql || \
		{ printf "$(RED)✗ Failed to create tables$(NC)\n"; exit 1; }
	@cqlsh $(SCYLLA_HOST) $(SCYLLA_PORT) -f $(SCHEMA_DIR)/cql/002_waypoint_columns.cql 2>/dev/null || true
	@printf "$(GREEN)✓ Production schema initialized$(NC)\n"

.PHONY: db-reset
db-reset:
	@printf "$(YELLOW)⚠ Dropping and recreating drone_ops keyspace...$(NC)\n"
	@$(PODMAN) exec -i scylla cqlsh -e "DROP KEYSPACE IF EXISTS drone_ops;" || true
	@$(MAKE) db-init-dev
	@printf "$(GREEN)✓ Database reset complete$(NC)\n"

.PHONY: db-status
db-status:
	@printf "$(CYAN)▶ Checking ScyllaDB status...$(NC)\n"
	@$(PODMAN) exec -i scylla cqlsh -e "DESCRIBE KEYSPACES;" && \
		printf "$(GREEN)✓ ScyllaDB is running$(NC)\n" || \
		printf "$(RED)✗ ScyllaDB not available$(NC)\n"

.PHONY: db-shell
db-shell:
	@$(PODMAN) exec -it scylla cqlsh

.PHONY: db-shell-ops
db-shell-ops:
	@$(PODMAN) exec -it scylla cqlsh -k drone_ops

.PHONY: redis-cli
redis-cli:
	@$(PODMAN) exec -it redis redis-cli

# ------------------------------------------------------------------------------
# Testing
# ------------------------------------------------------------------------------

.PHONY: test
test:
	@printf "$(CYAN)▶ Running tests...$(NC)\n"
	@$(CARGO) test --workspace --all-features
	@printf "$(GREEN)✓ All tests passed$(NC)\n"

.PHONY: test-unit
test-unit:
	@$(CARGO) test --workspace --lib

.PHONY: test-integration
test-integration:
	@$(CARGO) test --workspace --test '*'

# ------------------------------------------------------------------------------
# Linting
# ------------------------------------------------------------------------------

.PHONY: lint
lint: fmt-check clippy

.PHONY: fmt
fmt:
	@$(CARGO) fmt --all

.PHONY: fmt-check
fmt-check:
	@$(CARGO) fmt --all -- --check

.PHONY: clippy
clippy:
	@printf "$(CYAN)▶ Running Clippy...$(NC)\n"
	@$(CARGO) clippy --workspace --all-features -- -D warnings
	@printf "$(GREEN)✓ Clippy passed$(NC)\n"

.PHONY: audit
audit:
	@$(CARGO) audit

# ------------------------------------------------------------------------------
# Documentation
# ------------------------------------------------------------------------------

.PHONY: docs
docs:
	@$(CARGO) doc --workspace --no-deps --document-private-items
	@printf "$(GREEN)✓ Docs: $(TARGET_DIR)/doc/drone_domain/index.html$(NC)\n"

.PHONY: docs-open
docs-open: docs
	@open $(TARGET_DIR)/doc/drone_domain/index.html 2>/dev/null || \
		xdg-open $(TARGET_DIR)/doc/drone_domain/index.html

# ------------------------------------------------------------------------------
# Docker
# ------------------------------------------------------------------------------

.PHONY: image
image: image-api image-frontend

.PHONY: image-api
image-api:
	@printf "$(CYAN)▶ Building API Docker image...$(NC)\n"
	@$(PODMAN) build -f containers/Containerfile.api \
		--build-arg RUST_VERSION=$(RUST_VERSION) \
		-t $(IMAGE_REGISTRY)/$(PROJECT_NAME)-api:$(IMAGE_TAG) \
		-t $(IMAGE_REGISTRY)/$(PROJECT_NAME)-api:latest \
		--build-arg VERSION=$(VERSION) \
		--build-arg GIT_SHA=$(GIT_SHA) .
	@printf "$(GREEN)✓ API image built$(NC)\n"

.PHONY: image-frontend
image-frontend:
	@printf "$(CYAN)▶ Building frontend Docker image...$(NC)\n"
	@$(PODMAN) build -f containers/Containerfile.frontend \
		-t $(IMAGE_REGISTRY)/$(PROJECT_NAME)-frontend:$(IMAGE_TAG) \
		-t $(IMAGE_REGISTRY)/$(PROJECT_NAME)-frontend:latest .
	@printf "$(GREEN)✓ Frontend image built$(NC)\n"

.PHONY: stack-up
stack-up:
	@podman-compose -f containers/podman-compose.yml up -d
	@printf "$(GREEN)✓ Stack started$(NC)\n"
	@echo "  Frontend:  http://localhost:3000"
	@echo "  API:       http://localhost:8080/graphql"

.PHONY: stack-down
stack-down:
	@podman-compose -f containers/podman-compose.yml down

# ------------------------------------------------------------------------------
# Production
# ------------------------------------------------------------------------------

.PHONY: prod
prod: clean lint test build-backend build-frontend
	@echo ""
	@printf "$(GREEN)╔══════════════════════════════════════════════════════════════════╗$(NC)\n"
	@printf "$(GREEN)║              PRODUCTION BUILD COMPLETE                           ║$(NC)\n"
	@printf "$(GREEN)╚══════════════════════════════════════════════════════════════════╝$(NC)\n"
	@echo ""
	@echo "  API Binary:  $(TARGET_DIR)/release/drone-graphql-api"
	@echo "  Frontend:    $(FRONTEND_DIR)/dist/"
	@echo "  Version:     $(VERSION) ($(GIT_SHA))"
	@echo ""

.PHONY: package
package: prod
	@printf "$(CYAN)▶ Creating distribution package...$(NC)\n"
	@mkdir -p $(DIST_DIR)
	@cp $(TARGET_DIR)/release/drone-graphql-api $(DIST_DIR)/
	@cp -r $(FRONTEND_DIR)/dist $(DIST_DIR)/frontend
	@cp -r $(SCHEMA_DIR) $(DIST_DIR)/
	@cp README.md $(DIST_DIR)/
	@cd $(DIST_DIR) && tar -czf $(PROJECT_NAME)-$(VERSION).tar.gz *
	@printf "$(GREEN)✓ Package: $(DIST_DIR)/$(PROJECT_NAME)-$(VERSION).tar.gz$(NC)\n"

.PHONY: zip
zip:
	@mkdir -p $(DIST_DIR)
	@zip -r $(DIST_DIR)/$(PROJECT_NAME)-$(VERSION).zip . \
		-x "target/*" -x ".git/*" -x "dist/*" -x "*.zip"
	@printf "$(GREEN)✓ Archive: $(DIST_DIR)/$(PROJECT_NAME)-$(VERSION).zip$(NC)\n"

# ------------------------------------------------------------------------------
# Cleanup
# ------------------------------------------------------------------------------

.PHONY: clean
clean:
	@printf "$(CYAN)▶ Cleaning...$(NC)\n"
	@$(CARGO) clean
	@rm -rf $(FRONTEND_DIR)/dist
	@rm -rf $(DIST_DIR)
	@printf "$(GREEN)✓ Clean$(NC)\n"

# ------------------------------------------------------------------------------
# Utilities
# ------------------------------------------------------------------------------

.PHONY: loc
loc:
	@tokei . --exclude target --exclude dist 2>/dev/null || \
		find . -name "*.rs" -not -path "./target/*" | xargs wc -l | tail -1

.PHONY: deps
deps:
	@$(CARGO) tree --workspace

.PHONY: version
version:
	@echo "Project:    $(PROJECT_NAME)"
	@echo "Version:    $(VERSION)"
	@echo "Git SHA:    $(GIT_SHA)"
	@echo "Build Time: $(BUILD_TIME)"
	@echo "Rust:       $$(rustc --version)"

# ------------------------------------------------------------------------------
# CI
# ------------------------------------------------------------------------------

.PHONY: ci
ci: check-deps lint test build
	@printf "$(GREEN)✓ CI pipeline complete$(NC)\n"

.PHONY: ci-full
ci-full: ci audit image
	@printf "$(GREEN)✓ Full CI pipeline complete$(NC)\n"

# =============================================================================
# KUBERNETES -- KinD cluster + Helm chart (deploy/kubernetes, deploy/cluster)
# =============================================================================
CHART      := deploy/kubernetes/drone-convoy-attack-tracker
KIND_NS    ?= drone-ops
KIND_ENV   ?= nonprod

# The chart's schema-init hook needs the CQL files INSIDE the chart directory
# (Helm .Files cannot read outside it). schema/cql/ stays the single source of
# truth; this copies it in before any render/package and is gitignored there.
.PHONY: chart-sync
chart-sync:
	@mkdir -p $(CHART)/schema/cql && cp schema/cql/*.cql $(CHART)/schema/cql/

.PHONY: chart-lint
chart-lint: chart-sync
	helm lint $(CHART) -f $(CHART)/values-$(KIND_ENV).yaml

.PHONY: chart-template
chart-template: chart-sync
	helm template drone $(CHART) -n $(KIND_NS) -f $(CHART)/values-$(KIND_ENV).yaml

.PHONY: kind-up
kind-up:
	@bash deploy/cluster/kind-bootstrap.sh

.PHONY: kind-load
kind-load: images
	kind load docker-image $(IMAGE_REGISTRY)/$(PROJECT_NAME)-api:latest --name drone-ops
	kind load docker-image $(IMAGE_REGISTRY)/$(PROJECT_NAME)-frontend:latest --name drone-ops

.PHONY: kind-deploy
kind-deploy: chart-sync
	helm upgrade --install drone $(CHART) -n $(KIND_NS) --create-namespace \
		-f $(CHART)/values-$(KIND_ENV).yaml \
		--set image.registry=$(IMAGE_REGISTRY) \
		--set image.api.repository=$(PROJECT_NAME)-api --set image.api.tag=latest \
		--set image.frontend.repository=$(PROJECT_NAME)-frontend --set image.frontend.tag=latest \
		--wait --timeout 15m
	@bash deploy/cluster/kind-expose.sh $(KIND_NS) drone-gateway

.PHONY: kind-status
kind-status:
	kubectl -n $(KIND_NS) get gateway,httproute,certificate,externalsecret,scyllacluster,hpa,vpa,pods

.PHONY: kind-down
kind-down:
	kind delete cluster --name drone-ops
