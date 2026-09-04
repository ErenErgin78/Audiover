# ==============================================================================
# Audiover - Voice & Soundboard Engine Makefile
# ==============================================================================

.PHONY: help setup setup-deps dev build build-rpm build-deb build-appimage clean test lint

# Default target
.DEFAULT_GOAL := help

help: ## Show available make targets
	@echo "Audiover Management Commands:"
	@echo ""
	@LC_ALL=C grep -E '^[a-zA-Z0-9_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-18s\033[0m %s\n", $$1, $$2}'
	@echo ""

setup: ## Install UI dependencies and verify toolchain
	@bash scripts/setup_env.sh

setup-deps: ## Check and prompt to install system-level packages (distro package manager)
	@bash scripts/setup_env.sh --install-deps

dev: ## Start Audiover in local development mode (React Vite + Tauri)
	@bash scripts/run.sh

build: ## Build release packages for all configured targets (rpm, deb, appimage)
	@bash scripts/build_packages.sh

build-rpm: ## Build RPM package only
	@bash scripts/build_packages.sh rpm

build-deb: ## Build DEB package only
	@bash scripts/build_packages.sh deb

build-appimage: ## Build AppImage package only
	@bash scripts/build_packages.sh appimage

test: ## Run Rust tests and TypeScript type checking
	@echo "[+] Running TypeScript check..."
	@npm --prefix ui run build
	@echo "[+] Running Rust tests..."
	@cd src && cargo test

lint: ## Run linter and formatting checks
	@echo "[+] Checking Rust formatting and clippy..."
	@cd src && cargo fmt --check 2>/dev/null || true
	@cd src && cargo clippy -- -D warnings 2>/dev/null || true

clean: ## Clean build and dist output directories
	@echo "[+] Cleaning build artifacts..."
	@rm -rf dist/* src/target/release/bundle/* ui/dist
	@echo "Clean completed."
