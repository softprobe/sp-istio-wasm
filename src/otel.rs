use std::collections::HashMap;
// Note: SystemTime is not available in WASM runtime, will use proxy-wasm host functions
use prost::Message;
use serde::{Serialize, Deserialize};
use proxy_wasm;
// use std::sync::atomic::{AtomicU64, Ordering};

// Include generated protobuf types
pub mod opentelemetry {
    pub mod proto {
        pub mod common {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/opentelemetry.proto.common.v1.rs"));
            }
        }
        pub mod resource {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/opentelemetry.proto.resource.v1.rs"));
            }
        }
        pub mod trace {
            pub mod v1 {
                include!(concat!(env!("OUT_DIR"), "/opentelemetry.proto.trace.v1.rs"));
            }
        }
    }
}

// Re-export commonly used types
pub use opentelemetry::proto::common::v1::{AnyValue, KeyValue, any_value};
pub use opentelemetry::proto::resource::v1::Resource;
pub use opentelemetry::proto::trace::v1::{TracesData, ResourceSpans, ScopeSpans, Span, Status, span};

#[derive(Clone)]
pub struct SpanBuilder {
    trace_id: Vec<u8>,
    parent_span_id: Option<Vec<u8>>,
    current_span_id: Vec<u8>,  // 添加当前 span ID 字段
    service_name: String,
    traffic_direction: String,  // 添加traffic_direction字段
    api_key: String,
    session_id: String
}

impl SpanBuilder {
    pub fn new() -> Self {
        Self {
            trace_id: generate_trace_id(),
            parent_span_id: None,
            current_span_id: generate_span_id(),  // 初始化当前 span ID
            service_name: "default-service".to_string(),
            traffic_direction: "outbound".to_string(),  // 默认值
            api_key: String::new(),
            session_id: String::new()
        }
    }
    // 添加设置service_name的方法
    pub fn with_service_name(mut self, service_name: String) -> Self {
        self.service_name = service_name;
        self
    }

    // 添加设置traffic_direction的方法
    pub fn with_traffic_direction(mut self, traffic_direction: String) -> Self {
        self.traffic_direction = traffic_direction;
        self
    }

