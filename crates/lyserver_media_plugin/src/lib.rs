pub use lyserver_plugin_wasm_runtime::alloc;

use lyserver_plugin_wasm_runtime::{http::{router::LYServerHTTPRouter, LYServerHTTPRequest}, ipc::{self, LYServerMessageEvent}, log};

#[unsafe(no_mangle)]
extern "C" fn lyserver_plugin_init() {
    log::info!("Media plugin initialized!");

    let init_event = LYServerMessageEvent::new("plugin_init", "all", "media@lyserver", 0);
    ipc::tx(&init_event)
        .expect("Failed to send plugin init event");

    while let Some(message) = ipc::recv() {
        if message.event_type == "http_request" {
            let request = message.data_as::<LYServerHTTPRequest>()
                .expect("Failed to deserialize LYServerHTTPRequest");

            if request.match_request("GET", "/player.mp3").is_some() {
                let handled_message = message.reply("http_request_handle_intent", "media@lyserver".into(), ())
                    .expect("Failed to create reply message");

                ipc::tx(&handled_message)
                    .expect("Failed to send HTTP request handled message");

                log::info!("Handling request for player.mp3");

                let player_bytes = include_bytes!("../static/player.mp3");

                let response = request.build_response()
                    .body(player_bytes)
                    .build();

                let reply_message = message.reply("http_response", "media@lyserver".into(), response)
                    .expect("Failed to create reply message");

                ipc::tx(&reply_message)
                    .expect("Failed to send HTTP response");
            }
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn lyserver_plugin_destroy() {
    log::error!("good bye from LYServer Media Plugin!");
}