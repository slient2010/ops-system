use std::time::SystemTime;
use tokio::{ io::{ AsyncReadExt, AsyncWriteExt }, net::TcpStream };
use crate::shared_data_handle::{ SharedDataHandle };
use ops_common::{ClientInfo, tcp_auth::{TcpAuthMessage, TcpAuthenticator}};
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{ Deserialize, Serialize };
use tracing::{info, error, warn, debug};
use crate::command_results::CommandResult;
use std::collections::HashMap;

// 新增：定义消息类型枚举
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "data_type")] // 使用 data_type 字段作为区分枚举的依据
enum Message {
    #[serde(rename = "client_info")]
    ClientInfo {
        client_id: String,
        system_info: ops_common::HostInfo,
        version_info: Vec<ops_common::VersionInfo>,
        app_info: Vec<ops_common::AppInfo>,
        last_seen: SystemTime,
    },
    #[serde(rename = "command_response")]
    CommandResponse {
        command_id: String,
        client_id: String,
        command: String,
        output: String,
        error_output: String,
        exit_code: i32,
        executed_at: SystemTime,
    },
    #[serde(rename = "auth_challenge")]
    AuthChallenge {
        nonce: String,
        timestamp: u64,
    },
    #[serde(rename = "auth_response")]
    AuthResponse {
        client_id: String,
        nonce: String,
        response_hash: String,
        timestamp: u64,
    },
    #[serde(rename = "auth_result")]
    AuthResult {
        success: bool,
        message: String,
    },
}

// 连接状态枚举
#[derive(Debug, Clone, PartialEq)]
enum ConnectionState {
    Connected,        // 刚连接，等待认证
    Authenticated,    // 已认证，可以正常通信
    AuthFailed,       // 认证失败
}

// 客户端连接信息
#[derive(Debug, Clone)]
struct ClientConnection {
    stream: Arc<Mutex<TcpStream>>,
    state: ConnectionState,
    challenge_nonce: Option<String>,
    challenge_timestamp: Option<u64>,
}


/// 从流中读取数据 - 简单读取直到获得完整消息
async fn read_line_from_stream(stream: Arc<Mutex<tokio::net::TcpStream>>) -> std::io::Result<Vec<u8>> {
    let mut stream = stream.lock().await;
    let mut line_buffer = Vec::new();
    let mut byte_buffer = [0u8; 1];
    
    loop {
        let n = stream.read(&mut byte_buffer).await?;
        if n == 0 {
            if line_buffer.is_empty() {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "连接已关闭"));
            } else {
                // 返回未以换行结尾的数据
                break;
            }
        }
        
        let byte = byte_buffer[0];
        if byte == b'\n' {
            // 找到换行符，返回一行数据
            break;
        } else if byte != b'\r' {
            // 忽略回车符，添加其他字符
            line_buffer.push(byte);
        }
    }
    
    debug!("Read line: {} bytes", line_buffer.len());
    Ok(line_buffer)
}

// /// 解析客户端发送的 JSON 数据
// fn parse_client_data(data: &[u8]) -> Result<ClientInfo, serde_json::Error> {
//     serde_json::from_slice(data)
// }

/// 解析客户端发送的 JSON 数据
fn parse_client_data(data: &[u8]) -> Result<Message, serde_json::Error> {
    serde_json::from_slice(data)
}

/// 更新共享内存中的客户端信息
async fn update_shared_data(
    shared_data: &SharedDataHandle,
    client_data: ClientInfo
) -> std::io::Result<()> {
    let now = SystemTime::now();
    let mut shared_data = shared_data.lock().await;
    shared_data.client_data.insert(client_data.client_id.clone(), ClientInfo {
        last_seen: now,
        ..client_data
    });
    Ok(())
}

/// 向客户端发送 ACK
async fn send_ack(stream: &mut Arc<Mutex<TcpStream>>) -> std::io::Result<()> {
    // stream.lock().await.write_all(b"ACK").await
    stream.lock().await.write_all(b"ACK\n").await
}

/// 向客户端发送消息
async fn send_message(stream: &Arc<Mutex<TcpStream>>, message: &Message) -> std::io::Result<()> {
    let json_data = serde_json::to_vec(message)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    
    let mut stream_guard = stream.lock().await;
    stream_guard.write_all(&json_data).await?;
    stream_guard.write_all(b"\n").await?; // 添加换行符作为消息分隔符
    stream_guard.flush().await?;
    
    debug!("Message sent: {} bytes", json_data.len());
    Ok(())
}

// /// 向客户端发送任意消息
// async fn send_message_to_client(
//     stream: &mut Arc<Mutex<TcpStream>>,
//     message: &[u8]
// ) -> std::io::Result<()> {
//     stream.lock().await.write_all(message).await
// }

