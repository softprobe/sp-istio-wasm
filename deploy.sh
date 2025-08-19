#!/bin/bash

set -e

echo "SP-Istio Cache Deployment Script"
echo "================================"

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

# Check if cluster is accessible
if ! kubectl cluster-info &> /dev/null; then
    print_error "Cannot connect to Kubernetes cluster. Please check your kubeconfig."
    exit 1
fi

# Check if Istio is installed
if ! kubectl get namespace istio-system &> /dev/null; then
    print_error "Istio system namespace not found. Please install Istio first."
    exit 1
fi

# Function to update hash in configs
update_configs() {
    if [ ! -f "target/wasm32-unknown-unknown/release/sp_istio_cache.wasm.sha256" ]; then
        print_error "SHA256 hash file not found. Run ./build.sh first."
        exit 1
    fi
    
    HASH=$(cat target/wasm32-unknown-unknown/release/sp_istio_cache.wasm.sha256)
    print_status "Updating configs with SHA256: $HASH"
    
    # Update WasmPlugin
    sed -i.bak "s/sha256: .*/sha256: $HASH/" istio-configs/wasmplugin.yaml
    print_success "Updated wasmplugin.yaml"
    
    # Update EnvoyFilter
    sed -i.bak "s/sha256: .*/sha256: $HASH/" istio-configs/envoyfilter.yaml
    print_success "Updated envoyfilter.yaml"
}

# Function to deploy to cluster
deploy_to_cluster() {
    print_status "Deploying WASM extension to cluster..."
    
    # Apply WasmPlugin
    print_status "Applying WasmPlugin..."
    kubectl apply -f istio-configs/wasmplugin.yaml
    
    # Apply EnvoyFilter
    print_status "Applying EnvoyFilter..."
    kubectl apply -f istio-configs/envoyfilter.yaml
    
    print_success "Deployment completed!"
    
    # Check status
    print_status "Checking deployment status..."
    kubectl get wasmplugin -n default
    kubectl get envoyfilter -n default
}

# Function to check deployment status
check_status() {
    print_status "Checking extension status..."
    
    # Check if productpage pods are running
    if kubectl get pods -l app=productpage -n default &> /dev/null; then
        print_status "ProductPage pods:"
        kubectl get pods -l app=productpage -n default
        
        # Get pod logs to check for WASM loading
        POD=$(kubectl get pods -l app=productpage -n default -o jsonpath='{.items[0].metadata.name}' 2>/dev/null || echo "")
        if [ -n "$POD" ]; then
            print_status "Checking logs for WASM extension..."
            kubectl logs "$POD" -c istio-proxy | grep -E "(SP|WASM|wasm)" | tail -10 || print_warning "No relevant logs found"
        fi
    else
        print_warning "No productpage pods found. Make sure Istio bookinfo sample is deployed."
    fi
}

# Function to remove deployment
remove_deployment() {
    print_status "Removing WASM extension from cluster..."
    
    kubectl delete -f istio-configs/wasmplugin.yaml --ignore-not-found=true
    kubectl delete -f istio-configs/envoyfilter.yaml --ignore-not-found=true
    
    print_success "Extension removed!"
}

# Function to restart pods
restart_pods() {
    print_status "Restarting productpage pods to reload extension..."
    kubectl rollout restart deployment/productpage-v1 -n default || print_warning "productpage deployment not found"
    
    print_status "Waiting for rollout to complete..."
    kubectl rollout status deployment/productpage-v1 -n default || print_warning "Could not check rollout status"
}

# Main execution
case "${1:-deploy}" in
    "update-hash")
        update_configs
        ;;
    "deploy")
        update_configs
        deploy_to_cluster
        ;;
    "status")
        check_status
        ;;
    "restart")
        restart_pods
        ;;
    "remove")
        remove_deployment
        ;;
    "full")
        update_configs
        deploy_to_cluster
        restart_pods
        check_status
        ;;
    *)
        echo "Usage: $0 [update-hash|deploy|status|restart|remove|full]"
        echo "  update-hash - Update SHA256 hash in config files"
        echo "  deploy      - Deploy extension to cluster (default)"
        echo "  status      - Check deployment status"
        echo "  restart     - Restart pods to reload extension"
        echo "  remove      - Remove extension from cluster"
        echo "  full        - Complete deployment with restart and status check"
        exit 1
        ;;
esac