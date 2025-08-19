#!/bin/bash

set -e

echo "SP-Istio Cache Test Script"
echo "========================="

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

# Check if Envoy is available
if ! command -v envoy &> /dev/null; then
    print_error "Envoy not found. Please install Envoy for local testing."
    print_status "You can install Envoy using:"
    echo "  # On macOS:"
    echo "  brew install envoy"
    echo "  # On Ubuntu:"
    echo "  sudo apt-get update && sudo apt-get install getenvoy-envoy"
    exit 1
fi

# Check if WASM binary exists
if [ ! -f "target/wasm32-unknown-unknown/release/sp_istio_cache.wasm" ]; then
    print_error "WASM binary not found. Run ./build.sh first."
    exit 1
fi

# Function to test with Envoy
test_with_envoy() {
    print_status "Starting Envoy with WASM extension..."
    
    # Kill any existing Envoy processes
    pkill -f "envoy.*18000" || true
    sleep 1
    
    # Start Envoy in background
    envoy -c test/envoy.yaml --log-level info > envoy.log 2>&1 &
    ENVOY_PID=$!
    
    # Wait for Envoy to start
    print_status "Waiting for Envoy to start..."
    sleep 3
    
    # Check if Envoy is running
    if ! ps -p $ENVOY_PID > /dev/null; then
        print_error "Envoy failed to start. Check envoy.log for details."
        cat envoy.log
        exit 1
    fi
    
    print_success "Envoy started successfully (PID: $ENVOY_PID)"
    
    # Test basic connectivity
    print_status "Testing basic connectivity..."
    if curl -s -o /dev/null -w "%{http_code}" http://localhost:18000/get | grep -q "200"; then
        print_success "Basic connectivity test passed!"
    else
        print_warning "Basic connectivity test failed, but WASM may still be loaded"
    fi
    
    # Test POST request (should trigger cache logic)
    print_status "Testing POST request (cache logic)..."
    RESPONSE=$(curl -s -w "%{http_code}" -X POST http://localhost:18000/post \
        -H "Content-Type: application/json" \
        -d '{"test": "data"}' \
        -o /dev/null)
    
    if [ "$RESPONSE" = "200" ]; then
        print_success "POST request test passed!"
    else
        print_warning "POST request returned: $RESPONSE"
    fi
    
    # Show recent Envoy logs
    print_status "Recent Envoy logs:"
    echo "=================="
    tail -20 envoy.log | grep -E "(SP|WASM|error|ERROR)" || echo "No relevant logs found"
    
    # Cleanup
    print_status "Stopping Envoy..."
    kill $ENVOY_PID
    wait $ENVOY_PID 2>/dev/null || true
    
    print_success "Test completed. Check envoy.log for detailed logs."
}

# Function to validate WASM binary
validate_wasm() {
    print_status "Validating WASM binary..."
    
    # Check file size (should be reasonable, not too small or too large)
    SIZE=$(stat -f%z target/wasm32-unknown-unknown/release/sp_istio_cache.wasm 2>/dev/null || stat -c%s target/wasm32-unknown-unknown/release/sp_istio_cache.wasm)
    
    if [ "$SIZE" -lt 10000 ]; then
        print_warning "WASM binary seems very small ($SIZE bytes). This might indicate build issues."
    elif [ "$SIZE" -gt 10000000 ]; then
        print_warning "WASM binary is quite large ($SIZE bytes). Consider optimizing."
    else
        print_success "WASM binary size looks reasonable ($SIZE bytes)"
    fi
    
    # Check if it's a valid WASM file
    if file target/wasm32-unknown-unknown/release/sp_istio_cache.wasm | grep -q "WebAssembly"; then
        print_success "File is a valid WebAssembly binary"
    else
        print_error "File does not appear to be a valid WebAssembly binary"
        exit 1
    fi
}

# Main execution
case "${1:-all}" in
    "validate")
        validate_wasm
        ;;
    "envoy")
        validate_wasm
        test_with_envoy
        ;;
    "all")
        validate_wasm
        test_with_envoy
        ;;
    *)
        echo "Usage: $0 [validate|envoy|all]"
        echo "  validate - Only validate WASM binary"
        echo "  envoy    - Test with local Envoy"
        echo "  all      - Run all tests (default)"
        exit 1
        ;;
esac