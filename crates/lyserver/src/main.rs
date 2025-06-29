mod server;
mod plugins;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use clap::Parser;
use lyserver_shared_data::LYServerSharedData;

use crate::{server::LYServer};

#[tokio::main]
async fn main() {
    console_subscriber::init();

    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();

    let shared_data = LYServerSharedData::new_from_argv()
        .unwrap_or_else(|e| {
            log::error!("Failed to parse server arguments: {}", e);
            std::process::exit(1);
        });

    let mut server = LYServer::new(shared_data);

    server.run().await.unwrap_or_else(|e| {
        log::error!("Server failed to start: {}", e);
        std::process::exit(1);
    });
}
