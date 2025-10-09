#!/bin/bash

# SP Istio WASM - Demo Application Deployment Script
# This script deploys demo-ota and demo-airline applications and configures OpenTelemetry auto-injection

set -e

echo "🚀 Deploying demo applications (demo-ota and demo-airline)..."

# Check cluster connection
echo "🔍 Checking cluster connection..."
if ! kubectl cluster-info &> /dev/null; then
    echo "❌ Unable to connect to Kubernetes cluster, please run ./scripts/cluster-setup.sh first"
    exit 1
fi
echo "✅ Cluster connection normal"

# Check if OpenTelemetry Operator is ready
echo "📊 Checking OpenTelemetry Operator status..."
if ! kubectl get deployment opentelemetry-operator-controller-manager -n opentelemetry-operator-system &> /dev/null; then
    echo "❌ OpenTelemetry Operator not installed, please run ./scripts/cluster-setup.sh first"
    exit 1
fi
echo "✅ OpenTelemetry Operator is ready"

# Deploy applications
echo "📦 Deploying demo applications (demo-ota and demo-airline with OpenTelemetry auto-injection)..."
kubectl apply -f ../deploy/demo-apps-deployment.yaml

echo "🌐 Deploying Istio Gateway and VirtualService..."
kubectl apply -f ../deploy/demo-istio-gateway.yaml

# Wait for deployment to be ready
echo "⏳ Waiting for application deployment to be ready..."
kubectl wait --for=condition=available --timeout=300s deployment/demo-ota
kubectl wait --for=condition=available --timeout=300s deployment/demo-airline

# Check Pod status
echo "📋 Checking Pod status..."
kubectl get pods -l service=demo-ota
kubectl get pods -l service=demo-airline

# Verify OpenTelemetry injection
echo ""
echo "🔍 Verifying OpenTelemetry auto-injection..."
echo "Checking demo-ota service (Java):"
kubectl get pod -l service=demo-ota -o jsonpath='{.items[0].metadata.annotations}' | grep -i otel || echo "⚠️  No OpenTelemetry annotations found"

echo "Checking demo-airline service (Java):"
kubectl get pod -l service=demo-airline -o jsonpath='{.items[0].metadata.annotations}' | grep -i otel || echo "⚠️  No OpenTelemetry annotations found"

echo ""
echo "✅ Demo applications deployed successfully!"
echo ""
echo "📝 Deployed services:"
echo "  - demo-ota (Java) - Port 8080"
echo "  - demo-airline (Java) - Port 8081"
echo ""
echo "🔧 Accessing applications:"
echo "  1. Via Istio Gateway (recommended):"
echo "     kubectl port-forward -n istio-system service/istio-ingressgateway 8080:80"
echo "     Then access:"
echo "       http://localhost:8080/ (demo-ota)"
echo "       http://localhost:8080/airline/ (demo-airline)"
echo "       curl -H \"Host: ota.local\" http://localhost:8080/ (demo-ota)"
echo "       curl -H \"Host: airline.local\" http://localhost:8080/ (demo-airline)"
echo "  2. Direct port forwarding:"
echo "     kubectl port-forward service/demo-ota 8080:8080"
echo "     kubectl port-forward service/demo-airline 8081:8081"
echo "     Then access:"
echo "       http://localhost:8080/ (demo-ota)"
echo "       http://localhost:8081/ (demo-airline)"
