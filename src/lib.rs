use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use std::collections::HashMap;
use url::Url;
use regex::Regex;

mod otel;

use crate::otel::{SpanBuilder, serialize_traces_data};


#[derive(Debug, Clone)]
pub struct CollectionRule {
    pub http: HttpCollectionRule,
}


#[derive(Debug, Clone)]
pub struct HttpCollectionRule {
    pub server: ServerConfig,
    pub client: Vec<ClientConfig>,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub host: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub sp_backend_url: String,
    pub enable_inject: bool,
    pub service_name: String,       // 添加service_name字段
    pub traffic_direction: String,  // "inbound" 或 "outbound"
    pub collection_rules: Vec<CollectionRule>,
    pub api_key: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sp_backend_url: "http://o.softprobe.ai".to_string(),
            enable_inject: false,
            traffic_direction: "outbound".to_string(),
            service_name: "default-service".to_string(), // 默认服务名
            collection_rules: vec![],
            api_key: String::new(), // 默认空字符串
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub status_code: u32,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

// Main entry point for the WASM module
proxy_wasm::main! {{
    proxy_wasm::set_log_level(LogLevel::Debug);
    proxy_wasm::set_root_context(|_| -> Box<dyn RootContext> {
        Box::new(SpRootContext::new())
    });
}}

struct SpRootContext {
    config: Config,
}

impl SpRootContext {
    fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }
}

impl Context for SpRootContext {}

impl RootContext for SpRootContext {
    fn get_type(&self) -> Option<ContextType> {
        Some(ContextType::HttpContext)
    }

    fn create_http_context(&self, context_id: u32) -> Option<Box<dyn HttpContext>> {
        Some(Box::new(SpHttpContext::new(context_id, self.config.clone())))
    }

    fn on_configure(&mut self, _plugin_configuration_size: usize) -> bool {
        if let Some(config_bytes) = self.get_plugin_configuration() {
            if let Ok(config_str) = std::str::from_utf8(&config_bytes) {
                if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(config_str) {
                    // 解析现有配置
                    if let Some(backend_url) = config_json.get("sp_backend_url").and_then(|v| v.as_str()) {
                        self.config.sp_backend_url = backend_url.to_string();
                        log::info!("SP: Configured backend URL: {}", self.config.sp_backend_url);
                    }

                    // 解析 enable_inject
                    if let Some(enable_inject) = config_json.get("enable_inject").and_then(|v| v.as_bool()) {
                        self.config.enable_inject = enable_inject;
                        log::info!("SP: Configured injection enabled: {}", self.config.enable_inject);
                    }

                    // 解析 traffic_direction
                    if let Some(direction) = config_json.get("traffic_direction").and_then(|v| v.as_str()) {
                        self.config.traffic_direction = direction.to_string();
                        log::info!("SP: Configured traffic direction: {}", self.config.traffic_direction);
                    }

                    // 解析 service_name
                    if let Some(service_name) = config_json.get("service_name").and_then(|v| v.as_str()) {
                        self.config.service_name = service_name.to_string();
                        log::info!("SP: Configured service name: {}", self.config.service_name);
                    }
                    if let Some(api_key) = config_json.get("api_key").and_then(|v| v.as_str()) {
                        self.config.api_key = api_key.to_string();
                        log::info!("SP: Configured API key: {}", self.config.api_key);
                    }


                    // 解析 collectionRules
                    if let Some(rules) = config_json.get("collectionRules") {
                        // 解析 server paths
                        let mut server_paths = Vec::new();
                        if let Some(server_obj) = rules.get("http").and_then(|v| v.get("server")) {
                            if let Some(server_array) = server_obj.as_array() {
                                for server_entry in server_array {
                                    if let Some(path) = server_entry.get("path").and_then(|v| v.as_str()) {
                                        server_paths.push(path.to_string());
                                    }
                                }
                            }
                        }

                        // 解析 client configs
                        let mut client_configs = Vec::new();
                        if let Some(client_obj) = rules.get("http").and_then(|v| v.get("client")) {
                            if let Some(client_array) = client_obj.as_array() {
                                for client_entry in client_array {
                                    if let Some(host) = client_entry.get("host").and_then(|v| v.as_str()) {
                                        let mut paths = Vec::new();
                                        if let Some(paths_obj) = client_entry.get("paths") {
                                            if let Some(paths_array) = paths_obj.as_array() {
                                                for path_entry in paths_array {
                                                    if let Some(path) = path_entry.as_str() {
                                                        paths.push(path.to_string());
                                                    }
                                                }
                                            }
                                        }
                                        client_configs.push((host.to_string(), paths));
                                    }
                                }
                            }
                        }

                        // 为每个 server_path 创建独立的规则
                        for server_path in server_paths {
                            log::info!("SP: Added server collection rule - serverPath: {}", server_path);
                            self.config.collection_rules.push(CollectionRule {
                                http: HttpCollectionRule {
                                    server: ServerConfig {
                                        path: server_path.to_string(),
                                    },
                                    client: vec![], // 没有客户端配置
                                },
                            });
                        }

                        // 为每个 client 配置创建独立的规则
                        for (client_host, client_paths) in &client_configs {
                            log::info!("SP: Added client collection rule - clientHost: {}, clientPaths: {:?}",
                                      client_host, client_paths);
                            self.config.collection_rules.push(CollectionRule {
                                http: HttpCollectionRule {
                                    server: ServerConfig {
                                        path: String::new(), // 空字符串表示这是客户端规则
                                    },
                                    client: vec![
                                        ClientConfig {
                                            host: client_host.clone(),
                                            paths: client_paths.clone(),
                                        }
                                    ],
                                },
                            });
                        }
                    }
                }
            }
        }
        true
    }

}

