# SP-Istio Agent - Production Deployment Guide

Enterprise-grade deployment for SP-Istio Agent in production environments.

## Prerequisites

### Infrastructure Requirements

- **Kubernetes**: v1.24+ with RBAC enabled
- **Istio**: v1.18+ with sidecar injection
- **Resources**: 50MB memory, 0.1 CPU per proxy
- **Network**: Egress access to `o.softprobe.ai:443`

### Access Requirements

- `cluster-admin` role for initial setup
- `kubectl` access to target namespace(s)
- Docker registry access (if using custom images)

## Deployment Options

### Option 1: Global Deployment (Recommended)

Deploy across entire service mesh:

```bash
# Apply global configuration
kubectl apply -f https://raw.githubusercontent.com/softprobe/sp-istio/main/deploy/production.yaml
```

**Scope**: All workloads in all namespaces with Istio injection

### Option 2: Namespace-Specific Deployment

Deploy to specific namespaces:

```bash
# Download configuration
curl -O https://raw.githubusercontent.com/softprobe/sp-istio/main/deploy/production.yaml

# Edit namespace scope
vim deploy/production.yaml
# Modify metadata.namespace to target namespace

# Apply scoped configuration
kubectl apply -f deploy/production.yaml
```

### Option 3: Workload-Specific Deployment

Deploy to specific applications:

```yaml
# Example: Target only frontend applications
spec:
  selector:
    matchLabels:
      app: frontend
      tier: web
```

## Configuration Options

### Basic Configuration

```yaml
apiVersion: extensions.istio.io/v1alpha1
kind: WasmPlugin
metadata:
  name: sp-istio-agent
  namespace: istio-system
spec:
  url: oci://softprobe/sp-istio-wasm:latest
  sha256: [hash-will-be-updated]
  pluginConfig:
    sp_backend_url: "https://o.softprobe.ai"
    public_key: "your-api-key-here"
    traffic_direction: "outbound"
```

### Advanced Configuration

```yaml
pluginConfig:
  # Backend Configuration
  sp_backend_url: "https://o.softprobe.ai"
  public_key: "your-production-api-key"
  
  # Cache Configuration
  cache_ttl_seconds: 3600
  max_cache_size_mb: 100
  
  # Traffic Filtering
  traffic_direction: "outbound"
  collectionRules:
    http:
      client:
        # Only cache API calls
        - host: "api\\..+"
          paths: ["/v1/.*", "/v2/.*"]
          methods: ["GET", "POST"]
        # Exclude health checks
        - host: ".*"
          paths: ["/health", "/metrics"]
          exclude: true
  
  # Performance Tuning
  async_timeout_ms: 5000
  max_concurrent_requests: 100
  
  # Observability
  service_name: "production-cluster"
  enable_detailed_logging: false
```

## Environment-Specific Configurations

### Development Environment

```yaml
# config/development.yaml
pluginConfig:
  sp_backend_url: "https://dev.softprobe.ai"
  enable_detailed_logging: true
  cache_ttl_seconds: 300  # 5 minutes
```

### Staging Environment

```yaml
# config/staging.yaml
pluginConfig:
  sp_backend_url: "https://staging.softprobe.ai"
  enable_detailed_logging: false
  cache_ttl_seconds: 1800  # 30 minutes
```

### Production Environment

```yaml
# config/production.yaml
pluginConfig:
  sp_backend_url: "https://o.softprobe.ai"
  enable_detailed_logging: false
  cache_ttl_seconds: 3600  # 1 hour
  max_cache_size_mb: 200
```

## Security Configuration

### Network Policies

```yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: sp-istio-agent-egress
spec:
  podSelector:
    matchLabels:
      security.istio.io/tlsMode: istio
  policyTypes:
  - Egress
  egress:
  - to:
    - namespaceSelector: {}
    ports:
    - protocol: TCP
      port: 443
  - to: []
    ports:
    - protocol: TCP
      port: 443
    - protocol: UDP
      port: 53
```

### Public Key Management

```bash
# Create secret for Public key
kubectl create secret generic sp-istio-config \
  --from-literal=public-key=your-production-public-key \
  -n istio-system

# Reference in WasmPlugin
spec:
  pluginConfig:
    public_key: "{{ .Values.publicKey }}"
```

### Certificate Management

```yaml
# ServiceEntry for backend TLS
apiVersion: networking.istio.io/v1beta1
kind: ServiceEntry
metadata:
  name: softprobe-backend
  namespace: istio-system
spec:
  hosts:
  - o.softprobe.ai
  ports:
  - number: 443
    name: https
    protocol: HTTPS
  location: MESH_EXTERNAL
  resolution: DNS
```

## Monitoring and Observability

### Health Checks

```bash
# Check plugin status
kubectl get wasmplugin -n istio-system

# Verify plugin loading
kubectl logs -n istio-system deployment/istiod | grep sp-istio

# Check individual workloads
kubectl logs deployment/your-app -c istio-proxy | grep SP
```

### Metrics Integration

