use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use std::collections::HashMap;

mod cache;
mod otel;
mod http_client;

use cache::CacheHandler;

// Main entry point for the WASM module
// Sets up the root context which manages the entire filter lifecycle
proxy_wasm::main! {{
    proxy_wasm::set_log_level(LogLevel::Debug);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> {
        Box::new(SpCacheRoot)
    });
}}

// SpCacheRoot: The singleton RootContext for the WASM module
// 
// In proxy-wasm architecture, there are two types of contexts:
// 1. RootContext (this struct): One instance per worker thread, manages the entire module
//    - Creates HttpContext instances for each request
//    - Manages shared configuration and state
// 2. HttpContext: One instance per HTTP request flowing through the proxy
//    - Handles request/response processing for that specific request
//    - Handles HTTP call responses from its own dispatch_http_call() calls
//    - Maintains per-request state and buffers
struct SpCacheRoot;

impl Context for SpCacheRoot {}

impl RootContext for SpCacheRoot {
    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }

    fn create_http_context(&self, _context_id: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(SpCacheHttpContext::new()))
    }
}

// SpCacheHttpContext: Per-request context for HTTP processing
//
// This is created for each HTTP request flowing through the proxy. It:
// - Buffers request/response headers and bodies
// - Initiates cache lookups by calling the external service
// - Handles HTTP call responses from its own dispatch_http_call() calls
// - Maintains per-request state like pending call tokens
struct SpCacheHttpContext {
    context_id: u32,                           // Unique ID for this request context
    request_headers: HashMap<String, String>,  // Buffered request headers
    request_body: Vec<u8>,                     // Buffered request body
    response_headers: HashMap<String, String>, // Buffered response headers  
    response_body: Vec<u8>,                    // Buffered response body
    cache_handler: CacheHandler,               // Handles cache operations
    pending_cache_lookup: Option<u32>,         // Track cache lookup call token
}

impl SpCacheHttpContext {
    fn new() -> Self {
        Self {
            context_id: 0, // Not used in this simplified approach
            request_headers: HashMap::new(),
            request_body: Vec::new(),
            response_headers: HashMap::new(),
            response_body: Vec::new(),
            cache_handler: CacheHandler::new(),
            pending_cache_lookup: None,
        }
    }
}

impl Context for SpCacheHttpContext {
    fn on_http_call_response(&mut self, token_id: u32, _num_headers: usize, body_size: usize, _num_trailers: usize) {
        log::info!("SP Cache: *** HTTP CALL RESPONSE RECEIVED *** token: {}, body_size: {}", token_id, body_size);
        log::info!("SP Cache: pending_cache_lookup = {:?}", self.pending_cache_lookup);
        log::info!("SP Cache: All headers from response:");
        let response_headers = self.get_http_call_response_headers();
        for (key, value) in &response_headers {
            log::info!("SP Cache:   {}: {}", key, value);
        }
        
        // Check if this is the response to our cache lookup call
        if let Some(pending_token) = self.pending_cache_lookup {
            if pending_token == token_id {
                log::info!("SP Cache: Processing cache lookup response");
                self.pending_cache_lookup = None;
                
                // Get response status
                let status_code = self.get_http_call_response_header(":status")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(500);
                
                log::info!("SP Cache: Response status: {}", status_code);
                
                if status_code == 200 {
                    // Cache hit - parse and return cached response
                    if body_size > 0 {
                        let response_body = self.get_http_call_response_body(0, body_size)
                            .unwrap_or_default();
                        log::info!("SP Cache: Cache hit! Received {} bytes", response_body.len());
                        
                        // Parse the OTEL response and extract cached HTTP response
                        match parse_otel_cache_response(&response_body) {
                            Ok(Some(cached_response)) => {
                                log::info!("SP Cache: Successfully parsed cached response, status: {}, {} headers, {} bytes body", 
                                    cached_response.status_code, cached_response.headers.len(), cached_response.body.len());
                                
                                // Convert headers to &str format
                                let headers_refs: Vec<(&str, &str)> = cached_response.headers.iter()
                                    .map(|(k, v)| (k.as_str(), v.as_str()))
                                    .collect();
                                
                                // Send cached response 
                                let body = if cached_response.body.is_empty() { None } else { Some(cached_response.body.as_slice()) };
                                self.send_http_response(cached_response.status_code, headers_refs, body);
                                
                                log::info!("SP Cache: Successfully returned cached response");
                                return; // Don't resume - we've handled the response
                            }
                            Ok(None) => {
                                log::warn!("SP Cache: 200 response but no cached data found");
                            }
                            Err(e) => {
                                log::error!("SP Cache: Failed to parse cache response: {}", e);
                            }
                        }
                    }
                } else {
                    log::info!("SP Cache: Cache miss (status: {})", status_code);
                }
                
                // Resume the paused request
                self.resume_http_request();
            }
        }
    }
}