struct SpHttpContext {
    _context_id: u32,
    request_headers: HashMap<String, String>,
    request_body: Vec<u8>,
    response_headers: HashMap<String, String>,
    response_body: Vec<u8>,
    span_builder: SpanBuilder,
    pending_inject_call_token: Option<u32>,
    injected: bool,
    config: Config,
    url_host: Option<String>,
    url_path: Option<String>,
}

impl SpHttpContext {
    fn new(context_id: u32, config: Config) -> Self {
        let mut span_builder = SpanBuilder::new();
        span_builder = span_builder
            .with_service_name(config.service_name.clone())
            .with_traffic_direction(config.traffic_direction.clone());
        Self {
            _context_id: context_id,
            config: config,
            request_headers: HashMap::new(),
            request_body: Vec::new(),
            response_headers: HashMap::new(),
            response_body: Vec::new(),
            span_builder: span_builder,
            pending_inject_call_token: None,
            injected: false,
            url_host: None,
            url_path: None,
        }
    }

    fn update_url_info(&mut self) {
        // url.path from property system, fallback to :path header
        if let Some(prop) = self.get_property(vec!["request", "path"]) {
            if let Ok(path) = String::from_utf8(prop) {
                if !path.is_empty() {
                    self.url_path = Some(path);
                }
            }
        }
        if self.url_path.is_none() {
            if let Some(path_hdr) = self.request_headers.get(":path") {
                self.url_path = Some(path_hdr.clone());
            }
        }

        // url.host from :authority or host header
        let authority_or_host = self
            .request_headers
            .get(":authority")
            .cloned()
            .or_else(|| self.request_headers.get("host").cloned());

        // Keep port if present (use raw header value)
        if let Some(authority_value) = authority_or_host {
            if !authority_value.is_empty() {
                self.url_host = Some(authority_value);
            }
        }
    }

