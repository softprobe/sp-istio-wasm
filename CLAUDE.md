# Project
Transparent cache on Istio

## Overview
This project aims to extend Istio's by using a custom WebAssembly (WASM) extension written in Rust. The extension will intercept outgoing HTTP requests in the Envoy proxy, capture request and response headers and bodies, and do the following:

1. Forward request to Softprobe (https://o.softprobe.ai) by putting request header and bodies in the message.
2. Softprobe will examine the request and return 200 HTTP status code, along with response header and body if it decides to inject data, other wise, a 404 HTTP status code is returned with no HTTP body. We call 200 a cache hit, and 404 a cache miss.
3. The extension should return the injected HTTP header and body if 200 HTTP status code is retuened. Otherwise, it should continue the call to the original upstream.
4. If not cache hit and upstream response is returned and it's HTTP status code is 200, send the original request header and body and reponse header and body to Softprobe, asynchronously.

## Key Requirements

- Intercept Outgoing HTTP Requests: Use Istio and Envoy to capture traffic from applications to other services.
- WASM Hosting: Build and host the WASM binary on Cloudflare R2 object storage.
- Capture Data: Inject all HTTP request/response headers and bodies into OTEL span attributes.
- Implementation Language: Rust for the WASM extension.
- Environment: Istio 1.18 on MicroK8s cluster.

# Development Approach

- Use the Proxy-WASM Rust SDK to build the HTTP filter.
- Buffer and capture headers/bodies in the WASM filter.
- Modify the sidecar injector ConfigMap to add custom span tags from dynamic metadata (since Istio's API doesn't support metadata-type tags natively).
- Use WasmPlugin to deploy the extension, fetching from Cloudflare R2 with SHA256 integrity.
- Use EnvoyFilter to insert the WASM filter into the outbound HTTP chain and patch for HTTP export if needed.

# Subagents
This project is broken down into subagents for modular development:

- subagent-rust.md: Develop the Rust WASM code.
- subagent-build-host.md: Build the WASM binary and host on Cloudflare R2.
- subagent-istio-config.md: Configure Istio resources (Telemetry, WasmPlugin, EnvoyFilter, injector patch).
- subagent-deployment.md: Deployment and testing instructions on MicroK8s.

# Tools and Dependencies

- Rust toolchain with wasm32-unknown-unknown target.
- Cargo crates: proxy-wasm, serde_json (for serialization), base64 (for bodies).
- Cloudflare R2 CLI for uploading.
- kubectl and istioctl for Istio management.

# Potential Challenges

- Body buffering impacts performance; but we will take the performance hit for now, don't over engineering performance on this.
- Large attributes may be truncated by OTEL collectors; serialize compactly.
- Ensure the collector at https://o.softprobe.ai accepts OTLP/HTTP Protobuf and handles TLS.

Proceed by developing each subagent in sequence.
# important-instruction-reminders
Do what has been asked; nothing more, nothing less.
NEVER create files unless they're absolutely necessary for achieving your goal.
ALWAYS prefer editing an existing file to creating a new one.
NEVER proactively create documentation files (*.md) or README files. Only create documentation files if explicitly requested by the User.