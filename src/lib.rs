use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use std::collections::HashMap;

mod cache;
mod otel;
mod http_client;

use cache::CacheHandler;

proxy_wasm::main! {{
    proxy_wasm::set_log_level(LogLevel::Info);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> {
        Box::new(SpCacheRoot)
    });
}}

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

struct SpCacheHttpContext {
    request_headers: HashMap<String, String>,
    request_body: Vec<u8>,
    response_headers: HashMap<String, String>,
    response_body: Vec<u8>,
    cache_handler: CacheHandler,
    pending_cache_lookup: Option<u32>, // Track cache lookup call token
}

impl SpCacheHttpContext {
    fn new() -> Self {
        Self {
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
        log::info!("SP Cache: Received HTTP call response for token: {}", token_id);
        
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
                    // Cache hit - parse OTEL protobuf response
                    let response_body = self.get_http_call_response_body(0, body_size)
                        .unwrap_or_default();
                    
                    match parse_otel_cache_response(&response_body) {
                        Ok(Some(cache_response)) => {
                            log::info!("SP Cache: Cache hit, returning cached response");
                            let headers_ref: Vec<(&str, &str)> = cache_response.headers
                                .iter()
                                .map(|(k, v)| (k.as_str(), v.as_str()))
                                .collect();
                            
                            self.send_http_response(
                                cache_response.status_code,
                                headers_ref,
                                Some(&cache_response.body),
                            );
                            return; // Don't continue to upstream
                        }
                        Ok(None) => {
                            log::info!("SP Cache: No cached response found in OTEL data, continuing to upstream");
                        }
                        Err(e) => {
                            log::error!("SP Cache: Failed to parse OTEL response: {}, continuing to upstream", e);
                        }
                    }
                } else if status_code == 404 {
                    log::info!("SP Cache: Cache miss, continuing to upstream");
                } else {
                    log::warn!("SP Cache: Unexpected response status: {}, continuing to upstream", status_code);
                }
                
                // Resume the paused request to continue to upstream
                self.resume_http_request();
            }
        }
    }
}

impl HttpContext for SpCacheHttpContext {
    fn on_http_request_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        log::info!("SP Cache: Processing request headers");
        
        // Capture request headers
        for (key, value) in self.get_http_request_headers() {
            self.request_headers.insert(key, value);
        }

        // Update cache handler with trace context
        let headers_clone = self.request_headers.clone();
        self.cache_handler = CacheHandler::new().with_context(&headers_clone);
        
        Action::Continue
    }

    fn on_http_request_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        log::info!("SP Cache: Processing request body, size: {}", body_size);
        
        // Buffer request body
        if let Some(body) = self.get_http_request_body(0, body_size) {
            self.request_body.extend_from_slice(&body);
        }

        if end_of_stream {
            // Perform async cache lookup
            match self.cache_handler.lookup_cache_async(&self.request_headers, &self.request_body) {
                Ok(call_id) => {
                    log::info!("SP Cache: Cache lookup dispatched with call_id: {}, pausing request", call_id);
                    self.pending_cache_lookup = Some(call_id);
                    return Action::Pause; // Pause until we get the cache response
                }
                Err(e) => {
                    log::error!("SP Cache: Cache lookup error: {}, continuing to upstream", e);
                }
            }
        }

        Action::Continue
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
        log::info!("SP Cache: Processing response body, size: {}", body_size);
        
        // Buffer response body
        if let Some(body) = self.get_http_response_body(0, body_size) {
            self.response_body.extend_from_slice(&body);
        }

        if end_of_stream {
            // Check if response is successful (200)
            if let Some(status) = self.get_http_response_header(":status") {
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
                }
            }
        }

        Action::Continue
    }

}

// Helper function to parse OTEL cache response
fn parse_otel_cache_response(response_body: &[u8]) -> Result<Option<cache::CacheResponse>, cache::CacheError> {
    use prost::Message;
    use crate::otel::TracesData;
    
    // Decode OTEL protobuf response
    let traces_data = TracesData::decode(response_body)
        .map_err(|e| cache::CacheError::SerializationError(e.to_string()))?;
    
    // Extract cached HTTP response from span attributes
    for resource_span in &traces_data.resource_spans {
        for scope_span in &resource_span.scope_spans {
            for span in &scope_span.spans {
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
                    return Ok(Some(cache::CacheResponse {
                        status_code,
                        headers,
                        body,
                    }));
                }
            }
        }
    }
    
    Ok(None)
}

fn is_base64_encoded(s: &str) -> bool {
    // Simple heuristic: if string is longer than 100 chars and contains typical base64 chars
    s.len() > 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
}