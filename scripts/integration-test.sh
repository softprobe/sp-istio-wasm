#!/bin/bash

set -e

echo "SP-Istio Integration Test with Softprobe Backend Verification"
echo "============================================================="

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

# Configuration
ENVOY_CONTAINER="sp-envoy-test"
INBOUND_PORT=19006
OUTBOUND_PORT=19001
ADMIN_PORT=19002
BACKEND_URL="https://o.softprobe.ai"
API_KEY="test123"
SERVICE_NAME="sp-travel-ota"

# Test data
TEST_REQUEST_ID=$(date +%s)
TEST_SESSION_ID="test-session-${TEST_REQUEST_ID}"

# Cleanup function
cleanup() {
    print_status "Cleaning up test environment..."
    docker rm -f $ENVOY_CONTAINER 2>/dev/null || true
    docker rm -f test-envoy 2>/dev/null || true  # Clean up any existing test containers
    docker rm -f mock-server 2>/dev/null || true  # Clean up mock server
    docker network rm sp-test-network 2>/dev/null || true
}

# Set up cleanup trap
trap cleanup EXIT

print_status "Building WASM binary..."
make build

print_status "Setting up test environment..."

# Create test network
docker network create sp-test-network 2>/dev/null || true

# Start a simple HTTP mock server
print_status "Starting mock HTTP server..."
docker run -d \
    --name mock-server \
    --network sp-test-network \
    nginx:alpine

# Wait for mock server to start
sleep 2

# Start Envoy container with WASM plugin
print_status "Starting Envoy with SP-Istio WASM plugin..."
docker rm -f $ENVOY_CONTAINER 2>/dev/null || true

docker run -d \
    --name $ENVOY_CONTAINER \
    --network sp-test-network \
    -p $INBOUND_PORT:15006 \
    -p $OUTBOUND_PORT:15001 \
    -p $ADMIN_PORT:18001 \
    -v "$(pwd)/test/envoy.yaml:/etc/envoy/envoy.yaml" \
    -v "$(pwd)/target/wasm32-unknown-unknown/release/sp_istio_agent.wasm:/etc/envoy/sp_istio_agent.wasm" \
    envoyproxy/envoy:v1.27-latest \
    envoy -c /etc/envoy/envoy.yaml --log-level info

# Wait for Envoy to start
print_status "Waiting for Envoy to initialize..."
sleep 5

# Check if Envoy is healthy
if ! curl -s "http://localhost:$ADMIN_PORT/ready" > /dev/null; then
    print_error "Envoy failed to start properly"
    docker logs $ENVOY_CONTAINER
    exit 1
fi

print_success "Envoy started successfully"

# Function to wait for traces to be processed
wait_for_traces() {
    local wait_time=${1:-10}
    print_status "Waiting ${wait_time}s for traces to be sent to Softprobe backend..."
    sleep $wait_time
}

# Function to query traces from Softprobe backend
query_traces() {
    local service_name=$1
    local start_time=$2
    local limit=${3:-50}
    
    print_status "Querying traces from Softprobe backend for service: $service_name and start time: $start_time" >&2
    
    curl -s -X GET "${BACKEND_URL}/v1/traces" \
        -H "Accept: application/json" \
        -G \
        -d "serviceName=${service_name}" \
        -d "startTime=${start_time}" \
        -d "limit=${limit}" || echo "{}"
}

# Function to get session traces
get_session_traces() {
    local session_id=$1
    
    print_status "Getting traces for session: $session_id"
    
    curl -s -X GET "${BACKEND_URL}/api/trace/sessions/${session_id}/traces" \
        -H "Accept: application/json" || echo "{}"
}

# Test 1: Outbound HTTP request (client mode)
print_status "=== TEST 1: Outbound HTTP Request (Client Mode) ==="

# Record start time for trace queries
TEST_START_TIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

print_status "Making outbound HTTP request through Envoy proxy..."
print_status "Request will be captured by outbound WASM filter and sent to Softprobe"

# Make HTTP request through outbound listener
OUTBOUND_RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}\nTIME_TOTAL:%{time_total}s\n" \
    -X GET "http://localhost:$OUTBOUND_PORT/" \
    -H "Content-Type: application/json" \
    -H "X-Session-ID: $TEST_SESSION_ID" \
    -H "X-Test-Request-ID: $TEST_REQUEST_ID")

