#!/bin/bash

echo "🔍 Advanced Jaeger Request Debugging Tool"
echo "=========================================="

# Function to get demo pods
get_demo_pods() {
    kubectl get pods -l app=demo-airline -o name 2>/dev/null | head -1 | cut -d'/' -f2
    kubectl get pods -l app=demo-ota -o name 2>/dev/null | head -1 | cut -d'/' -f2
}

# Function to enable detailed HTTP tracing
enable_http_tracing() {
    local pod=$1
    echo "🔧 Enabling HTTP tracing for $pod..."
    
    # Enable all relevant debug logging
    kubectl exec $pod -c istio-proxy -- curl -s -X POST \
        "localhost:15000/logging?wasm=debug&http=debug&connection=debug&upstream=debug&router=debug&filter=debug&client=debug&misc=debug&assert=debug" \
        || echo "⚠️  Failed to set debug logging for $pod"
    
    # Enable HTTP/2 and HTTP/1.1 detailed logging
    kubectl exec $pod -c istio-proxy -- curl -s -X POST \
        "localhost:15000/logging?http2=debug&http1=debug" \
        || echo "⚠️  Failed to set HTTP protocol logging for $pod"
}

# Function to capture HTTP request details
capture_http_details() {
    local pod=$1
    echo "📡 Capturing HTTP details for $pod..."
    
    # Get current Envoy configuration
    kubectl exec $pod -c istio-proxy -- curl -s "localhost:15000/config_dump" | \
        jq '.configs[] | select(.["@type"] | contains("Listener")) | .dynamic_listeners[].active_state.listener.filter_chains[].filters[] | select(.name == "envoy.filters.network.http_connection_manager")' > /tmp/${pod}_http_config.json 2>/dev/null || echo "Config dump failed for $pod"
    
    # Get cluster information
    kubectl exec $pod -c istio-proxy -- curl -s "localhost:15000/clusters" | \
        grep -E "(jaeger|softprobe)" > /tmp/${pod}_clusters.txt 2>/dev/null || echo "Cluster info failed for $pod"
    
    # Get stats related to upstream connections
    kubectl exec $pod -c istio-proxy -- curl -s "localhost:15000/stats" | \
        grep -E "(jaeger|softprobe|upstream.*400|http.*400)" > /tmp/${pod}_stats.txt 2>/dev/null || echo "Stats failed for $pod"
}

# Function to test Jaeger endpoint directly
test_jaeger_endpoint() {
    echo "🧪 Testing Jaeger endpoint directly..."
    
    # Test basic connectivity
    echo "Testing basic connectivity to jaeger.softprobe.ai..."
    curl -v -X POST "https://jaeger.softprobe.ai/api/traces" \
        -H "Content-Type: application/json" \
        -d '{"data":[{"traceID":"test","spans":[]}]}' \
        --connect-timeout 10 \
        --max-time 30 2>&1 | head -20
    
    echo ""
    echo "Testing OTLP endpoint..."
    curl -v -X POST "https://jaeger.softprobe.ai/v1/traces" \
        -H "Content-Type: application/x-protobuf" \
        -H "Content-Encoding: gzip" \
        --data-binary @<(echo "test" | gzip) \
        --connect-timeout 10 \
        --max-time 30 2>&1 | head -20
}

# Main execution
DEMO_PODS=$(get_demo_pods)

if [ -z "$DEMO_PODS" ]; then
    echo "⚠️  No demo pods found. Testing Jaeger endpoint directly..."
    test_jaeger_endpoint
    exit 1
fi

echo "📋 Found demo pods: $DEMO_PODS"
echo ""

# Enable tracing for all pods
for pod in $DEMO_PODS; do
    if [ ! -z "$pod" ]; then
        enable_http_tracing $pod
        capture_http_details $pod
    fi
done

echo ""
echo "🔍 Starting comprehensive monitoring..."
echo "This will capture:"
echo "  ✓ Full HTTP request/response cycle"
echo "  ✓ Upstream connection details"
echo "  ✓ SSL/TLS handshake information"
echo "  ✓ DNS resolution details"
echo "  ✓ WASM plugin execution logs"
echo "  ✓ Error codes and reasons"
echo ""

# Test endpoint first
test_jaeger_endpoint

echo ""
echo "📊 Monitoring pod logs (Press Ctrl+C to stop)..."

# Enhanced monitoring with multiple patterns
for pod in $DEMO_PODS; do
    if [ ! -z "$pod" ]; then
        echo "Starting monitor for $pod..."
        kubectl logs -f $pod -c istio-proxy --tail=0 2>/dev/null | \
            grep -E "(jaeger\.softprobe\.ai|POST.*traces|400|Bad Request|upstream_host|:authority|content-type|content-length|user-agent|wasm.*sp-istio|error|failed|timeout|connection.*refused|ssl|tls|dns)" | \
            while IFS= read -r line; do
                timestamp=$(date '+%Y-%m-%d %H:%M:%S.%3N')
                echo "[$timestamp][$pod] $line"
                
                # If we see a 400 error, capture additional context
                if echo "$line" | grep -q "400"; then
                    echo "🚨 400 ERROR DETECTED - Capturing additional context..."
                    kubectl exec $pod -c istio-proxy -- curl -s "localhost:15000/stats" | \
                        grep -E "(upstream.*jaeger|http.*400|connection.*error)" | \
                        head -10 | \
                        while IFS= read -r stat; do
                            echo "[$timestamp][$pod][STATS] $stat"
                        done
                fi
            done &
    fi
done

# Monitor istio-system logs
kubectl logs -f -n istio-system deployment/istiod --tail=0 2>/dev/null | \
    grep -E "(jaeger\.softprobe\.ai|400|error|wasm|trace|POST)" | \
    while IFS= read -r line; do
        timestamp=$(date '+%Y-%m-%d %H:%M:%S.%3N')
        echo "[$timestamp][istiod] $line"
    done &

echo ""
echo "💡 Debugging Tips:"
echo "  1. Look for 'upstream_host' to see the actual target"
echo "  2. Check ':authority' header for the Host header value"
echo "  3. Watch for SSL/TLS errors if using HTTPS"
echo "  4. Monitor 'content-type' and 'content-length' headers"
echo "  5. Check for DNS resolution issues"
echo ""
echo "📁 Additional debug files created in /tmp/:"
for pod in $DEMO_PODS; do
    if [ ! -z "$pod" ]; then
        echo "  - /tmp/${pod}_http_config.json"
        echo "  - /tmp/${pod}_clusters.txt"
        echo "  - /tmp/${pod}_stats.txt"
    fi
done

# Wait for user to stop
wait