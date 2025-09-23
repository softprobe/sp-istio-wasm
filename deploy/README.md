# Softprobe Istio Agent - Quick Deployment

This directory contains deployment configurations for the Softprobe Istio WASM agent that enables transparent HTTP traffic recording and caching.

## 🚀 One-Click Installation

Deploy the agent globally across your Istio mesh:

```bash
kubectl apply -f https://raw.githubusercontent.com/softprobe/sp-istio/main/deploy/sp-istio-agent.yaml
```

That's it! Your HTTP traffic will now be automatically recorded and sent to Softprobe's backend.

## 📊 What This Does

- **Captures all outbound HTTP traffic** from your services
- **Records request/response headers and bodies** 
- **Sends telemetry data** to `https://o.softprobe.ai` asynchronously
- **Stores data in S3/GCS** automatically via Softprobe backend
- **Zero application code changes** required

## 🎯 Deployment Options

### Global Deployment (Recommended)
The default configuration applies to all services in your mesh:
- Deployed in `istio-system` namespace
- No selector = applies globally
- Captures all outbound HTTP traffic

### Namespace-Specific Deployment
Uncomment the namespace-specific section in the YAML to apply only to certain namespaces:
```yaml
metadata:
  namespace: production  # Your target namespace
```

### Service-Specific Deployment  
Use selectors to target specific services:
```yaml
spec:
  selector:
    matchLabels:
      app: my-service
```

## ⚙️ Configuration Options

### Traffic Direction
```yaml
traffic_direction: "outbound"  # or "inbound"
```

### Enable Cache Injection
```yaml
enable_inject: true  # Enable transparent caching (use with caution)
```

### Collection Rules
Filter which traffic to capture:
```yaml
collectionRules:
  http:
    client:
      - host: "api\\.example\\.com"
        paths: ["/v1/.*"]
```

## 🔍 Verification

Check that the plugin is loaded:
```bash
kubectl get wasmplugin -A
kubectl logs -n istio-system deployment/istiod | grep sp-istio
```

View captured traffic in your Softprobe dashboard at `https://o.softprobe.ai`.

## 🛠️ Troubleshooting

### Plugin Not Loading
1. Verify Istio version compatibility (1.18+)
2. Check WASM binary accessibility
3. Review istiod logs for errors

### No Traffic Captured
1. Ensure services have Istio sidecars injected
2. Verify traffic direction configuration
3. Check collection rules match your traffic patterns

### Performance Impact
- The agent buffers request/response bodies for analysis
- Monitor memory usage if handling large payloads
- Use collection rules to filter unnecessary traffic

## 📋 Requirements

- Istio 1.18+
- Kubernetes cluster with Istio mesh
- Internet connectivity to `gcr.io` and `https://o.softprobe.ai`
- Kubernetes cluster access to Google Container Registry (for pulling WASM binary)