/// 主函数：处理客户端连接
pub async fn handle_client_connection(
    stream: tokio::net::TcpStream, // 客户端连接的流
    shared_data: SharedDataHandle // 共享的数据结构
) -> std::io::Result<()> {
    let peer_addr = stream.peer_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    
    let mut client_id = String::new();
    let mut connection_state = ConnectionState::Connected;
    let mut challenge_nonce: Option<String> = None;
    let mut challenge_timestamp: Option<u64> = None;
    
    info!("Handling client connection from: {}", peer_addr);

    // 将 stream 包装为 Arc<Mutex<_>> 以便多处借用
    let stream = Arc::new(Mutex::new(stream));
    
    // 创建认证器
    let tcp_auth_secret = std::env::var("OPS_TCP_AUTH_SECRET")
        .unwrap_or_else(|_| "default-tcp-secret-key".to_string());
    let authenticator = TcpAuthenticator::new(tcp_auth_secret);
    
    // 如果启用了TCP认证，先发送认证质询
    let tcp_auth_enabled = std::env::var("OPS_TCP_AUTH_ENABLED")
        .map(|v| v.to_lowercase() == "true" || v == "1")
        .unwrap_or(false);
        
    if tcp_auth_enabled {
        info!("TCP authentication enabled, sending challenge to {}", peer_addr);
        let challenge = TcpAuthenticator::generate_challenge();
        
        if let TcpAuthMessage::Challenge { nonce, timestamp } = challenge {
            challenge_nonce = Some(nonce.clone());
            challenge_timestamp = Some(timestamp);
            
            let challenge_msg = Message::AuthChallenge { nonce, timestamp };
            if let Err(e) = send_message(&stream, &challenge_msg).await {
                error!("Failed to send authentication challenge to {}: {}", peer_addr, e);
                return Err(e);
            }
            
            debug!("Authentication challenge sent to {}", peer_addr);
        }
    } else {
        info!("TCP authentication disabled, allowing connection from {}", peer_addr);
        connection_state = ConnectionState::Authenticated;
    }

    loop {
        debug!("Waiting for data from client: {}", peer_addr);

        // 1. 读取一行数据（以换行符分割）
        let data = match read_line_from_stream(Arc::clone(&stream)).await {
            Ok(data) => data,
            Err(e) => {
                warn!("Failed to read data from {}: {}", peer_addr, e);
                {
                    let mut data = shared_data.lock().await;
                    data.remove_client_connection(&client_id).await;
                }
                return Err(e);
            }
        };
        
        // 跳过空行
        if data.is_empty() {
            debug!("Received empty line from {}, continuing...", peer_addr);
            continue;
        }


                // 添加调试信息：显示接收到的原始数据
        let data_str = String::from_utf8_lossy(&data);
        debug!("Raw data from {}: {}", peer_addr, data_str);

        // 检查是否是回环命令数据（服务器发送给客户端的命令被错误读取）
        if data_str.trim().starts_with("CMD:") {
            warn!("Server read loop detected command data meant for client: {}", data_str.trim());
            continue;
        }

        // 解析消息
        let message = match parse_client_data(&data) {
            Ok(msg) => msg,
            Err(e) => {
                error!("Failed to parse data from {}: {}", peer_addr, e);
                error!("Raw data that failed to parse: {}", data_str);
                continue;
            }
        };

        match message {
            Message::AuthResponse { client_id: auth_client_id, nonce, response_hash, timestamp } => {
                if !tcp_auth_enabled {
                    warn!("Received auth response but authentication is disabled from {}", peer_addr);
                    continue;
                }
                
                info!("Received auth response from: {} (ID: {})", peer_addr, auth_client_id);
                
                // 验证认证响应
                let auth_msg = TcpAuthMessage::Response {
                    client_id: auth_client_id.clone(),
                    nonce: nonce.clone(),
                    response_hash,
                    timestamp,
                };
                
                let is_valid = match (&challenge_nonce, &challenge_timestamp) {
                    (Some(orig_nonce), Some(orig_timestamp)) => {
                        match authenticator.verify_response(&auth_msg, orig_nonce, *orig_timestamp) {
                            Ok(valid) => valid,
                            Err(e) => {
                                error!("Authentication verification error for {}: {}", peer_addr, e);
                                false
                            }
                        }
                    }
                    _ => {
                        error!("No challenge issued but received auth response from {}", peer_addr);
                        false
                    }
                };
                
                if is_valid {
                    info!("Authentication successful for client {} from {}", auth_client_id, peer_addr);
                    connection_state = ConnectionState::Authenticated;
                    client_id = auth_client_id;
                    
                    // 发送认证成功消息
                    let success_msg = Message::AuthResult {
                        success: true,
                        message: "Authentication successful".to_string(),
                    };
                    
                    if let Err(e) = send_message(&stream, &success_msg).await {
                        error!("Failed to send auth success message to {}: {}", peer_addr, e);
                        return Err(e);
                    }
                } else {
                    warn!("Authentication failed for client {} from {}", auth_client_id, peer_addr);
                    connection_state = ConnectionState::AuthFailed;
                    
                    // 发送认证失败消息
                    let failure_msg = Message::AuthResult {
                        success: false,
                        message: "Authentication failed".to_string(),
                    };
                    
                    if let Err(e) = send_message(&stream, &failure_msg).await {
                        error!("Failed to send auth failure message to {}: {}", peer_addr, e);
                    }
                    
                    // 断开连接
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::PermissionDenied, 
                        "Authentication failed"
                    ));
                }
            }
            Message::ClientInfo { client_id: msg_client_id, system_info, version_info, app_info, last_seen } => {
                // 检查连接是否已认证
                if tcp_auth_enabled && connection_state != ConnectionState::Authenticated {
                    warn!("Received client info before authentication from {}", peer_addr);
                    continue;
                }
                
                info!("Received client info from: {} (ID: {})", peer_addr, msg_client_id);
                client_id = msg_client_id.clone();

                // 添加连接到共享数据
                {
                    let mut data = shared_data.lock().await;
                    if let Err(e) = data.add_client_connection(client_id.clone(), Arc::clone(&stream)).await {
                        error!("Failed to add client connection {}: {}", client_id, e);
                        // 发送拒绝连接的消息
                        let _ = stream.lock().await.write_all(b"CONNECTION_REJECTED: Too many connections").await;
                        return Err(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e));
                    }
                }

                // 重构 ClientInfo 结构
                let client_info = ClientInfo {
                    client_id: msg_client_id,
                    system_info,
                    version_info,
                    app_info,
                    last_seen,
                };

                // 更新共享数据
                if let Err(e) = update_shared_data(&shared_data, client_info).await {
                    error!("Failed to update shared data for {}: {}", client_id, e);
                }

                // 发送 ACK
                if let Err(e) = send_ack(&mut Arc::clone(&stream)).await {
                    error!("Failed to send ACK to {}: {}", client_id, e);
                    return Err(e);
                }
            }
            Message::CommandResponse { 
                command_id, 
                client_id: resp_client_id, 
                command, 
                output, 
                error_output, 
                exit_code, 
                executed_at 
            } => {
                // 检查连接是否已认证
                if tcp_auth_enabled && connection_state != ConnectionState::Authenticated {
                    warn!("Received command response before authentication from {}", peer_addr);
                    continue;
                }
                
                info!("Received command response from client {}: command_id={}, exit_code={}", 
                      resp_client_id, command_id, exit_code);
                
                // 创建命令结果对象
                let command_result = CommandResult {
                    command_id,
                    client_id: resp_client_id,
                    command,
                    output,
                    error_output,
                    exit_code,
                    executed_at,
                    received_at: SystemTime::now(),
                };
                
                // 存储命令结果
                let mut data = shared_data.lock().await;
                data.command_results.store_result(command_result).await;
            }
            Message::AuthChallenge { .. } | Message::AuthResult { .. } => {
                // 这些消息类型不应该从客户端接收
                warn!("Received unexpected auth message type from client {}", peer_addr);
            }
        }







        // // 2. 解析数据
        // let client_data = match parse_client_data(&data) {
        //     Ok(data) => data,
        //     Err(e) => {
        //         eprintln!("解析数据失败: {}", e);
        //         continue;
        //     }
        // };

        // client_id = client_data.client_id.clone();

        // // 3. 添加连接到共享数据
        // {
        //     // let mut data = shared_data.lock().await;
        //     // data.add_client_connection(client_id.clone(), Arc::clone(&stream)).await;
        //     // if client_id.is_empty() {
        //     // println!("新增客户端连接: {}", client_id.clone());
        //     let mut data = shared_data.lock().await;
        //     data.add_client_connection(client_id.clone(), Arc::clone(&stream)).await;
        //     // }
        // }

        // // 4. 更新共享数据
        // if let Err(e) = update_shared_data(&shared_data, client_data).await {
        //     eprintln!("更新共享数据失败: {}", e);
        // }

        // // 5. 发送 ACK
        // if let Err(e) = send_ack(&mut Arc::clone(&stream)).await {
        //     eprintln!("发送 ACK 失败: {}", e);
        //     return Ok(());
        // }

        // 示例：发送自定义消息给客户端
        // let custom_message = b"Hello from server!";
        // if let Err(e) = send_message_to_client(&mut Arc::clone(&stream), custom_message).await {
        //     eprintln!("发送自定义消息失败: {}", e);
        // }

        // {
        //     let mut data = shared_data.lock().await;
        //     data.remove_client_connection(&client_id).await;
        // }
    }
}
