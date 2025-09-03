// ops-common/src/lib.rs

pub mod config;
pub mod security;
pub mod tcp_auth;

use serde::{ Deserialize, Serialize };
use std::time::SystemTime;
use sysinfo::System;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HostInfo {
    pub hostname: String,
    pub cpu_model: String,
    pub cpu_usage: f32,
    pub total_memory: u64,
    pub free_memory: u64,
    pub used_memory: u64,
    pub ip_addresses: Vec<String>,
}

impl HostInfo {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let hostname = hostname
            ::get()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown".to_string());

        // let cpu = sys.cpus().first().cloned().unwrap_or_default();
        let cpu = sys
            .cpus()
            .first()
            .map(|cpu| cpu);
        let cpu_model = cpu
            .as_ref()
            .map(|c| c.brand().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let cpu_usage = cpu.map(|c| c.cpu_usage()).unwrap_or(0.0);

        let total_memory = sys.total_memory();
        let free_memory = sys.free_memory();
        let used_memory = sys.used_memory();

        let ip_addresses = get_ip_addresses();
        Self {
            hostname,
            cpu_model,
            cpu_usage,
            total_memory,
            free_memory,
            used_memory,
            ip_addresses,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VersionInfo {
    pub app: String,
    pub created_time: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub deploy_time: String,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub service_status: ServiceStatus,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServiceStatus {
    Running(String), // PID
    Stopped,
    Unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientInfo {
    pub client_id: String,
    pub system_info: HostInfo,
    pub version_info: Vec<VersionInfo>,
    pub app_info: Vec<AppInfo>,
    pub last_seen: SystemTime,
}


pub fn get_ip_addresses() -> Vec<String> {
    let interfaces = pnet_datalink::interfaces();
    interfaces
        .into_iter()
        .filter(|iface| iface.is_up() && !iface.name.contains("lo"))
        .flat_map(|iface| {
            iface.ips.into_iter().map(|ip| ip.to_string())
        })
        .collect()
}