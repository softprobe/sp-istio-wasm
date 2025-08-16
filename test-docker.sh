#!/bin/bash

set -e

echo "SP-Istio Cache Docker Test Script"
echo "================================="

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

# Check if Docker is available
if ! command -v docker &> /dev/null; then
    print_error "Docker not found. Please install Docker."
    exit 1
fi

# Check if WASM binary exists
if [ ! -f "target/wasm32-unknown-unknown/release/sp_istio_cache.wasm" ]; then
    print_error "WASM binary not found. Run ./build.sh first."
    exit 1
fi

# Function to cleanup
cleanup() {
    print_status "Cleaning up..."
    docker stop envoy-wasm-test 2>/dev/null || true
    docker rm envoy-wasm-test 2>/dev/null || true
}

# Trap cleanup on exit
trap cleanup EXIT

# Stop any existing container
cleanup

print_status "Starting Envoy container with WASM extension..."

# Run Envoy in Docker container
docker rm -f envoy-wasm-test
docker run -d \
    --name envoy-wasm-test \
    -p 18000:18000 \
    -p 18001:18001 \
    -v "$(pwd)/target/wasm32-unknown-unknown/release/sp_istio_cache.wasm:/tmp/sp_istio_cache.wasm:ro" \
    -v "$(pwd)/test/docker-envoy.yaml:/etc/envoy/envoy.yaml:ro" \
    envoyproxy/envoy:v1.27-latest \
    envoy -c /etc/envoy/envoy.yaml --log-level info

# Wait for Envoy to start
print_status "Waiting for Envoy to start..."
sleep 5

# Check if container is running
if ! docker ps | grep -q envoy-wasm-test; then
    print_error "Envoy container failed to start"
    docker logs envoy-wasm-test
    exit 1
fi

print_success "Envoy container started successfully"

# Test basic connectivity
print_status "Testing basic connectivity..."
for i in {1..10}; do
    if curl -s -f http://localhost:18000/get > /dev/null 2>&1; then
        print_success "Basic connectivity test passed!"
        break
    fi
    if [ $i -eq 10 ]; then
        print_warning "Basic connectivity test failed after 10 attempts"
        break
    fi
    sleep 1
done

# Test POST request (should trigger cache logic)
print_status "Testing POST request (cache logic)..."
RESPONSE=$(curl -s -w "%{http_code}" -X POST http://localhost:18000/post \
    -H "Content-Type: application/json" \
    -d '{"test": "cache trigger"}' \
    -o /dev/null 2>/dev/null || echo "000")

if [ "$RESPONSE" = "200" ]; then
    print_success "POST request test passed!"
else
    print_warning "POST request returned: $RESPONSE"
fi

# Show container logs
print_status "Recent Envoy logs:"
echo "=================="
docker logs envoy-wasm-test 2>&1 | grep -E "(SP Cache|WASM|wasm|error|ERROR)" | tail -20 || echo "No relevant logs found"

# Show container stats
print_status "Container status:"
docker ps | grep envoy-wasm-test

print_status "Test completed. Container will be stopped automatically."
print_status "To view full logs: docker logs envoy-wasm-test"
print_status "To access admin interface: http://localhost:18001"