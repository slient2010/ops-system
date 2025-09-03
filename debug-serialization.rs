use std::time::SystemTime;
use serde_json;

fn main() {
    let now = SystemTime::now();
    println!("SystemTime 原始值: {:?}", now);
    
    // 测试直接序列化
    match serde_json::to_string(&now) {
        Ok(json) => println!("SystemTime JSON 序列化: {}", json),
        Err(e) => println!("SystemTime 序列化失败: {}", e),
    }
    
    // 测试在 json! 宏中使用
    let message = serde_json::json!({
        "data_type": "client_info",
        "client_id": "test-client",
        "last_seen": now
    });
    
    println!("完整消息 JSON: {}", serde_json::to_string_pretty(&message).unwrap());
}