echo "Outbound Response:"
echo "$OUTBOUND_RESPONSE"

# Note: We'll extract trace IDs from the session query later, since
# trace headers are added by the WASM plugin and may not be in curl output
echo

# Test 2: Inbound HTTP request (server mode)
print_status "=== TEST 2: Inbound HTTP Request (Server Mode) ==="

print_status "Making inbound HTTP request to Envoy proxy..."
print_status "Request will be captured by inbound WASM filter and sent to Softprobe"

# Make HTTP request to inbound listener
INBOUND_RESPONSE=$(curl -s -w "\nHTTP_CODE:%{http_code}\nTIME_TOTAL:%{time_total}s\n" \
    -X GET "http://localhost:$INBOUND_PORT/" \
    -H "Content-Type: application/json" \
    -H "X-Session-ID: $TEST_SESSION_ID" \
    -H "X-Test-Request-ID: $TEST_REQUEST_ID")

echo "Inbound Response:"
echo "$INBOUND_RESPONSE"

# Note: We'll validate traces using session queries since that's more reliable
echo

# Wait for traces to be sent to backend
wait_for_traces 10

# Test 3: Verify data was sent to Softprobe backend
print_status "=== TEST 3: Verify Data Ingestion to Softprobe Backend ==="

# Query traces by service name
print_status "Querying traces from Softprobe backend..."
TRACE_QUERY_RESPONSE=$(query_traces "$SERVICE_NAME" "$TEST_START_TIME")

echo "Trace Query Response:"
echo "$TRACE_QUERY_RESPONSE" | jq . 2>/dev/null || echo "$TRACE_QUERY_RESPONSE"
echo

# Check if we received any traces
TRACE_COUNT=$(echo "$TRACE_QUERY_RESPONSE" | jq '.resourceSpans | length' 2>/dev/null || echo "0")

echo "Found $TRACE_COUNT traces"

if [ "$TRACE_COUNT" -gt 0 ]; then
    print_success "✓ Found $TRACE_COUNT trace(s) in Softprobe backend"
    
    # Extract trace IDs from backend response
    BACKEND_TRACE_IDS=$(echo "$TRACE_QUERY_RESPONSE" | jq -r '.resourceSpans[].scopeSpans[].spans[].traceId' 2>/dev/null)
    
    print_status "Backend trace IDs found:"
    echo "$BACKEND_TRACE_IDS"    
else
    print_warning "✗ No traces found in Softprobe backend"
    print_status "This could indicate:"
    print_status "  1. WASM plugin is not sending data correctly"
    print_status "  2. Softprobe backend is not receiving the data"
    print_status "  3. Data hasn't been processed yet (try increasing wait time)"
fi

# Test 4: Check session-based queries
print_status "=== TEST 4: Session-Based Trace Queries ==="

SESSION_TRACES=$(get_session_traces "$TEST_SESSION_ID")
echo "Session Traces Response:"
echo "$SESSION_TRACES" | jq . 2>/dev/null || echo "$SESSION_TRACES"

# Extract information from session traces for validation - use grep as fallback if jq fails
SESSION_TRACE_COUNT=$(echo "$SESSION_TRACES" | jq -r '.totalTraces // 0' 2>/dev/null || echo "$SESSION_TRACES" | grep -o '"totalTraces":[0-9]*' | cut -d: -f2 || echo "0")
SESSION_SPAN_COUNT=$(echo "$SESSION_TRACES" | jq -r '.totalSpans // 0' 2>/dev/null || echo "$SESSION_TRACES" | grep -o '"totalSpans":[0-9]*' | cut -d: -f2 || echo "0")

print_status "Session validation results:"
print_status "  Session ID: $TEST_SESSION_ID"
print_status "  Traces found: $SESSION_TRACE_COUNT"
print_status "  Spans found: $SESSION_SPAN_COUNT"

