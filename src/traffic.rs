use crate::config::Config;
use proxy_wasm::traits::Context;
use regex::Regex;
use std::collections::HashMap;
// use url::Url; // no longer needed here

pub trait TrafficAnalyzer {
    fn detect_traffic_direction(&self) -> String;
    fn is_from_istio_ingressgateway(&self) -> bool;
    fn should_collect_by_rules(&self, config: &Config, request_headers: &HashMap<String, String>) -> bool;
    fn is_exempted(&self, config: &Config, request_headers: &HashMap<String, String>) -> bool;
}

pub trait RequestHeadersAccess {
    fn get_context_property(&self, path: Vec<&str>) -> Option<Vec<u8>>;
    fn get_request_header(&self, name: &str) -> Option<String>;
}

impl<T: Context> TrafficAnalyzer for T where T: RequestHeadersAccess {
    fn detect_traffic_direction(&self) -> String {
        // Method 1: Check listener direction
        if let Some(listener_direction) = self.get_context_property(vec!["listener_direction"]) {
            if let Ok(direction) = String::from_utf8(listener_direction) {
                crate::sp_debug!("Detected listener_direction: {}", direction);
                return direction;
            }
        }

        // Method 2: Check metadata direction
        if let Some(metadata) = self.get_context_property(vec![
            "metadata",
            "filter_metadata",
            "envoy.common",
            "direction",
        ]) {
            if let Ok(direction) = String::from_utf8(metadata) {
                crate::sp_debug!("Detected direction from metadata: {}", direction);
                return direction;
            }
        }

        // Method 3: Check cluster name pattern
        if let Some(cluster_name) = self.get_context_property(vec!["cluster_name"]) {
            if let Ok(cluster) = String::from_utf8(cluster_name) {
                crate::sp_debug!("Detected cluster_name: {}", cluster);
                if cluster.starts_with("inbound|") {
                    return "inbound".to_string();
                } else if cluster.starts_with("outbound|") {
                    return "outbound".to_string();
                }
            }
        }

        // Method 4: Check by port range (source address)
        if let Some(downstream_local_address) = self.get_context_property(vec!["source", "address"]) {
            if let Ok(address) = String::from_utf8(downstream_local_address) {
                crate::sp_debug!("Detected downstream address: {}", address);
                if address.contains(":15006") {
                    return "inbound".to_string();
                }
                if address.contains(":15001") {
                    return "outbound".to_string();
                }
            }
        }

        // Method 5: Heuristic using request headers
        if self.get_request_header("x-forwarded-for").is_some() {
            crate::sp_debug!("Found x-forwarded-for header, likely inbound traffic");
            return "inbound".to_string();
        }

        if let Some(host) = self.get_request_header("host").or_else(|| self.get_request_header(":authority")) {
            if !host.contains("localhost") && !host.contains("127.0.0.1") && !host.contains(".local") {
                crate::sp_debug!("External host detected: {}, likely outbound traffic", host);
                return "outbound".to_string();
            }
        }

        crate::sp_debug!("Could not determine traffic direction, using 'auto'");
        "auto".to_string()
    }