    // 添加设置api_key的方法
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = api_key;
        self
    }
    
    /// Check if session_id is present and not empty
    pub fn has_session_id(&self) -> bool {
        !self.session_id.is_empty()
    }
    
    /// Get session_id value for logging purposes
    pub fn get_session_id(&self) -> &str {
        &self.session_id
    }
    
    /// Get trace_id as hex string
    pub fn get_current_span_id_hex(&self) -> String {
        self.current_span_id.iter().map(|b| format!("{:02x}", b)).collect::<String>()
    }

    pub fn get_trace_id_hex(&self) -> String {
        self.trace_id.iter().map(|b| format!("{:02x}", b)).collect::<String>()
    }

    pub fn with_context(mut self, headers: &HashMap<String, String>) -> Self {
        // Extract trace context from tracestate x-sp-traceparent if present
        if let Some(tracestate) = headers.get("tracestate") {
            log::error!("DEBUG: Found tracestate in headers: {}", tracestate);
            
            // 解析 tracestate 中的 x-sp-traceparent
            for entry in tracestate.split(',') {
                let entry = entry.trim();
                if let Some(value) = entry.strip_prefix("x-sp-traceparent=") {
                    log::error!("DEBUG: Found x-sp-traceparent in tracestate: {}", value);
                    // 解析完整的 traceparent 格式: 00-trace_id-span_id-01
                    if let Some((trace_id, span_id)) = parse_traceparent(value) {
                        self.trace_id = trace_id;
                        self.parent_span_id = Some(span_id);
                        log::error!("DEBUG: Successfully parsed trace context from x-sp-traceparent");
                        break;
                    }
                }
            }
        }

        // Get session ID from headers directly
        log::error!("DEBUG: Looking for session_id in headers...");
        let session_id_found = headers.get("x-sp-session-id")
            .or_else(|| headers.get("sp_session_id"));

        if let Some(session_id) = session_id_found {
            log::error!("DEBUG: Found session_id in headers: '{}'", session_id);
            self.session_id = session_id.clone();
        } else {
            log::error!("DEBUG: No session_id found in headers: {:?}", headers.keys().collect::<Vec<_>>());
        }

        // If no valid trace context found, generate new one
        if self.trace_id.is_empty() {
            self.trace_id = generate_trace_id();
        }
        
        self
    }

    pub fn create_inject_span(
        &self,
        request_headers: &HashMap<String, String>,
        request_body: &[u8],
        url_host: Option<&str>,
        url_path: Option<&str>,
    ) -> TracesData {
        let span_id = self.current_span_id.clone();  // 使用 SpanBuilder 中的 current_span_id
        let mut attributes = Vec::new();

        // Add service name attribute
        let service_name = if self.service_name.is_empty() {
            "default-service".to_string()
        } else {
            self.service_name.clone()
        };

        attributes.push(KeyValue {
            key: "sp.service.name".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(service_name)),
            }),
        });

        // Add span type attribute
        attributes.push(KeyValue {
            key: "span.type".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue("sp-envoy-proxy".to_string())),
            }),
        });

        // Add traffic direction attribute
        attributes.push(KeyValue {
            key: "sp.traffic.direction".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(self.traffic_direction.clone())),
            }),
        });

        // Add API key attribute if present
        if !self.api_key.is_empty() {
            attributes.push(KeyValue {
                key: "sp.api.key".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(self.api_key.clone())),
                }),
            });
        }

        // Add span type attribute
        attributes.push(KeyValue {
            key: "sp.span.type".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue("inject".to_string())),
            }),
        });

        // Add session ID attribute if present
        if !self.session_id.is_empty() {
            attributes.push(KeyValue {
                key: "sp.session.id".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(self.session_id.clone())),
                }),
            });
        }
        
        // Add request headers as attributes
        for (key, value) in request_headers {
            if !should_skip_header(key) {
                attributes.push(KeyValue {
                    key: format!("http.request.header.{}", key.to_lowercase()),
                    value: Some(AnyValue {
                        value: Some(any_value::Value::StringValue(value.clone())),
                    }),
                });
            }
        }
        
        // Add url attributes if available
        if let Some(path) = url_path {
            attributes.push(KeyValue {
                key: "url.path".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(path.to_string())),
                }),
            });
        }
        if let Some(host) = url_host {
            attributes.push(KeyValue {
                key: "url.host".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(host.to_string())),
                }),
            });
        }

        // Add request body if present and text-based
        if !request_body.is_empty() {
            let body_value = if is_text_content(request_headers) {
                String::from_utf8_lossy(request_body).to_string()
            } else {
                use base64::{Engine as _, engine::general_purpose};
                general_purpose::STANDARD.encode(request_body)
            };
            
            attributes.push(KeyValue {
                key: "http.request.body".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(body_value)),
                }),
            });
        }
        
        let span = Span {
            trace_id: self.trace_id.clone(),
            span_id,
            parent_span_id: self.parent_span_id.clone().unwrap_or_default(),
            name: url_path.unwrap_or("unknown_path").to_string(),
            kind: span::SpanKind::Client as i32,
            start_time_unix_nano: get_current_timestamp_nanos(),
            end_time_unix_nano: get_current_timestamp_nanos(),
            attributes,
            flags: 0,
            ..Default::default()
        };
        
        self.create_traces_data(span)
    }

    pub fn create_extract_span(
        &self,
        request_headers: &HashMap<String, String>,
        request_body: &[u8],
        response_headers: &HashMap<String, String>,
        response_body: &[u8],
        url_host: Option<&str>,
        url_path: Option<&str>,
    ) -> TracesData {
        let span_id = self.current_span_id.clone();
        let mut attributes = Vec::new();

        log::error!("DEBUG: service_name value: '{}'", self.service_name);
        attributes.push(KeyValue {
            key: "sp.service.name".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(self.service_name.clone())),
            }),
        });

        // Add traffic direction attribute
        log::error!("DEBUG: traffic_direction value: '{}'", self.traffic_direction);
        attributes.push(KeyValue {
            key: "sp.traffic.direction".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue(self.traffic_direction.clone())),
            }),
        });

        // Add span type attribute
        attributes.push(KeyValue {
            key: "span.type".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue("sp-envoy-proxy".to_string())),
            }),
        });

        // Add extract span type attribute
        attributes.push(KeyValue {
            key: "sp.span.type".to_string(),
            value: Some(AnyValue {
                value: Some(any_value::Value::StringValue("extract".to_string())),
            }),
        });

        // Add API key attribute if present
        log::error!("DEBUG: api_key value: '{}'", self.api_key);
        if !self.api_key.is_empty() {
            log::error!("DEBUG: Adding api_key attribute");
            attributes.push(KeyValue {
                key: "sp.api.key".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(self.api_key.clone())),
                }),
            });
        } else {
            log::error!("DEBUG: api_key is empty, not adding attribute");
        }

        // Add session ID attribute if present
        log::error!("DEBUG: session_id value: '{}'", self.session_id);
        if !self.session_id.is_empty() {
            log::error!("DEBUG: Adding session_id attribute");
            attributes.push(KeyValue {
                key: "sp.session.id".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(self.session_id.clone())),
                }),
            });
        } else {
            log::error!("DEBUG: session_id is empty, not adding attribute");
        }

        // Add request headers
        for (key, value) in request_headers {
            if !should_skip_header(key) {
                attributes.push(KeyValue {
                    key: format!("http.request.header.{}", key.to_lowercase()),
                    value: Some(AnyValue {
                        value: Some(any_value::Value::StringValue(value.clone())),
                    }),
                });
            }
        }

        // Add url attributes if available
        if let Some(path) = url_path {
            attributes.push(KeyValue {
                key: "url.path".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(path.to_string())),
                }),
            });
        }
        if let Some(host) = url_host {
            attributes.push(KeyValue {
                key: "url.host".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(host.to_string())),
                }),
            });
        }

        // Add request body
        if !request_body.is_empty() {
            let body_value = if is_text_content(request_headers) {
                String::from_utf8_lossy(request_body).to_string()
            } else {
                use base64::{Engine as _, engine::general_purpose};
                general_purpose::STANDARD.encode(request_body)
            };

            attributes.push(KeyValue {
                key: "http.request.body".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(body_value)),
                }),
            });
        }

        // Add response headers
        for (key, value) in response_headers {
            if !should_skip_header(key) {
                attributes.push(KeyValue {
                    key: format!("http.response.header.{}", key.to_lowercase()),
                    value: Some(AnyValue {
                        value: Some(any_value::Value::StringValue(value.clone())),
                    }),
                });
            }
        }

        // Add response status code
        if let Some(status) = response_headers.get(":status") {
            if let Ok(status_code) = status.parse::<i64>() {
                attributes.push(KeyValue {
                    key: "http.response.status_code".to_string(),
                    value: Some(AnyValue {
                        value: Some(any_value::Value::IntValue(status_code)),
                    }),
                });
            }
        }

        // Add response body
        if !response_body.is_empty() {
            let body_value = if is_text_content(response_headers) {
                String::from_utf8_lossy(response_body).to_string()
            } else {
                use base64::{Engine as _, engine::general_purpose};
                general_purpose::STANDARD.encode(response_body)
            };

            attributes.push(KeyValue {
                key: "http.response.body".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(body_value)),
                }),
            });
        }

        let span = Span {
            trace_id: self.trace_id.clone(),
            span_id,
            parent_span_id: self.parent_span_id.clone().unwrap_or_default(),
            name: url_path.unwrap_or("unknown_path").to_string(),
            kind: span::SpanKind::Server as i32,
            start_time_unix_nano: get_current_timestamp_nanos(),
            end_time_unix_nano: get_current_timestamp_nanos(),
            attributes,
            status: Some(Status {
                code: 1, // STATUS_CODE_OK
                message: String::new(),
            }),
            flags: 0,
            ..Default::default()
        };

        self.create_traces_data(span)
    }

    fn create_traces_data(&self, span: Span) -> TracesData {
        // Create resource with service.name attribute
        let service_name = if self.service_name.is_empty() {
            "default-service".to_string()
        } else {
            self.service_name.clone()
        };

        let resource = Resource {
            attributes: vec![KeyValue {
                key: "service.name".to_string(),
                value: Some(AnyValue {
                    value: Some(any_value::Value::StringValue(service_name)),
                }),
            }],
            dropped_attributes_count: 0,
            entity_refs: vec![],
        };

        TracesData {
            resource_spans: vec![ResourceSpans {
                resource: Some(resource),
                scope_spans: vec![ScopeSpans {
                    spans: vec![span],
                    ..Default::default()
                }],
                ..Default::default()
            }],
        }
    }

    /// Generate W3C traceparent header value
    /// Format: 00-{trace_id}-{span_id}-{trace_flags}
    pub fn generate_traceparent(&self, span_id: &[u8]) -> String {
        let version = "00";
        let trace_id_hex = hex_encode(&self.trace_id);
        let span_id_hex = hex_encode(span_id);
        let trace_flags = "01"; // sampled flag set
        
        format!("{}-{}-{}-{}", version, trace_id_hex, span_id_hex, trace_flags)
    }

    }