# Check if our test session has traces (ensure we have a valid number)
SESSION_TRACE_COUNT_NUM=${SESSION_TRACE_COUNT:-0}
if [ "$SESSION_TRACE_COUNT_NUM" -gt 0 ] 2>/dev/null; then
    print_success "✓ Found $SESSION_TRACE_COUNT trace(s) for test session $TEST_SESSION_ID"
    
    # Extract trace IDs from session response
    SESSION_TRACE_IDS=$(echo "$SESSION_TRACES" | jq -r '.traces[].traceId' 2>/dev/null || echo "")
    print_status "Session trace IDs:"
    echo "$SESSION_TRACE_IDS"
    
    # Validate we have traces with our test request ID
    TEST_REQUEST_FOUND=false
    echo "DEBUG: Searching for test request ID: $TEST_REQUEST_ID"
    echo "DEBUG: Raw JSON from traces:"
    echo "$SESSION_TRACES" | jq -r '.traces[].spans[].attributes.raw_json' 2>/dev/null | head -3
    
    if echo "$SESSION_TRACES" | jq -r '.traces[].spans[].attributes.raw_json' 2>/dev/null | grep -q "$TEST_REQUEST_ID"; then
        TEST_REQUEST_FOUND=true
        print_success "✓ Found traces containing our test request ID: $TEST_REQUEST_ID"
    else
        print_warning "✗ No traces found with our test request ID: $TEST_REQUEST_ID"
    fi
else
    print_warning "✗ No traces found for test session $TEST_SESSION_ID"
fi

echo

# Test 5: Check Envoy logs for WASM plugin activity
print_status "=== TEST 5: Envoy WASM Plugin Log Analysis ==="

print_status "Checking Envoy logs for SP-Istio plugin activity..."
docker logs $ENVOY_CONTAINER 2>&1 | grep -E "(SP|wasm|sp_agent)" | tail -20 || echo "No SP-related logs found"

# Test 6: Envoy admin interface checks
print_status "=== TEST 6: Envoy Admin Interface Checks ==="

print_status "Checking WASM extension status..."
curl -s "http://localhost:$ADMIN_PORT/stats" | grep -E "(wasm|sp_agent)" || echo "No WASM stats found"

echo
print_status "Checking active listeners..."
curl -s "http://localhost:$ADMIN_PORT/listeners" | jq . 2>/dev/null || curl -s "http://localhost:$ADMIN_PORT/listeners"

# Test 7: Health check Softprobe backend
print_status "=== TEST 7: Softprobe Backend Health Check ==="

BACKEND_HEALTH=$(curl -s -X GET "${BACKEND_URL}/health" || echo '{"status": "unreachable"}')
echo "Backend Health:"
echo "$BACKEND_HEALTH" | jq . 2>/dev/null || echo "$BACKEND_HEALTH"

# Summary
echo
print_status "=== TEST SUMMARY ==="

# Determine overall test success based on session validation
OVERALL_SUCCESS=false
if [ "$SESSION_TRACE_COUNT_NUM" -gt 0 ] 2>/dev/null && [ "$TEST_REQUEST_FOUND" = true ]; then
    OVERALL_SUCCESS=true
fi

if [ "$OVERALL_SUCCESS" = true ]; then
    print_success "✓ Integration test PASSED"
    print_success "✓ WASM plugin successfully captured and sent HTTP traffic data"
    print_success "✓ Softprobe backend received $SESSION_TRACE_COUNT_NUM trace(s) for our test session"
    print_success "✓ Test request ID ($TEST_REQUEST_ID) validated in backend traces"
    print_success "✓ Session-based trace queries working correctly"
    print_success "✓ End-to-end telemetry pipeline fully functional"
    
    # Additional validation details
    if [ "$TRACE_COUNT" -gt 0 ]; then
        print_success "✓ General trace query also returned $TRACE_COUNT trace(s)"
    fi
else
    if [ "$SESSION_TRACE_COUNT_NUM" -gt 0 ] 2>/dev/null; then
        print_warning "✗ Integration test PARTIAL SUCCESS"
        print_warning "✗ Traces found for session but test request ID not validated"
        print_warning "✗ Check test request ID: $TEST_REQUEST_ID"
    elif [ "$TRACE_COUNT" -gt 0 ]; then
        print_warning "✗ Integration test PARTIAL SUCCESS"  
        print_warning "✗ General traces found but no traces for our test session"
        print_warning "✗ Check session ID: $TEST_SESSION_ID"
    else
        print_warning "✗ Integration test FAILED"
        print_warning "✗ No traces found in Softprobe backend"
        print_warning "✗ Check WASM plugin configuration and network connectivity"
    fi
fi

print_status "Test completed. Check the logs above for detailed analysis."

# Exit with appropriate code based on test results
if [ "$OVERALL_SUCCESS" = true ]; then
    exit 0
else
    exit 1
fi