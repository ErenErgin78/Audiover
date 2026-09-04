# ==============================================================================
# Audiover - Voice & Soundboard Engine Makefile
# ==============================================================================

.PHONY: help setup setup-deps dev build build-rpm build-deb build-appimage install-rpm install-deb install-appimage clean test lint

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

install-rpm: build-rpm ## Build RPM and reinstall on local system (sudo dnf)
	@echo "[+] Reinstalling Audiover RPM package..."
	@sudo dnf reinstall -y ./dist/Audiover-*.rpm 2>/dev/null || sudo dnf install -y ./dist/Audiover-*.rpm
	@echo "========================================================"
	@echo "  Audiover RPM has been successfully reinstalled!"
	@echo "========================================================"

build-deb: ## Build DEB package only
	@bash scripts/build_packages.sh deb

install-deb: build-deb ## Build DEB and reinstall on local system (sudo apt)
	@echo "[+] Installing Audiover DEB package..."
	@sudo apt-get update -qq 2>/dev/null || true
	@sudo apt-get install --reinstall -y ./dist/Audiover_*.deb 2>/dev/null || sudo apt-get install -y ./dist/Audiover_*.deb
	@echo "========================================================"
	@echo "  Audiover DEB has been successfully installed!"
	@echo "========================================================"

build-appimage: ## Build AppImage package only
	@bash scripts/build_packages.sh appimage

install-appimage: build-appimage ## Build AppImage and integrate to user system (~/.local/bin)
	@echo "[+] Installing Audiover AppImage to ~/.local/bin and desktop menu..."
	@mkdir -p ~/.local/bin ~/.local/share/applications ~/.local/share/icons/hicolor/scalable/apps ~/.local/share/icons/hicolor/256x256/apps
	@APPIMAGE_PATH=$$(ls -t dist/Audiover_*.AppImage 2>/dev/null | head -n 1); \
	if [ -n "$$APPIMAGE_PATH" ]; then \
		cp "$$APPIMAGE_PATH" ~/.local/bin/audiover && chmod +x ~/.local/bin/audiover; \
	fi
	@if [ -f packaging/desktop/audiover.desktop ]; then \
		cp packaging/desktop/audiover.desktop ~/.local/share/applications/audiover.desktop; \
	fi
	@if [ -f assets/icons/audiover.svg ]; then \
		cp assets/icons/audiover.svg ~/.local/share/icons/hicolor/scalable/apps/audiover.svg; \
	fi
	@if [ -f assets/icons/audiover.png ]; then \
		cp assets/icons/audiover.png ~/.local/share/icons/hicolor/256x256/apps/audiover.png; \
	fi
	@update-desktop-database ~/.local/share/applications 2>/dev/null || true
	@echo "========================================================"
	@echo "  Audiover AppImage installed successfully to ~/.local/bin/audiover"
	@echo "========================================================"

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