    fn should_collect_by_rules(&self) -> bool {
        // 如果没有配置规则，则默认采集所有请求
        if self.config.collection_rules.is_empty() {
            log::info!("SP: No collection rules configured, collecting all requests");
            return true;
        }
        // 根据流量方向应用不同规则
        match self.config.traffic_direction.as_str() {
            "inbound" => {
                log::info!("SP: process inbound");
                if let Some(request_path) = self.request_headers.get(":path") {
                    // 获取当前请求路径
                    log::debug!("SP: Checking collection rules for path: {}, traffic_direction: {}",request_path, self.config.traffic_direction);
                    // Server 流量：只匹配 server_path
                    log::debug!("SP: Processing inbound traffic rules, request_path: {}", request_path);
                    for (i, rule) in self.config.collection_rules.iter().enumerate() {
                        // 检查规则是否为服务器规则（path不为空）
                        if !rule.http.server.path.is_empty() {
                            log::debug!("SP: Checking inbound rule {}: serverPath='{}'", i, rule.http.server.path);
                            if self.match_pattern(&rule.http.server.path, request_path) {
                                log::debug!("SP: Inbound request matched server_path: {}", rule.http.server.path);
                                return true;
                            }
                        }
                    }
                    log::info!("SP: No inbound rules matched for path: {}", request_path);
                    return false;
                }

            }
            "outbound" => {
                log::info!("SP: process outbound");
                // Client 流量：需要匹配 client_host 和 client_paths
                let (client_host, client_path) = self.extract_client_info();

                for (i, rule) in self.config.collection_rules.iter().enumerate() {
                    // 处理客户端规则（检查client数组不为空）
                    if !rule.http.client.is_empty() {
                        for client_config in &rule.http.client {
                            log::debug!("SP: Checking outbound rule {}: clientHost={}, clientPaths={:?}",
                                       i, client_config.host, client_config.paths);

                            // 检查 client_host
                            if let Some(ref actual_client_host) = client_host {
                                if !self.match_pattern(&client_config.host, actual_client_host) {
                                    log::debug!("SP: Client host did not match: expected={}, actual={}",
                                               client_config.host, actual_client_host);
                                    continue;
                                }
                            } else {
                                log::debug!("SP: No client host info available, but rule requires it");
                                continue;
                            }

                            log::debug!("SP: Client host matched");

                            // 检查 client_paths（如果配置了）
                            if !client_config.paths.is_empty() {
                                log::debug!("SP: Rule requires client paths: {:?}", client_config.paths);
                                let client_path_matched = if let Some(ref actual_client_path) = client_path {
                                    let matched = client_config.paths.iter().any(|client_path| {
                                        let matches = self.match_pattern(client_path, actual_client_path);
                                        log::debug!("SP: Client path match check: pattern='{}', text='{}', result={}",
                                                   client_path, actual_client_path, matches);
                                        matches
                                    });
                                    log::debug!("SP: Client path matched any: {}", matched);
                                    matched
                                } else {
                                    log::debug!("SP: No client path info available");
                                    false
                                };

                                if !client_path_matched {
                                    log::info!("SP: Client paths did not match");
                                    continue;
                                }
                            } else {
                                log::info!("SP: Rule does not require client paths");
                            }

                            log::info!("SP: Outbound request matched all criteria - client_host: {}, client_paths: {:?}",
                                      client_config.host, client_config.paths);
                            return true;
                        }
                    }
                }

                // 检查是否有任何适用的客户端规则
                let has_client_rules = self.config.collection_rules.iter().any(|rule| !rule.http.client.is_empty());
                if has_client_rules {
                    log::info!("SP: No outbound rules matched, not collecting");
                    return false;
                } else {
                    log::info!("SP: No client rules configured, collecting all outbound requests");
                    return true;
                }
            }
            _ => {
                log::warn!("SP: Unknown traffic direction: {}", self.config.traffic_direction);
                return false;
            }

        }

        // 无法确定路径，默认不采集
        log::warn!("SP: Unable to determine request path, not collecting");
        false
    }


    // 提取客户端信息
    fn extract_client_info(&self) -> (Option<String>, Option<String>) {
        let mut client_host = None;
        let mut client_path = None;

        // 从 Referer 头部提取
        if let Some(referer) = self.request_headers.get("referer") {
            log::info!("SP: Found referer header: {}", referer);
            if let Ok(url) = url::Url::parse(referer) {
                client_host = url.host_str().map(|h| h.to_string());
                client_path = Some(url.path().to_string());
                log::info!("SP: Parsed from referer - host: {:?}, path: {:?}", client_host, client_path);
            } else {
                log::info!("SP: Failed to parse referer as URL: {}", referer);
            }
        }

        // 从 Origin 头部提取（如果 Referer 不存在）
        if client_host.is_none() {
            if let Some(origin) = self.request_headers.get("origin") {
                log::info!("SP: Found origin header: {}", origin);
                if let Ok(url) = url::Url::parse(origin) {
                    client_host = url.host_str().map(|h| h.to_string());
                    log::info!("SP: Parsed from origin - host: {:?}", client_host);
                } else {
                    log::info!("SP: Failed to parse origin as URL: {}", origin);
                }
            }
        }

        // 从 Host 头部提取（作为备选）
        if client_host.is_none() {
            if let Some(host) = self.request_headers.get("host") {
                log::info!("SP: Found host header: {}", host);
                // 尝试从 host 提取主机名（可能包含端口）
                if let Ok(url) = url::Url::parse(&format!("http://{}", host)) {
                    client_host = url.host_str().map(|h| h.to_string());
                    log::info!("SP: Parsed from host - host: {:?}", client_host);
                }
            }
        }
        // 从 Host 头部获取客户端域名
        if client_host.is_none() {
            client_host = self.request_headers.get("host")
                .or_else(|| self.request_headers.get(":authority"))
                .cloned();
        }


        // 直接从请求路径获取客户端路径
        if client_path.is_none() {
            client_path = self.request_headers.get(":path").cloned();
        }

        log::info!("SP: Final client info - host: {:?}, path: {:?}", client_host, client_path);
        (client_host, client_path)
    }

