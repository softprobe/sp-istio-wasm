#!/bin/bash

set -e

echo "SP-Istio Agent Health Check"
echo "=========================="

# Function to print colored output
print_status() {
    echo -e "\033[1;34m[INFO]\033[0m $1"
}

print_success() {
    echo -e "\033[1;32m[SUCCESS]\033[0m $1"
}

print_error() {
    echo -e "\033[1;31m[ERROR]\033[0m $1"
}

print_warning() {
    echo -e "\033[1;33m[WARNING]\033[0m $1"
}

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    print_error "kubectl not found. Please install kubectl."
    exit 1
fi

# Check cluster connectivity
print_status "Checking cluster connectivity..."
if ! kubectl cluster-info &> /dev/null; then
    print_error "Cannot connect to Kubernetes cluster"
    exit 1
fi
print_success "Cluster connectivity OK"

# Check Istio installation
print_status "Checking Istio installation..."
if ! kubectl get namespace istio-system &> /dev/null; then
    print_error "Istio system namespace not found"
    exit 1
fi

if ! kubectl get deployment istiod -n istio-system &> /dev/null; then
    print_error "Istio control plane not found"
    exit 1
fi
print_success "Istio installation OK"

# Check WASM plugin status
print_status "Checking SP-Istio Agent plugin..."
if kubectl get wasmplugin -n istio-system &> /dev/null; then
    PLUGIN_COUNT=$(kubectl get wasmplugin -n istio-system --no-headers | wc -l)
    print_success "Found $PLUGIN_COUNT WASM plugin(s)"
    
    # Check specific plugin
    if kubectl get wasmplugin sp-istio-agent -n istio-system &> /dev/null 2>&1; then
        print_success "SP-Istio Agent plugin found"
    else
        print_warning "SP-Istio Agent plugin not found"
    fi
else
    print_warning "No WASM plugins found"
fi

# Check demo applications
print_status "Checking demo applications..."
DEMO_APPS=("demo-ota" "demo-airline")
for app in "${DEMO_APPS[@]}"; do
    if kubectl get deployment $app &> /dev/null; then
        READY=$(kubectl get deployment $app -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
        DESIRED=$(kubectl get deployment $app -o jsonpath='{.spec.replicas}' 2>/dev/null || echo "0")
        if [ "$READY" = "$DESIRED" ] && [ "$READY" != "0" ]; then
            print_success "$app: $READY/$DESIRED pods ready"
        else
            print_warning "$app: $READY/$DESIRED pods ready"
        fi
    else
        print_warning "$app: deployment not found"
    fi
done

# Check service mesh injection
print_status "Checking sidecar injection..."
if kubectl get namespace default -o jsonpath='{.metadata.labels.istio-injection}' | grep -q enabled; then
    print_success "Istio injection enabled in default namespace"
else
    print_warning "Istio injection not enabled in default namespace"
fi

# Check plugin functionality
print_status "Testing plugin functionality..."
POD=$(kubectl get pod -l app=demo-ota -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
if [ -n "$POD" ]; then
    # Check if plugin is loaded in proxy
    if kubectl logs $POD -c istio-proxy --tail=100 | grep -q "sp-istio\|SP" &> /dev/null; then
        print_success "Plugin activity detected in proxy logs"
    else
        print_warning "No plugin activity found in proxy logs"
    fi
else
    print_warning "No demo pods found for testing"
fi

# Check external connectivity
print_status "Testing external connectivity..."
if kubectl get serviceentry -n istio-system | grep -q softprobe &> /dev/null; then
    print_success "Softprobe backend ServiceEntry found"
else
    print_warning "Softprobe backend ServiceEntry not found"
fi

# Performance metrics
print_status "Checking performance metrics..."
if [ -n "$POD" ]; then
    # Check memory usage
    MEMORY=$(kubectl top pod $POD --containers 2>/dev/null | grep istio-proxy | awk '{print $3}' || echo "N/A")
    CPU=$(kubectl top pod $POD --containers 2>/dev/null | grep istio-proxy | awk '{print $2}' || echo "N/A")
    print_status "Istio proxy resources: CPU=$CPU, Memory=$MEMORY"
fi

# Summary
echo ""
print_status "Health Check Summary:"
echo "========================"

# Overall status
ERRORS=0
WARNINGS=0

# Count issues (this is a simplified check)
if ! kubectl get wasmplugin sp-istio-agent -n istio-system &> /dev/null; then
    ((WARNINGS++))
fi

if ! kubectl get deployment demo-ota &> /dev/null; then
    ((WARNINGS++))
fi

if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    print_success "✅ All checks passed - SP-Istio Agent is healthy"
    exit 0
elif [ $ERRORS -eq 0 ]; then
    print_warning "⚠️  $WARNINGS warning(s) found - SP-Istio Agent is functional but has issues"
    exit 0
else
    print_error "❌ $ERRORS error(s) and $WARNINGS warning(s) found - SP-Istio Agent needs attention"
    exit 1
fi