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
                log::info!("SP: Detected listener_direction: {}", direction);
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
                log::info!("SP: Detected direction from metadata: {}", direction);
                return direction;
            }
        }

        // Method 3: Check cluster name pattern
        if let Some(cluster_name) = self.get_context_property(vec!["cluster_name"]) {
            if let Ok(cluster) = String::from_utf8(cluster_name) {
                log::info!("SP: Detected cluster_name: {}", cluster);
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
                log::info!("SP: Detected downstream address: {}", address);
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
            log::info!("SP: Found x-forwarded-for header, likely inbound traffic");
            return "inbound".to_string();
        }

        if let Some(host) = self.get_request_header("host").or_else(|| self.get_request_header(":authority")) {
            if !host.contains("localhost") && !host.contains("127.0.0.1") && !host.contains(".local") {
                log::info!("SP: External host detected: {}, likely outbound traffic", host);
                return "outbound".to_string();
            }
        }

        log::info!("SP: Could not determine traffic direction, using 'auto'");
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
            log::info!("SP: Request is exempted from collection");
            return false;
        }

        // If no rules configured, collect all requests
        if config.collection_rules.is_empty() {
            log::info!("SP: No collection rules configured, collecting all requests");
            return true;
        }

        log::info!(
            "SP: Checking collection rules, total rules: {}",
            config.collection_rules.len()
        );

        // Try inbound rules matching
        let inbound_matched = check_inbound_rules(config, request_headers);
        if inbound_matched {
            log::info!("SP: Request matched inbound rules, collecting");
            return true;
        }

        // Try outbound rules matching
        let outbound_matched = check_outbound_rules(config, request_headers);
        if outbound_matched {
            log::info!("SP: Request matched outbound rules, collecting");
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
            log::info!("SP: No specific rules configured, collecting all requests");
            return true;
        }

        log::info!("SP: No rules matched, not collecting");
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

        log::debug!(
            "SP: Checking exemption - request_host: {:?}, request_path: {:?}, client_host: {:?}, client_path: {:?}",
            request_host, request_path, client_host, client_path
        );

        for rule in &config.exemption_rules {
            let host_matched = check_host_patterns(&rule.host_patterns, &request_host, &client_host);
            let path_matched = check_path_patterns(&rule.path_patterns, &request_path, &client_path);

            if host_matched && path_matched {
                log::info!(
                    "SP: Request exempted by rule - hostPatterns: {:?}, pathPatterns: {:?}",
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
        log::debug!("SP: Checking inbound rules for path: {}", request_path);

        for (i, rule) in config.collection_rules.iter().enumerate() {
            if !rule.http.server.path.is_empty() {
                log::debug!(
                    "SP: Checking inbound rule {}: serverPath='{}'",
                    i,
                    rule.http.server.path
                );
                if match_pattern(&rule.http.server.path, request_path) {
                    log::debug!(
                        "SP: Inbound request matched server_path: {}",
                        rule.http.server.path
                    );
                    return true;
                }
            }
        }
    }
    false
}

fn check_outbound_rules(config: &Config, request_headers: &HashMap<String, String>) -> bool {
    let (client_host, client_path) = crate::http_helpers::extract_client_info(request_headers);
    log::debug!(
        "SP: Checking outbound rules with client_host: {:?}, client_path: {:?}",
        client_host, client_path
    );

    for (i, rule) in config.collection_rules.iter().enumerate() {
        if !rule.http.client.is_empty() {
            for client_config in &rule.http.client {
                log::debug!(
                    "SP: Checking outbound rule {}: clientHost={}, clientPaths={:?}",
                    i, client_config.host, client_config.paths
                );

                // Check client host
                if let Some(ref actual_client_host) = client_host {
                    if !match_pattern(&client_config.host, actual_client_host) {
                        log::debug!(
                            "SP: Client host did not match: expected={}, actual={}",
                            client_config.host, actual_client_host
                        );
                        continue;
                    }
                } else {
                    log::debug!("SP: No client host info available, but rule requires it");
                    continue;
                }

                log::debug!("SP: Client host matched");

                // Check client paths if configured
                if !client_config.paths.is_empty() {
                    if let Some(ref actual_client_path) = client_path {
                        let matched = client_config.paths.iter().any(|client_path| {
                            let matches = match_pattern(client_path, actual_client_path);
                            log::debug!(
                                "SP: Client path match check: pattern='{}', text='{}', result={}",
                                client_path, actual_client_path, matches
                            );
                            matches
                        });
                        if !matched {
                            log::info!("SP: Client paths did not match");
                            continue;
                        }
                    } else {
                        log::debug!("SP: No client path info available");
                        continue;
                    }
                }

                log::info!(
                    "SP: Outbound request matched all criteria - client_host: {}, client_paths: {:?}",
                    client_config.host, client_config.paths
                );
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
                log::debug!("SP: Host pattern '{}' matched request host '{}'", pattern, host);
                return true;
            }
        }
    }

    // Check outbound client host
    if let Some(ref host) = client_host {
        for pattern in host_patterns {
            if match_pattern(pattern, host) {
                log::debug!("SP: Host pattern '{}' matched client host '{}'", pattern, host);
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
                log::debug!("SP: Path pattern '{}' matched request path '{}'", pattern, path);
                return true;
            }
        }
    }

    // Check outbound client path
    if let Some(ref path) = client_path {
        for pattern in path_patterns {
            if match_pattern(pattern, path) {
                log::debug!("SP: Path pattern '{}' matched client path '{}'", pattern, path);
                return true;
            }
        }
    }

    false
}

fn match_pattern(pattern: &str, text: &str) -> bool {
    log::debug!("SP: Matching pattern '{}' against text '{}'", pattern, text);
    match Regex::new(pattern) {
        Ok(re) => {
            let result = re.is_match(text);
            log::debug!("SP: Regex match result: {}", result);
            result
        }
        Err(e) => {
            log::warn!("SP: Invalid regex pattern '{}': {}", pattern, e);
            let result = pattern == text;
            log::debug!("SP: Fallback to exact match: {}", result);
            result
        }
    }
}