impl HttpContext for SpCacheHttpContext {
    fn on_http_request_headers(&mut self, _num_headers: usize, end_of_stream: bool) -> Action {
        log::info!("SP Cache: Processing request headers");
        
        // Capture request headers
        for (key, value) in self.get_http_request_headers() {
            self.request_headers.insert(key, value);
        }

        // Update cache handler with trace context
        let headers_clone = self.request_headers.clone();
        self.cache_handler = CacheHandler::new().with_context(&headers_clone);
        
        // If this is the end of the stream (no body), perform cache lookup now
        if end_of_stream {
            log::info!("SP Cache: No request body, performing cache lookup immediately");
            match self.cache_handler.prepare_cache_lookup(&self.request_headers, &self.request_body) {
                Ok(call_data) => {
                    log::info!("SP Cache: Dispatching cache lookup HTTP call");
                    match self.dispatch_http_call(
                        "local_backend",
                        call_data.headers_as_refs(),
                        Some(&call_data.body),
                        vec![],
                        std::time::Duration::from_secs(30),
                    ) {
                        Ok(call_id) => {
                            log::info!("SP Cache: Cache lookup dispatched with call_id: {}, pausing request", call_id);
                            self.pending_cache_lookup = Some(call_id);
                            return Action::Pause; // MUST pause until we get the cache response
                        }
                        Err(e) => {
                            log::error!("SP Cache: Failed to dispatch cache lookup: {:?}, continuing to upstream", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("SP Cache: Cache lookup preparation error: {}, continuing to upstream", e);
                }
            }
        }
        
        Action::Continue
    }

    fn on_http_request_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        log::debug!("SP Cache: Processing request body, size: {}", body_size);
        
        // Buffer request body
        if let Some(body) = self.get_http_request_body(0, body_size) {
            self.request_body.extend_from_slice(&body);
        }

        if end_of_stream {
            // Perform async cache lookup
            match self.cache_handler.prepare_cache_lookup(&self.request_headers, &self.request_body) {
                Ok(call_data) => {
                    log::info!("SP Cache: Dispatching cache lookup HTTP call");
                    match self.dispatch_http_call(
                        "local_backend",
                        call_data.headers_as_refs(),
                        Some(&call_data.body),
                        vec![],
                        std::time::Duration::from_secs(30),
                    ) {
                        Ok(call_id) => {
                            log::info!("SP Cache: Cache lookup dispatched with call_id: {}, pausing request", call_id);
                            self.pending_cache_lookup = Some(call_id);
                            return Action::Pause; // MUST pause until we get the cache response
                        }
                        Err(e) => {
                            log::error!("SP Cache: Failed to dispatch cache lookup: {:?}, continuing to upstream", e);
                        }
                    }
                }
                Err(e) => {
                    log::error!("SP Cache: Cache lookup preparation error: {}, continuing to upstream", e);
                }
            }
        }

        Action::Pause
    }

    fn on_http_response_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        log::info!("SP Cache: Processing response headers");
        
        // Capture response headers
        for (key, value) in self.get_http_response_headers() {
            self.response_headers.insert(key, value);
        }
        
        Action::Continue
    }

    fn on_http_response_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        log::debug!("SP Cache: Processing response body, size: {}", body_size);
        
        // Buffer response body
        if let Some(body) = self.get_http_response_body(0, body_size) {
            self.response_body.extend_from_slice(&body);
        }

        if end_of_stream {
            // Check if response is successful (200) using already captured headers
            if let Some(status) = self.response_headers.get(":status") {
                if status == "200" {
                    log::info!("SP Cache: Successful response, storing in cache asynchronously");
                    
                    // Send to Softprobe asynchronously (fire and forget)
                    if let Err(e) = self.cache_handler.store_cache_async(
                        &self.request_headers,
                        &self.request_body,
                        &self.response_headers,
                        &self.response_body,
                    ) {
                        log::error!("SP Cache: Failed to store cache: {}", e);
                    }
                } else {
                    log::info!("SP Cache: Response status {} - not caching", status);
                }
            } else {
                log::warn!("SP Cache: No :status header found in response");
            }
        }

        Action::Continue
    }

}

