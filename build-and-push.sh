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

VERSION=$1
ENVOY_IMAGE_NAME="sp-envoy"
WASM_IMAGE_NAME="sp-istio-wasm"
DOCKERHUB_REGISTRY="softprobe"
WASM_FULL_IMAGE_NAME="${DOCKERHUB_REGISTRY}/${WASM_IMAGE_NAME}:${VERSION}"
ENVOY_FULL_IMAGE_NAME="${DOCKERHUB_REGISTRY}/${ENVOY_IMAGE_NAME}:${VERSION}"

if [ -z "$VERSION" ]; then
    print_error "Version is required"
    exit 1
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

echo "🚀 Starting Docker build and push process..."
echo "📦 WASMImage: ${WASM_FULL_IMAGE_NAME}"
echo "📦 ENVOY Image: ${ENVOY_FULL_IMAGE_NAME}"
echo "🏷️  Version: ${VERSION}"
echo ""

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "❌ Error: Docker is not running or not accessible"
    exit 1
fi

echo "🔨 Building Docker image..."
docker build -t ${WASM_FULL_IMAGE_NAME} -f Dockerfile .
docker build -t ${ENVOY_FULL_IMAGE_NAME} -f Dockerfile.envoy .

echo "📤 Pushing image to Docker Hub..."
docker push ${WASM_FULL_IMAGE_NAME}
docker push ${ENVOY_FULL_IMAGE_NAME}

echo "✅ Successfully built and pushed to both registries:"
echo "   📦 WASM: ${WASM_FULL_IMAGE_NAME}"
echo "   📦 ENVOY: ${ENVOY_FULL_IMAGE_NAME}"

# Clean up local tags
echo "🧹 Cleaning up local tags..."
docker rmi ${WASM_FULL_IMAGE_NAME} ${ENVOY_FULL_IMAGE_NAME}

echo "🎉 Done! Images are now available in both registries:"
echo "   🔒 WASM: ${WASM_FULL_IMAGE_NAME}"
echo "   🌍 ENVOY: ${ENVOY_FULL_IMAGE_NAME}"