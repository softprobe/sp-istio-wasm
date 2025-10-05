#!/bin/bash

echo "🔧 Enabling detailed Istio/Envoy logging for debugging..."

# Apply access log configuration
echo "📝 Applying access log configuration..."
kubectl apply -f envoy-access-log-config.yaml

# Apply debug logging configuration
echo "🐛 Applying debug logging configuration..."
kubectl apply -f envoy-debug-detailed.yaml

# Enable access logs via istioctl
echo "🔍 Enabling access logs via istioctl..."
istioctl install --set meshConfig.accessLogFile=/dev/stdout --set values.pilot.env.PILOT_LOG_LEVEL=debug

# Restart istio-proxy sidecars to pick up new logging configuration
echo "🔄 Restarting Istio proxies to apply new logging..."
kubectl rollout restart deployment -n istio-system
kubectl rollout restart daemonset -n istio-system

# Enable debug logging for specific pods (if any demo apps are running)
echo "🎯 Enabling debug logging for demo applications..."
kubectl get pods -l app=demo-airline -o name | xargs -I {} kubectl annotate {} sidecar.istio.io/logLevel=debug --overwrite
kubectl get pods -l app=demo-ota -o name | xargs -I {} kubectl annotate {} sidecar.istio.io/logLevel=debug --overwrite

# Set component log levels for existing proxies
echo "⚙️  Setting component log levels..."
kubectl get pods -A -l security.istio.io/tlsMode=istio -o jsonpath='{range .items[*]}{.metadata.namespace}{" "}{.metadata.name}{"\n"}{end}' | while read namespace pod; do
    echo "Setting log level for $namespace/$pod"
    kubectl exec -n $namespace $pod -c istio-proxy -- curl -X POST "localhost:15000/logging?wasm=debug&http=debug&connection=debug&upstream=debug&router=debug&filter=debug" || true
done

echo "✅ Detailed logging enabled!"
echo ""
echo "📋 To view logs:"
echo "   kubectl logs -f -n istio-system deployment/istiod"
echo "   kubectl logs -f -n <namespace> <pod-name> -c istio-proxy"
echo ""
echo "🔍 To check access logs:"
echo "   kubectl logs -f -n <namespace> <pod-name> -c istio-proxy | grep -E '(POST|GET|PUT|DELETE)'"
echo ""
echo "🌐 To view specific WASM plugin logs:"
echo "   kubectl logs -f -n <namespace> <pod-name> -c istio-proxy | grep -i wasm"