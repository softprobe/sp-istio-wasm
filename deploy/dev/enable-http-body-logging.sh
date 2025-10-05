#!/bin/bash

echo "🔧 Enabling detailed HTTP body logging for debugging 400 errors..."

# Function to get demo pods
get_demo_pods() {
    kubectl get pods -l app=demo-airline -o name 2>/dev/null | head -1 | cut -d'/' -f2
    kubectl get pods -l app=demo-ota -o name 2>/dev/null | head -1 | cut -d'/' -f2
}

# Get demo pods
DEMO_PODS=$(get_demo_pods)

if [ -z "$DEMO_PODS" ]; then
    echo "⚠️  No demo pods found. Please deploy demo applications first."
    echo "   Run: kubectl apply -f demo-airline-deployment.yaml"
    echo "   Run: kubectl apply -f demo-ota-deployment.yaml"
    exit 1
fi

echo "📝 Configuring enhanced HTTP logging..."

for pod in $DEMO_PODS; do
    if [ ! -z "$pod" ]; then
        echo "🎯 Configuring enhanced logging for pod: $pod"
        
        # Enable maximum debug logging for HTTP components
        kubectl exec $pod -c istio-proxy -- curl -X POST \
            "localhost:15000/logging?wasm=debug&http=debug&connection=debug&upstream=debug&router=debug&filter=debug&client=debug&misc=debug" \
            2>/dev/null || echo "Failed to set log levels for $pod"
        
        # Enable access log with request/response bodies
        kubectl exec $pod -c istio-proxy -- curl -X POST \
            "localhost:15000/logging" \
            -H "Content-Type: application/json" \
            -d '{"access_log_format": "[%START_TIME%] \"%REQ(:METHOD)% %REQ(X-ENVOY-ORIGINAL-PATH?:PATH)% %PROTOCOL%\" %RESPONSE_CODE% %RESPONSE_FLAGS% %BYTES_RECEIVED% %BYTES_SENT% %DURATION% %RESP(X-ENVOY-UPSTREAM-SERVICE-TIME)% \"%REQ(X-FORWARDED-FOR)%\" \"%REQ(USER-AGENT)%\" \"%REQ(X-REQUEST-ID)%\" \"%REQ(:AUTHORITY)%\" \"%UPSTREAM_HOST%\" REQUEST_HEADERS: %REQ_HEADERS% RESPONSE_HEADERS: %RESP_HEADERS% REQUEST_BODY: %REQ_BODY% RESPONSE_BODY: %RESP_BODY%"}' \
            2>/dev/null || echo "Note: Access log format may not be dynamically configurable"
        
        # Set debug annotation
        kubectl annotate pod $pod sidecar.istio.io/logLevel=debug --overwrite 2>/dev/null || echo "Failed to annotate $pod"
        
        # Enable stats for HTTP debugging
        kubectl exec $pod -c istio-proxy -- curl -X POST \
            "localhost:15000/stats/prometheus" 2>/dev/null > /dev/null || echo "Stats endpoint check for $pod"
    fi
done

echo ""
echo "🚀 Starting enhanced log monitoring..."
echo "📋 This will show:"
echo "   - Complete HTTP request URLs and methods"
echo "   - Full request and response headers"
echo "   - Request and response bodies (if available)"
echo "   - Upstream connection details"
echo "   - WASM plugin execution details"
echo "   - Error details and stack traces"
echo ""
echo "🔍 Monitoring for jaeger.softprobe.ai requests..."
echo "Press Ctrl+C to stop monitoring"
echo ""

# Enhanced log monitoring with more detailed patterns
for pod in $DEMO_PODS; do
    if [ ! -z "$pod" ]; then
        echo "📊 Enhanced monitoring for pod: $pod"
        kubectl logs -f $pod -c istio-proxy 2>/dev/null | \
            grep -E "(jaeger\.softprobe\.ai|POST.*v1/traces|400|Bad Request|wasm|sp-istio-agent|REQUEST_BODY|RESPONSE_BODY|upstream_host|:authority)" | \
            while read line; do
                echo "$(date '+%Y-%m-%d %H:%M:%S') [$pod] $line"
            done &
    fi
done

# Also monitor istio-system logs with enhanced patterns
echo "📊 Enhanced Istio system monitoring..."
kubectl logs -f -n istio-system deployment/istiod 2>/dev/null | \
    grep -E "(jaeger\.softprobe\.ai|400|error|wasm|trace|POST)" | \
    while read line; do
        echo "$(date '+%Y-%m-%d %H:%M:%S') [istiod] $line"
    done &

# Wait for user to stop
wait