// Helper function to parse OTEL cache response
fn parse_otel_cache_response(response_body: &[u8]) -> Result<Option<cache::CacheResponse>, cache::CacheError> {
    use prost::Message;
    use crate::otel::TracesData;
    
    log::debug!("SP Cache: Starting protobuf decode of {} bytes", response_body.len());
    
    // Decode OTEL protobuf response
    let traces_data = TracesData::decode(response_body)
        .map_err(|e| {
            log::error!("SP Cache: Protobuf decode failed: {}", e);
            cache::CacheError::SerializationError(e.to_string())
        })?;
    
    log::debug!("SP Cache: Successfully decoded protobuf, found {} resource spans", traces_data.resource_spans.len());
    
    // Extract cached HTTP response from span attributes
    for (i, resource_span) in traces_data.resource_spans.iter().enumerate() {
        log::debug!("SP Cache: Processing resource span {}, found {} scope spans", i, resource_span.scope_spans.len());
        for (j, scope_span) in resource_span.scope_spans.iter().enumerate() {
            log::debug!("SP Cache: Processing scope span {}, found {} spans", j, scope_span.spans.len());
            for (k, span) in scope_span.spans.iter().enumerate() {
                log::debug!("SP Cache: Processing span {}, name: '{}', {} attributes", k, span.name, span.attributes.len());
                // Look for cached response data in span attributes
                let mut status_code = 200u32;
                let mut headers = Vec::new();
                let mut body = Vec::new();
                
                for attr in &span.attributes {
                    match attr.key.as_str() {
                        "http.response.status_code" => {
                            if let Some(value) = &attr.value {
                                if let Some(crate::otel::any_value::Value::IntValue(code)) = &value.value {
                                    status_code = *code as u32;
                                }
                            }
                        }
                        key if key.starts_with("http.response.header.") => {
                            let header_name = &key[21..]; // Remove "http.response.header." prefix
                            if let Some(value) = &attr.value {
                                if let Some(crate::otel::any_value::Value::StringValue(header_value)) = &value.value {
                                    headers.push((header_name.to_string(), header_value.clone()));
                                }
                            }
                        }
                        "http.response.body" => {
                            if let Some(value) = &attr.value {
                                if let Some(crate::otel::any_value::Value::StringValue(body_str)) = &value.value {
                                    // Decode base64 if it's binary data, otherwise use as-is
                                    body = if is_base64_encoded(body_str) {
                                        use base64::{Engine as _, engine::general_purpose};
                                        general_purpose::STANDARD.decode(body_str)
                                            .unwrap_or_else(|_| body_str.as_bytes().to_vec())
                                    } else {
                                        body_str.as_bytes().to_vec()
                                    };
                                }
                            }
                        }
                        _ => {}
                    }
                }
                
                // If we found response data, return it
                if !body.is_empty() || !headers.is_empty() {
                    log::info!("SP Cache: Found cached response data in span '{}': status={}, {} headers, {} byte body", 
                        span.name, status_code, headers.len(), body.len());
                    return Ok(Some(cache::CacheResponse {
                        status_code,
                        headers,
                        body,
                    }));
                } else {
                    log::debug!("SP Cache: No cached response data found in span '{}'", span.name);
                }
            }
        }
    }
    
    log::debug!("SP Cache: No cached response found in any spans");
    Ok(None)
}

fn is_base64_encoded(s: &str) -> bool {
    // Simple heuristic: if string is longer than 100 chars and contains typical base64 chars
    s.len() > 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
}