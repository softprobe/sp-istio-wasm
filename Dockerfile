FROM scratch
COPY /target/wasm32-unknown-unknown/release/sp_istio_agent.wasm /plugin.wasm
