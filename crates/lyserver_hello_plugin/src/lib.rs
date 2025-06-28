pub use lyserver_plugin_wasm_runtime::alloc;

use lyserver_plugin_wasm_runtime::{http::LYServerHTTPRequest, ipc::{self, LYServerMessageEvent}, log};

#[unsafe(no_mangle)]
extern "C" fn lyserver_plugin_init() {
    log::info!("✨ Hello from LYServer Hello Plugin!");

    log::info!("This is a simple plugin that demonstrates how to use the LYServer Plugin API.");
    let init_event = LYServerMessageEvent::new("plugin_init", "all", "hello@lyserver", 0);
    ipc::tx(&init_event)
        .expect("Failed to send plugin init event");

    while let Some(message) = ipc::recv() {
        if message.event_type == "http_request" {
            let request = message.data_as::<LYServerHTTPRequest>()
                .expect("Failed to deserialize LYServerHTTPRequest");

            if request.match_request("GET", "/hello").is_some() {
                let response = request.build_response()
                    .body("hello from WASM!")
                    .build();

                let reply_message = message.reply("http_response", "hello@lyserver".into(), response)
                    .expect("Failed to create reply message");

                ipc::tx(&reply_message)
                    .expect("Failed to send HTTP response");
            }
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn lyserver_plugin_destroy() {
    log::error!("💀 Goodbye from LYServer Hello Plugin!");
}