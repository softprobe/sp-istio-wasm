# Softprobe Istio Agent - Deploying the WASM Plugin

This directory contains Kubernetes manifests to deploy the Softprobe Istio WASM agent that records outbound HTTP traffic and (optionally) injects cached responses.

## 🚀 Quick Start (Minimal one-file install)

Apply the minimal global install (includes HTTPS `ServiceEntry`):

```bash
kubectl apply -f https://raw.githubusercontent.com/softprobe/sp-istio/main/deploy/sp-istio-agent-minimal.yaml
```

Your mesh’s outbound HTTP traffic will be recorded and sent to Softprobe over HTTPS.

## 🚀 Quick Start (Global)

Deploy the agent globally across your mesh:

```bash
kubectl apply -f https://raw.githubusercontent.com/softprobe/sp-istio/main/deploy/sp-istio-agent.yaml
```

Your mesh’s outbound HTTP traffic will be recorded and sent to Softprobe over HTTPS.

## 🧪 Try It With Istio Bookinfo (Scoped Test)

Use the provided `deploy/test-bookinfo.yaml` to scope the plugin to the `productpage` service in the `default` namespace.

1) Install Istio and the Bookinfo demo (if not already):
```bash
# Install Istio (see Istio docs for your platform) and enable sidecar injection
kubectl label namespace default istio-injection=enabled --overwrite
kubectl apply -f https://raw.githubusercontent.com/istio/istio/release-1.22/samples/bookinfo/platform/kube/bookinfo.yaml
kubectl apply -f https://raw.githubusercontent.com/istio/istio/release-1.22/samples/bookinfo/networking/bookinfo-gateway.yaml
```

2) Deploy the plugin scoped to `productpage` and register the Softprobe backend:
```bash
kubectl apply -f deploy/test-bookinfo.yaml
```

3) Generate traffic:
```bash
export GATEWAY_URL=$(kubectl -n istio-system get svc istio-ingressgateway -o jsonpath='{.status.loadBalancer.ingress[0].ip}')
curl -sf "http://${GATEWAY_URL}/productpage" >/dev/null
```

4) Verify:
```bash
kubectl get wasmplugin -A
kubectl logs deploy/productpage-v1 -c istio-proxy | grep -E "SP|sp-istio" || true
```

Notes:
- Both the global and test manifests include a `ServiceEntry` for `o.softprobe.ai` on port 443 (HTTPS) so sidecars can reach Softprobe securely.
- The plugin uses the HTTPS cluster `outbound|443||o.softprobe.ai` and defaults `sp_backend_url` to `https://o.softprobe.ai`.
- Ensure egress to `o.softprobe.ai:443` is allowed in your environment.
- The test manifest pins a known-good image and sha. Use it as-is to validate, then switch to your own version when ready.

## 🎯 Deployment Modes

- **Global (Recommended)**: `deploy/sp-istio-agent.yaml` (namespace `istio-system`, no selector)
- **Namespace-specific**: set `metadata.namespace` in the manifest
- **Service-specific**: use `spec.selector.matchLabels` to target workloads

## ⚙️ Key Configuration

Within `spec.pluginConfig`:
- `sp_backend_url`: Softprobe backend URL (e.g., `https://o.softprobe.ai`)
- `traffic_direction`: usually `outbound`
- `service_name`, `api_key`: optional identification fields
- `collectionRules.http.client`: filter which outbound traffic to record

Example rule:
```yaml
collectionRules:
  http:
    client:
      - host: "api\\.example\\.com"
        paths: ["/v1/.*"]
```

## 🔍 Verification

```bash
kubectl get wasmplugin -A
kubectl logs -n istio-system deploy/istiod | grep sp-istio || true
```

If using a scoped deployment, also check the target workload’s proxy logs:
```bash
kubectl logs deploy/<workload> -c istio-proxy | grep -E "SP|sp-istio" || true
```

## 🧹 Uninstall

```bash
# Global install
kubectl delete -f deploy/sp-istio-agent.yaml || true

# Bookinfo test
kubectl delete -f deploy/test-bookinfo.yaml || true
```

## 🛠️ Troubleshooting

- Ensure Istio sidecars are injected in the target namespace/workloads
- Confirm the WASM image and `sha256` are accessible from your cluster
- If egress is restricted, add a `ServiceEntry` for `o.softprobe.ai`
- Check that `traffic_direction` and `collectionRules` match your traffic

## 📋 Requirements

- Istio 1.18+
- Kubernetes cluster with Istio installed
- Egress access to `o.softprobe.ai` and your container registry