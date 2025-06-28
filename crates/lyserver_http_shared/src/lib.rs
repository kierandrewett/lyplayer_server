use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Serialize, Deserialize, Clone)]
pub struct LYServerHTTPResponse {
    pub request: LYServerHTTPRequest,
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

pub struct LYServerHTTPResponseBuilder {
    response: LYServerHTTPResponse,
}

impl LYServerHTTPResponseBuilder {
    pub fn new(request: LYServerHTTPRequest) -> Self {
        Self {
            response: LYServerHTTPResponse {
                request,
                status_code: 200,
                headers: HashMap::new(),
                body: String::new(),
            },
        }
    }

    pub fn status_code(mut self, code: u16) -> Self {
        self.response.status_code = code;
        self
    }

    pub fn header(mut self, key: String, value: String) -> Self {
        self.response.headers.insert(key, value);
        self
    }

    pub fn body(mut self, body: impl Into<String>) -> Self {
        self.response.body = body.into();
        self
    }

    pub fn json(mut self, data: impl Serialize) -> Self {
        match serde_json::to_string(&data) {
            Ok(json_body) => {
                self.response.body = json_body;
                self.response.headers.insert("content-type".to_string(), "application/json".to_string());
            }
            Err(_) => {
                self.response.status_code = 500; // Internal Server Error
                self.response.body = "Failed to serialize JSON".to_string();
            }
        }
        self
    }

    pub fn build(mut self) -> LYServerHTTPResponse {
        self.response.headers.insert("content-length".to_string(), self.response.body.len().to_string());

        self.response
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct LYServerHTTPRequest {
    pub method: String,
    pub uri: String,
    pub version: String,
    pub headers: HashMap<String, String>,
}

impl LYServerHTTPRequest {
    pub fn match_request(&self, method: &str, uri: &str) -> bool {
        self.method.eq_ignore_ascii_case(method) && self.uri == uri
    }

    pub fn build_response(&self) -> LYServerHTTPResponseBuilder {
        LYServerHTTPResponseBuilder::new(self.clone())
    }

    pub fn build_error_response(&self, status_code: u16, error: impl Serialize) -> LYServerHTTPResponseBuilder {
        let error = serde_json::to_value(&error)
            .unwrap_or_else(|_| "Internal Server Error".into());

        let message = json!({ 
            "ok": false, 
            "error": error, 
            "code": status_code
        });

        LYServerHTTPResponseBuilder::new(self.clone())
            .status_code(status_code)
            .json(message)
    }

    pub fn not_found_response(&self) -> LYServerHTTPResponse {
        self.build_error_response(404, "Page or resource not found")
            .build()
    }
}