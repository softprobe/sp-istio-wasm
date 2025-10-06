# SP-Istio Agent - Development Guide

Complete guide for developing, building, testing, and contributing to SP-Istio Agent.

## Development Environment Setup

### Prerequisites

- **Rust**: Latest stable version (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Docker**: For local testing and building
- **Kind**: For local Kubernetes clusters (`brew install kind`)
- **kubectl**: Kubernetes CLI (`brew install kubectl`)
- **Istio CLI**: Service mesh management (`curl -L https://istio.io/downloadIstio | sh -`)

### Project Setup

```bash
# Clone the repository
git clone https://github.com/softprobe/sp-istio.git
cd sp-istio

# Install WASM target
rustup target add wasm32-unknown-unknown

# Verify setup
make check-deps
```

## Development Workflow

### Quick Development Loop

```bash
# Build and test locally
make build
make test

# Full development environment
make dev-setup
```

### Detailed Development Steps

1. **Build WASM Binary**
```bash
make build
# Builds target/wasm32-unknown-unknown/release/sp_istio_agent.wasm
# Calculates SHA256 hash
# Updates deployment configurations
```

2. **Local Testing**
```bash
make test
# Starts local Envoy with WASM plugin
# Runs integration tests
# Shows cache hit/miss behavior
```

3. **Deploy to Development Cluster**
```bash
make cluster-setup     # Create Kind cluster with Istio
make deploy-demo       # Deploy demo applications
make install-plugin    # Install WASM plugin
make start-forwarding  # Enable local access
```

4. **Verify Changes**
```bash
make status  # Check deployment status
make logs    # View plugin logs
```

## Code Structure

### Project Layout

```
src/
├── lib.rs              # Main WASM extension entry point
├── http_context.rs     # HTTP request/response handling
├── cache.rs            # Cache management logic
├── backend.rs          # Softprobe backend integration
├── config.rs           # Configuration parsing
├── metrics.rs          # Performance metrics
├── logging.rs          # Debug logging utilities
└── tests/
    ├── unit/           # Unit tests
    ├── integration/    # Integration tests
    └── fixtures/       # Test data
```

### Key Components

#### 1. HTTP Context Handler (`src/http_context.rs`)

```rust
impl HttpContext for SpIstioContext {
    fn on_http_request_headers(&mut self) -> Action {
        // 1. Extract request information
        // 2. Check cache for existing response
        // 3. Forward to backend if cache miss
        // 4. Return cached response if cache hit
    }
    
    fn on_http_response_headers(&mut self) -> Action {
        // 1. Store successful responses for future caching
        // 2. Update metrics and logs
    }
}
```

#### 2. Cache Management (`src/cache.rs`)

```rust
pub struct CacheManager {
    // LRU cache implementation
    // TTL management
    // Size limits
}

impl CacheManager {
    pub fn get(&self, key: &str) -> Option<CachedResponse>
    pub fn put(&mut self, key: String, response: CachedResponse)
    pub fn evict_expired(&mut self)
}
```

#### 3. Backend Integration (`src/backend.rs`)

```rust
pub struct BackendClient {
    url: String,
    api_key: Option<String>,
    timeout: Duration,
}

impl BackendClient {
    pub async fn check_cache(&self, request: &HttpRequest) -> BackendResponse
    pub async fn store_response(&self, request: &HttpRequest, response: &HttpResponse)
}
```

## Testing Strategy

### Unit Tests

```bash
# Run unit tests
cargo test --lib

# Run specific test
cargo test cache_manager::tests::test_lru_eviction

# Test with coverage
cargo tarpaulin --out Html
```

### Integration Tests

```bash
# Local Envoy integration tests
make test

# Full cluster integration tests
make test-integration

# Performance tests
make test-performance
```

### Test Configuration

```yaml
# test/envoy.yaml - Local Envoy configuration
static_resources:
  listeners:
  - name: main
    address:
      socket_address:
        address: 0.0.0.0
        port_value: 18000
    filter_chains:
    - filters:
      - name: envoy.filters.network.http_connection_manager
        typed_config:
          "@type": type.googleapis.com/envoy.extensions.filters.network.http_connection_manager.v3.HttpConnectionManager
          http_filters:
          - name: envoy.filters.http.wasm
            typed_config:
              "@type": type.googleapis.com/envoy.extensions.filters.http.wasm.v3.Wasm
              config:
                name: sp_istio_agent
                root_id: sp_istio_agent
                vm_config:
                  vm_id: sp_istio_agent
                  runtime: envoy.wasm.runtime.v8
                  code:
                    local:
                      filename: "/etc/envoy/sp_istio_agent.wasm"
```

## Debugging

### Local Debugging

```bash
# Enable debug logging
export RUST_LOG=debug

# Build debug version
cargo build --target wasm32-unknown-unknown

# Run with verbose logging
./scripts/test.sh --verbose
```

### Cluster Debugging

```bash
# View plugin logs
make logs

# Check plugin configuration
kubectl get wasmplugin sp-istio-agent -n istio-system -o yaml

# Debug specific pod
POD=$(kubectl get pod -l app=demo-ota -o jsonpath='{.items[0].metadata.name}')
kubectl logs $POD -c istio-proxy -f | grep SP
```

### Performance Profiling

```bash
# Profile memory usage
cargo build --target wasm32-unknown-unknown --release
wasm-opt --enable-gc target/wasm32-unknown-unknown/release/sp_istio_agent.wasm -o optimized.wasm

# Analyze WASM size
wasm-objdump -h target/wasm32-unknown-unknown/release/sp_istio_agent.wasm
```

## Configuration Development

### Adding New Configuration Options

1. **Update Configuration Struct** (`src/config.rs`):
```rust
#[derive(Deserialize, Debug)]
pub struct PluginConfig {
    pub sp_backend_url: String,
    pub api_key: Option<String>,
    pub enable_inject: bool,
    // Add new option here
    pub new_feature_enabled: Option<bool>,
}
```

2. **Update Default Values**:
```rust
impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            // ... existing defaults
            new_feature_enabled: Some(false),
        }
    }
}
```

3. **Add Validation**:
```rust
impl PluginConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Add validation logic
    }
}
```

4. **Update Documentation**:
```yaml
# deploy/production.yaml
pluginConfig:
  # New feature configuration
  new_feature_enabled: true  # Enable new feature (default: false)
```

## Building and Releasing

### Local Builds

```bash
# Development build
make build

# Optimized release build
cargo build --target wasm32-unknown-unknown --release
wasm-opt --strip-debug --optimize-level=3 \
  target/wasm32-unknown-unknown/release/sp_istio_agent.wasm \
  -o target/wasm32-unknown-unknown/release/sp_istio_agent_optimized.wasm
```

### Docker Images

```bash
# Build Docker images
make docker-build VERSION=v1.2.0

# Push to registry
make docker-push VERSION=v1.2.0
```

### Release Process

1. **Version Bump**:
```bash
# Update Cargo.toml
version = "1.2.0"

# Update deployment configs
# scripts/version.sh handles this automatically
./scripts/version.sh bump 1.2.0
```

2. **Build and Test**:
```bash
make clean
make build
make test
make test-integration
```

3. **Create Release**:
```bash
# Tag release
git tag v1.2.0
git push origin v1.2.0

# Build and push images
make docker-push VERSION=v1.2.0

# Update deployment manifests
./scripts/version.sh update-manifests v1.2.0
```

## Contributing Guidelines

### Code Style

```bash
# Format code
cargo fmt

# Lint code
cargo clippy -- -D warnings

# Check documentation
cargo doc --no-deps --open
```

### Commit Messages

Follow conventional commits:
```
feat: add response compression support
fix: resolve memory leak in cache eviction
docs: update deployment guide with security section
test: add integration tests for cache TTL
```

### Pull Request Process

1. **Create Feature Branch**:
```bash
git checkout -b feature/add-compression-support
```

2. **Develop and Test**:
```bash
make build
make test
make test-integration
```

3. **Update Documentation**:
- Update relevant docs in `docs/`
- Add configuration examples
- Update CHANGELOG.md

4. **Submit PR**:
- Clear description of changes
- Link to relevant issues
- Include test results

### Testing Requirements

All PRs must include:
- Unit tests for new functionality
- Integration tests for API changes
- Performance tests for optimizations
- Documentation updates

## Advanced Development

### Custom Backend Integration

```rust
// src/backend/custom.rs
pub struct CustomBackend {
    endpoint: String,
    auth_token: String,
}

impl BackendTrait for CustomBackend {
    async fn check_cache(&self, request: &HttpRequest) -> BackendResponse {
        // Custom implementation
    }
}
```

### Plugin Extensions

```rust
// src/extensions/
pub trait PluginExtension {
    fn on_request(&self, request: &HttpRequest) -> Result<Action>;
    fn on_response(&self, response: &HttpResponse) -> Result<Action>;
}
```

### Performance Optimizations

1. **Memory Management**:
```rust
// Use object pools for frequent allocations
pub struct ObjectPool<T> {
    objects: Vec<T>,
    factory: Box<dyn Fn() -> T>,
}
```

2. **Async Processing**:
```rust
// Non-blocking cache operations
async fn store_response_async(&self, request: HttpRequest, response: HttpResponse) {
    tokio::spawn(async move {
        // Background storage
    });
}
```

## Troubleshooting Development Issues

### Common Build Issues

1. **WASM Target Missing**:
```bash
rustup target add wasm32-unknown-unknown
```

2. **Memory Limits**:
```bash
# Increase WASM memory limit
wasm-ld --max-memory=67108864  # 64MB
```

3. **Dependency Conflicts**:
```bash
cargo tree
cargo update
```

### Runtime Issues

1. **Plugin Not Loading**:
```bash
# Check Envoy logs
docker logs envoy 2>&1 | grep -i wasm
```

2. **Memory Leaks**:
```bash
# Enable memory tracking
RUST_LOG=sp_istio_agent::memory=debug
```

3. **Performance Issues**:
```bash
# Profile execution
perf record ./envoy
perf report
```

## Development Resources

### Documentation
- [Proxy-WASM Rust SDK](https://github.com/proxy-wasm/proxy-wasm-rust-sdk)
- [Envoy WASM Filter](https://www.envoyproxy.io/docs/envoy/latest/configuration/http/http_filters/wasm_filter)
- [Istio WASM Plugin](https://istio.io/latest/docs/reference/config/proxy_extensions/wasm-plugin/)

### Tools
- [wasm-pack](https://rustwasm.github.io/wasm-pack/): WASM building tool
- [wasm-opt](https://github.com/WebAssembly/binaryen): WASM optimizer
- [wasmtime](https://wasmtime.dev/): WASM runtime for testing

### Community
- GitHub Discussions
- Slack: #sp-istio-dev
- Weekly dev meetings: Wednesdays 10 AM PST