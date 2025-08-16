use std::time::Duration;
use proxy_wasm::types::Status;

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status_code: u32,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[derive(Debug)]
pub enum HttpError {
    DispatchError(Status),
    TimeoutError,
    ParseError(String),
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpError::DispatchError(status) => write!(f, "Dispatch error: {:?}", status),
            HttpError::TimeoutError => write!(f, "Request timeout"),
            HttpError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

pub struct HttpClient;

impl HttpClient {
    pub fn new() -> Self {
        Self
    }

    pub fn dispatch_async_post(
        &self,
        url: &str,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Result<u32, HttpError> {
        log::info!("SP Cache: Dispatching async POST to {}", url);
        
        // Convert headers to the format expected by dispatch_http_call
        let header_vec: Vec<(&str, &str)> = headers
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        
        // We only support calls to the Softprobe endpoint
        if !url.starts_with("https://o.softprobe.ai") {
            log::error!("SP Cache: Unsupported URL: {}", url);
            return Err(HttpError::DispatchError(Status::BadArgument));
        }
        
        let upstream_name = "outbound|443||o.softprobe.ai";
        
        // Create proper HTTP/2 headers for external HTTPS requests
        let mut http_headers = vec![
            (":method", "POST"),
            (":path", "/v1/traces"), // OpenTelemetry traces endpoint
            (":scheme", "https"),
            (":authority", "o.softprobe.ai"),
        ];
        
        // Add custom headers (avoid duplicate host header)
        for (key, value) in &header_vec {
            if key.to_lowercase() != "host" {
                http_headers.push((key, value));
            }
        }
        
        log::debug!("SP Cache: Using upstream: {}, headers: {:?}", upstream_name, http_headers);
        log::debug!("SP Cache: Request body size: {} bytes", body.len());
        log::debug!("SP Cache: Timeout: 30 seconds");
        
        // Dispatch the HTTP call
        log::debug!("SP Cache: About to call dispatch_http_call...");
        match proxy_wasm::hostcalls::dispatch_http_call(
            upstream_name,
            http_headers,
            Some(&body),
            vec![], // no trailers
            Duration::from_secs(30),
        ) {
            Ok(call_id) => {
                log::debug!("SP Cache: HTTP call dispatched successfully with ID: {}", call_id);
                Ok(call_id)
            }
            Err(status) => {
                log::error!("SP Cache: Failed to dispatch HTTP call: {:?}", status);
                Err(HttpError::DispatchError(status))
            }
        }
    }

    // Synchronous POST method (not used in WASM context, kept for completeness)
    pub fn post_sync(
        &self,
        _url: &str,
        _headers: Vec<(String, String)>,
        _body: Vec<u8>,
    ) -> Result<HttpResponse, HttpError> {
        // Synchronous HTTP calls are not supported in WASM context
        // This method is kept for interface compatibility but should not be used
        log::error!("SP Cache: Synchronous HTTP calls not supported in WASM context");
        Err(HttpError::ParseError("Synchronous calls not supported in WASM".to_string()))
    }
}