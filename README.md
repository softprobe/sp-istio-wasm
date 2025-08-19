# SP-Istio Cache

A transparent cache extension for Istio using WebAssembly (WASM) written in Rust.

## Overview

This project extends Istio's capabilities by implementing a custom WASM extension that intercepts outgoing HTTP requests, integrates with Softprobe for caching decisions, and manages cache storage asynchronously.

## Quick Start

### 1. Build the WASM Extension

```bash
./build.sh
```

This will:
- Build the WASM binary for the `wasm32-unknown-unknown` target
- Calculate the SHA256 hash
- Show commands to update Istio configurations

### 2. Test Locally (Recommended)

```bash
./test.sh
```

This will:
- Validate the WASM binary
- Start a local Envoy instance
- Test the extension functionality
- Show relevant logs

### 3. Deploy to Istio

```bash
./deploy.sh full
```

This will:
- Update configuration files with the latest SHA256 hash
- Deploy WasmPlugin and EnvoyFilter to the cluster
- Restart affected pods
- Check deployment status

## Manual Operations

### Build Only
```bash
cargo build --target wasm32-unknown-unknown --release
```

### Test Individual Components
```bash
./test.sh validate  # Only validate WASM binary
./test.sh envoy     # Test with local Envoy
```

### Deployment Operations
```bash
./deploy.sh deploy     # Deploy to cluster
./deploy.sh status     # Check status
./deploy.sh restart    # Restart pods
./deploy.sh remove     # Remove extension
```

## Architecture

### Components

- **src/lib.rs**: Main WASM extension logic with HTTP context handling
- **src/cache.rs**: Cache operations and Softprobe integration
- **src/otel.rs**: OpenTelemetry span creation and serialization
- **src/http_client.rs**: HTTP dispatch functionality for external calls

### Flow

1. **Request Interception**: Extension captures outgoing HTTP requests
2. **Cache Lookup**: Sends request data to Softprobe for cache check
3. **Cache Hit**: Returns cached response if available (HTTP 200)
4. **Cache Miss**: Continues to upstream service (HTTP 404)
5. **Response Storage**: Asynchronously stores successful responses for future caching

### Configuration

- **istio-configs/wasmplugin.yaml**: WasmPlugin resource for Istio
- **istio-configs/envoyfilter.yaml**: EnvoyFilter for advanced configuration
- **test/envoy.yaml**: Local Envoy configuration for testing

## Development

### Prerequisites

- Rust toolchain with `wasm32-unknown-unknown` target
- Envoy (for local testing)
- kubectl and Istio (for deployment)

### Adding the WASM Target

```bash
rustup target add wasm32-unknown-unknown
```

### Project Structure

```
sp-istio/
├── src/
│   ├── lib.rs           # Main WASM extension
│   ├── cache.rs         # Cache operations
│   ├── otel.rs          # OpenTelemetry integration
│   └── http_client.rs   # HTTP client
├── istio-configs/       # Istio resource configurations
├── test/               # Local testing configurations
├── opentelemetry/      # OpenTelemetry proto files
├── build.sh           # Build script
├── test.sh           # Test script
└── deploy.sh         # Deployment script
```

## Troubleshooting

### WASM Loading Issues

1. Check Envoy logs for WASM-related errors:
```bash
kubectl logs <pod-name> -c istio-proxy | grep -i wasm
```

2. Verify SHA256 hash matches between binary and configuration:
```bash
shasum -a 256 target/wasm32-unknown-unknown/release/sp_istio_cache.wasm
```

### Cache Not Working

1. Check extension logs for "SP" messages:
```bash
kubectl logs <pod-name> -c istio-proxy | grep "SP"
```

2. Verify Softprobe endpoint connectivity
3. Check request/response flow in logs

### Local Testing

Use the local Envoy setup for debugging:
```bash
./test.sh envoy
tail -f envoy.log | grep "SP"
```

## Performance Considerations

- Body buffering impacts performance for large payloads
- Extension adds latency for cache lookups
- Async storage minimizes impact on response time
- Consider implementing size limits for buffered content