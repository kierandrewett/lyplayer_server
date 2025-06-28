use std::{collections::HashMap, sync::Arc, task::{Context, Poll}, time::Duration};

use actix_web::{body::BoxBody, dev::{Service, ServiceRequest, ServiceResponse, Transform}, http::StatusCode, web, Error, HttpMessage};
use futures_util::future::{ok, LocalBoxFuture, Ready};
use lyserver_http_shared::{LYServerHTTPRequest, LYServerHTTPResponse};
use lyserver_messaging_shared::LYServerMessageEventTarget;
use lyserver_plugin_shared_data::LYServerPluginSharedData;
use lyserver_shared_data::{LYServerSharedData, LYServerSharedDataMessaging, LYServerSharedDataPlugins};
use serde::Serialize;
use serde_json::json;

pub struct LYServerRouterPluginMiddlewareFactory;

impl<S> Transform<S, ServiceRequest> for LYServerRouterPluginMiddlewareFactory
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Transform = LYServerRouterPluginMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(LYServerRouterPluginMiddleware { service: Arc::new(service) })
    }
}

pub struct LYServerRouterPluginMiddleware<S> {
    service: Arc<S>,
}

impl<S> Service<ServiceRequest> for LYServerRouterPluginMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static
{
    type Response = ServiceResponse;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let (req_head, req_payload) = req.into_parts();
    
        let req_for_response = ServiceRequest::from_parts(req_head, req_payload);
        let shared_plugin_data_opt = req_for_response.app_data::<web::Data<LYServerPluginSharedData>>().cloned();
        let service = Arc::clone(&self.service);

        log::info!("Request: {} {} {:?}", req_for_response.method(), req_for_response.uri(), req_for_response.version());
    
        Box::pin(async move {
            let plugin_response: anyhow::Result<ServiceResponse> = if let Some(shared_plugin_data) = shared_plugin_data_opt {
                let http_req = {
                    let req_ref = req_for_response.request();
    
                    let version = match req_ref.version() {
                        actix_web::http::Version::HTTP_10 => "HTTP/1.0".into(),
                        actix_web::http::Version::HTTP_2 => "HTTP/2".into(),
                        actix_web::http::Version::HTTP_3 => "HTTP/3".into(),
                        actix_web::http::Version::HTTP_09 => "HTTP/0.9".into(),
                        _ => "HTTP/1.1".into(),
                    };

                    let headers = req_ref.headers().iter()
                        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                        .collect();

                    LYServerHTTPRequest::new(
                        req_ref.method().to_string(),
                        req_ref.uri().to_string(),
                        version,
                        headers,
                        None
                    )
                };
    
                let msg = shared_plugin_data
                    .create_event("http_request", LYServerMessageEventTarget::All, http_req)
                    .await
                    .map_err(|e| {
                        log::error!("Failed to create HTTP event: {}", e);
                        actix_web::error::ErrorInternalServerError("event err")
                    })?;
    
                let msg_id = msg.event_id.clone();
    
                shared_plugin_data.dispatch_event(msg)
                    .map_err(|e| {
                        log::error!("Failed to dispatch HTTP event: {}", e);
                        actix_web::error::ErrorInternalServerError("dispatch err")
                    })?;
    
                if let Some(reply) = shared_plugin_data
                    .wait_until_event(
                        |event| event.event_type == "http_response" && event.event_id == msg_id,
                        Duration::from_secs(5),
                    )
                    .await
                {
                    let http_resp: LYServerHTTPResponse = reply.data_as().map_err(|e| {
                        log::error!("Plugin response deserialisation failed: {}", e);
                        actix_web::error::ErrorInternalServerError("deser fail")
                    })?;
    
                    let mut builder = actix_web::HttpResponse::build(
                        StatusCode::from_u16(http_resp.status_code)
                            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    );
    
                    for (k, v) in http_resp.headers {
                        builder.insert_header((k, v));
                    }

                    let plugin_id = reply.event_sender.to_string();
                    if let Some(handled_by_plugin) = shared_plugin_data.app_shared_data.get_plugin_metadata_by_id(&plugin_id).await {
                        builder.insert_header(("x-lyserver-plugin-id", handled_by_plugin.id));
                        builder.insert_header(("x-lyserver-plugin-version", handled_by_plugin.version));
                        builder.insert_header(("x-lyserver-plugin-name", handled_by_plugin.name));
                    } else {
                        log::warn!("Plugin ID {} not found in shared data", plugin_id);
                    }
    
                    let final_resp = builder.body(http_resp.body).map_into_boxed_body();
                    let response = ServiceResponse::new(req_for_response.request().clone(), final_resp);
                    Ok(response)
                } else {
                    Err(anyhow::anyhow!("No response received from plugin").into())
                }
            } else {
                Err(anyhow::anyhow!("No shared plugin data found in request").into())                                                                                                               
            };

            let response = match plugin_response {
                Ok(resp) => resp,
                Err(e) => {
                    log::error!("Plugin middleware error: {}", e);
                    log::warn!("Falling back to default service call due to plugin error.");

                    let fallback = service.call(req_for_response).await?;
                    fallback
                }
            };

            log::info!("Response: {} {} {:?} => {} ({})", response.request().method(), response.request().uri().to_string(), response.request().version(), response.status().as_u16(), response.headers().get("x-lyserver-plugin-id").map_or("no plugin", |v| v.to_str().unwrap()));

            Ok(response)
        })
    }
}