    fn is_from_istio_ingressgateway(&self) -> bool {
        let ingress_patterns = [
            ("node", "metadata", "WORKLOAD_NAME"),
            ("node", "metadata", "app"),
            ("node", "metadata", "NAME"),
        ];

        for pattern in &ingress_patterns {
            if let Some(value) = self.get_context_property(vec![pattern.0, pattern.1, pattern.2]) {
                if let Ok(value_str) = String::from_utf8(value) {
                    if value_str.contains("istio-ingressgateway") {
                        return true;
                    }
                }
            }
        }

        // Check cluster metadata
        if let Some(cluster_metadata) = self.get_context_property(vec!["cluster_metadata"]) {
            if let Ok(metadata) = String::from_utf8(cluster_metadata) {
                if metadata.contains("istio-ingressgateway") {
                    return true;
                }
            }
        }

        // Check source workload
        if let Some(source_workload) = self.get_context_property(vec!["source", "workload", "name"]) {
            if let Ok(workload) = String::from_utf8(source_workload) {
                if workload.contains("istio-ingressgateway") {
                    return true;
                }
            }
        }

        // Check node ID
        if let Some(node_id) = self.get_context_property(vec!["node", "id"]) {
            if let Ok(id) = String::from_utf8(node_id) {
                if id.contains("istio-ingressgateway") {
                    return true;
                }
            }
        }

        // Check peer metadata header
        if let Some(peer_metadata) = self.get_request_header("x-envoy-peer-metadata-id") {
            if peer_metadata.contains("istio-ingressgateway") {
                return true;
            }
        }

        // Check labels
        if let Some(labels) = self.get_context_property(vec!["node", "metadata", "LABELS"]) {
            if let Ok(labels_str) = String::from_utf8(labels) {
                if labels_str.contains("istio-ingressgateway") {
                    return true;
                }
            }
        }

        false
    }

    fn should_collect_by_rules(&self, config: &Config, request_headers: &HashMap<String, String>) -> bool {
        // First check exemption rules
        if self.is_exempted(config, request_headers) {
            crate::sp_debug!("Request is exempted from collection");
            return false;
        }

        // If no rules configured, collect all requests
        if config.collection_rules.is_empty() {
            crate::sp_debug!("No collection rules configured, collecting all requests");
            return true;
        }

        crate::sp_debug!("Checking collection rules, total rules: {}", config.collection_rules.len());

        // Try inbound rules matching
        let inbound_matched = check_inbound_rules(config, request_headers);
        if inbound_matched {
            crate::sp_debug!("Request matched inbound rules, collecting");
            return true;
        }

        // Try outbound rules matching
        let outbound_matched = check_outbound_rules(config, request_headers);
        if outbound_matched {
            crate::sp_debug!("Request matched outbound rules, collecting");
            return true;
        }

        // Check if any rules are configured
        let has_server_rules = config
            .collection_rules
            .iter()
            .any(|rule| !rule.http.server.path.is_empty());
        let has_client_rules = config
            .collection_rules
            .iter()
            .any(|rule| !rule.http.client.is_empty());

        if !has_server_rules && !has_client_rules {
            crate::sp_debug!("No specific rules configured, collecting all requests");
            return true;
        }

        crate::sp_debug!("No rules matched, not collecting");
        false
    }

    fn is_exempted(&self, config: &Config, request_headers: &HashMap<String, String>) -> bool {
        if config.exemption_rules.is_empty() {
            return false;
        }

        let request_host = request_headers
            .get("host")
            .or_else(|| request_headers.get(":authority"))
            .cloned();
        let request_path = request_headers.get(":path").cloned();

        let (client_host, client_path) = crate::http_helpers::extract_client_info(request_headers);

        crate::sp_debug!(
            "Checking exemption - request_host: {:?}, request_path: {:?}, client_host: {:?}, client_path: {:?}",
            request_host, request_path, client_host, client_path
        );

        for rule in &config.exemption_rules {
            let host_matched = check_host_patterns(&rule.host_patterns, &request_host, &client_host);
            let path_matched = check_path_patterns(&rule.path_patterns, &request_path, &client_path);

            if host_matched && path_matched {
                crate::sp_info!(
                    "Request exempted by rule - hostPatterns: {:?}, pathPatterns: {:?}",
                    rule.host_patterns, rule.path_patterns
                );
                return true;
            }
        }

        false
    }
}

// Implement RequestHeadersAccess for concrete contexts (e.g., SpHttpContext) in their modules

fn check_inbound_rules(config: &Config, request_headers: &HashMap<String, String>) -> bool {
    if let Some(request_path) = request_headers.get(":path") {
        crate::sp_debug!("Checking inbound rules for path: {}", request_path);

        for (i, rule) in config.collection_rules.iter().enumerate() {
            if !rule.http.server.path.is_empty() {
                crate::sp_debug!("Checking inbound rule {}: serverPath='{}'", i, rule.http.server.path);
                if match_pattern(&rule.http.server.path, request_path) {
                    crate::sp_debug!("Inbound request matched server_path: {}", rule.http.server.path);
                    return true;
                }
            }
        }
    }
    false
}

