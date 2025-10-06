#!/bin/bash

set -e

echo "SP-Istio Agent Log Viewer"
echo "========================"

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

# Show help
show_help() {
    cat << EOF
Usage: $0 [OPTIONS] [POD_NAME]

View SP-Istio Agent plugin logs from Istio proxy sidecars.

OPTIONS:
    -h, --help          Show this help message
    -f, --follow        Follow log output (like tail -f)
    -n, --lines N       Number of lines to show (default: 100)
    -a, --all           Show logs from all pods with SP-Istio plugin
    -c, --container     Show logs from main container instead of istio-proxy
    --demo              Show logs from demo applications only
    --filter PATTERN    Filter logs by pattern (grep)

EXAMPLES:
    $0                          # Show recent SP plugin logs from demo apps
    $0 -f                       # Follow SP plugin logs
    $0 -a                       # Show logs from all pods
    $0 --filter "cache"         # Show only cache-related logs
    $0 my-pod-name             # Show logs from specific pod

EOF
}

# Default values
FOLLOW=false
LINES=100
ALL_PODS=false
CONTAINER="istio-proxy"
DEMO_ONLY=false
FILTER_PATTERN=""
TARGET_POD=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--help)
            show_help
            exit 0
            ;;
        -f|--follow)
            FOLLOW=true
            shift
            ;;
        -n|--lines)
            LINES="$2"
            shift 2
            ;;
        -a|--all)
            ALL_PODS=true
            shift
            ;;
        -c|--container)
            CONTAINER="app"
            shift
            ;;
        --demo)
            DEMO_ONLY=true
            shift
            ;;
        --filter)
            FILTER_PATTERN="$2"
            shift 2
            ;;
        *)
            if [[ -z "$TARGET_POD" ]]; then
                TARGET_POD="$1"
            else
                print_error "Unknown argument: $1"
                show_help
                exit 1
            fi
            shift
            ;;
    esac
done

# Check if kubectl is available
if ! command -v kubectl &> /dev/null; then
    print_error "kubectl not found. Please install kubectl."
    exit 1
fi

# Check cluster connectivity
if ! kubectl cluster-info &> /dev/null; then
    print_error "Cannot connect to Kubernetes cluster"
    exit 1
fi

# Build kubectl logs command
build_logs_cmd() {
    local pod="$1"
    local cmd="kubectl logs $pod -c $CONTAINER"
    
    if [ "$FOLLOW" = true ]; then
        cmd="$cmd -f"
    else
        cmd="$cmd --tail=$LINES"
    fi
    
    echo "$cmd"
}

# Filter logs for SP-Istio content
filter_logs() {
    local grep_pattern="SP|sp-istio|cache|inject"
    
    if [ -n "$FILTER_PATTERN" ]; then
        grep_pattern="$FILTER_PATTERN"
    fi
    
    if [ "$CONTAINER" = "istio-proxy" ]; then
        grep -E "$grep_pattern" --color=always
    else
        cat  # Don't filter main container logs
    fi
}

# Get pods to monitor
get_target_pods() {
    if [ -n "$TARGET_POD" ]; then
        echo "$TARGET_POD"
        return
    fi
    
    local selector=""
    if [ "$DEMO_ONLY" = true ]; then
        selector="-l app in (demo-ota,demo-airline)"
    elif [ "$ALL_PODS" = true ]; then
        # Get all pods with istio-proxy sidecar
        kubectl get pods --all-namespaces -o jsonpath='{range .items[*]}{.metadata.namespace}{" "}{.metadata.name}{" "}{range .spec.containers[*]}{.name}{" "}{end}{"\n"}{end}' | \
        grep istio-proxy | awk '{print $2}' | head -10
        return
    else
        # Default to demo applications
        selector="-l app in (demo-ota,demo-airline)"
    fi
    
    kubectl get pods $selector -o jsonpath='{range .items[*]}{.metadata.name}{"\n"}{end}' 2>/dev/null || echo ""
}

# Main execution
PODS=$(get_target_pods)

if [ -z "$PODS" ]; then
    print_error "No pods found matching criteria"
    print_status "Available pods:"
    kubectl get pods -o wide 2>/dev/null || true
    exit 1
fi

POD_COUNT=$(echo "$PODS" | wc -l)
print_status "Found $POD_COUNT pod(s) to monitor"

if [ "$FOLLOW" = true ] && [ $POD_COUNT -gt 1 ]; then
    print_warning "Follow mode with multiple pods - logs will be interleaved"
    print_status "Press Ctrl+C to stop following logs"
    echo ""
fi

# Process each pod
for POD in $PODS; do
    if [ $POD_COUNT -gt 1 ]; then
        echo ""
        print_status "=== Logs from pod: $POD ==="
    fi
    
    # Check if pod exists and has the required container
    if ! kubectl get pod "$POD" &> /dev/null; then
        print_error "Pod $POD not found"
        continue
    fi
    
    if ! kubectl get pod "$POD" -o jsonpath='{.spec.containers[*].name}' | grep -q "$CONTAINER"; then
        print_warning "Container $CONTAINER not found in pod $POD"
        continue
    fi
    
    # Get logs
    LOG_CMD=$(build_logs_cmd "$POD")
    
    if [ "$FOLLOW" = true ]; then
        # For follow mode, run in background and add pod prefix
        if [ $POD_COUNT -gt 1 ]; then
            eval "$LOG_CMD" 2>/dev/null | sed "s/^/[$POD] /" | filter_logs &
        else
            eval "$LOG_CMD" 2>/dev/null | filter_logs
        fi
    else
        # For non-follow mode, process immediately
        print_status "Recent logs from $POD ($CONTAINER container):"
        eval "$LOG_CMD" 2>/dev/null | filter_logs || print_warning "No logs found or error reading logs"
    fi
done

# If following multiple pods, wait for user interrupt
if [ "$FOLLOW" = true ] && [ $POD_COUNT -gt 1 ]; then
    wait
fi

# Show helpful information
if [ "$FOLLOW" = false ]; then
    echo ""
    print_status "💡 Helpful commands:"
    echo "  Follow logs: $0 -f"
    echo "  Filter logs: $0 --filter \"error\""
    echo "  All pods: $0 -a"
    echo "  Specific pod: $0 <pod-name>"
fi