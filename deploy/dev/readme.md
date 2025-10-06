# SP Istio WASM Plugin - Local Deployment Guide

[中文文档](readme-zh.md) | **English**

SP Istio WASM Plugin is a distributed tracing enhancement plugin based on Istio service mesh that adds richer service mesh-level monitoring information to existing OpenTelemetry tracing data without modifying application code.

## 🚀 Quick Start

### Prerequisites

- **Operating System**: macOS
- **Required Tools**:
  - [Docker Desktop](https://www.docker.com/products/docker-desktop)
  - [Kind](https://kind.sigs.k8s.io/) - `brew install kind`
  - [kubectl](https://kubernetes.io/docs/tasks/tools/install-kubectl-macos/) - `brew install kubectl`
  - [Istio CLI](https://istio.io/latest/docs/setup/getting-started/#download) - `curl -L https://istio.io/downloadIstio | sh -`

### One-Click Deployment Steps

#### Step 1: Create Base Environment
```bash
./cluster-setup.sh
```
**Purpose**: Creates Kind cluster, installs Istio service mesh, starts Jaeger tracing service, installs OpenTelemetry Operator

#### Step 2: Deploy Demo Applications
```bash
./deploy-apps.sh
```
**Purpose**: Deploys demo-ota and demo-airline Java applications with OpenTelemetry auto-instrumentation

#### Step 3: Install SP Istio WASM Plugin
```bash
./install-wasm.sh
```
**Purpose**: Installs SP Istio Agent WASM plugin to add enhanced monitoring capabilities to the service mesh

#### Step 4: Start Port Forwarding
```bash
./start-port-forward.sh
```
**Purpose**: Starts port forwarding to enable local access to cluster applications and Jaeger UI

## 🎯 View Results

### Access Applications
- **demo-ota Application**: http://localhost:8080/
- **demo-airline Application**: http://localhost:8081/
- **Jaeger Tracing UI**: https://jaeger.softprobe.ai/

### Test Distributed Tracing
```bash
# Send test request to demo-ota
curl -X POST http://localhost:8080/api/flights/search \
  -H "Content-Type: application/json" \
  -d '{
    "fromCity": "New York",
    "toCity": "Los Angeles",
    "departureDate": "2025-09-30",
    "tripType": "ONE_WAY",
    "cabinClass": "ECONOMY",
    "passengerInfo": {
        "adults": 1
    }
  }'

# Send test request to demo-airline
curl http://localhost:8081/api/flights
```

### View Tracing Data in Jaeger
1. Visit https://jaeger.softprobe.ai/
2. Select `demo-ota` or `demo-airline` from the Service dropdown
3. Click "Find Traces" to view tracing data

## 🧹 Cleanup Environment
```bash
./cleanup.sh
```
**Purpose**: Completely cleans up all resources including cluster, containers, and images

## 📊 How It Works

### SP Istio WASM Plugin Functions

The SP Istio WASM Plugin runs within Envoy proxies in the Istio service mesh, adding additional monitoring information to each HTTP request, including:

- **Service Name Detection**: Automatically detects service names from environment variables
- **Header Injection**: Adds service identification-related HTTP headers
- **Tracing Enhancement**: Adds service mesh-level information to existing OpenTelemetry traces

### Impact on Existing OpenTelemetry

**Important**: The SP Istio WASM Plugin **does not affect** existing OpenTelemetry configurations and data collection. It only adds additional information at the service mesh level, working in parallel with application-level OpenTelemetry tracing.

### Tracing Data Comparison

#### Trace Tree Before Plugin Installation
```
demo-ota
└── HTTP GET /api/flights/search
    ├── Business Logic Processing
    └── Database Query
```

#### Trace Tree After Plugin Installation
```
demo-ota
├── [Istio Ingress] HTTP Request (New)
│   ├── Service: demo-ota
│   ├── Headers: x-sp-service-name, x-service-name
│   └── Envoy Proxy Processing (New)
└── HTTP GET /api/flights/search (Existing)
    ├── Business Logic Processing (Existing)
    ├── [Istio Egress] Outbound Request (New)
    │   ├── Target Service: demo-airline
    │   └── Service Mesh Routing (New)
    └── Database Query (Existing)
```

#### New Tracing Information

1. **Envoy Proxy Layer Tracing**:
   - Inbound request processing (SIDECAR_INBOUND)
   - Outbound request processing (SIDECAR_OUTBOUND)
   - Service mesh routing information

2. **Service Identity Information**:
   - Auto-detected service names
   - Inter-service call relationships
   - Traffic paths within the mesh

3. **Enhanced Metadata**:
   - Pod information (hostname, namespace)
   - Service account information
   - Istio proxy version and configuration

### Data Flow Diagram

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Client        │    │   demo-ota       │    │   demo-airline  │
│                 │    │                  │    │                 │
│                 │───▶│ [Envoy Proxy]    │───▶│ [Envoy Proxy]   │
│                 │    │ ├─ WASM Plugin   │    │ ├─ WASM Plugin  │
│                 │    │ └─ OTel Agent    │    │ └─ OTel Agent   │
│                 │    │                  │    │                 │
│                 │    │ [Application]    │    │ [Application]   │
│                 │    │ └─ OTel SDK      │    │ └─ OTel SDK     │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                │                        │
                                ▼                        ▼
                       ┌─────────────────────────────────────┐
                       │         Jaeger Backend              │
                       │                                     │
                       │ ┌─────────────┐ ┌─────────────────┐ │
                       │ │ Istio Mesh  │ │ Application     │ │
                       │ │ Traces      │ │ Traces          │ │
                       │ │ (New)       │ │ (Existing)      │ │
                       │ └─────────────┘ └─────────────────┘ │
                       └─────────────────────────────────────┘
```

### Core Advantages

1. **Zero Intrusion**: No need to modify application code or existing OpenTelemetry configurations
2. **Enhanced Observability**: Provides additional monitoring dimensions at the service mesh level
3. **Complete Trace Chain**: Combines application-level and mesh-level tracing data for complete request lifecycle visibility
4. **Automatic Service Discovery**: Automatically identifies and tags services without manual configuration

## 📁 Project Structure

```
deploy/dev/
├── cluster-setup.sh              # Cluster and infrastructure setup
├── deploy-apps.sh               # Demo application deployment
├── install-wasm.sh              # WASM plugin installation
├── start-port-forward.sh        # Port forwarding startup
├── cleanup.sh                   # Environment cleanup
├── auto-instrumentation.yaml   # OpenTelemetry auto-instrumentation config
├── demo-apps-deployment.yaml   # Demo application deployment config
└── sp-istio-agent-minimal.yaml # SP Istio Agent WASM plugin config
```

## 🔍 Troubleshooting

### Check Plugin Status
```bash
# Check if WASM plugin is loaded correctly
kubectl get wasmplugin -n istio-system

# Check Envoy configuration
kubectl exec <pod-name> -c istio-proxy -- curl localhost:15000/config_dump
```

### View Logs
```bash
# View application logs
kubectl logs -l app=demo-ota
kubectl logs -l app=demo-airline

# View Istio proxy logs
kubectl logs <pod-name> -c istio-proxy
```

## 📚 Technical Details

- **WASM Plugin**: Based on WebAssembly technology, runs within Envoy proxies
- **Istio Integration**: Uses Istio's WasmPlugin CRD for configuration and deployment
- **OpenTelemetry Compatible**: Fully compatible with standard OpenTelemetry ecosystem
- **High Performance**: WASM runtime provides near-native performance