    // 使用正则表达式匹配
    fn match_pattern(&self, pattern: &str, text: &str) -> bool {
        log::debug!("SP: Matching pattern '{}' against text '{}'", pattern, text);
        match Regex::new(pattern) {
            Ok(re) => {
                let result = re.is_match(text);
                log::debug!("SP: Regex match result: {}", result);
                result
            },
            Err(e) => {
                log::warn!("SP: Invalid regex pattern '{}': {}", pattern, e);
                // 如果正则表达式无效，回退到精确匹配
                let result = pattern == text;
                log::debug!("SP: Fallback to exact match: {}", result);
                result
            }
        }
    }

    fn get_backend_authority(&self) -> String {
        match Url::parse(&self.config.sp_backend_url) {
            Ok(url) => {
                if let Some(host) = url.host_str() {
                    match url.port() {
                        Some(port) => format!("{}:{}", host, port),
                        None => {
                            let default_port = match url.scheme() {
                                "https" => 443,
                                "http" => 80,
                                _ => 80,
                            };
                            format!("{}:{}", host, default_port)
                        }
                    }
                } else {
                    "o.softprobe.ai".to_string()
                }
            }
            Err(_) => "o.softprobe.ai".to_string(),
        }
    }

    // Dispatch injection HTTP call directly using context's dispatch_http_call method
    fn dispatch_injection_lookup(&mut self) -> Result<u32, String> {
        if !self.config.enable_inject {
            return Err("Injection is not allowed".to_string());
        }

        log::info!("SP Injection: Preparing injection lookup data");

        // Create inject span for injection lookup using references to avoid cloning
        let traces_data = self.span_builder.create_inject_span(
            &self.request_headers,
            &self.request_body,
            self.url_host.as_deref(),
            self.url_path.as_deref(),
        );

        // Serialize to protobuf
        let otel_data = serialize_traces_data(&traces_data)
            .map_err(|e| format!("Serialization error: {}", e))?;

        // Get backend authority from configured URL
        let authority = self.get_backend_authority();

        // Prepare HTTP headers for the injection lookup call
        let content_length = otel_data.len().to_string();
        let http_headers = vec![
            (":method", "POST"),
            (":path", "/v1/inject"),
            (":authority", &authority),
            ("content-type", "application/x-protobuf"),
            ("content-length", &content_length),
        ];

        log::info!("SP Injection: Dispatching injection lookup call, body size: {}", otel_data.len());

        // Use the context's dispatch_http_call method to maintain context
        match self.dispatch_http_call(
            "sp_backend",
            http_headers,
            Some(&otel_data),
            vec![],
            std::time::Duration::from_secs(30),
        ) {
            Ok(call_id) => {
                log::info!("SP Injection: Injection lookup dispatched with call_id: {}", call_id);
                Ok(call_id)
            }
            Err(e) => {
                log::error!("SP Injection: Failed to dispatch injection lookup: {:?}", e);
                Err(format!("Dispatch failed: {:?}", e))
            }
        }
    }

    // Dispatch async call to save extracted data
    fn dispatch_async_extraction_save(&mut self) -> Result<(), String> {
        // 检查采集规则
        if !self.should_collect_by_rules() {
            log::info!("SP: Data extraction skipped based on collection rules");
            return Err("Data collection skipped based on collection rules".to_string());
        }
        log::info!("SP: Storing agent data asynchronously");

        // Create extract span using references to avoid cloning
        let traces_data = self.span_builder.create_extract_span(
            &self.request_headers,
            &self.request_body,
            &self.response_headers,
            &self.response_body,
            self.url_host.as_deref(),
            self.url_path.as_deref(),
        );

        // Serialize to protobuf
        let otel_data = serialize_traces_data(&traces_data)
            .map_err(|e| format!("Serialization error: {}", e))?;

        // Get backend authority from configured URL
        let authority = self.get_backend_authority();

        // Prepare HTTP headers for the async save call
        let content_length = otel_data.len().to_string();
        let http_headers = vec![
            (":method", "POST"),
            (":path", "/v1/traces"),
            (":authority", &authority),
            ("content-type", "application/x-protobuf"),
            ("content-length", &content_length),
        ];

        log::info!("SP Extraction: Dispatching async save call, body size: {}", otel_data.len());

        // Fire and forget async call to /v1/traces endpoint for storage
        match self.dispatch_http_call(
            "sp_backend",
            http_headers,
            Some(&otel_data),
            vec![],
            std::time::Duration::from_secs(30),
        ) {
            Ok(call_id) => {
                log::info!("SP Extraction: Async save dispatched with call_id: {}", call_id);
                Ok(())
            }
            Err(e) => {
                log::error!("SP Extraction: Failed to dispatch async save: {:?}", e);
                Err(format!("Async save failed: {:?}", e))
            }
        }
    }
}

