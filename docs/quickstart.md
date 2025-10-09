# SP-Istio Agent - Quick Start Guide

Get SP-Istio Agent running with a complete demo environment in under 10 minutes.

## Prerequisites

- **Operating System**: macOS (or Linux with docker)
- **Required Tools**:
  - [Docker Desktop](https://www.docker.com/products/docker-desktop)
  - [Kind](https://kind.sigs.k8s.io/) - `brew install kind`
  - [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl-macos/) - `brew install kubectl`
  - [Istio CLI](https://istio.io/latest/docs/setup/getting-started/#download) - `curl -L https://istio.io/downloadIstio | sh -`

## 4-Step Quick Start

### Step 1: Create Base Environment
```bash
./scripts/cluster-setup.sh
```
**Creates**: Kind cluster, Istio service mesh, Softprobe tracing, OpenTelemetry Operator

### Step 2: Deploy Demo Applications  
```bash
./scripts/deploy-demo-apps.sh
```
**Deploys**: demo-ota and demo-airline Java applications with OpenTelemetry auto-instrumentation

### Step 3: Install SP Istio WASM Plugin
```bash
./scripts/install-wasm-plugin.sh
```
**Installs**: SP Istio Agent WASM plugin with enhanced monitoring capabilities

### Step 4: Start Port Forwarding
```bash
./scripts/start-port-forwarding.sh
```
**Enables**: Local access to cluster applications and Softprobe UI

## 🎯 Test the Setup

### Access Applications
- **demo-ota Application**: http://localhost:8080/
- **demo-airline Application**: http://localhost:8081/
- **Softprobe Tracing UI**: https://o.softprobe.ai/

### Send Test Requests
```bash
# Test demo-ota service
curl -X POST http://localhost:8080/api/flights/search \
  -H "Content-Type: application/json" \
  -d '{
    "fromCity": "New York",
    "toCity": "Los Angeles", 
    "departureDate": "2025-09-30",
    "tripType": "ONE_WAY",
    "cabinClass": "ECONOMY",
    "passengerInfo": {"adults": 1}
  }'

# Test demo-airline service
curl http://localhost:8081/api/flights
```

### View Enhanced Tracing
1. Visit https://o.softprobe.ai/
2. Select `demo-ota` or `demo-airline` from Service dropdown
3. Click "Find Traces" to see enhanced service mesh tracing

## 🔍 Verify Plugin is Working

Check that SP-Istio plugin is enhancing traces:

```bash
# Check WASM plugin status
kubectl get wasmplugin -n istio-system

# View plugin activity in proxy logs
kubectl logs -l app=demo-ota -c istio-proxy | grep "SP"
```

You should see additional tracing spans for:
- Envoy proxy processing (SIDECAR_INBOUND/OUTBOUND)
- Service mesh routing information  
- Auto-detected service identity headers

## 🧹 Cleanup

Remove the entire demo environment:
```bash
./scripts/cleanup.sh
```

## What You Just Experienced

The SP-Istio Agent transparently added service mesh-level observability to your applications without any code changes:

- **Zero Intrusion**: No application code modifications required
- **Enhanced Traces**: Service mesh routing and proxy processing visibility
- **Automatic Discovery**: Service identity detection and header injection
- **Complete Lifecycle**: Full request tracing across service boundaries

## Next Steps

- **Production Deployment**: See [deployment.md](deployment.md) for enterprise setup
- **Use Cases**: Learn about performance benefits in [use-cases.md](use-cases.md)  
- **Development**: Explore the codebase with [development.md](development.md)
- **Troubleshooting**: Common issues in [troubleshooting.md](troubleshooting.md)