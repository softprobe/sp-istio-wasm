use proxy_wasm::traits::*;
use proxy_wasm::types::*;
use regex::Regex;
use std::collections::HashMap;
use url::Url;

mod otel;

use crate::otel::{serialize_traces_data, SpanBuilder};

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
pub struct ExemptionRule {
    pub host_patterns: Vec<String>,
    pub path_patterns: Vec<String>,
}

impl Default for ExemptionRule {
    fn default() -> Self {
        Self {
            host_patterns: vec![],
            path_patterns: vec![
                "/v1/traces".to_string(),
                "/api/traces".to_string(),
                "/v1/metrics".to_string(),
                "/api/metrics".to_string(),
                "/v1/logs".to_string(),
                "/api/logs".to_string(),
                "/otlp/v1/traces".to_string(),
                "/otlp/v1/metrics".to_string(),
                "/otlp/v1/logs".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub sp_backend_url: String,
    //pub enable_inject: bool,
    pub service_name: String,              // 添加service_name字段
    pub traffic_direction: Option<String>, // 改为可选字段
    pub collection_rules: Vec<CollectionRule>,
    pub exemption_rules: Vec<ExemptionRule>, // 添加豁免规则字段
    pub api_key: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sp_backend_url: "https://o.softprobe.ai".to_string(),
            // enable_inject: false,
            traffic_direction: None, // 默认为 None，表示自动检测
            service_name: "default-service".to_string(), // 默认服务名
            collection_rules: vec![],
            exemption_rules: vec![], // 默认空的豁免规则
            api_key: String::new(),  // 默认空字符串
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
        Some(Box::new(SpHttpContext::new(
            context_id,
            self.config.clone(),
        )))
    }

    fn on_configure(&mut self, _plugin_configuration_size: usize) -> bool {
        if let Some(config_bytes) = self.get_plugin_configuration() {
            if let Ok(config_str) = std::str::from_utf8(&config_bytes) {
                if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(config_str) {
                    // 解析现有配置
                    if let Some(backend_url) =
                        config_json.get("sp_backend_url").and_then(|v| v.as_str())
                    {
                        self.config.sp_backend_url = backend_url.to_string();
                        log::info!("SP: Configured backend URL: {}", self.config.sp_backend_url);
                    }

                  /* // 解析 enable_inject
                    if let Some(enable_inject) =
                        config_json.get("enable_inject").and_then(|v| v.as_bool())
                    {
                        self.config.enable_inject = enable_inject;
                        log::info!(
                            "SP: Configured injection enabled: {}",
                            self.config.enable_inject
                        );
                    } */ 

                    // 解析 traffic_direction
                    if let Some(direction) = config_json
                        .get("traffic_direction")
                        .and_then(|v| v.as_str())
                    {
                        self.config.traffic_direction = Some(direction.to_string());
                        log::info!(
                            "SP: Configured traffic direction: {:?}",
                            self.config.traffic_direction
                        );
                    }

                    // 解析 service_name
                    if let Some(service_name) =
                        config_json.get("service_name").and_then(|v| v.as_str())
                    {
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
                                    if let Some(path) =
                                        server_entry.get("path").and_then(|v| v.as_str())
                                    {
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
                                    if let Some(host) =
                                        client_entry.get("host").and_then(|v| v.as_str())
                                    {
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
                            log::info!(
                                "SP: Added server collection rule - serverPath: {}",
                                server_path
                            );
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
                                    client: vec![ClientConfig {
                                        host: client_host.clone(),
                                        paths: client_paths.clone(),
                                    }],
                                },
                            });
                        }
                    }

                    // 解析 exemptionRules
                    if let Some(exemption_rules) = config_json.get("exemptionRules") {
                        if let Some(exemption_array) = exemption_rules.as_array() {
                            for exemption_entry in exemption_array {
                                let mut host_patterns = Vec::new();
                                let mut path_patterns = Vec::new();

                                if let Some(hosts) = exemption_entry.get("hostPatterns") {
                                    if let Some(hosts_array) = hosts.as_array() {
                                        for host_entry in hosts_array {
                                            if let Some(host) = host_entry.as_str() {
                                                host_patterns.push(host.to_string());
                                            }
                                        }
                                    }
                                }

                                if let Some(paths) = exemption_entry.get("pathPatterns") {
                                    if let Some(paths_array) = paths.as_array() {
                                        for path_entry in paths_array {
                                            if let Some(path) = path_entry.as_str() {
                                                path_patterns.push(path.to_string());
                                            }
                                        }
                                    }
                                } else {
                                    // 如果没有指定pathPatterns，使用默认值
                                    path_patterns = ExemptionRule::default().path_patterns;
                                }

                                // 只要有path_patterns就添加规则（host_patterns可以为空）
                                if !path_patterns.is_empty() {
                                    log::info!("SP: Added exemption rule - hostPatterns: {:?}, pathPatterns: {:?}",
                                              host_patterns, path_patterns);
                                    self.config.exemption_rules.push(ExemptionRule {
                                        host_patterns,
                                        path_patterns,
                                    });
                                }
                            }
                        }
                    } else {
                        // 如果没有配置exemptionRules，添加默认的豁免规则
                        let default_rule = ExemptionRule::default();
                        log::info!("SP: Added default exemption rule - hostPatterns: {:?}, pathPatterns: {:?}",
                                  default_rule.host_patterns, default_rule.path_patterns);
                        self.config.exemption_rules.push(default_rule);
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
    pending_save_call_token: Option<u32>,
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
            .with_traffic_direction(
                config
                    .traffic_direction
                    .clone()
                    .unwrap_or_else(|| "auto".to_string()),
            );
        Self {
            _context_id: context_id,
            config: config,
            request_headers: HashMap::new(),
            request_body: Vec::new(),
            response_headers: HashMap::new(),
            response_body: Vec::new(),
            span_builder: span_builder,
            pending_inject_call_token: None,
            pending_save_call_token: None,
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
        // 首先检查豁免规则
        if self.is_exempted() {
            log::info!("SP: Request is exempted from collection");
            return false;
        }

        // 如果没有配置规则，则默认采集所有请求
        if self.config.collection_rules.is_empty() {
            log::info!("SP: No collection rules configured, collecting all requests");
            return true;
        }
        log::info!(
            "SP: Checking collection rules, total rules: {}",
            self.config.collection_rules.len()
        );

        // 智能检测流量方向：尝试两种规则类型
        let mut inbound_matched = false;
        let mut outbound_matched = false;

        // 首先尝试 inbound 规则匹配
        if let Some(request_path) = self.request_headers.get(":path") {
            log::debug!("SP: Checking inbound rules for path: {}", request_path);

            for (i, rule) in self.config.collection_rules.iter().enumerate() {
                // 检查是否有服务器规则（path不为空）
                if !rule.http.server.path.is_empty() {
                    log::debug!(
                        "SP: Checking inbound rule {}: serverPath='{}'",
                        i,
                        rule.http.server.path
                    );
                    if self.match_pattern(&rule.http.server.path, request_path) {
                        log::debug!(
                            "SP: Inbound request matched server_path: {}",
                            rule.http.server.path
                        );
                        inbound_matched = true;
                        break;
                    }
                }
            }
        }

        // 然后尝试 outbound 规则匹配
        let (client_host, client_path) = self.extract_client_info();
        log::debug!(
            "SP: Checking outbound rules with client_host: {:?}, client_path: {:?}",
            client_host,
            client_path
        );

        for (i, rule) in self.config.collection_rules.iter().enumerate() {
            // 处理客户端规则（检查client数组不为空）
            if !rule.http.client.is_empty() {
                for client_config in &rule.http.client {
                    log::debug!(
                        "SP: Checking outbound rule {}: clientHost={}, clientPaths={:?}",
                        i,
                        client_config.host,
                        client_config.paths
                    );

                    // 检查 client_host
                    if let Some(ref actual_client_host) = client_host {
                        if !self.match_pattern(&client_config.host, actual_client_host) {
                            log::debug!(
                                "SP: Client host did not match: expected={}, actual={}",
                                client_config.host,
                                actual_client_host
                            );
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
                        let client_path_matched = if let Some(ref actual_client_path) = client_path
                        {
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
                    outbound_matched = true;
                    break;
                }
                if outbound_matched {
                    break;
                }
            }
        }

        // 根据匹配结果决定是否采集
        if inbound_matched {
            log::info!("SP: Request matched inbound rules, collecting");
            return true;
        }

        if outbound_matched {
            log::info!("SP: Request matched outbound rules, collecting");
            return true;
        }

        // 检查是否有任何规则配置
        let has_server_rules = self
            .config
            .collection_rules
            .iter()
            .any(|rule| !rule.http.server.path.is_empty());
        let has_client_rules = self
            .config
            .collection_rules
            .iter()
            .any(|rule| !rule.http.client.is_empty());

        if !has_server_rules && !has_client_rules {
            log::info!("SP: No specific rules configured, collecting all requests");
            return true;
        }

        log::info!("SP: No rules matched, not collecting");
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
                log::info!(
                    "SP: Parsed from referer - host: {:?}, path: {:?}",
                    client_host,
                    client_path
                );
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
            client_host = self
                .request_headers
                .get("host")
                .or_else(|| self.request_headers.get(":authority"))
                .cloned();
        }

        // 直接从请求路径获取客户端路径
        if client_path.is_none() {
            client_path = self.request_headers.get(":path").cloned();
        }

        log::info!(
            "SP: Final client info - host: {:?}, path: {:?}",
            client_host,
            client_path
        );
        (client_host, client_path)
    }

    // 检查请求是否应该被豁免
    fn is_exempted(&self) -> bool {
        // 如果没有配置豁免规则，则不豁免任何请求
        if self.config.exemption_rules.is_empty() {
            return false;
        }

        // 获取请求的host和path信息
        let request_host = self
            .request_headers
            .get("host")
            .or_else(|| self.request_headers.get(":authority"))
            .cloned();
        let request_path = self.request_headers.get(":path").cloned();

        // 获取客户端信息（用于outbound请求）
        let (client_host, client_path) = self.extract_client_info();

        log::debug!("SP: Checking exemption - request_host: {:?}, request_path: {:?}, client_host: {:?}, client_path: {:?}",
                   request_host, request_path, client_host, client_path);

        // 检查每个豁免规则
        for rule in &self.config.exemption_rules {
            let mut host_matched = false;
            let mut path_matched = false;

            // 检查host模式匹配
            if rule.host_patterns.is_empty() {
                host_matched = true; // 如果没有配置host模式，则认为匹配
            } else {
                // 检查inbound请求的host
                if let Some(ref host) = request_host {
                    for pattern in &rule.host_patterns {
                        if self.match_pattern(pattern, host) {
                            host_matched = true;
                            log::debug!(
                                "SP: Host pattern '{}' matched request host '{}'",
                                pattern,
                                host
                            );
                            break;
                        }
                    }
                }

                // 检查outbound请求的client host
                if !host_matched {
                    if let Some(ref host) = client_host {
                        for pattern in &rule.host_patterns {
                            if self.match_pattern(pattern, host) {
                                host_matched = true;
                                log::debug!(
                                    "SP: Host pattern '{}' matched client host '{}'",
                                    pattern,
                                    host
                                );
                                break;
                            }
                        }
                    }
                }
            }

            // 检查path模式匹配
            if rule.path_patterns.is_empty() {
                path_matched = true; // 如果没有配置path模式，则认为匹配
            } else {
                // 检查inbound请求的path
                if let Some(ref path) = request_path {
                    for pattern in &rule.path_patterns {
                        if self.match_pattern(pattern, path) {
                            path_matched = true;
                            log::debug!(
                                "SP: Path pattern '{}' matched request path '{}'",
                                pattern,
                                path
                            );
                            break;
                        }
                    }
                }

                // 检查outbound请求的client path
                if !path_matched {
                    if let Some(ref path) = client_path {
                        for pattern in &rule.path_patterns {
                            if self.match_pattern(pattern, path) {
                                path_matched = true;
                                log::debug!(
                                    "SP: Path pattern '{}' matched client path '{}'",
                                    pattern,
                                    path
                                );
                                break;
                            }
                        }
                    }
                }
            }

            // 如果host和path都匹配，则豁免该请求
            if host_matched && path_matched {
                log::info!(
                    "SP: Request exempted by rule - hostPatterns: {:?}, pathPatterns: {:?}",
                    rule.host_patterns,
                    rule.path_patterns
                );
                return true;
            }
        }

        false
    }

    // 使用正则表达式匹配
    fn match_pattern(&self, pattern: &str, text: &str) -> bool {
        log::debug!("SP: Matching pattern '{}' against text '{}'", pattern, text);
        match Regex::new(pattern) {
            Ok(re) => {
                let result = re.is_match(text);
                log::debug!("SP: Regex match result: {}", result);
                result
            }
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
                    // For HTTPS, don't include the default port 443
                    // For HTTP, don't include the default port 80
                    match url.port() {
                        Some(port) => {
                            let default_port = match url.scheme() {
                                "https" => 443,
                                "http" => 80,
                                _ => 80,
                            };
                            if port == default_port {
                                host.to_string()
                            } else {
                                format!("{}:{}", host, port)
                            }
                        }
                        None => host.to_string(),
                    }
                } else {
                    "o.softprobe.ai".to_string()
                }
            }
            Err(_) => "o.softprobe.ai".to_string(),
        }
    }

    // Build Envoy cluster name from backend URL
    fn get_backend_cluster_name(&self) -> String {
        match Url::parse(&self.config.sp_backend_url) {
            Ok(url) => {
                if let Some(host) = url.host_str() {
                    let port = match url.scheme() {
                        "https" => url.port().unwrap_or(443),
                        "http" => url.port().unwrap_or(80),
                        _ => url.port().unwrap_or(80),
                    };
                    format!("outbound|{}||{}", port, host)
                } else {
                    "outbound|443||o.softprobe.ai".to_string()
                }
            }
            Err(_) => "outbound|443||o.softprobe.ai".to_string(),
        }
    }

    // Dispatch injection HTTP call directly using context's dispatch_http_call method
    fn dispatch_injection_lookup(&mut self) -> Result<u32, String> {
        return Ok(0);

        /*log::info!("SP Injection: Preparing injection lookup data");

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
        log::info!("SP Injection: Authority: {}, Headers: {:?}", authority, http_headers);

        // Use the context's dispatch_http_call method to maintain context
        // Using dynamically built Envoy cluster name based on backend URL
        let cluster_name = self.get_backend_cluster_name();
        log::info!("SP Injection: Using cluster name: {}", cluster_name);

        match self.dispatch_http_call(
            &cluster_name,
            http_headers,
            Some(&otel_data),
            vec![],
            std::time::Duration::from_secs(5),
        ) {
            Ok(call_id) => {
                log::info!("SP Injection: Injection lookup dispatched with call_id: {}", call_id);
                Ok(call_id)
            }
            Err(e) => {
                log::error!("SP Injection: Failed to dispatch injection lookup: {:?}", e);
                Err(format!("Dispatch failed: {:?}", e))
            }
        }*/
    }

    // Dispatch async call to save extracted data
    fn dispatch_async_extraction_save(&mut self) -> Result<(), String> {
        log::info!("SP: Starting async extraction save");

        // 检查是否解析到 session_id
        let has_session_id = self.span_builder.has_session_id();
        log::info!(
            "SP: Session ID found: {}, value: '{}'",
            has_session_id,
            self.span_builder.get_session_id()
        );

        // 如果没有解析到 session_id，强制上传 trace
        if !has_session_id {
            log::info!("SP: No session ID found, forcing trace upload for isolation");
        } else {
            // 检查采集规则
            if !self.should_collect_by_rules() {
                log::info!("SP: Data extraction skipped based on collection rules");
                return Err("Data collection skipped based on collection rules".to_string());
            }
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

        log::info!(
            "SP Extraction: Serialized traces data size: {} bytes",
            otel_data.len()
        );

        let backend_url = format!("{}/v1/traces", self.config.sp_backend_url);
        log::error!("SP Extraction: - Target URL: {}", backend_url);
        log::info!(
            "SP Extraction: - Request body size: {} bytes",
            otel_data.len()
        );

        // Get backend authority from configured URL
        let authority = self.get_backend_authority();

        // Prepare HTTP headers for the async save call
        let content_length = otel_data.len().to_string();
        let mut http_headers = vec![
            (":method", "POST"),
            (":path", "/v1/traces"),
            (":authority", &authority),
            ("content-type", "application/x-protobuf"),
            ("content-length", &content_length),
            ("x-api-key", &self.config.api_key),
        ];

        log::info!("SP Extraction: Dispatching HTTP call with headers:");
        for (key, value) in &http_headers {
            log::info!("SP Extraction:   {}: {}", key, value);
        }

        // 添加超时时间的日志
        let timeout = std::time::Duration::from_secs(5);
        log::info!("SP Extraction: Using timeout: {:?}", timeout);

        // Fire and forget async call to /v1/traces endpoint for storage
        // Using dynamically built Envoy cluster name based on backend URL
        let cluster_name = self.get_backend_cluster_name();
        log::info!("SP Extraction: Using cluster name: {}", cluster_name);

        match self.dispatch_http_call(
            &cluster_name,
            http_headers,
            Some(&otel_data),
            vec![],
            timeout,
        ) {
            Ok(call_id) => {
                log::info!("SP Extraction: HTTP call dispatched successfully!");
                log::info!("SP Extraction: - Call ID: {}", call_id);
                log::info!(
                    "SP Extraction: - Waiting for response in on_http_call_response callback..."
                );

                self.pending_save_call_token = Some(call_id);
                log::info!("SP: Async extraction save dispatched successfully");
                log::info!("SP: HTTP response will be available in on_http_call_response callback");

                // 添加额外的调试信息
                log::info!(
                    "SP: Current pending tokens - inject: {:?}, save: {:?}",
                    self.pending_inject_call_token,
                    self.pending_save_call_token
                );

                Ok(())
            }
            Err(status) => {
                let error_msg = format!(
                    "SP Extraction: Failed to dispatch HTTP call, status: {:?}",
                    status
                );
                log::error!("{}", error_msg);
                Err(error_msg)
            }
        }
    }

    /// Inject W3C Trace Context headers into the outgoing request
    fn inject_trace_context_headers(&mut self) {
        log::error!("SP: *** INJECT_TRACE_CONTEXT_HEADERS CALLED *** NEW VERSION");
        
        // 首先检查当前的 tracestate
        if let Some(current_tracestate) = self.request_headers.get("tracestate") {
            log::error!("SP: Current tracestate before modification: {}", current_tracestate);
        } else {
            log::error!("SP: No existing tracestate found");
        }
        
        // 透传所有请求header，不做任何修改
        // 只在 tracestate 中添加 x-sp-traceparent 键

        // 生成当前 WASM 的 span ID 和使用现有的 trace ID
        let current_span_id = crate::otel::generate_span_id();
        let current_span_id_hex = current_span_id
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>();

        // 获取当前的 trace ID (从 span_builder 中获取)
        let trace_id_hex = self.span_builder.get_trace_id_hex();

        // 生成标准的 traceparent 格式: 00-trace_id-span_id-01
        let traceparent_value = format!("00-{}-{}-01", trace_id_hex, current_span_id_hex);
        
        log::error!("SP: Generated traceparent_value: {}", traceparent_value);

        // 获取现有的 tracestate
        let mut tracestate_entries = Vec::new();

        if let Some(existing_tracestate) = self.request_headers.get("tracestate") {
            // 解析现有的 tracestate，保留其他条目
            for entry in existing_tracestate.split(',') {
                let entry = entry.trim();
                if !entry.starts_with("x-sp-traceparent=") {
                    tracestate_entries.push(entry.to_string());
                }
            }
        }

        // 添加 x-sp-traceparent 条目，使用完整的 traceparent 格式
        tracestate_entries.insert(0, format!("x-sp-traceparent={}", traceparent_value));

        // 构建新的 tracestate
        let new_tracestate = tracestate_entries.join(",");

        log::error!(
            "SP: Adding x-sp-traceparent to tracestate: {}",
            new_tracestate
        );

        // 先删除现有的 tracestate header，然后添加新的
        log::error!("SP: *** BEFORE remove_http_request_header *** NEW VERSION");
        self.remove_http_request_header("tracestate");
        log::error!("SP: *** AFTER remove_http_request_header *** NEW VERSION");
        
        log::error!("SP: *** BEFORE add_http_request_header *** NEW VERSION");
        self.add_http_request_header("tracestate", &new_tracestate);
        log::error!("SP: Successfully added tracestate header - NEW VERSION");
        
        // 同时更新本地缓存的 request_headers
        self.request_headers.insert("tracestate".to_string(), new_tracestate.clone());
        log::error!("SP: *** AFTER add_http_request_header *** NEW VERSION");
        
        // 验证修改是否成功
        if let Some(updated_tracestate) = self.request_headers.get("tracestate") {
            log::error!("SP: Verified updated tracestate in cache: {}", updated_tracestate);
        }
    }

    /// Extract and propagate W3C Trace Context from response headers
    fn extract_and_propagate_trace_context(&mut self) {
        // 从请求 header 中提取 tracestate
        let mut parent_span_id: Option<Vec<u8>> = None;
        let mut trace_id: Option<Vec<u8>> = None;

        if let Some(tracestate) = self.request_headers.get("tracestate") {
            log::error!("SP: Found tracestate in request: {}", tracestate);

            // 解析 tracestate 中的 x-sp-traceparent
            for entry in tracestate.split(',') {
                let entry = entry.trim();
                if let Some(value) = entry.strip_prefix("x-sp-traceparent=") {
                    // 解析完整的 traceparent 格式: 00-trace_id-span_id-01
                    if let Some((parsed_trace_id, parsed_span_id)) =
                        self.parse_traceparent_value(value)
                    {
                        trace_id = Some(parsed_trace_id);
                        parent_span_id = Some(parsed_span_id);
                        log::error!(
                            "SP: Extracted trace context from x-sp-traceparent: {}",
                            value
                        );
                        break;
                    }
                }
            }
        }

        // 如果从 tracestate 中解析到了 trace context，更新 span builder
        if let (Some(trace_id), Some(parent_id)) = (trace_id, parent_span_id) {
            let mut updated_headers = HashMap::new();

            // 构造标准的 traceparent
            let trace_id_hex = trace_id
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
            let parent_id_hex = parent_id
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
            let traceparent = format!("00-{}-{}-01", trace_id_hex, parent_id_hex);

            updated_headers.insert("traceparent".to_string(), traceparent);

            // 保留原始的 tracestate
            if let Some(tracestate) = self.request_headers.get("tracestate") {
                updated_headers.insert("tracestate".to_string(), tracestate.clone());
            }

            // 更新 span builder
            self.span_builder = self.span_builder.clone().with_context(&updated_headers);
        }

        // 检查响应中是否包含 W3C Trace Context headers（保持原有逻辑）
        if let Some(traceparent) = self.response_headers.get("traceparent") {
            log::error!("SP: Found traceparent in response: {}", traceparent);

            // 传播 trace context 到下游响应
            self.propagate_trace_context_to_response();
        } else {
            log::debug!("SP: No traceparent found in response headers");
        }
    }

    /// Helper function to decode hex string to bytes
    fn hex_decode(&self, hex: &str) -> Option<Vec<u8>> {
        if hex.len() % 2 != 0 {
            return None;
        }

        let mut bytes = Vec::new();
        for i in (0..hex.len()).step_by(2) {
            if let Ok(byte) = u8::from_str_radix(&hex[i..i + 2], 16) {
                bytes.push(byte);
            } else {
                return None;
            }
        }
        Some(bytes)
    }

    /// Parse traceparent value in format: 00-trace_id-span_id-01
    fn parse_traceparent_value(&self, traceparent: &str) -> Option<(Vec<u8>, Vec<u8>)> {
        let parts: Vec<&str> = traceparent.split('-').collect();
        if parts.len() != 4 {
            return None;
        }

        let trace_id = self.hex_decode(parts[1])?;
        let span_id = self.hex_decode(parts[2])?;

        Some((trace_id, span_id))
    }

    /// Propagate trace context to the downstream response
    fn propagate_trace_context_to_response(&mut self) {
        // Generate a new span ID for the response
        let span_id = crate::otel::generate_span_id();

        // Generate traceparent header for the response
        let traceparent = self.span_builder.generate_traceparent(&span_id);
        log::info!("SP: Propagating traceparent to response: {}", traceparent);

        // Add traceparent header to the response
        let _ = self.add_http_response_header("traceparent", &traceparent);
    }

    // 自动检测流量方向
    fn detect_traffic_direction(&self) -> String {
        // 方法1: 通过监听器地址检测
        if let Some(listener_direction) = self.get_property(vec!["listener_direction"]) {
            if let Ok(direction) = String::from_utf8(listener_direction) {
                log::info!("SP: Detected listener_direction: {}", direction);
                return direction;
            }
        }

        // 方法2: 通过监听器元数据检测
        if let Some(metadata) = self.get_property(vec![
            "metadata",
            "filter_metadata",
            "envoy.common",
            "direction",
        ]) {
            if let Ok(direction) = String::from_utf8(metadata) {
                log::info!("SP: Detected direction from metadata: {}", direction);
                return direction;
            }
        }

        // 方法3: 通过集群名称模式检测
        if let Some(cluster_name) = self.get_property(vec!["cluster_name"]) {
            if let Ok(cluster) = String::from_utf8(cluster_name) {
                log::info!("SP: Detected cluster_name: {}", cluster);
                if cluster.starts_with("inbound|") {
                    return "inbound".to_string();
                } else if cluster.starts_with("outbound|") {
                    return "outbound".to_string();
                }
            }
        }

        // 方法4: 通过端口范围推断
        if let Some(downstream_local_address) = self.get_property(vec!["source", "address"]) {
            if let Ok(address) = String::from_utf8(downstream_local_address) {
                log::info!("SP: Detected downstream address: {}", address);
                // Istio inbound 通常使用 15006 端口
                if address.contains(":15006") {
                    return "inbound".to_string();
                }
                // Istio outbound 通常使用 15001 端口
                if address.contains(":15001") {
                    return "outbound".to_string();
                }
            }
        }

        // 方法5: 通过请求特征推断
        // 检查是否有 x-forwarded-for 头部（通常表示 inbound）
        if self.request_headers.contains_key("x-forwarded-for") {
            log::info!("SP: Found x-forwarded-for header, likely inbound traffic");
            return "inbound".to_string();
        }

        // 检查 Host 头部是否为外部域名（通常表示 outbound）
        if let Some(host) = self
            .request_headers
            .get("host")
            .or_else(|| self.request_headers.get(":authority"))
        {
            if !host.contains("localhost")
                && !host.contains("127.0.0.1")
                && !host.contains(".local")
            {
                log::info!(
                    "SP: External host detected: {}, likely outbound traffic",
                    host
                );
                return "outbound".to_string();
            }
        }

        // 默认返回 auto
        log::info!("SP: Could not determine traffic direction, using 'auto'");
        "auto".to_string()
    }
}

impl Context for SpHttpContext {
    fn on_http_call_response(
        &mut self,
        token_id: u32,
        _num_headers: usize,
        body_size: usize,
        _num_trailers: usize,
    ) {
        log::info!(
            "SP: *** HTTP CALL RESPONSE RECEIVED *** token: {}, body_size: {}",
            token_id,
            body_size
        );
        log::info!(
            "SP: pending_inject_call_token = {:?}",
            self.pending_inject_call_token
        );
        log::info!(
            "SP: pending_save_call_token = {:?}",
            self.pending_save_call_token
        );

        // Get response status
        let status_code = self
            .get_http_call_response_header(":status")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(500);

        // Get all response headers
        let response_headers = self.get_http_call_response_headers();

        // Get response body
        let response_body = if body_size > 0 {
            self.get_http_call_response_body(0, body_size)
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        // Check if this is the response to our async save call
        if let Some(pending_save_token) = self.pending_save_call_token {
            if pending_save_token == token_id {
                log::info!("SP: *** PROCESSING ASYNC SAVE RESPONSE ***");
                self.pending_save_call_token = None;

                // 打印HTTP响应的完整信息
                log::info!("SP: HTTP Response Status: {}", status_code);
                log::info!(
                    "SP: HTTP Response Headers ({} headers):",
                    response_headers.len()
                );
                for (key, value) in &response_headers {
                    log::info!("SP:   {}: {}", key, value);
                }

                // 打印response body (无论状态码如何)
                if response_body.len() > 0 {
                    log::info!(
                        "SP: HTTP Response Body ({} bytes): {}",
                        response_body.len(),
                        String::from_utf8_lossy(&response_body)
                    );

                    // 如果是二进制数据，也打印十六进制格式
                    if !response_body
                        .iter()
                        .all(|&b| b.is_ascii() && !b.is_ascii_control())
                    {
                        let hex_preview = if response_body.len() > 50 {
                            let hex_str: String = response_body[..50]
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<Vec<_>>()
                                .join(" ");
                            format!(
                                "{} ... (truncated, total {} bytes)",
                                hex_str,
                                response_body.len()
                            )
                        } else {
                            response_body
                                .iter()
                                .map(|b| format!("{:02x}", b))
                                .collect::<Vec<_>>()
                                .join(" ")
                        };
                        log::info!("SP: HTTP Response Body (hex): {}", hex_preview);
                    }
                } else {
                    log::info!("SP: HTTP Response Body: (empty)");
                }

                // 根据状态码进行处理
                if status_code >= 200 && status_code < 300 {
                    log::info!(
                        "SP: Async save completed successfully (status: {})",
                        status_code
                    );
                } else {
                    log::warn!("SP: Async save failed with status: {}", status_code);
                }

                return;
            }
        }

        // Check if this is the response to our agent lookup call
        if let Some(pending_token) = self.pending_inject_call_token {
            if pending_token == token_id {
                log::info!("SP: Processing injection lookup response");
                self.pending_inject_call_token = None;
                // Get response status
                let status_code = self
                    .get_http_call_response_header(":status")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(500);

                log::info!("SP: Injection response status: {}", status_code);

                if status_code == 200 {
                    // Injection hit - parse and return injection response
                    if body_size > 0 {
                        let response_body = self
                            .get_http_call_response_body(0, body_size)
                            .unwrap_or_default();
                        log::info!("SP: Received {} bytes for injection", response_body.len());

                        // Parse the OTEL response and extract agentd HTTP response
                        match parse_otel_injection_response(&response_body) {
                            Ok(Some(injected_response)) => {
                                log::info!("SP: Successfully parsed injection response, status: {}, {} headers, {} bytes body",
                                    injected_response.status_code, injected_response.headers.len(), injected_response.body.len());

                                // Convert headers to &str format
                                let headers_refs: Vec<(&str, &str)> = injected_response
                                    .headers
                                    .iter()
                                    .map(|(k, v)| (k.as_str(), v.as_str()))
                                    .collect();

                                // Send agentd response
                                let body = if injected_response.body.is_empty() {
                                    None
                                } else {
                                    Some(injected_response.body.as_slice())
                                };
                                self.send_http_response(
                                    injected_response.status_code,
                                    headers_refs,
                                    body,
                                );

                                log::info!("SP: Successfully injected response");
                                return; // Don't resume - we've handled the response
                            }
                            Ok(None) => {
                                log::warn!(
                                    "SP: 200 Injection response but no injection data found"
                                );
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
        let traffic_direction = self.detect_traffic_direction();
        log::error!("SP: *** {} REQUEST HEADERS CALLBACK INVOKED ***", traffic_direction);

        // 首先获取初始的请求头用于 span builder 更新
        let mut initial_headers = HashMap::new();
        for (key, value) in self.get_http_request_headers() {
            initial_headers.insert(key, value);
        }

        // 使用自动检测的流量方向，而不是配置中的 traffic_direction
        let service_name = self.config.service_name.clone();
        let api_key = self.config.api_key.clone();

        log::error!("DEBUG: Before span builder update - service_name: '{}', traffic_direction: '{}' (auto-detected), api_key: '{}'",
                   service_name, traffic_direction, api_key);

        // Update url.host and url.path from properties/headers
        self.update_url_info();

        // Update span builder with trace context and session ID (使用初始头部)
        self.span_builder = self
            .span_builder
            .clone()
            .with_service_name(service_name)
            .with_traffic_direction(traffic_direction)
            .with_api_key(api_key)
            .with_context(&initial_headers);

        // 将初始头部复制到 request_headers 缓存
        self.request_headers = initial_headers;

        // Inject W3C Trace Context headers - 这会修改发送给上游的头部
        self.inject_trace_context_headers();

        // 重新获取请求头以获取修改后的版本，但不要覆盖整个 request_headers
        // 只记录所有头部用于调试
        for (key, value) in self.get_http_request_headers() {
            log::error!("SP: Request header: {}: {}", key, value);
            // 不要覆盖 request_headers，因为 inject_trace_context_headers 已经更新了它
        }

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
                    log::error!(
                        "SP Injection: Injection lookup error: {}, continuing to upstream",
                        e
                    );
                }
            }
        }

        Action::Continue
    }

    fn on_http_request_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        log::error!(
            "SP: Processing request body, size: {}, end_of_stream: {}",
            body_size,
            end_of_stream
        );

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
                    log::error!(
                        "SP Injection: Injection lookup error: {}, continuing to upstream",
                        e
                    );
                }
            }
        }

        Action::Continue
    }

    fn on_http_response_headers(&mut self, num_headers: usize, end_of_stream: bool) -> Action {
        log::error!(
            "SP: *** RESPONSE HEADERS CALLBACK INVOKED *** num_headers: {}, end_of_stream: {}",
            num_headers,
            end_of_stream
        );

        // Don't extract injected data
        if self.injected {
            return Action::Continue;
        }

        // Capture response headers
        for (key, value) in self.get_http_response_headers() {
            self.response_headers.insert(key, value);
        }

        // Extract and propagate W3C Trace Context from response headers
        self.extract_and_propagate_trace_context();

        Action::Continue
    }

    fn on_http_response_body(&mut self, body_size: usize, end_of_stream: bool) -> Action {
        log::error!(
            "SP: *** RESPONSE BODY CALLBACK INVOKED *** size: {}, end_of_stream: {}",
            body_size,
            end_of_stream
        );

        // Don't extract injected data
        if self.injected {
            log::info!("SP: Skipping extraction because response was injected");
            return Action::Continue;
        }

        // Buffer response body
        if let Some(body) = self.get_http_response_body(0, body_size) {
            self.response_body.extend_from_slice(&body);
        }

        if end_of_stream {
            log::info!("SP: End of response stream reached");
            // Check response status using already captured headers
            if let Some(status) = self.response_headers.get(":status") {
                log::info!("SP: Response status: {}", status);
                // For testing purposes, process all responses (not just 200)
                log::info!(
                    "SP: Processing response (status: {}), storing in agent asynchronously",
                    status
                );
                // Send to Softprobe asynchronously (fire and forget)
                match self.dispatch_async_extraction_save() {
                    Ok(()) => {
                        log::error!("SP: Async extraction save dispatched successfully");
                        log::info!(
                            "SP: HTTP response will be available in on_http_call_response callback"
                        );
                    }
                    Err(e) => {
                        log::error!("SP: Failed to store agent: {}", e);
                    }
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
    use crate::otel::TracesData;
    use prost::Message;

    log::info!(
        "SP: Starting protobuf decode of {} bytes",
        response_body.len()
    );

    // Decode OTEL protobuf response
    let traces_data = TracesData::decode(response_body).map_err(|e| {
        log::error!("SP: Protobuf decode failed: {}", e);
        format!("Serialization error: {}", e)
    })?;

    log::info!(
        "SP: Successfully decoded protobuf, found {} resource spans",
        traces_data.resource_spans.len()
    );

    // Extract agentd HTTP response from span attributes
    for (i, resource_span) in traces_data.resource_spans.iter().enumerate() {
        log::debug!(
            "SP: Processing resource span {}, found {} scope spans",
            i,
            resource_span.scope_spans.len()
        );
        for (j, scope_span) in resource_span.scope_spans.iter().enumerate() {
            log::debug!(
                "SP: Processing scope span {}, found {} spans",
                j,
                scope_span.spans.len()
            );
            for (k, span) in scope_span.spans.iter().enumerate() {
                log::debug!(
                    "SP: Processing span {}, name: '{}', {} attributes",
                    k,
                    span.name,
                    span.attributes.len()
                );
                // Look for agentd response data in span attributes
                let mut status_code = 200u32;
                let mut headers = Vec::new();
                let mut body = Vec::new();

                for attr in &span.attributes {
                    match attr.key.as_str() {
                        "http.response.status_code" => {
                            if let Some(value) = &attr.value {
                                if let Some(crate::otel::any_value::Value::IntValue(code)) =
                                    &value.value
                                {
                                    status_code = *code as u32;
                                }
                            }
                        }
                        key if key.starts_with("http.response.header.") => {
                            let header_name = &key[21..]; // Remove "http.response.header." prefix
                            if let Some(value) = &attr.value {
                                if let Some(crate::otel::any_value::Value::StringValue(
                                    header_value,
                                )) = &value.value
                                {
                                    headers.push((header_name.to_string(), header_value.clone()));
                                }
                            }
                        }
                        "http.response.body" => {
                            if let Some(value) = &attr.value {
                                if let Some(crate::otel::any_value::Value::StringValue(body_str)) =
                                    &value.value
                                {
                                    // Decode base64 if it's binary data, otherwise use as-is
                                    body = if is_base64_encoded(body_str) {
                                        use base64::{engine::general_purpose, Engine as _};
                                        general_purpose::STANDARD
                                            .decode(body_str)
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
    s.len() > 100
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
}