```yaml
# Prometheus ServiceMonitor
apiVersion: monitoring.coreos.com/v1
kind: ServiceMonitor
metadata:
  name: sp-istio-metrics
spec:
  selector:
    matchLabels:
      app: istio-proxy
  endpoints:
  - port: http-monitoring
    path: /stats/prometheus
```

### Dashboard Configuration

```yaml
# Grafana Dashboard ConfigMap
apiVersion: v1
kind: ConfigMap
metadata:
  name: sp-istio-dashboard
data:
  dashboard.json: |
    {
      "dashboard": {
        "title": "SP-Istio Agent Performance",
        "panels": [
          {
            "title": "Cache Hit Rate",
            "type": "stat",
            "targets": [
              {
                "expr": "rate(sp_cache_hits_total[5m]) / rate(sp_requests_total[5m])"
              }
            ]
          }
        ]
      }
    }
```

## Performance Tuning

### Resource Limits

```yaml
# Set appropriate resource limits
spec:
  workloadSelector:
    labels:
      app: your-app
  template:
    spec:
      containers:
      - name: istio-proxy
        resources:
          limits:
            cpu: 200m
            memory: 128Mi
          requests:
            cpu: 100m
            memory: 64Mi
```

### Cache Configuration

```yaml
pluginConfig:
  # Tune based on your traffic patterns
  cache_ttl_seconds: 3600        # 1 hour
  max_cache_size_mb: 200         # 200MB per proxy
  max_concurrent_requests: 100   # Concurrent backend requests
  async_timeout_ms: 5000         # 5 second timeout
```

## High Availability

### Multi-Region Deployment

```yaml
# Primary region configuration
apiVersion: extensions.istio.io/v1alpha1
kind: WasmPlugin
metadata:
  name: sp-istio-agent-primary
  namespace: istio-system
spec:
  pluginConfig:
    sp_backend_url: "https://primary.softprobe.ai"
    region: "us-east-1"
```

### Failover Configuration

```yaml
pluginConfig:
  # Fallback configuration
  fallback_enabled: true
  fallback_timeout_ms: 2000
  circuit_breaker:
    max_failures: 5
    timeout_seconds: 30
```

## Rolling Updates

### Update Strategy

```bash
# 1. Update WASM binary hash
HASH=$(shasum -a 256 new-binary.wasm | cut -d' ' -f1)
kubectl patch wasmplugin sp-istio-agent -n istio-system \
  --type='json' -p="[{\"op\": \"replace\", \"path\": \"/spec/sha256\", \"value\": \"$HASH\"}]"

# 2. Rolling restart (if needed)
kubectl rollout restart deployment/your-app

# 3. Verify deployment
kubectl rollout status deployment/your-app
```

### Canary Deployment

```yaml
# Deploy to subset of traffic first
spec:
  selector:
    matchLabels:
      version: canary
  pluginConfig:
    # New configuration for canary
```

## Troubleshooting Production Issues

### Common Issues

1. **Plugin Not Loading**
```bash
# Check Envoy config
kubectl exec deployment/your-app -c istio-proxy -- \
  curl localhost:15000/config_dump | grep sp-istio
```

2. **High Memory Usage**
```bash
# Check memory metrics
kubectl top pod your-app
kubectl logs your-app -c istio-proxy | grep "memory"
```

3. **Backend Connectivity**
```bash
# Test backend connectivity
kubectl exec deployment/your-app -c istio-proxy -- \
  curl -v https://o.softprobe.ai/health
```

### Performance Debugging

```bash
# Enable debug logging temporarily via Istio componentLogLevel
# Example (workload annotation):
# sidecar.istio.io/componentLogLevel: "wasm:debug"

# View detailed logs
kubectl logs deployment/your-app -c istio-proxy | grep SP
```

## Backup and Recovery

### Configuration Backup

```bash
# Backup current configuration
kubectl get wasmplugin sp-istio-agent -n istio-system -o yaml > sp-istio-backup.yaml

# Backup related resources
kubectl get serviceentry,destinationrule -n istio-system -l app=sp-istio -o yaml >> sp-istio-backup.yaml
```

### Disaster Recovery

```bash
# Quick removal if needed
kubectl delete wasmplugin sp-istio-agent -n istio-system

# Restore from backup
kubectl apply -f sp-istio-backup.yaml
```

## Support and Maintenance

### Regular Maintenance Tasks

1. **Weekly**: Review cache hit rates and performance metrics
2. **Monthly**: Update WASM binary to latest version
3. **Quarterly**: Review and optimize cache configuration

### Getting Support

- **Documentation**: [troubleshooting.md](troubleshooting.md)
- **Community**: GitHub Issues
- **Enterprise**: Contact support@softprobe.ai

### Version Management

```bash
# Check current version
kubectl get wasmplugin sp-istio-agent -n istio-system -o jsonpath='{.spec.url}'

# Update to specific version
kubectl patch wasmplugin sp-istio-agent -n istio-system \
  --type='json' -p='[{"op": "replace", "path": "/spec/url", "value": "oci://softprobe/sp-istio-wasm:v1.2.0"}]'
```