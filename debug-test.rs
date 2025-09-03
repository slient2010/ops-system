use ops_common::{ClientInfo, HostInfo, VersionInfo, config::ServerConfig};
use std::time::SystemTime;

fn main() {
    println!("=== 测试 ops-common 模块 ===");
    
    // 测试配置
    let server_config = ServerConfig::from_env();
    println!("服务端配置: {:?}", server_config);
    
    // 测试系统信息收集
    println!("收集系统信息...");
    let host_info = HostInfo::new();
    println!("系统信息: hostname={}, CPU={}", host_info.hostname, host_info.cpu_model);
    
    // 测试客户端信息创建
    let client_info = ClientInfo {
        client_id: "test-client".to_string(),
        system_info: host_info,
        version_info: vec![VersionInfo {
            app: "test-app".to_string(),
            created_time: "2024-01-01".to_string(),
        }],
        last_seen: SystemTime::now(),
    };
    
    // 测试JSON序列化
    match serde_json::to_string_pretty(&client_info) {
        Ok(json) => println!("客户端信息JSON序列化成功:\n{}", json),
        Err(e) => println!("JSON序列化失败: {}", e),
    }
    
    println!("=== ops-common 模块测试完成 ===");
}