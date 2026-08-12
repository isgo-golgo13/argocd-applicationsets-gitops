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
VERSION := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
GIT_SHA := $(shell git rev-parse --short HEAD 2>/dev/null || echo "unknown")
BUILD_TIME := $(shell date -u +"%Y-%m-%dT%H:%M:%SZ")

ROOT_DIR := $(shell pwd)
TARGET_DIR := $(ROOT_DIR)/target
DIST_DIR := $(ROOT_DIR)/dist
FRONTEND_DIR := $(ROOT_DIR)/crates/drone-frontend
SCHEMA_DIR := $(ROOT_DIR)/schema

CARGO := cargo
RUSTFLAGS_RELEASE := -C target-cpu=native -C opt-level=3 -C lto=fat -C codegen-units=1
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

.PHONY: help
help:
	@printf "\n"
	@printf "$(CYAN)╔══════════════════════════════════════════════════════════════════╗$(NC)\n"
	@printf "$(CYAN)║         DRONE CONVOY TRACKER - Build System                      ║$(NC)\n"
	@printf "$(CYAN)╚══════════════════════════════════════════════════════════════════╝$(NC)\n"
	@printf "\n"
	@printf "$(YELLOW)Usage:$(NC) make [target]\n"
	@printf "\n"
	@printf "$(GREEN)Build Targets:$(NC)\n"
	@printf "  $(BLUE)all$(NC)              Build everything (backend + frontend)\n"
	@printf "  $(BLUE)quick$(NC)            Quick debug build (backend only, no frontend)\n"
	@printf "  $(BLUE)build-backend$(NC)    Build all backend crates (release)\n"
	@printf "  $(BLUE)build-frontend$(NC)   Build Leptos frontend (WASM)\n"
	@printf "  $(BLUE)build-debug$(NC)      Build all crates (debug mode)\n"
	@printf "  $(BLUE)build-api$(NC)        Build only GraphQL API server\n"
	@printf "  $(BLUE)build-simulator$(NC)  Build only drone simulator\n"
	@printf "\n"
	@printf "$(GREEN)Run Targets:$(NC)\n"
	@printf "  $(BLUE)run-api$(NC)          Run GraphQL API (debug)\n"
	@printf "  $(BLUE)run-api-release$(NC)  Run GraphQL API (release)\n"
	@printf "  $(BLUE)run-simulator$(NC)    Run drone simulator (debug)\n"
	@printf "  $(BLUE)run-simulator-release$(NC) Run drone simulator (release)\n"
	@printf "\n"
	@printf "$(GREEN)Development:$(NC)\n"
	@printf "  $(BLUE)dev$(NC)              Start full development environment\n"
	@printf "  $(BLUE)dev-backend$(NC)      Start API in watch mode\n"
	@printf "  $(BLUE)dev-frontend$(NC)     Start frontend dev server\n"
	@printf "  $(BLUE)dev-db$(NC)           Start development databases\n"
	@printf "\n"
	@printf "$(GREEN)Database:$(NC)\n"
	@printf "  $(BLUE)db-init$(NC)          Initialize schema (dev mode)\n"
	@printf "  $(BLUE)db-init-dev$(NC)      Initialize with SimpleStrategy\n"
	@printf "  $(BLUE)db-init-prod$(NC)     Initialize with NetworkTopologyStrategy\n"
	@printf "  $(BLUE)db-reset$(NC)         Drop and recreate keyspace\n"
	@printf "  $(BLUE)db-status$(NC)        Check ScyllaDB connection\n"
	@printf "  $(BLUE)db-shell$(NC)         Open cqlsh shell\n"
	@printf "\n"
	@printf "$(GREEN)WASM/Frontend:$(NC)\n"
	@printf "  $(BLUE)setup-wasm$(NC)       Install WASM toolchain and trunk\n"
	@printf "  $(BLUE)wasm-check$(NC)       Verify WASM environment is ready\n"
	@printf "\n"
	@printf "$(GREEN)Testing:$(NC)\n"
	@printf "  $(BLUE)test$(NC)             Run all tests\n"
	@printf "  $(BLUE)test-unit$(NC)        Run unit tests\n"
	@printf "  $(BLUE)lint$(NC)             Run linters (fmt + clippy)\n"
	@printf "\n"
	@printf "$(GREEN)Production:$(NC)\n"
	@printf "  $(BLUE)prod$(NC)             Full production build\n"
	@printf "  $(BLUE)image$(NC)           Build container images with Podman\n"
	@printf "  $(BLUE)package$(NC)          Create distribution package\n"
	@printf "\n"
	@printf "$(GREEN)Utilities:$(NC)\n"
	@printf "  $(BLUE)setup$(NC)            Install development dependencies\n"
	@printf "  $(BLUE)clean$(NC)            Clean build artifacts\n"
	@printf "  $(BLUE)docs$(NC)             Generate documentation\n"
	@printf "\n"
	@printf "$(PURPLE)Version: $(VERSION) | SHA: $(GIT_SHA)$(NC)\n"
	@printf "\n"

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
build: all

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

.PHONY: run-simulator
run-simulator:
	@printf "$(CYAN)▶ Starting Drone Simulator...$(NC)\n"
	@$(CARGO) run --package drone-simulator

.PHONY: run-simulator-release
run-simulator-release: build-simulator
	@printf "$(CYAN)▶ Starting Drone Simulator (release)...$(NC)\n"
	@$(TARGET_DIR)/release/drone-simulator

# ------------------------------------------------------------------------------
# Development
# ------------------------------------------------------------------------------

.PHONY: dev
dev:
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
	@printf "$(GREEN)✓ Dev schema initialized$(NC)\n"

.PHONY: db-init-prod
db-init-prod:
	@printf "$(CYAN)▶ Initializing ScyllaDB schema (production)...$(NC)\n"
	@printf "$(YELLOW)⚠ Using NetworkTopologyStrategy - ensure datacenters exist$(NC)\n"
	@cqlsh $(SCYLLA_HOST) $(SCYLLA_PORT) -f $(SCHEMA_DIR)/cql/000_keyspace_prod.cql || \
		{ printf "$(RED)✗ Failed to create keyspace$(NC)\n"; exit 1; }
	@cqlsh $(SCYLLA_HOST) $(SCYLLA_PORT) -f $(SCHEMA_DIR)/cql/001_core_schema.cql || \
		{ printf "$(RED)✗ Failed to create tables$(NC)\n"; exit 1; }
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