// 简化的JSON结构用于序列化
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonTracesData {
    pub resource_spans: Vec<JsonResourceSpans>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonResourceSpans {
    pub resource: JsonResource,
    pub scope_spans: Vec<JsonScopeSpans>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonResource {
    pub attributes: Vec<JsonKeyValue>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonScopeSpans {
    pub spans: Vec<JsonSpan>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonSpan {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: i32,
    pub start_time_unix_nano: u64,
    pub end_time_unix_nano: u64,
    pub attributes: Vec<JsonKeyValue>,
    pub status: JsonStatus,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonKeyValue {
    pub key: String,
    pub value: JsonAnyValue,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonAnyValue {
    pub string_value: Option<String>,
    pub int_value: Option<i64>,
    pub double_value: Option<f64>,
    pub bool_value: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonStatus {
    pub code: i32,
    pub message: String,
}

// 转换函数：从protobuf转换为JSON结构
fn convert_traces_data_to_json(traces_data: &TracesData) -> JsonTracesData {
    JsonTracesData {
        resource_spans: traces_data.resource_spans.iter().map(|rs| {
            JsonResourceSpans {
                resource: JsonResource {
                    attributes: rs.resource.as_ref().map_or(Vec::new(), |r| {
                        r.attributes.iter().map(|attr| {
                            JsonKeyValue {
                                key: attr.key.clone(),
                                value: convert_any_value(&attr.value),
                            }
                        }).collect()
                    }),
                },
                scope_spans: rs.scope_spans.iter().map(|ss| {
                    JsonScopeSpans {
                        spans: ss.spans.iter().map(|span| {
                            JsonSpan {
                                trace_id: hex_encode(&span.trace_id),
                                span_id: hex_encode(&span.span_id),
                                parent_span_id: if span.parent_span_id.is_empty() {
                                    None
                                } else {
                                    Some(hex_encode(&span.parent_span_id))
                                },
                                name: span.name.clone(),
                                kind: span.kind,
                                start_time_unix_nano: span.start_time_unix_nano,
                                end_time_unix_nano: span.end_time_unix_nano,
                                attributes: span.attributes.iter().map(|attr| {
                                    JsonKeyValue {
                                        key: attr.key.clone(),
                                        value: convert_any_value(&attr.value),
                                    }
                                }).collect(),
                                status: JsonStatus {
                                    code: span.status.as_ref().map_or(0, |s| s.code),
                                    message: span.status.as_ref().map_or(String::new(), |s| s.message.clone()),
                                },
                            }
                        }).collect(),
                    }
                }).collect(),
            }
        }).collect(),
    }
}

fn convert_any_value(value: &Option<AnyValue>) -> JsonAnyValue {
    match value {
        Some(av) => {
            match &av.value {
                Some(any_value::Value::StringValue(s)) => JsonAnyValue {
                    string_value: Some(s.clone()),
                    int_value: None,
                    double_value: None,
                    bool_value: None,
                },
                Some(any_value::Value::IntValue(i)) => JsonAnyValue {
                    string_value: None,
                    int_value: Some(*i),
                    double_value: None,
                    bool_value: None,
                },
                Some(any_value::Value::DoubleValue(d)) => JsonAnyValue {
                    string_value: None,
                    int_value: None,
                    double_value: Some(*d),
                    bool_value: None,
                },
                Some(any_value::Value::BoolValue(b)) => JsonAnyValue {
                    string_value: None,
                    int_value: None,
                    double_value: None,
                    bool_value: Some(*b),
                },
                _ => JsonAnyValue {
                    string_value: None,
                    int_value: None,
                    double_value: None,
                    bool_value: None,
                },
            }
        }
        None => JsonAnyValue {
            string_value: None,
            int_value: None,
            double_value: None,
            bool_value: None,
        },
    }
}

// 添加JSON序列化函数
pub fn serialize_traces_data_json(traces_data: &TracesData) -> Result<Vec<u8>, serde_json::Error> {
    let json_data = convert_traces_data_to_json(traces_data);
    let json_string = serde_json::to_string(&json_data)?;
    Ok(json_string.into_bytes())
}

// 保留原有的protobuf序列化函数
pub fn serialize_traces_data(traces_data: &TracesData) -> Result<Vec<u8>, prost::EncodeError> {
    let mut buf = Vec::new();
    traces_data.encode(&mut buf)?;
    Ok(buf)
}

fn generate_trace_id() -> Vec<u8> {
    let mut trace_id = vec![0u8; 16];
    
    // Use current timestamp as source of randomness
    let now_nanos = get_current_timestamp_nanos();
    let secs = (now_nanos / 1_000_000_000) as u64;
    let nanos = (now_nanos % 1_000_000_000) as u64;
    
    // Fill first 8 bytes with seconds
    trace_id[0..8].copy_from_slice(&secs.to_be_bytes());
    // Fill last 8 bytes with nanoseconds
    trace_id[8..16].copy_from_slice(&nanos.to_be_bytes());
    
    trace_id
}

pub fn generate_span_id() -> Vec<u8> {
    let mut span_id = vec![0u8; 8];
    
    // Use current timestamp as source of randomness
    let now_nanos = get_current_timestamp_nanos();
    
    // Add some variation to make it different from trace ID
    let varied_nanos = now_nanos ^ 0xCAFEBABE;
    span_id.copy_from_slice(&varied_nanos.to_be_bytes());
    
    span_id
}

fn parse_traceparent(traceparent: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let parts: Vec<&str> = traceparent.split('-').collect();
    if parts.len() != 4 {
        return None;
    }
    
    let trace_id = hex_decode(parts[1])?;
    let span_id = hex_decode(parts[2])?;
    
    Some((trace_id, span_id))
}

fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if hex.len() % 2 != 0 {
        return None;
    }
    
    let mut result = Vec::new();
    for i in (0..hex.len()).step_by(2) {
        if let Ok(byte) = u8::from_str_radix(&hex[i..i+2], 16) {
            result.push(byte);
        } else {
            return None;
        }
    }
    
    Some(result)
}

fn get_current_timestamp_nanos() -> u64 {
    match proxy_wasm::hostcalls::get_current_time() {
        Ok(system_time) => {
            // Convert SystemTime to nanoseconds
            system_time.duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos() as u64)
                .unwrap_or_else(|_| {
                    // If system_time is before UNIX_EPOCH, use fallback
                    use std::sync::atomic::{AtomicU64, Ordering};
                    static TIMESTAMP_COUNTER: AtomicU64 = AtomicU64::new(1609459200000000000_u64); // Start at Jan 1, 2021
                    TIMESTAMP_COUNTER.fetch_add(1000000, Ordering::Relaxed)
                })
        },
        Err(_) => {
            // Fallback to counter-based approach if host function fails
            use std::sync::atomic::{AtomicU64, Ordering};
            static TIMESTAMP_COUNTER: AtomicU64 = AtomicU64::new(1609459200000000000_u64); // Start at Jan 1, 2021
            TIMESTAMP_COUNTER.fetch_add(1000000, Ordering::Relaxed)
        }
    }
}

fn should_skip_header(key: &str) -> bool {
    matches!(key.to_lowercase().as_str(), 
        "authorization" | "cookie" | "set-cookie" | 
        "x-api-key" | "x-auth-token" | "bearer" |
        "proxy-authorization"
    )
}

fn is_text_content(headers: &HashMap<String, String>) -> bool {
    if let Some(content_type) = headers.get("content-type") {
        content_type.starts_with("text/") || 
        content_type.starts_with("application/json") ||
        content_type.starts_with("application/xml") ||
        content_type.starts_with("application/x-www-form-urlencoded")
    } else {
        false
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}