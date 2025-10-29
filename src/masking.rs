// Data masking module for sensitive information protection
// Masks sensitive data before sending to backend

use regex::Regex;
use lazy_static::lazy_static;

use crate::config::MaskingConfig;

// Import OTEL types
use crate::otel::{KeyValue, any_value};

/// Sensitive data types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum SensitiveDataType {
    Phone,          // Phone number
    IdCard,         // ID card number
    Email,          // Email address
    BankCard,       // Bank card number
    Password,       // Password
    Token,          // Token/API Key
    IpAddress,      // IP address
    Unknown,        // Unknown type
}

/// Detect sensitive data type from string value
#[allow(dead_code)]
pub fn detect_sensitive_type(value: &str) -> SensitiveDataType {
    // Phone: 1[3-9]\d{9}
    if PHONE_REGEX.is_match(value) {
        return SensitiveDataType::Phone;
    }

    // ID card: 18 digits or 17 digits + X
    if ID_CARD_REGEX.is_match(value) {
        return SensitiveDataType::IdCard;
    }

    // Email
    if EMAIL_REGEX.is_match(value) {
        return SensitiveDataType::Email;
    }

    // Bank card: 13-19 digits
    if BANK_CARD_REGEX.is_match(value) {
        return SensitiveDataType::BankCard;
    }

    // Token/Key: Bearer/sk-/api_key prefix
    if TOKEN_REGEX.is_match(value) {
        return SensitiveDataType::Token;
    }

    // IP address
    if IP_REGEX.is_match(value) {
        return SensitiveDataType::IpAddress;
    }

    SensitiveDataType::Unknown
}

/// Mask a single string value
/// Keeps prefix and suffix characters, replaces middle with asterisks
pub fn mask_string(value: &str, keep_prefix: usize, keep_suffix: usize) -> String {
    let len = value.chars().count();

    // If too short, mask all
    if len <= keep_prefix + keep_suffix {
        return "*".repeat(len);
    }

    let prefix: String = value.chars().take(keep_prefix).collect();
    let suffix: String = value.chars().skip(len - keep_suffix).collect();
    let mask_len = len - keep_prefix - keep_suffix;

   // crate::sp_debug!("Masked string body result {})", format!("{}{}{}", prefix, "*".repeat(mask_len), suffix));

    format!("{}{}{}", prefix, "*".repeat(mask_len), suffix)
}

