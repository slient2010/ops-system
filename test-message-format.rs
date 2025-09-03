fn main() {
    println!("测试消息格式...");
    
    // 模拟客户端心跳消息
    let client_message = r#"
    {
        "data_type": "client_info",
        "client_id": "test-client-123",
        "system_info": {
            "hostname": "test-host",
            "cpu_model": "test-cpu",
            "cpu_usage": 50.0,
            "total_memory": 8000000000,
            "free_memory": 4000000000,
            "used_memory": 4000000000,
            "ip_addresses": ["127.0.0.1"]
        },
        "version_info": [],
        "last_seen": {"secs_since_epoch": 1724899200, "nanos_since_epoch": 0}
    }
    "#;
    
    println!("客户端消息格式示例:");
    println!("{}", client_message);
    
    // 模拟命令响应消息
    let command_response = r#"
    {
        "data_type": "command_response",
        "command_id": "cmd-123",
        "client_id": "test-client-123",
        "command": "whoami",
        "output": "root\n",
        "error_output": "",
        "exit_code": 0,
        "executed_at": {"secs_since_epoch": 1724899200, "nanos_since_epoch": 0}
    }
    "#;
    
    println!("\n命令响应消息格式示例:");
    println!("{}", command_response);
}