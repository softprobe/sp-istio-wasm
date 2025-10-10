#!/bin/bash

# Demo application port forwarding script
# This script is used to start port forwarding for demo applications

set -e

echo "🔗 Starting port forwarding..."

# Check kubectl connection
if ! kubectl cluster-info &> /dev/null; then
    echo "❌ Cannot connect to Kubernetes cluster"
    exit 1
fi

# Check if applications are running
if ! kubectl get pod -l app=demo-ota | grep -q Running; then
    echo "❌ demo-ota application is not running, please run ./scripts/deploy-demo-apps.sh first"
    exit 1
fi

# Stop existing port forwarding
echo "🛑 Stopping existing port forwarding..."
pkill -f "kubectl port-forward.*demo-ota" 2>/dev/null || true
pkill -f "kubectl port-forward.*demo-airline" 2>/dev/null || true

sleep 2

# Start demo-ota port forwarding (8080)
echo "📱 Starting demo-ota port forwarding (8080 -> 8080)..."
kubectl port-forward -n istio-system svc/istio-ingressgateway 8080:80 &
OTA_PF_PID=$!
echo "✅ demo-ota port forwarding started (PID: $OTA_PF_PID)"

sleep 3

echo ""
echo "🎉 Port forwarding started!"
echo ""
echo "📱 Access applications:"
echo "   demo-ota: http://localhost:8080"
echo ""
echo "🛑 Stop port forwarding:"
echo "   Press Ctrl+C or run: pkill -f 'kubectl port-forward'"