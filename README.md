# SP-Istio Agent

**Business-level distributed tracing and analytics for Istio service mesh**

Zero-code changes required • Complete request visibility • Advanced troubleshooting

## What is SP-Istio Agent?

SP-Istio Agent is a WebAssembly (WASM) plugin for Istio that captures complete HTTP request/response data and sends it to Softprobe for business-level analytics and troubleshooting without modifying application code.

### Key Benefits

- **🔍 Complete Visibility**: Capture full HTTP request/response data across your service mesh
- **🚀 Faster Troubleshooting**: Business-level tracing reduces debugging time from hours to minutes  
- **📊 Data Analytics**: Rich insights into API usage patterns and business flows
- **⚡ Zero Intrusion**: No application code changes required
- **🔒 Enterprise Ready**: Production-grade security and performance

## Quick Start

### Prerequisites

- **Operating System**: macOS (or Linux with docker)
- **Required Tools**:
  - [Docker Desktop](https://www.docker.com/products/docker-desktop)
  - [Kind](https://kind.sigs.k8s.io/) - `brew install kind`
  - [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl-macos/) - `brew install kubectl`
  - [Istio CLI](https://istio.io/latest/docs/setup/getting-started/#download) - `brew install istioctl`

Or install these tools all at once. 

```bash
brew install kind kubectl istioctl
```

### 1. Set up a `Kind` cluster, `Istio` and `OpenTelemetry Operator`.

```bash
curl -L https://raw.githubusercontent.com/softprobe/sp-istio-wasm/refs/heads/main/scripts/cluster-setup.sh | sh
```

### 2. Install the travel demo

```bash
# Install Softprobe Istio WASM Plugin
kubectl apply -f https://raw.githubusercontent.com/softprobe/sp-istio-wasm/refs/heads/main/deploy/minimal.yaml
# Install demo app
kubectl apply -f https://raw.githubusercontent.com/softprobe/sp-istio-wasm/refs/heads/main/examples/travel/*.yaml
# Expose the demo
kubectl port-forward -n istio-system svc/istio-ingressgateway 8080:80
```

Play with the demo travel app by open [`http://localhost:8080/`](http://localhost:8080/) in browser, select a pari of cities and do a search, book and payment (fill any fake information). Then you can go to [Softprobe Dashboard](https://dashboard.softprobe.ai), check `Trave View` on the left navagation menu.

https://github.com/user-attachments/assets/dc8c68db-dd8b-4da8-a6e2-346adf6ecffb


### 3. Cleanup

```bash
kind delete cluster --name sp-demo-cluster
```

### Production Deployment

```bash
kubectl apply -f https://raw.githubusercontent.com/softprobe/sp-istio/main/deploy/minimal.yaml
```

### 1. Build the WASM Extension

```bash
make build
```

This will:
- Build the WASM binary for the `wasm32-unknown-unknown` target
- Calculate the SHA256 hash
- Show commands to update Istio configurations

### 2. Test with local envoy and docker (Recommended)

```bash
make integration-test
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
make build
```

## Architecture


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
- Protocol Buffers compiler (`protobuf-compiler`)
- Envoy (for local testing)
- kubectl and Istio (for deployment)

### Setup Development Environment

```bash
# Install Rust WASM target
rustup target add wasm32-unknown-unknown

# Install Protocol Buffers compiler
# On Debian/Ubuntu:
sudo apt-get install protobuf-compiler

# On macOS:
brew install protobuf

# On other systems, download from:
# https://github.com/protocolbuffers/protobuf/releases
```

## CI/CD Pipeline

This project includes automated GitHub Actions workflows:

### Integration Tests
- **Trigger**: Push to main/bill/deploy branches, Pull Requests
- **Workflow**: `.github/workflows/integration-test.yml`
- **Actions**: 
  - Builds WASM binary
  - Runs integration tests with Softprobe backend
  - Validates end-to-end telemetry pipeline

### Release Process
- **Trigger**: Git tags with format `v*.*.*` (e.g., `v1.2.3`)
- **Workflow**: `.github/workflows/release.yml`
- **Actions**:
  - Updates `Cargo.toml` version from tag
  - Builds and tests WASM binary
  - Publishes Docker images to `softprobe/sp-istio-wasm` and `softprobe/sp-envoy`
  - Creates GitHub release with WASM binary and deployment files

#### Required GitHub Secrets for Release
- `DOCKERHUB_USERNAME`: Docker Hub username
- `DOCKERHUB_TOKEN`: Docker Hub access token

#### Creating a Release
```bash
git tag v1.2.3
git push origin v1.2.3
```

The release workflow will automatically:
1. Extract version from tag
2. Update Cargo.toml version
3. Build and test
4. Publish Docker images 
5. Create GitHub release with assets
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


## Performance Considerations

- Body buffering impacts performance for large payloads
- Extension adds latency for agent lookups
- Async storage minimizes impact on response time
- Consider implementing size limits for buffered content
