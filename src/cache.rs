use std::collections::HashMap;

use crate::otel::{SpanBuilder, serialize_traces_data};
use crate::http_client::{HttpClient, HttpResponse};

#[derive(Debug, Clone)]
pub struct CacheResponse {
    pub status_code: u32,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub enum CacheError {
    HttpError(String),
    SerializationError(String),
    TimeoutError,
    InvalidResponse,
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            CacheError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            CacheError::TimeoutError => write!(f, "Timeout error"),
            CacheError::InvalidResponse => write!(f, "Invalid response"),
        }
    }
}

pub struct CacheHandler {
    http_client: HttpClient,
    softprobe_endpoint: String,
    span_builder: SpanBuilder,
}

impl CacheHandler {
    pub fn new() -> Self {
        Self {
            http_client: HttpClient::new(),
            softprobe_endpoint: "https://o.softprobe.ai".to_string(),
            span_builder: SpanBuilder::new(),
        }
    }

    pub fn with_context(mut self, headers: &HashMap<String, String>) -> Self {
        self.span_builder = self.span_builder.with_context(headers);
        self
    }

    pub fn lookup_cache_async(
        &mut self,
        request_headers: &HashMap<String, String>,
        request_body: &[u8],
    ) -> Result<u32, CacheError> {
        log::info!("SP Cache: Performing async cache lookup");

        // Create inject span for cache lookup
        let traces_data = self.span_builder.create_inject_span(request_headers, request_body);
        
        // Send OTEL span to Softprobe asynchronously
        let otel_data = serialize_traces_data(&traces_data)
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;
        
        let headers = vec![
            ("content-type".to_string(), "application/x-protobuf".to_string()),
            ("content-length".to_string(), otel_data.len().to_string()),
        ];
        
        // Dispatch async call for cache lookup
        self.http_client.dispatch_async_post(&self.softprobe_endpoint, headers, otel_data)
            .map_err(|e| CacheError::HttpError(e.to_string()))
    }

    pub fn store_cache_async(
        &mut self,
        request_headers: &HashMap<String, String>,
        request_body: &[u8],
        response_headers: &HashMap<String, String>,
        response_body: &[u8],
    ) -> Result<(), CacheError> {
        log::info!("SP Cache: Storing cache data asynchronously");
        
        // Create extract span
        let traces_data = self.span_builder.create_extract_span(
            request_headers,
            request_body,
            response_headers,
            response_body,
        );
        
        // Send extract span to Softprobe (fire and forget)
        self.send_otel_traces_async(&traces_data)
    }

    fn send_otel_traces(&mut self, traces_data: &crate::otel::TracesData) -> Result<HttpResponse, CacheError> {
        let otel_data = serialize_traces_data(traces_data)
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;
        
        let headers = vec![
            ("content-type".to_string(), "application/x-protobuf".to_string()),
            ("content-length".to_string(), otel_data.len().to_string()),
        ];
        
        self.http_client.post_sync(&self.softprobe_endpoint, headers, otel_data)
            .map_err(|e| CacheError::HttpError(e.to_string()))
    }

    fn send_otel_traces_async(&mut self, traces_data: &crate::otel::TracesData) -> Result<(), CacheError> {
        let otel_data = serialize_traces_data(traces_data)
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;
        
        let headers = vec![
            ("content-type".to_string(), "application/x-protobuf".to_string()),
            ("content-length".to_string(), otel_data.len().to_string()),
        ];
        
        // Fire and forget async call
        match self.http_client.dispatch_async_post(&self.softprobe_endpoint, headers, otel_data) {
            Ok(_call_id) => {
                log::info!("SP Cache: Async request dispatched successfully");
                Ok(())
            }
            Err(e) => {
                log::error!("SP Cache: Failed to dispatch async request: {}", e);
                Err(CacheError::HttpError(e.to_string()))
            }
        }
    }

}


fn is_text_content(headers: &HashMap<String, String>) -> bool {
    if let Some(content_type) = headers.get("content-type") {
        content_type.starts_with("text/") || 
        content_type.starts_with("application/json") ||
        content_type.starts_with("application/xml")
    } else {
        false
    }
}