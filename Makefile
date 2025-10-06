# SP-Istio Agent Makefile
.PHONY: help build test clean push deploy status logs install-deps check-deps

# Configuration
BINARY_NAME := sp_istio_agent
WASM_TARGET := wasm32-unknown-unknown
BUILD_DIR := target/$(WASM_TARGET)/release
WASM_FILE := $(BUILD_DIR)/$(BINARY_NAME).wasm
HASH_FILE := $(WASM_FILE).sha256

# Docker configuration
REGISTRY := softprobe
WASM_IMAGE := $(REGISTRY)/sp-istio-wasm
ENVOY_IMAGE := $(REGISTRY)/sp-envoy
VERSION ?= latest

# Colors for output
GREEN := \033[32m
BLUE := \033[34m
YELLOW := \033[33m
RED := \033[31m
RESET := \033[0m

define print_success
	@echo "$(GREEN)✅ $(1)$(RESET)"
endef

define print_info
	@echo "$(BLUE)ℹ️  $(1)$(RESET)"
endef

define print_warning
	@echo "$(YELLOW)⚠️  $(1)$(RESET)"
endef

define print_error
	@echo "$(RED)❌ $(1)$(RESET)"
endef

# Default target
help: ## Show this help message
	@echo "SP-Istio Agent Build System"
	@echo "=========================="
	@echo ""
	@echo "Available targets:"
	@awk 'BEGIN {FS = ":.*##"} /^[a-zA-Z_-]+:.*##/ { printf "  %-15s %s\n", $$1, $$2 }' $(MAKEFILE_LIST)

check-deps: ## Check if required dependencies are installed
	$(call print_info,"Checking dependencies...")
	@command -v cargo >/dev/null || ($(call print_error,"Rust/Cargo not found. Install from https://rustup.rs/") && exit 1)
	@command -v kubectl >/dev/null || ($(call print_warning,"kubectl not found. Install for Kubernetes deployment"))
	@rustup target list --installed | grep -q $(WASM_TARGET) || ($(call print_info,"Installing WASM target...") && rustup target add $(WASM_TARGET))
	$(call print_success,"Dependencies check completed")

clean: ## Clean build artifacts
	$(call print_info,"Cleaning build artifacts...")
	@cargo clean
	@rm -f $(HASH_FILE)
	$(call print_success,"Clean completed")

build: check-deps ## Build WASM binary
	$(call print_info,"Building WASM binary...")
	@cargo build --target $(WASM_TARGET) --release
	@if [ -f "$(WASM_FILE)" ]; then \
		$(call print_success,"WASM binary built: $(WASM_FILE)"); \
		$(MAKE) hash; \
		$(MAKE) update-configs; \
	else \
		$(call print_error,"Build failed!"); \
		exit 1; \
	fi

hash: ## Calculate SHA256 hash of WASM binary
	$(call print_info,"Calculating SHA256 hash...")
	@if command -v sha256sum >/dev/null; then \
		sha256sum $(WASM_FILE) | cut -d' ' -f1 > $(HASH_FILE); \
	elif command -v shasum >/dev/null; then \
		shasum -a 256 $(WASM_FILE) | cut -d' ' -f1 > $(HASH_FILE); \
	else \
		$(call print_error,"Neither sha256sum nor shasum found"); \
		exit 1; \
	fi
	@HASH=$$(cat $(HASH_FILE)); \
	SIZE=$$(ls -lh $(WASM_FILE) | awk '{print $$5}'); \
	$(call print_success,"SHA256: $$HASH"); \
	$(call print_info,"File size: $$SIZE")

update-configs: ## Update deployment configs with new WASM hash
	$(call print_info,"Updating deployment configurations...")
	@if [ -f "$(HASH_FILE)" ]; then \
		HASH=$$(cat $(HASH_FILE)); \
		if [ -f "deploy/minimal.yaml" ]; then \
			sed -i.bak "s/sha256: .*/sha256: $$HASH/" deploy/minimal.yaml; \
			$(call print_success,"Updated deploy/minimal.yaml"); \
		fi; \
		if [ -f "deploy/production.yaml" ]; then \
			sed -i.bak "s/sha256: .*/sha256: $$HASH/" deploy/production.yaml; \
			$(call print_success,"Updated deploy/production.yaml"); \
		fi; \
	else \
		$(call print_error,"Hash file not found. Run 'make build' first"); \
		exit 1; \
	fi

test: build ## Run local tests with Envoy
	$(call print_info,"Running local tests...")
	@./scripts/test.sh

docker-build: build ## Build Docker images (requires VERSION)
	@if [ -z "$(VERSION)" ] || [ "$(VERSION)" = "latest" ]; then \
		$(call print_error,"VERSION required. Usage: make docker-build VERSION=v1.0.0"); \
		exit 1; \
	fi
	$(call print_info,"Building Docker images for version $(VERSION)...")
	@docker build -t $(WASM_IMAGE):$(VERSION) -f Dockerfile .
	@docker build -t $(ENVOY_IMAGE):$(VERSION) -f Dockerfile.envoy .
	$(call print_success,"Docker images built")

docker-push: docker-build ## Build and push Docker images (requires VERSION)
	$(call print_info,"Pushing images to registry...")
	@docker push $(WASM_IMAGE):$(VERSION)
	@docker push $(ENVOY_IMAGE):$(VERSION)
	$(call print_success,"Images pushed to registry")
	$(call print_info,"Cleaning local tags...")
	@docker rmi $(WASM_IMAGE):$(VERSION) $(ENVOY_IMAGE):$(VERSION) || true

push: docker-push ## Alias for docker-push

install-local: build ## Install plugin to local Kind cluster
	$(call print_info,"Installing to local cluster...")
	@kubectl apply -f deploy/minimal.yaml
	$(call print_success,"Plugin installed to cluster")

uninstall-local: ## Remove plugin from local Kind cluster
	$(call print_info,"Removing from local cluster...")
	@kubectl delete -f deploy/minimal.yaml --ignore-not-found=true
	$(call print_success,"Plugin removed from cluster")

cluster-setup: ## Set up development cluster
	$(call print_info,"Setting up development cluster...")
	@./scripts/cluster-setup.sh

deploy-demo: ## Deploy demo applications
	$(call print_info,"Deploying demo applications...")
	@./scripts/deploy-demo-apps.sh

install-plugin: ## Install WASM plugin to cluster
	$(call print_info,"Installing WASM plugin...")
	@./scripts/install-wasm-plugin.sh

start-forwarding: ## Start port forwarding
	$(call print_info,"Starting port forwarding...")
	@./scripts/start-port-forwarding.sh

cleanup: ## Clean up development environment
	$(call print_info,"Cleaning up environment...")
	@./scripts/cleanup.sh

dev-setup: cluster-setup deploy-demo install-plugin start-forwarding ## Complete development setup
	$(call print_success,"Development environment ready!")
	$(call print_info,"Demo apps: http://localhost:8080 and http://localhost:8081")
	$(call print_info,"Jaeger UI: https://jaeger.softprobe.ai")

status: ## Check deployment status
	$(call print_info,"Checking deployment status...")
	@kubectl get wasmplugin -n istio-system 2>/dev/null || $(call print_warning,"No WASM plugins found")
	@kubectl get pods -l app=demo-ota 2>/dev/null || $(call print_warning,"Demo apps not found")

logs: ## View plugin logs
	$(call print_info,"Viewing plugin logs...")
	@POD=$$(kubectl get pod -l app=demo-ota -o jsonpath='{.items[0].metadata.name}' 2>/dev/null); \
	if [ -n "$$POD" ]; then \
		kubectl logs $$POD -c istio-proxy | grep "SP" || $(call print_warning,"No SP logs found"); \
	else \
		$(call print_warning,"No demo pods found"); \
	fi

# Convenience aliases
all: build ## Build everything
rebuild: clean build ## Clean and build
dev: dev-setup ## Alias for dev-setup
install: install-local ## Alias for install-local
uninstall: uninstall-local ## Alias for uninstall-local