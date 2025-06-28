mod plugin;

use actix_web::{dev::HttpServiceFactory, web::{self, Data}, HttpResponse, Responder, Scope};
use lyserver_plugin_shared_data::LYServerPluginSharedData;
use lyserver_shared_data::{LYServerSharedData, LYServerSharedDataPlugins as _, LYServerSharedDataStatus};

use crate::api::plugin::LYServerRouterPluginMiddlewareFactory;

pub fn router() -> impl HttpServiceFactory {
    web::scope("")
        .wrap(LYServerRouterPluginMiddlewareFactory)
}