fn check_outbound_rules(config: &Config, request_headers: &HashMap<String, String>) -> bool {
    let (client_host, client_path) = crate::http_helpers::extract_client_info(request_headers);
    crate::sp_debug!("Checking outbound rules with client_host: {:?}, client_path: {:?}", client_host, client_path);

    for (i, rule) in config.collection_rules.iter().enumerate() {
        if !rule.http.client.is_empty() {
            for client_config in &rule.http.client {
                crate::sp_debug!("Checking outbound rule {}: clientHost={}, clientPaths={:?}", i, client_config.host, client_config.paths);

                // Check client host
                if let Some(ref actual_client_host) = client_host {
                    if !match_pattern(&client_config.host, actual_client_host) {
                        crate::sp_debug!("Client host mismatch: expected={}, actual={}", client_config.host, actual_client_host);
                        continue;
                    }
                } else {
                    crate::sp_debug!("No client host info available, but rule requires it");
                    continue;
                }

                crate::sp_debug!("Client host matched");

                // Check client paths if configured
                if !client_config.paths.is_empty() {
                    if let Some(ref actual_client_path) = client_path {
                        let matched = client_config.paths.iter().any(|client_path| {
                            let matches = match_pattern(client_path, actual_client_path);
                            crate::sp_debug!("Client path match: pattern='{}' result={}", client_path, matches);
                            matches
                        });
                        if !matched {
                            crate::sp_debug!("Client paths did not match");
                            continue;
                        }
                    } else {
                        crate::sp_debug!("No client path info available");
                        continue;
                    }
                }

                crate::sp_debug!("Outbound request matched all criteria - client_host: {}, client_paths: {:?}", client_config.host, client_config.paths);
                return true;
            }
        }
    }
    false
}

// client info extraction is provided by crate::http_helpers::extract_client_info

fn check_host_patterns(
    host_patterns: &[String],
    request_host: &Option<String>,
    client_host: &Option<String>,
) -> bool {
    if host_patterns.is_empty() {
        return true;
    }

    // Check inbound request host
    if let Some(ref host) = request_host {
        for pattern in host_patterns {
            if match_pattern(pattern, host) {
                crate::sp_debug!("Host pattern '{}' matched request host '{}'", pattern, host);
                return true;
            }
        }
    }

    // Check outbound client host
    if let Some(ref host) = client_host {
        for pattern in host_patterns {
            if match_pattern(pattern, host) {
                crate::sp_debug!("Host pattern '{}' matched client host '{}'", pattern, host);
                return true;
            }
        }
    }

    false
}

fn check_path_patterns(
    path_patterns: &[String],
    request_path: &Option<String>,
    client_path: &Option<String>,
) -> bool {
    if path_patterns.is_empty() {
        return true;
    }

    // Check inbound request path
    if let Some(ref path) = request_path {
        for pattern in path_patterns {
            if match_pattern(pattern, path) {
                crate::sp_debug!("Path pattern '{}' matched request path '{}'", pattern, path);
                return true;
            }
        }
    }

    // Check outbound client path
    if let Some(ref path) = client_path {
        for pattern in path_patterns {
            if match_pattern(pattern, path) {
                crate::sp_debug!("Path pattern '{}' matched client path '{}'", pattern, path);
                return true;
            }
        }
    }

    false
}

fn match_pattern(pattern: &str, text: &str) -> bool {
    crate::sp_debug!("Matching pattern '{}' against text '{}'", pattern, text);
    match Regex::new(pattern) {
        Ok(re) => {
            let result = re.is_match(text);
            crate::sp_debug!("Regex match result: {}", result);
            result
        }
        Err(e) => {
            crate::sp_warn!("Invalid regex pattern '{}': {}", pattern, e);
            let result = pattern == text;
            crate::sp_debug!("Fallback to exact match: {}", result);
            result
        }
    }
}