/// Mask sensitive fields in JSON body
/// Uses simple regex replacement to avoid full JSON parsing (for performance)
pub fn mask_json_body(json_str: &str, config: &MaskingConfig) -> String {
    let mut result = json_str.to_string();

    // Sensitive field names
    let sensitive_fields = vec![
        "phone", "mobile", "tel", "telephone", "cellphone",
        "idCard", "id_card", "identity", "identityCard",
        "email", "mail", "emailAddress",
        "password", "pwd", "passwd", "pass",
        "bankCard", "bank_card", "card_no", "cardNo", "cardNumber",
        "token", "accessToken", "access_token", "refreshToken",
        "apiKey", "api_key", "secret", "secretKey",
    ];

    for field in sensitive_fields {
        // Match "field": "value" or "field":"value" format
        let pattern = format!(r#""{}"\s*:\s*"([^"]+)""#, field);
        if let Ok(re) = Regex::new(&pattern) {
            result = re.replace_all(&result, |caps: &regex::Captures| {
                let value = &caps[1];
                let masked = mask_string(value, config.keep_prefix_length, config.keep_suffix_length);
                format!(r#""{}": "{}""#, field, masked)
            }).to_string();
        }
    }
  //  crate::sp_debug!("Masked json body result {})", result);
    result
}

/// Mask span attributes based on configuration
pub fn mask_span_attributes(
    attributes: &mut Vec<KeyValue>,
    config: &MaskingConfig,
) {
    if !config.enabled {
        return;
    }

    crate::sp_debug!("Masking span attributes (enabled={}, request_headers={:?}, response_headers={:?})",
        config.enabled, config.mask_request_headers, config.mask_response_headers);

    for attr in attributes.iter_mut() {
        // Determine if this attribute should be masked
        let should_mask =
            // Mask request headers
            (attr.key.starts_with("http.request.header.") &&
             config.mask_request_headers.iter().any(|h| {
                 let header_key = attr.key.trim_start_matches("http.request.header.");
                 header_key.eq_ignore_ascii_case(h)
             })) ||
            // Mask response headers
            (attr.key.starts_with("http.response.header.") &&
             config.mask_response_headers.iter().any(|h| {
                 let header_key = attr.key.trim_start_matches("http.response.header.");
                 header_key.eq_ignore_ascii_case(h)
             })) ||
            // Mask request body
            (attr.key == "http.request.body" && config.mask_request_body) ||
            // Mask response body
            (attr.key == "http.response.body" && config.mask_response_body);

        if !should_mask {
            continue;
        }

        crate::sp_debug!("Masking attribute: {}", attr.key);

        // Mask the value
        if let Some(ref mut any_value) = attr.value {
            if let Some(any_value::Value::StringValue(ref mut value)) = any_value.value {
                let original_len = value.len();

                // If it's a body, try JSON masking
                if attr.key.ends_with(".body") {
                    *value = mask_json_body(value, config);
                } else {
                    // Plain string masking
                    *value = mask_string(value, config.keep_prefix_length, config.keep_suffix_length);
                }

                crate::sp_debug!("Masked {} (length: {} -> {})", attr.key, original_len, value.len());
            }
        }
    }

    crate::sp_info!("Span attributes masking completed");
}

// Regex patterns using lazy_static for compilation once
lazy_static! {
    /// Chinese mobile phone number pattern
    static ref PHONE_REGEX: Regex = Regex::new(r"1[3-9]\d{9}").unwrap();

    /// Chinese ID card pattern (18 digits or 17 digits + X)
    static ref ID_CARD_REGEX: Regex = Regex::new(r"\d{17}[\dXx]").unwrap();

    /// Email pattern
    static ref EMAIL_REGEX: Regex = Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap();

    /// Bank card pattern (13-19 digits)
    static ref BANK_CARD_REGEX: Regex = Regex::new(r"\d{13,19}").unwrap();

    /// Token/Key pattern (Bearer, sk-, api_key, etc.)
    static ref TOKEN_REGEX: Regex = Regex::new(r"(Bearer|sk-|api_key|token)[\w\-._]+").unwrap();

    /// IP address pattern
    static ref IP_REGEX: Regex = Regex::new(r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}").unwrap();
}

#[cfg(test)]
mod tests {
    use crate::otel::AnyValue;
    use super::*;

    #[test]
    fn test_mask_string() {
        assert_eq!(mask_string("13812345678", 3, 4), "138*****678");
        assert_eq!(mask_string("abc", 3, 4), "***");
        assert_eq!(mask_string("", 3, 4), "");
        assert_eq!(mask_string("hello", 2, 2), "he*lo");
        assert_eq!(mask_string("x", 1, 1), "*");
    }

    #[test]
    fn test_detect_sensitive_type() {
        assert_eq!(detect_sensitive_type("13812345678"), SensitiveDataType::Phone);
        assert_eq!(detect_sensitive_type("110101199001011234"), SensitiveDataType::IdCard);
        assert_eq!(detect_sensitive_type("alice@example.com"), SensitiveDataType::Email);
        assert_eq!(detect_sensitive_type("6222021234567890123"), SensitiveDataType::BankCard);
        assert_eq!(detect_sensitive_type("Bearer eyJhbGciOiJIUzI1NiJ9"), SensitiveDataType::Token);
        assert_eq!(detect_sensitive_type("192.168.1.1"), SensitiveDataType::IpAddress);
        assert_eq!(detect_sensitive_type("normal text"), SensitiveDataType::Unknown);
    }

    #[test]
    fn test_mask_json_body() {
        let json = r#"{"phone":"13812345678","name":"Alice","email":"alice@example.com"}"#;
        let config = MaskingConfig::default();
        let masked = mask_json_body(json, &config);

        // Phone should be masked
        assert!(masked.contains("138*****678") || masked.contains("138******78"));
        // Email should be masked
        assert!(masked.contains("***"));
        // Name should not be masked
        assert!(masked.contains("Alice"));
    }

    #[test]
    fn test_mask_span_attributes() {
        let mut attrs = vec![
            KeyValue {
                key: "http.request.header.authorization".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue("Bearer token123456".to_string())),
                }),
            },
            KeyValue {
                key: "http.request.header.content-type".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue("application/json".to_string())),
                }),
            },
        ];

        let config = MaskingConfig::default();
        mask_span_attributes(&mut attrs, &config);

        // Authorization should be masked
        if let Some(AnyValue { value: Some(any_value::Value::StringValue(v)) }) = &attrs[0].value {
            assert!(v.contains("***"));
        }

        // Content-Type should NOT be masked
        if let Some(AnyValue { value: Some(any_value::Value::StringValue(v)) }) = &attrs[1].value {
            assert_eq!(v, "application/json");
        }
    }

    #[test]
    fn test_mask_disabled() {
        let mut attrs = vec![
            KeyValue {
                key: "http.request.header.authorization".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue("Bearer token123456".to_string())),
                }),
            },
        ];

        let mut config = MaskingConfig::default();
        config.enabled = false;

        mask_span_attributes(&mut attrs, &config);

        // Should NOT be masked
        if let Some(AnyValue { value: Some(any_value::Value::StringValue(v)) }) = &attrs[0].value {
            assert_eq!(v, "Bearer token123456");
        }
    }
}
