mod directories;
mod status;
mod plugins;
mod messaging;
mod database;

pub use directories::LYServerSharedDataDirectories;
use lyserver_messaging_shared::LYServerMessageEvent;
pub use status::LYServerSharedDataStatus;
pub use plugins::{LYServerPluginInstance, LYServerSharedDataPlugins};
pub use messaging::{LYServerSharedDataMessaging};
pub use database::{LYServerSharedDataDatabase};

use lyserver_plugin_common::{LYServerPlugin, LYServerPluginMetadata};
use sysinfo::System;
use tokio::sync::{broadcast::Sender, mpsc::{UnboundedReceiver, UnboundedSender}, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use std::{collections::{HashMap, HashSet}, net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener}, path::{Path, PathBuf}, sync::{atomic::AtomicBool, Arc}, time::SystemTime};

use clap::Parser;

const SERVER_DEFAULT_PORT: u16 = 4774;
const SERVER_DEFAULT_ADDRESS: IpAddr = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));
#[cfg(target_os = "linux")]
const SERVER_DEFAULT_DATA_DIR: &str = "/var/opt/lyserver";
#[cfg(target_os = "macos")]
const SERVER_DEFAULT_DATA_DIR: &str = "/Library/Application Support/lyserver";
#[cfg(target_os = "windows")]
const SERVER_DEFAULT_DATA_DIR: &str = r"C:\ProgramData\lyserver";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Port to bind to
    #[arg(short, long, default_value_t = SERVER_DEFAULT_PORT)]
    port: u16,

    /// Address to bind to
    #[arg(short, long, default_value_t = SERVER_DEFAULT_ADDRESS)]
    address: IpAddr,

    /// Directory to store server data
    #[arg(short, long, default_value = SERVER_DEFAULT_DATA_DIR)]
    data_dir: String,
}

#[derive(Clone)]
pub struct LYServerSharedData {
    pub bind_address: SocketAddr,
    pub data_dir: PathBuf,
    pub version: &'static str,
    pub start_ts: SystemTime,

    pub shutdown_flag: Arc<AtomicBool>,

    pub loaded_plugins: Arc<RwLock<Vec<(LYServerPluginInstance, LYServerPluginMetadata, CancellationToken)>>>,
    pub plugin_message: Arc<Mutex<Option<String>>>,

    pub messaging_global_tx: Arc<Sender<LYServerMessageEvent>>,
    pub messaging_plugin_tx: Arc<RwLock<HashMap<String, Arc<RwLock<Sender<LYServerMessageEvent>>>>>>,
    pub consumed_message_ids: Arc<RwLock<HashSet<String>>>,

    pub pid: u32,
    pub system: Arc<RwLock<System>>,
}

impl LYServerSharedData {
    fn new(bind_address: SocketAddr, data_dir: PathBuf) -> Self {
        let (tx, _) = tokio::sync::broadcast::channel::<LYServerMessageEvent>(512);

        let pid = std::process::id();
        let system = Arc::new(RwLock::new(System::new_all()));

        let data = Self {
            bind_address,
            data_dir,

            version: env!("CARGO_PKG_VERSION"),
            start_ts: SystemTime::now(),

            shutdown_flag: Arc::new(AtomicBool::new(false)),

            loaded_plugins: Arc::new(RwLock::new(Vec::new())),
            plugin_message: Arc::new(Mutex::new(None)),

            messaging_global_tx: Arc::new(tx),
            messaging_plugin_tx: Arc::new(HashMap::new().into()),
            consumed_message_ids: Arc::new(RwLock::new(HashSet::new())),

            pid,
            system,
        };

        let system_clone = Arc::clone(&data.system);
        tokio::spawn(async move {
            loop {
                {
                    let mut sys = system_clone.write().await;
                    sys.refresh_all();
                }

                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });

        log::info!("Welcome to LYServer v{}.", data.version);
        log::info!("Server Options:");
        log::info!("    Bind Address: {}", data.bind_address);
        log::info!("    Data Directory: {}", data.data_dir.display());

        data
    }

    pub fn new_from_argv() -> anyhow::Result<Self> {
        let args = Args::parse();

        let bind_address = SocketAddr::new(args.address, args.port);
        if let Err(e) = TcpListener::bind(bind_address) {
            return Err(anyhow::anyhow!(
                "Failed to bind to address {}: {}",
                bind_address,
                e
            ));
        }

        let data_dir = PathBuf::from(&args.data_dir);
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir)
                .map_err(|e| anyhow::anyhow!("Failed to create data directory '{}': {}", data_dir.display(), e))?;
        }
        
        if !data_dir.is_dir() {
            return Err(anyhow::anyhow!(
                "Data directory '{}' is not a directory",
                data_dir.display()
            ));
        }

        std::fs::read_dir(&data_dir)
            .map_err(|e| anyhow::anyhow!("Cannot read data directory '{}': {}", data_dir.display(), e))?;
 
        Ok(Self::new(bind_address, data_dir))
    }
}

