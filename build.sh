#!/bin/bash

set -e

echo "SP-Istio Agent Build Script"
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

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    print_error "Cargo not found. Please install Rust toolchain."
    exit 1
fi

# Check if wasm32-unknown-unknown target is installed
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    print_status "Installing wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

# Clean previous build
print_status "Cleaning previous build..."
cargo clean

# Build the WASM binary
print_status "Building WASM binary..."
cargo build --target wasm32-unknown-unknown --release

# Check if build was successful
if [ -f "target/wasm32-unknown-unknown/release/sp_istio_agent.wasm" ]; then
    print_success "WASM binary built successfully!"
    
    # Calculate SHA256 hash
    print_status "Calculating SHA256 hash..."
    if command -v sha256sum &> /dev/null; then
        HASH=$(sha256sum target/wasm32-unknown-unknown/release/sp_istio_agent.wasm | cut -d' ' -f1)
    elif command -v shasum &> /dev/null; then
        HASH=$(shasum -a 256 target/wasm32-unknown-unknown/release/sp_istio_agent.wasm | cut -d' ' -f1)
    else
        print_error "Neither sha256sum nor shasum found. Cannot calculate hash."
        exit 1
    fi
    
    print_success "SHA256: $HASH"
    echo "$HASH" > target/wasm32-unknown-unknown/release/sp_istio_agent.wasm.sha256
    
    # Show file size
    SIZE=$(ls -lh target/wasm32-unknown-unknown/release/sp_istio_agent.wasm | awk '{print $5}')
    print_status "WASM file size: $SIZE"
    
    # Offer to update Istio configs
    echo ""
    echo "To update Istio configurations with this hash, run:"
    echo "  sed -i 's/sha256: .*/sha256: $HASH/' istio-configs/wasmplugin.yaml"
    echo "  sed -i 's/sha256: .*/sha256: $HASH/' istio-configs/envoyfilter.yaml"
    
else
    print_error "Build failed!"
    exit 1
fi