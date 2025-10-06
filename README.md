# SP-Istio Agent

A transparent agent extension for Istio using WebAssembly (WASM) written in Rust.

## Overview

This project extends Istio's capabilities by implementing a custom WASM extension that intercepts outgoing HTTP requests, integrates with Softprobe for caching decisions, and manages agent storage asynchronously. It also includes OpenTelemetry integration for distributed tracing.

## Quick Start

### Minimal one-file install (cluster)

```bash
kubectl apply -f https://raw.githubusercontent.com/softprobe/sp-istio/main/deploy/sp-istio-agent-minimal.yaml
```

This installs the global WasmPlugin and the HTTPS ServiceEntry in one step.

### Local Development Environment

For a complete local development setup with Istio, Bookinfo demo, and OpenTelemetry tracing:

```bash
cd deploy/dev
./cluster-setup.sh
./deploy-apps.sh
./install-wasm.sh
./start-port-forward.sh
```

This will set up:
- Kind Kubernetes cluster
- Istio service mesh with automatic sidecar injection
- Jaeger distributed tracing (Docker)
- OpenTelemetry Operator with automatic instrumentation
- Bookinfo demo application
- SP Istio Agent WASM plugin

Access the applications:
- Jaeger UI: https://jaeger.softprobe.ai/

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

For production/global install, apply the WasmPlugin manifest under `deploy/`:

```bash
kubectl apply -f deploy/sp-istio-agent.yaml
```

To test safely with the Istio Bookinfo demo, use the scoped test manifest which targets only `productpage` and includes a `ServiceEntry` for `o.softprobe.ai`:

```bash
kubectl label namespace default istio-injection=enabled --overwrite
kubectl apply -f https://raw.githubusercontent.com/istio/istio/release-1.22/samples/bookinfo/platform/kube/bookinfo.yaml
kubectl apply -f https://raw.githubusercontent.com/istio/istio/release-1.22/samples/bookinfo/networking/bookinfo-gateway.yaml
kubectl apply -f deploy/test-bookinfo.yaml
```

Then generate traffic and verify:

```bash
export GATEWAY_URL=$(kubectl -n istio-system get svc istio-ingressgateway -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
curl -sf "http://${GATEWAY_URL}/productpage" >/dev/null
kubectl get wasmplugin -A
```

## Manual Operations

### Build Only
```bash
cargo build --target wasm32-unknown-unknown --release
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
- **src/otel.rs**: OpenTelemetry span creation and serialization

### Flow

1. **Request Interception**: Extension captures outgoing HTTP requests
2. **Agent Lookup**: Sends request data to Softprobe for agent check
3. **Agent Hit**: Returns agentd response if available (HTTP 200)
4. **Agent Miss**: Continues to upstream service (HTTP 404)
5. **Response Storage**: Asynchronously stores successful responses for future caching

### Configuration

- **deploy/sp-istio-agent.yaml**: Global WasmPlugin manifest
- **deploy/test-bookinfo.yaml**: Scoped test manifest for Bookinfo
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
│   ├── otel.rs          # OpenTelemetry integration
├── deploy/
│   ├── dev/             # Local development environment
│   │   ├── cluster-setup.sh      # Setup Kind cluster, Istio, Jaeger, OpenTelemetry
│   │   ├── deploy-apps.sh        # Deploy Bookinfo application
│   │   ├── install-wasm.sh       # Install SP Istio Agent WASM plugin
│   │   ├── start-port-forward.sh # Start port forwarding
│   │   ├── cleanup.sh            # Clean up all resources
│   │   ├── instrumentation.yaml  # OpenTelemetry auto-instrumentation config
│   │   └── readme.md             # Local development guide
│   ├── sp-istio-agent.yaml       # Global WasmPlugin manifest
│   └── test-bookinfo.yaml        # Scoped test manifest for Bookinfo
├── test/               # Local testing configurations
├── opentelemetry/      # OpenTelemetry proto files
├── build.sh            # Build script
├── test.sh             # Test script
└── build-and-push.sh   # Image build/publish script
```

## Troubleshooting

### WASM Loading Issues

1. Check Envoy logs for WASM-related errors:
```bash
kubectl logs <pod-name> -c istio-proxy | grep -i wasm
```

2. Verify SHA256 hash matches between binary and configuration:
```bash
shasum -a 256 target/wasm32-unknown-unknown/release/sp_istio_agent.wasm
```

### Agent Not Working

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
- Extension adds latency for agent lookups
- Async storage minimizes impact on response time
- Consider implementing size limits for buffered content