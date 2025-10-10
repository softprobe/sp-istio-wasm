#!/bin/bash

# SP Istio WASM - Environment Cleanup Script
# This script is used to clean up all deployed resources and clusters

set -e

echo "🧹 Starting SP Istio WASM environment cleanup..."

# Function: Safely execute commands, ignore errors
safe_execute() {
    local cmd="$1"
    local description="$2"
    echo "🔄 $description..."
    if eval "$cmd" 2>/dev/null; then
        echo "✅ $description completed"
    else
        echo "⚠️  $description skipped (resource may not exist)"
    fi
}

# 1. Stop WASM file server
echo "🛑 Stopping WASM file server..."
pkill -f "python3 -m http.server 8000" 2>/dev/null || echo "⚠️  No running WASM file server"

# 2. Stop port forwarding
echo "🛑 Stopping port forwarding..."
pkill -f "kubectl port-forward" 2>/dev/null || echo "⚠️  No running port forwarding"

# 3. Clean up WASM plugin
echo "🔧 Cleaning up WASM plugin..."
safe_execute "kubectl delete wasmplugin -n istio-system sp-istio-agent" "Delete WASM plugin"
safe_execute "kubectl delete serviceentry -n istio-system softprobe-backend" "Delete SoftProbe ServiceEntry"
safe_execute "kubectl delete destinationrule -n istio-system softprobe-backend-tls" "Delete SoftProbe DestinationRule"

# 4. Clean up demo applications
echo "📱 Cleaning up demo applications..."
safe_execute "kubectl delete -f deploy/demo-apps-deployment.yaml" "Delete demo applications"
safe_execute "kubectl delete -f deploy/demo-istio-gateway.yaml" "Delete Istio Gateway"

# 5. Clean up OpenTelemetry configuration
echo "📊 Cleaning up OpenTelemetry configuration..."
safe_execute "kubectl delete instrumentation default-instrumentation" "Delete OpenTelemetry Instrumentation"

# 6. Clean up OpenTelemetry Operator
echo "🔧 Cleaning up OpenTelemetry Operator..."
safe_execute "kubectl delete -f opentelemetry-operator.yaml" "Delete OpenTelemetry Operator"

# 7. Clean up cert-manager
echo "🔐 Cleaning up cert-manager..."
safe_execute "kubectl delete -f https://github.com/cert-manager/cert-manager/releases/download/v1.13.0/cert-manager.yaml" "Delete cert-manager"

# 8. Clean up Istio
echo "🌐 Cleaning up Istio..."
safe_execute "istioctl uninstall --purge -y" "Uninstall Istio"
safe_execute "kubectl delete namespace istio-system" "Delete istio-system namespace"

# 9. Clean up other possible resources
echo "🧽 Cleaning up other resources..."
safe_execute "kubectl delete namespace opentelemetry-operator-system" "Delete opentelemetry-operator-system namespace"
safe_execute "kubectl delete namespace cert-manager" "Delete cert-manager namespace"

# Wait for resource cleanup to complete
echo "⏳ Waiting for resource cleanup to complete..."
sleep 10

# 10. Delete Kind cluster
echo "🗑️  Deleting Kind cluster..."
if kind get clusters | grep -q "sp-demo-cluster"; then
    kind delete cluster --name sp-demo-cluster
    echo "✅ Kind cluster deleted"
else
    echo "⚠️  sp-demo-cluster cluster does not exist"
fi

echo ""
echo "🎉 Environment cleanup completed!"
echo ""
echo "📋 Cleanup Summary:"
echo "✅ WASM file server stopped"
echo "✅ Port forwarding stopped"
echo "✅ WASM plugin deleted"
echo "✅ Demo applications deleted"
echo "✅ OpenTelemetry configuration deleted"
echo "✅ OpenTelemetry Operator deleted"
echo "✅ cert-manager deleted"
echo "✅ Istio uninstalled"
echo "✅ Kind cluster deleted"
echo ""
echo "💡 Tips:"
echo "- To redeploy, run ./scripts/cluster-setup.sh"
echo "- All local data has been cleaned up, including trace data"