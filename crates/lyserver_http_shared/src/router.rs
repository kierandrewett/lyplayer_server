use std::{collections::HashMap, pin::Pin};

use path_tree::PathTree;

use crate::{LYServerHTTPRequest, LYServerHTTPResponse, LYServerHTTPResponseBuilder};

pub struct LYServerHTTPRoute {
    pub method: String,
    pub uri: String,
    pub requested_uri: String,
    pub params: HashMap<String, String>,
    pub request: LYServerHTTPRequest,
}

pub struct LYServerHTTPRouter {
    matchers: Vec<(String, String, BoxedRouteHandler)>,
}

type BoxedRouteHandler = Box<
    dyn Fn(LYServerHTTPRoute) -> Pin<Box<dyn Future<Output = anyhow::Result<LYServerHTTPResponse>> + Send>>
        + Send
        + Sync,
>;

impl LYServerHTTPRouter {
    pub fn new() -> Self {
        Self {
            matchers: Vec::new(),
        }
    }


    pub fn add_matcher<F, Fut>(&mut self, method: &str, path: &str, handler: F)
    where
        F: Fn(LYServerHTTPRoute) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = anyhow::Result<LYServerHTTPResponse>> + Send + 'static,
    {
        let boxed_handler: BoxedRouteHandler = Box::new(move |route| Box::pin(handler(route)));

        self.matchers
            .push((method.to_string(), path.to_string(), boxed_handler));
    }

    pub fn match_request(&self, request: LYServerHTTPRequest, method: &str, uri: &str) -> Option<LYServerHTTPRoute> {
        LYServerHTTPRequest::static_match_request(request, method, uri)
    }

    pub async fn respond(&self, request: LYServerHTTPRequest) -> Option<LYServerHTTPResponse> {
        for (method, uri, handler) in &self.matchers {
            if let Some(route) = self.match_request(request.clone(), method, uri) {
                let result = match handler(route).await {
                    Ok(response) => response,
                    Err(e) => {
                        return Some(
                            request
                                .build_error_response(400, format!("{}", e))
                                .build(),
                        );
                    }
                };

                return Some(result);
            }
        }

        None
    }
}