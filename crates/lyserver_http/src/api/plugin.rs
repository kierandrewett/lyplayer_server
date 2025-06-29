use std::{collections::HashMap, sync::Arc, task::{Context, Poll}, time::Duration};

use actix_web::{body::BoxBody, dev::{Service, ServiceRequest, ServiceResponse, Transform}, http::StatusCode, web::{self, BytesMut}, Error, HttpMessage};
use futures_util::{future::{ok, LocalBoxFuture, Ready}, StreamExt};
use lyserver_http_shared::{LYServerHTTPRequest, LYServerHTTPResponse};
use lyserver_messaging_shared::{LYServerMessageEvent, LYServerMessageEventTarget};
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
        let (req_head, mut req_payload) = req.into_parts();
        let service = Arc::clone(&self.service);
    
        Box::pin(async move {
            let mut body_bytes = BytesMut::new();
            while let Some(chunk) = req_payload.next().await {
                body_bytes.extend_from_slice(&chunk?);
            }
    
            let req_for_response = ServiceRequest::from_parts(req_head, req_payload);
    
            let shared_plugin_data_opt = req_for_response
                .app_data::<web::Data<LYServerPluginSharedData>>()
                .cloned();
    
            let plugin_response: anyhow::Result<ServiceResponse> = if let Some(shared_plugin_data) = shared_plugin_data_opt {
                let req_ref = req_for_response.request();
    
                let version = match req_ref.version() {
                    actix_web::http::Version::HTTP_10 => "HTTP/1.0",
                    actix_web::http::Version::HTTP_2 => "HTTP/2",
                    actix_web::http::Version::HTTP_3 => "HTTP/3",
                    actix_web::http::Version::HTTP_09 => "HTTP/0.9",
                    _ => "HTTP/1.1",
                }.into();
    
                let headers = req_ref
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();
    
                let http_req = LYServerHTTPRequest::new(
                    req_ref.method().to_string(),
                    req_ref.uri().to_string(),
                    version,
                    headers,
                    Some(body_bytes.to_vec()),
                );
    
                let msg = shared_plugin_data
                    .create_event("http_request", LYServerMessageEventTarget::All, http_req)
                    .await
                    .map_err(|e| {
                        log::error!("Failed to create HTTP event: {}", e);
                        actix_web::error::ErrorInternalServerError("event err")
                    })?;
    
                let msg_id = msg.event_id.clone();
    
                shared_plugin_data.dispatch_event(msg).map_err(|e| {
                    log::error!("Failed to dispatch HTTP event: {}", e);
                    actix_web::error::ErrorInternalServerError("dispatch err")
                })?;
    
                log::debug!("WAITING FOR HANDLE INTENT....");

                let shared_plugin_data_clone = Arc::clone(&shared_plugin_data);
                let resp: anyhow::Result<LYServerMessageEvent> = {
                    let msg_id_clone = msg_id.clone();

                    if let Some(reply) = shared_plugin_data_clone
                        .wait_until_event(
                            move |event| event.event_type == "http_request_handle_intent" && event.event_id == msg_id_clone.clone(),
                            Duration::from_secs(5),
                        )
                        .await
                    {
                        log::debug!("Plugin '{}' handling HTTP request event: {}", reply.event_sender.to_string(), reply.event_id);

                        let msg_id_clone = msg_id.clone();

                        if let Some(reply) = shared_plugin_data_clone
                            .wait_until_event(
                                move |event| event.event_type == "http_response" && event.event_id == msg_id_clone.clone(),
                                Duration::from_secs(60),
                            )
                            .await
                        {
                            Ok(reply)
                        } else {
                            Err(anyhow::anyhow!("Plugin response timed out after 60s").into())
                        }
                    } else {
                        Err(anyhow::anyhow!("No plugin responded with intent").into())
                    }
                };

                match resp {
                    Ok(reply) => {
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
                        if let Some(plugin_meta) = shared_plugin_data
                            .app_shared_data
                            .get_plugin_metadata_by_id(&plugin_id)
                            .await
                        {
                            builder.insert_header(("x-lyserver-plugin-id", plugin_meta.id));
                            builder.insert_header(("x-lyserver-plugin-version", plugin_meta.version));
                            builder.insert_header(("x-lyserver-plugin-name", plugin_meta.name));
                        }
        
                        let final_resp = builder.body(http_resp.body).map_into_boxed_body();
                        Ok(ServiceResponse::new(
                            req_for_response.request().clone(),
                            final_resp,
                        ))
                    },
                    Err(e) => {
                        let new_resp = actix_web::HttpResponse::InternalServerError()
                            .body(format!("{}", e));

                        let resp = ServiceResponse::new(
                            req_for_response.request().clone(),
                            new_resp.map_into_boxed_body(),
                        );

                        Ok(resp)
                    }
                }
            } else {
                Err(anyhow::anyhow!("Missing plugin data").into())
            };
    
            let response = match plugin_response {
                Ok(resp) => resp,
                Err(e) => {
                    log::error!("Plugin middleware error: {}", e);
                    service.call(req_for_response).await?
                }
            };
    
            Ok(response)
        })
    }
}