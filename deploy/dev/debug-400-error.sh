#!/bin/bash

echo "🔍 Debugging 400 error for jaeger.softprobe.ai..."

# Function to get pod names
get_demo_pods() {
    kubectl get pods -l app=demo-airline -o name 2>/dev/null | head -1 | cut -d'/' -f2
    kubectl get pods -l app=demo-ota -o name 2>/dev/null | head -1 | cut -d'/' -f2
}

# Enable detailed logging for WASM and HTTP
echo "📝 Enabling detailed WASM and HTTP logging..."
DEMO_PODS=$(get_demo_pods)

if [ -z "$DEMO_PODS" ]; then
    echo "⚠️  No demo pods found. Please deploy demo applications first."
    echo "   Run: kubectl apply -f demo-airline-deployment.yaml"
    echo "   Run: kubectl apply -f demo-ota-deployment.yaml"
    exit 1
fi

for pod in $DEMO_PODS; do
    if [ ! -z "$pod" ]; then
        echo "🎯 Configuring logging for pod: $pod"
        
        # Set component log levels
        kubectl exec $pod -c istio-proxy -- curl -X POST "localhost:15000/logging?wasm=debug&http=debug&connection=debug&upstream=debug&router=debug&filter=debug" 2>/dev/null || echo "Failed to set log levels for $pod"
        
        # Enable access logs
        kubectl annotate pod $pod sidecar.istio.io/logLevel=debug --overwrite 2>/dev/null || echo "Failed to annotate $pod"
    fi
done

echo ""
echo "🚀 Starting log monitoring..."
echo "📋 This will show:"
echo "   - Full HTTP request details (method, path, headers)"
echo "   - Full HTTP response details (status, headers, body)"
echo "   - WASM plugin execution logs"
echo "   - Upstream connection details"
echo ""
echo "🔍 Looking for requests to jaeger.softprobe.ai..."
echo "Press Ctrl+C to stop monitoring"
echo ""

# Monitor logs from all demo pods
for pod in $DEMO_PODS; do
    if [ ! -z "$pod" ]; then
        echo "📊 Monitoring logs from pod: $pod"
        kubectl logs -f $pod -c istio-proxy 2>/dev/null | grep -E "(jaeger\.softprobe\.ai|POST.*v1/traces|400|Bad Request|wasm|sp-istio-agent)" &
    fi
done

# Also monitor istio-system logs
echo "📊 Monitoring Istio system logs..."
kubectl logs -f -n istio-system deployment/istiod 2>/dev/null | grep -E "(jaeger\.softprobe\.ai|400|error|wasm)" &

# Wait for user to stop
wait