use std::{
    path::{Path, PathBuf}, sync::Arc, time::Duration
};

use lyserver_plugin_common::LYServerPluginMetadata;
use serde::{Deserialize, Serialize};
use sysinfo::{Pid, System};

use crate::LYServerSharedData;

#[derive(Serialize, Deserialize, Debug)]
pub struct LYServerSharedDataStatusData {
    pub data_dir: String,
    pub version: String,
    pub uptime: u128,
    pub start_time: u128,
    pub loaded_plugins: Vec<LYServerPluginMetadata>,
    pub pid: u32,
    pub used_memory: u64,
    pub cpu_count: usize,
    pub platform: String,
    pub os: String,
    pub os_version: String,
    pub os_kernel_version: String,
}

pub trait LYServerSharedDataStatus {
    fn get_server_status(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = LYServerSharedDataStatusData> + Send>>;
}

impl LYServerSharedDataStatus for LYServerSharedData {
    fn get_server_status(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = LYServerSharedDataStatusData> + Send>> {
        let maybe_canonicalized_data_dir = self
            .data_dir
            .canonicalize()
            .map(|p| p.display().to_string())
            .unwrap_or(self.data_dir.display().to_string());

        let version = self.version;
        let start_ts = self.start_ts.clone();
        let loaded_plugins = self.loaded_plugins.clone();

        let system_clone = Arc::clone(&self.system);
        let system_cpu_count_clone = self.system_cpu_count.clone();
        let pid_clone = self.pid.clone();
        let platform_clone = self.platform.clone();
        let os_clone = self.os.clone();
        let os_version_clone = self.os_version.clone();
        let os_kernel_version_clone = self.os_kernel_version.clone();

        Box::pin(async move {
            let loaded_plugins = loaded_plugins.read().await
                .iter()
                .map(|(_, metadata, _)| metadata.clone())
                .collect::<Vec<LYServerPluginMetadata>>();

            let system = system_clone.read().await;
            let proc = system.process(Pid::from(pid_clone as usize)).unwrap();
            let used_memory = proc.memory();

            LYServerSharedDataStatusData {
                data_dir: maybe_canonicalized_data_dir,
                version: version.to_string(),
                uptime: start_ts.elapsed().unwrap_or_default().as_millis(),
                start_time: start_ts
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs() as u128,
                loaded_plugins,
                pid: pid_clone,
                used_memory,
                cpu_count: system_cpu_count_clone,
                platform: platform_clone,
                os: os_clone,
                os_version: os_version_clone,
                os_kernel_version: os_kernel_version_clone,
            }
        })
    }
}
