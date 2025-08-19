#!/bin/bash

set -e

echo "SP-Istio Cache Miss/Hit Test"
echo "==========================="

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

# Test URL - using httpbin.org/json as requested
TEST_URL="http://localhost:18000/json"
HEADERS=(-H "accept: application/json" -H "user-agent: cache-test")

print_status "Testing cache miss/hit scenario with: curl -X GET \"$TEST_URL\" ${HEADERS[*]}"
echo

# Test 1: First request (should be cache miss)
print_status "Test 1: First request (expected: CACHE MISS)"
echo "Command: curl -X GET \"$TEST_URL\" ${HEADERS[*]}"
echo "Response:"

RESPONSE1=$(curl -s -w "\nHTTP_CODE:%{http_code}\nTIME_TOTAL:%{time_total}\n" -X GET "$TEST_URL" "${HEADERS[@]}")
DURATION1=$(echo "$RESPONSE1" | grep "TIME_TOTAL:" | cut -d: -f2 | cut -d. -f1)

echo "$RESPONSE1"
echo "Duration: ${DURATION1}ms"
echo

# Wait a moment
sleep 2

# Test 2: Second request (should be cache hit if caching is working)
print_status "Test 2: Second request (expected: CACHE HIT if caching works)"
echo "Command: curl -X GET \"$TEST_URL\" ${HEADERS[*]}"
echo "Response:"

RESPONSE2=$(curl -s -w "\nHTTP_CODE:%{http_code}\nTIME_TOTAL:%{time_total}\n" -X GET "$TEST_URL" "${HEADERS[@]}")
DURATION2=$(echo "$RESPONSE2" | grep "TIME_TOTAL:" | cut -d: -f2 | cut -d. -f1)

echo "$RESPONSE2"
echo "Duration: ${DURATION2}ms"
echo

# Compare responses
print_status "Analysis:"
if [ "$RESPONSE1" = "$RESPONSE2" ]; then
    print_success "✓ Responses are identical (content consistency)"
else
    print_warning "✗ Responses differ"
fi

# Check if second request was faster (indicating cache hit)
if [ "$DURATION2" -lt "$DURATION1" ]; then
    IMPROVEMENT=$((DURATION1 - DURATION2))
    print_success "✓ Second request was ${IMPROVEMENT}ms faster (${DURATION2}ms vs ${DURATION1}ms)"
    print_success "✓ This suggests caching is working!"
else
    print_warning "✗ Second request was not faster (${DURATION2}ms vs ${DURATION1}ms)"
fi

echo
print_status "Checking Envoy logs for SP (Softprobe) entries..."
echo "Recent SP-related logs:"
echo "======================="
docker logs envoy 2>&1 | grep -E "(SP|Injection|Extraction)" | tail -10 || echo "No SP logs found"

echo
print_status "Checking if local backend received requests..."
echo "Backend logs (if available):"
echo "============================="
# Try to get logs from the backend
curl -s http://localhost:8080/logs 2>/dev/null || echo "Backend logs endpoint not available"

echo
print_status "Test completed!"
print_status "Expected behavior:"
print_status "1. First request: Cache miss → Request goes to httpbin.org → Response cached"
print_status "2. Second request: Cache hit → Cached response returned directly"
print_status ""
print_status "To manually test:"
print_status "curl -X GET \"http://localhost:18000/json\" -H \"accept: application/json\""