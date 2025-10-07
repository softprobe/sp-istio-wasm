use std::collections::HashMap;

/// Detect service name from headers or configuration
pub fn detect_service_name(
    request_headers: &HashMap<String, String>,
    config_service_name: &str,
) -> String {
    // Use configured service_name if it's not default
    if !config_service_name.is_empty() && config_service_name != "default-service" {
        log::error!("SP: ✓ Using configured service_name: {}", config_service_name);
        return config_service_name.to_string();
    }

    let current_service_headers = vec!["x-sp-service-name"];
    for header_name in current_service_headers {
        if let Some(header_value) = request_headers.get(header_name) {
            if !header_value.is_empty() {
                log::error!(
                    "SP: ✓ Got service_name from {} header: {}",
                    header_name,
                    header_value
                );
                return header_value.clone();
            }
        }
    }
    config_service_name.to_string()
}

/// Build new tracestate with x-sp-traceparent entry
pub fn build_new_tracestate(
    request_headers: &HashMap<String, String>,
    traceparent_value: &str,
) -> String {
    let mut tracestate_entries = Vec::new();

    if let Some(existing_tracestate) = request_headers.get("tracestate") {
        // Parse existing tracestate, preserve other entries
        for entry in existing_tracestate.split(',') {
            let entry = entry.trim();
            if !entry.starts_with("x-sp-traceparent=") {
                tracestate_entries.push(entry.to_string());
            }
        }
    }

    // Add x-sp-traceparent entry with full traceparent format
    tracestate_entries.insert(0, format!("x-sp-traceparent={}", traceparent_value));

    let new_tracestate = tracestate_entries.join(",");
    log::debug!("SP: Adding x-sp-traceparent to tracestate: {}", new_tracestate);

    new_tracestate
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_detect_service_name_with_configured_name() {
        let headers = HashMap::new();
        let config_name = "my-service";
        
        let result = detect_service_name(&headers, config_name);
        assert_eq!(result, "my-service");
    }

    #[test]
    fn test_detect_service_name_with_default_config() {
        let headers = HashMap::new();
        let config_name = "default-service";
        
        let result = detect_service_name(&headers, config_name);
        assert_eq!(result, "default-service");
    }

    #[test]
    fn test_detect_service_name_from_header() {
        let mut headers = HashMap::new();
        headers.insert("x-sp-service-name".to_string(), "header-service".to_string());
        let config_name = "default-service";
        
        let result = detect_service_name(&headers, config_name);
        assert_eq!(result, "header-service");
    }

    #[test]
    fn test_detect_service_name_header_overrides_config() {
        let mut headers = HashMap::new();
        headers.insert("x-sp-service-name".to_string(), "header-service".to_string());
        let config_name = "my-service";
        
        let result = detect_service_name(&headers, config_name);
        assert_eq!(result, "my-service"); // Config takes precedence if not default
    }

    #[test]
    fn test_detect_service_name_empty_header() {
        let mut headers = HashMap::new();
        headers.insert("x-sp-service-name".to_string(), "".to_string());
        let config_name = "default-service";
        
        let result = detect_service_name(&headers, config_name);
        assert_eq!(result, "default-service");
    }

    #[test]
    fn test_build_new_tracestate_with_no_existing() {
        let headers = HashMap::new();
        let traceparent = "00-12345678901234567890123456789012-1234567890123456-01";
        
        let result = build_new_tracestate(&headers, traceparent);
        assert_eq!(result, "x-sp-traceparent=00-12345678901234567890123456789012-1234567890123456-01");
    }

    #[test]
    fn test_build_new_tracestate_with_existing_entries() {
        let mut headers = HashMap::new();
        headers.insert("tracestate".to_string(), "vendor1=value1,vendor2=value2".to_string());
        let traceparent = "00-12345678901234567890123456789012-1234567890123456-01";
        
        let result = build_new_tracestate(&headers, traceparent);
        assert_eq!(result, "x-sp-traceparent=00-12345678901234567890123456789012-1234567890123456-01,vendor1=value1,vendor2=value2");
    }

    #[test]
    fn test_build_new_tracestate_replaces_existing_sp_entry() {
        let mut headers = HashMap::new();
        headers.insert("tracestate".to_string(), "x-sp-traceparent=old-value,vendor1=value1".to_string());
        let traceparent = "00-12345678901234567890123456789012-1234567890123456-01";
        
        let result = build_new_tracestate(&headers, traceparent);
        assert_eq!(result, "x-sp-traceparent=00-12345678901234567890123456789012-1234567890123456-01,vendor1=value1");
    }

    #[test]
    fn test_build_new_tracestate_handles_whitespace() {
        let mut headers = HashMap::new();
        headers.insert("tracestate".to_string(), " vendor1=value1 , vendor2=value2 ".to_string());
        let traceparent = "00-12345678901234567890123456789012-1234567890123456-01";
        
        let result = build_new_tracestate(&headers, traceparent);
        assert_eq!(result, "x-sp-traceparent=00-12345678901234567890123456789012-1234567890123456-01,vendor1=value1,vendor2=value2");
    }

    #[test]
    fn test_build_new_tracestate_empty_existing() {
        let mut headers = HashMap::new();
        headers.insert("tracestate".to_string(), "".to_string());
        let traceparent = "00-12345678901234567890123456789012-1234567890123456-01";
        
        let result = build_new_tracestate(&headers, traceparent);
        assert_eq!(result, "x-sp-traceparent=00-12345678901234567890123456789012-1234567890123456-01");
    }
}