impl Context for SpHttpContext {

    fn on_http_call_response(&mut self, token_id: u32, _num_headers: usize, body_size: usize, _num_trailers: usize) {
        log::info!("SP: *** HTTP CALL RESPONSE RECEIVED *** token: {}, body_size: {}", token_id, body_size);
        log::info!("SP: pending_inject_call_token = {:?}", self.pending_inject_call_token);
        log::info!("SP: All headers from response:");
        let response_headers = self.get_http_call_response_headers();
        for (key, value) in &response_headers {
            log::info!("SP:   {}: {}", key, value);
        }

        // Check if this is the response to our agent lookup call
        if let Some(pending_token) = self.pending_inject_call_token {
            if pending_token == token_id {
                log::info!("SP: Processing injection lookup response");
                self.pending_inject_call_token = None;
                // Get response status
                let status_code = self.get_http_call_response_header(":status")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(500);

                log::info!("SP: Injection response status: {}", status_code);

                if status_code == 200 {
                    // Injection hit - parse and return injection response
                    if body_size > 0 {
                        let response_body = self.get_http_call_response_body(0, body_size)
                            .unwrap_or_default();
                        log::info!("SP: Received {} bytes for injection", response_body.len());

                        // Parse the OTEL response and extract agentd HTTP response
                        match parse_otel_injection_response(&response_body) {
                            Ok(Some(injected_response)) => {
                                log::info!("SP: Successfully parsed injection response, status: {}, {} headers, {} bytes body",
                                    injected_response.status_code, injected_response.headers.len(), injected_response.body.len());

                                // Convert headers to &str format
                                let headers_refs: Vec<(&str, &str)> = injected_response.headers.iter()
                                    .map(|(k, v)| (k.as_str(), v.as_str()))
                                    .collect();

                                // Send agentd response
                                let body = if injected_response.body.is_empty() { None } else { Some(injected_response.body.as_slice()) };
                                self.send_http_response(injected_response.status_code, headers_refs, body);

                                log::info!("SP: Successfully injected response");
                                return; // Don't resume - we've handled the response
                            }
                            Ok(None) => {
                                log::warn!("SP: 200 Injection response but no injection data found");
                            }
                            Err(e) => {
                                log::error!("SP: Failed to parse injection response: {}", e);
                            }
                        }
                    }
                } else {
                    log::info!("SP: No data for injection (status: {})", status_code);
                }

                // Resume the paused request
                self.resume_http_request();
            }
        }
    }
}

impl HttpContext for SpHttpContext {
    fn on_http_request_headers(&mut self, _num_headers: usize, end_of_stream: bool) -> Action {
        log::debug!("SP: Processing request headers");

        // Capture request headers
        for (key, value) in self.get_http_request_headers() {
            self.request_headers.insert(key, value);
        }

        let traffic_direction = self.config.traffic_direction.clone() ;
        let service_name = self.config.service_name.clone();
        let api_key = self.config.api_key.clone();

        log::info!("DEBUG: Before span builder update - service_name: '{}', traffic_direction: '{}', api_key: '{}'",
                   service_name, traffic_direction, api_key);

        // Update url.host and url.path from properties/headers
        self.update_url_info();

        // Update span builder with trace context and session ID
        let headers_clone = self.request_headers.clone();
        log::info!("DEBUG: Available headers: {:?}", headers_clone.keys().collect::<Vec<_>>());

        self.span_builder = self.span_builder.clone()
            .with_service_name(service_name)
            .with_traffic_direction(traffic_direction)
            .with_api_key(api_key)
            .with_context(&headers_clone);

        // If this is the end of the stream (no body), perform injection lookup now
        if end_of_stream {
            log::info!("SP Injection: No request body, performing injection lookup immediately");
            match self.dispatch_injection_lookup() {
                Ok(call_id) => {
                    log::info!("SP Injection: Injection lookup dispatched with call_id: {}, pausing request", call_id);
                    self.pending_inject_call_token = Some(call_id);
                    return Action::Pause; // MUST pause until we get the injection response
                }
                Err(e) => {
                    log::error!("SP Injection: Injection lookup error: {}, continuing to upstream", e);
                }
            }
        }

        Action::Continue
    }

