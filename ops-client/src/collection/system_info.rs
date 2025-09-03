use sysinfo::System;
use ops_common::HostInfo;
use ops_common::get_ip_addresses;

pub struct HostInfoWrapper(pub HostInfo);

impl HostInfoWrapper {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        let hostname = hostname::get()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|_| "unknown".to_string());

        // let cpu = sys.cpus().first().cloned().unwrap_or_default();
        let cpu = sys.cpus().first().map(|cpu| cpu);
        let cpu_model = cpu.as_ref().map(|c| c.brand().to_string()).unwrap_or_else(|| "unknown".to_string());
        let cpu_usage = cpu.map(|c| c.cpu_usage()).unwrap_or(0.0);

        let total_memory = sys.total_memory();
        let free_memory = sys.free_memory();
        let used_memory = sys.used_memory();

        let ip_addresses = get_ip_addresses();

        Self(HostInfo { 
            hostname,
            cpu_model,
            cpu_usage,
            total_memory,
            free_memory,
            used_memory,
            ip_addresses,
        })
    }

}

// fn get_ip_addresses() -> Vec<String> {
//     let interfaces = pnet_datalink::interfaces();
//     interfaces
//         .into_iter()
//         .filter(|iface| iface.is_up() && !iface.name.contains("lo"))
//         .flat_map(|iface| {
//             iface.ips.into_iter().map(|ip| ip.to_string())
//         })
//         .collect()
// }