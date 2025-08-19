#!/bin/bash

set -e

echo "SP-Istio Injection Agent Test"
echo "============================"

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

# Start docker container
docker rm -f test-envoy
docker run -d --name test-envoy -p 18000:18000 -p 18001:18001 -v "$(pwd)/test/envoy.yaml:/etc/envoy/envoy.yaml" -v \
    "$(pwd)/target/wasm32-unknown-unknown/release/sp_istio_agent.wasm:/etc/envoy/sp_istio_agent.wasm" envoyproxy/envoy:v1.27-latest \
    envoy -c /etc/envoy/envoy.yaml --log-level debug
sleep 1

# Test URL - using httpbin.org/delay/2 as suggested
TEST_URL="http://localhost:18000/delay/2"
HEADERS=(-H "accept: application/json")
DATA="test"

print_status "Testing injection agent with: curl -X POST \"$TEST_URL\" ${HEADERS[*]} -d \"$DATA\""
print_status "Key indicator: X-Amzn-Trace-Id should be SAME on cache hit, DIFFERENT on cache miss"
echo

# Test 1: First request (should be cache miss)
print_status "Test 1: First request (expected: CACHE MISS)"
echo "Command: curl -X POST \"$TEST_URL\" ${HEADERS[*]} -d \"$DATA\""
echo "Response:"

RESPONSE1=$(curl -s -w "\nHTTP_CODE:%{http_code}\nTIME_TOTAL:%{time_total}s\n" -X POST "$TEST_URL" "${HEADERS[@]}" -d "$DATA")
echo "$RESPONSE1"

# Extract trace ID from first response
TRACE_ID1=$(echo "$RESPONSE1" | grep -o '"X-Amzn-Trace-Id": "[^"]*"' | cut -d'"' -f4)
DURATION1=$(echo "$RESPONSE1" | grep "TIME_TOTAL:" | cut -d: -f2 | cut -ds -f1)

print_status "First request trace ID: $TRACE_ID1"
print_status "First request duration: ${DURATION1}s"
echo

# Wait a moment to ensure any async operations complete
print_status "Waiting 3 seconds for agent storage to complete..."
sleep 3

# Test 2: Second request (should be cache hit if injection is working)
print_status "Test 2: Second request (expected: CACHE HIT if injection works)"
echo "Command: curl -X POST \"$TEST_URL\" ${HEADERS[*]} -d \"$DATA\""
echo "Response:"

RESPONSE2=$(curl -s -w "\nHTTP_CODE:%{http_code}\nTIME_TOTAL:%{time_total}s\n" -X POST "$TEST_URL" "${HEADERS[@]}" -d "$DATA")
echo "$RESPONSE2"

# Extract trace ID from second response
TRACE_ID2=$(echo "$RESPONSE2" | grep -o '"X-Amzn-Trace-Id": "[^"]*"' | cut -d'"' -f4)
DURATION2=$(echo "$RESPONSE2" | grep "TIME_TOTAL:" | cut -d: -f2 | cut -ds -f1)

print_status "Second request trace ID: $TRACE_ID2"
print_status "Second request duration: ${DURATION2}s"
echo

# Compare responses and trace IDs
print_status "Analysis:"
if [ "$TRACE_ID1" = "$TRACE_ID2" ]; then
    print_success "✓ CACHE HIT! Trace IDs are identical: $TRACE_ID1"
    print_success "✓ This indicates the response was served from injection agent"
else
    print_warning "✗ CACHE MISS! Trace IDs are different:"
    print_warning "   First:  $TRACE_ID1"
    print_warning "   Second: $TRACE_ID2"
    print_warning "   This indicates both requests went to httpbin.org"
fi

# Check response time difference
if command -v bc >/dev/null 2>&1; then
    if [ -n "$DURATION1" ] && [ -n "$DURATION2" ]; then
        IMPROVEMENT=$(echo "$DURATION1 - $DURATION2" | bc -l)
        if (( $(echo "$IMPROVEMENT > 0.1" | bc -l) )); then
            print_success "✓ Second request was ${IMPROVEMENT}s faster (injection saved time)"
        elif (( $(echo "$IMPROVEMENT < -0.1" | bc -l) )); then
            print_warning "✗ Second request was slower (no caching benefit)"
        else
            print_status "≈ Response times similar (${DURATION1}s vs ${DURATION2}s)"
        fi
    fi
fi

echo
print_status "Checking Envoy logs for injection activity..."
echo "Recent injection logs:"
echo "======================"
docker logs envoy 2>&1 | grep -E "(SP|Injection|injection|Extract)" | tail -15 || echo "No injection logs found"

echo
print_status "Test completed!"
print_status ""
print_status "Expected behavior:"
print_status "1. First request: Cache miss → Request to /v1/inject returns 404 → Goes to httpbin.org → Stores response"
print_status "2. Second request: Cache hit → /v1/inject returns agentd data → Same trace ID returned"
print_status ""
print_status "Current status:"
if [ "$TRACE_ID1" = "$TRACE_ID2" ]; then
    print_success "✓ INJECTION CACHE IS WORKING!"
else
    print_warning "✗ Cache miss - check if backend supports /v1/inject endpoint properly"
fi