    fn on_http_request_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        log::debug!("SP: Processing request body, size: {}", body_size);

        // Buffer request body
        if let Some(body) = self.get_http_request_body(0, body_size) {
            self.request_body.extend_from_slice(&body);
        }

        if end_of_stream {
            // Perform async injection lookup
            match self.dispatch_injection_lookup() {
                Ok(call_id) => {
                    log::info!("SP Injection: Injection lookup dispatched with call_id: {}, pausing request", call_id);
                    self.pending_inject_call_token = Some(call_id);
                    return Action::Pause; // MUST pause until we get the injection response
                }
                Err(e) => {
                    log::error!("SP Injection: Injection lookup error: {}, continuing to upstream", e);
                }
            }
        }

        Action::Continue
    }

    fn on_http_response_headers(&mut self, _num_headers: usize, _end_of_stream: bool) -> Action {
        log::debug!("SP: Processing response headers");

        // Don't extract injected data
        if self.injected {
            return Action::Continue
        }

        // Capture response headers
        for (key, value) in self.get_http_response_headers() {
            self.response_headers.insert(key, value);
        }

        Action::Continue
    }

    fn on_http_response_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        log::debug!("SP: Processing response body, size: {}", body_size);

        // Don't extract injected data
        if self.injected {
            return Action::Continue
        }

        // Buffer response body
        if let Some(body) = self.get_http_response_body(0, body_size) {
            self.response_body.extend_from_slice(&body);
        }

        if end_of_stream {
            // Check if response is successful (200) using already captured headers
            if let Some(status) = self.response_headers.get(":status") {
                if status == "200" {
                    log::debug!("SP: Successful response, storing in agent asynchronously");
                    // Send to Softprobe asynchronously (fire and forget)
                    if let Err(e) = self.dispatch_async_extraction_save() {
                        log::error!("SP: Failed to store agent: {}", e);
                    }
                } else {
                    log::debug!("SP: Response status {} - not caching", status);
                }
            } else {
                log::warn!("SP: No :status header found in response");
            }
        }

        Action::Continue
    }
}

// Helper function to parse OTEL agent response
fn parse_otel_injection_response(response_body: &[u8]) -> Result<Option<AgentResponse>, String> {
    use prost::Message;
    use crate::otel::TracesData;

    log::info!("SP: Starting protobuf decode of {} bytes", response_body.len());

    // Decode OTEL protobuf response
    let traces_data = TracesData::decode(response_body)
        .map_err(|e| {
            log::error!("SP: Protobuf decode failed: {}", e);
            format!("Serialization error: {}", e)
        })?;

    log::info!("SP: Successfully decoded protobuf, found {} resource spans", traces_data.resource_spans.len());

    // Extract agentd HTTP response from span attributes
    for (i, resource_span) in traces_data.resource_spans.iter().enumerate() {
        log::debug!("SP: Processing resource span {}, found {} scope spans", i, resource_span.scope_spans.len());
        for (j, scope_span) in resource_span.scope_spans.iter().enumerate() {
            log::debug!("SP: Processing scope span {}, found {} spans", j, scope_span.spans.len());
            for (k, span) in scope_span.spans.iter().enumerate() {
                log::debug!("SP: Processing span {}, name: '{}', {} attributes", k, span.name, span.attributes.len());
                // Look for agentd response data in span attributes
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
                    log::info!("SP: Found agentd response data in span '{}': status={}, {} headers, {} byte body",
                        span.name, status_code, headers.len(), body.len());
                    return Ok(Some(AgentResponse {
                        status_code,
                        headers,
                        body,
                    }));
                } else {
                    log::info!("SP: No agentd response data found in span '{}'", span.name);
                }
            }
        }
    }

    log::info!("SP: No agentd response found in any spans");
    Ok(None)
}

fn is_base64_encoded(s: &str) -> bool {
    // Simple heuristic: if string is longer than 100 chars and contains typical base64 chars
    s.len() > 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
}
