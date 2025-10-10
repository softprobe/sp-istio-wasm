#!/bin/bash

# SP Istio Agent WASM Plugin Installation Script
# This script is used to install and configure the SP Istio Agent WASM plugin

set -e

echo "🔧 Starting SP Istio Agent WASM plugin installation..."

# Check Kubernetes cluster connection
if ! kubectl cluster-info &> /dev/null; then
    echo "❌ Unable to connect to Kubernetes cluster, please ensure the cluster is running"
    exit 1
fi

# Check if Istio is installed
if ! kubectl get namespace istio-system &> /dev/null; then
    echo "❌ Istio not installed, please run ./scripts/cluster-setup.sh first"
    exit 1
fi

# Check if demo applications are deployed
if ! kubectl get deployment demo-ota &> /dev/null; then
    echo "❌ Demo applications not deployed, please run ./scripts/deploy-demo-apps.sh first"
    exit 1
fi

# Get user input for API Key
echo ""
echo "🔑 Configure API Key"
echo "Please enter your SoftProbe API Key (leave empty if you don't have one):"
read -p "API Key: " api_key

# Create temporary configuration file
temp_config=$(mktemp)
cp deploy/minimal.yaml "$temp_config"

# If user entered API Key, update configuration file
if [ -n "$api_key" ]; then
    echo "🔧 Setting API Key..."
    # Use sed to replace api_key value
    sed -i "" "s/api_key: \"\"/api_key: \"$api_key\"/" "$temp_config"
    echo "✅ API Key has been set"
else
    echo "⚠️  No API Key set, will use default empty value"
    echo ""
    echo "💡 To get an API Key, please visit:"
    echo "   🌐 https://softprobe.ai/"
    echo "   Register an account to get your exclusive API Key"
    echo ""
fi

# Install SP Istio Agent WASM plugin
echo "📦 Installing WASM plugin configuration..."
kubectl apply -f "$temp_config"
echo "✅ SP Istio Agent WASM plugin has been installed"

# Clean up temporary file
rm -f "$temp_config"

# Wait for plugin to take effect
echo "⏳ Waiting for WASM plugin to take effect..."
sleep 10

# Restart demo applications to apply WASM plugin
echo "🔄 Restarting demo applications to apply WASM plugin..."
kubectl rollout restart deployment/demo-ota
kubectl rollout restart deployment/demo-airline

# Wait for restart to complete
echo "⏳ Waiting for application restart to complete..."
kubectl rollout status deployment/demo-ota
kubectl rollout status deployment/demo-airline

echo ""
echo "🎉 WASM plugin installation completed!"
echo ""
echo "📋 Next steps:"
echo "1. Run ./scripts/start-port-forwarding.sh to start port forwarding"
echo "2. Access http://localhost:8080/ to test demo-ota application"
echo "3. Access http://localhost:8081/ to test demo-airline application"
echo "4. Access https://o.softprobe.ai to view Softprobe traces"
echo ""
echo "💡 Tips:"
echo "- WASM plugin will intercept all HTTP requests and send trace data to Softprobe"
echo "- You can see detailed request trace information in the Softprobe UI"