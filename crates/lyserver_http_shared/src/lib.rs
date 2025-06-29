use std::{collections::HashMap};
use bytes::Bytes;
use path_tree::PathTree;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

use crate::router::LYServerHTTPRoute;

pub mod router;

#[derive(Serialize, Deserialize, Clone)]
pub struct LYServerHTTPResponse {
    pub request: LYServerHTTPRequest,
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
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
                body: Vec::new(),
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

    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.response.body = body.into();
        self
    }

    pub fn json(mut self, data: impl Serialize) -> Self {
        match serde_json::to_string(&data) {
            Ok(json_body) => {
                self.response.body = json_body.into();
                self.response.headers.insert("content-type".to_string(), "application/json".to_string());
            }
            Err(_) => {
                self.response.status_code = 500; // Internal Server Error
                self.response.body = "Failed to serialize JSON".to_string().into();
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
    pub params: HashMap<String, String>,
    pub version: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

impl LYServerHTTPRequest {
    pub fn new(method: String, uri: String, version: String, headers: HashMap<String, String>, body: Option<Vec<u8>>) -> Self {
        Self {
            method,
            uri,
            params: HashMap::new(),
            version,
            headers,
            body,
        }
    }

    pub fn match_request(&self, method: &str, uri: &str) -> Option<LYServerHTTPRoute> {
        LYServerHTTPRequest::static_match_request(self.clone(), method, uri)
    }

    pub fn static_match_request(request: Self, method: &str, uri: &str) -> Option<LYServerHTTPRoute> {
        if !request.method.eq_ignore_ascii_case(method) {
            return None;
        }

        let mut tree = PathTree::new();
        let _ = tree.insert(uri, 0);
        if let Some((_, url)) = tree.find(&request.uri) {
            let params = url.params()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();

            Some(LYServerHTTPRoute {
                method: request.method.clone(),
                uri: uri.to_string(),
                requested_uri: request.uri.clone(),
                params,
                request: request.clone(),
            })
        } else {
            None
        }
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

    pub fn body(&self) -> Option<Bytes> {
        self.body.as_ref().map(|b| Bytes::from(b.clone()))
    }

    pub fn body_json<T: DeserializeOwned>(&self) -> anyhow::Result<T> {
        match &self.body {
            Some(body) => serde_json::from_slice(body).map_err(anyhow::Error::from),
            None => Err(anyhow::Error::msg("No body available")),
        }
    }
}