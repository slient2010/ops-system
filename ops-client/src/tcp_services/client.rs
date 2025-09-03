use std::net::{ TcpStream, SocketAddr };
use std::sync::Arc;
use std::fs;
 
use std::time::{ Duration, SystemTime };
use crate::collection::version_collector;
use crate::collection::app_info::AppInfoCollector;
use crate::tcp_services::client;
use socket2::{ Socket, Domain, Type, TcpKeepalive };
use tokio::sync::Mutex;
use tokio::io::{ AsyncReadExt, AsyncWriteExt };
use tokio::net::TcpStream as AsyncTcpStream;
use ops_common::{ ClientInfo, HostInfo, config::ClientConfig, security::{CommandValidator, ValidationResult}, tcp_auth::{TcpAuthMessage, TcpAuthenticator} };
use tracing::{info, error, warn, debug};
use serde::{Deserialize, Serialize};
pub fn get_or_create_client_id(config: &ClientConfig) -> Result<String, std::io::Error> {
    if let Ok(id) = fs::read_to_string(&config.client_id_file) {
        Ok(id.trim().to_string())
    } else {
        let new_id = uuid::Uuid::new_v4().to_string();
        fs::write(&config.client_id_file, &new_id)?;
        info!("Created new client ID: {} in {}", new_id, config.client_id_file);
        Ok(new_id)
    }
}

/// 客户端消息类型，与服务器端对应
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "data_type")]
enum ClientMessage {
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
    #[serde(rename = "auth_response")]
    AuthResponse {
        client_id: String,
        nonce: String,
        response_hash: String,
        timestamp: u64,
    },
}

/// 服务器消息类型
#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "data_type")]
enum ServerMessage {
    #[serde(rename = "auth_challenge")]
    AuthChallenge {
        nonce: String,
        timestamp: u64,
    },
    #[serde(rename = "auth_result")]
    AuthResult {
        success: bool,
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
enum ClientState {
    Connected,       // 刚连接
    Authenticating, // 正在认证中
    Authenticated,  // 已认证
    AuthFailed,     // 认证失败
}

pub struct TcpSession {
    stream: Arc<Mutex<AsyncTcpStream>>,
    addr: String,
    config: ClientConfig,
    validator: CommandValidator,
    state: Arc<Mutex<ClientState>>,
    authenticator: Option<TcpAuthenticator>,
}

impl TcpSession {
    pub async fn new(config: ClientConfig) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let addr = config.server_address();
        let stream = Self::connect_with_retry(&addr, &config).await?;
        
        // 检查是否启用TCP认证
        let tcp_auth_secret = std::env::var("OPS_TCP_AUTH_SECRET")
            .unwrap_or_else(|_| "default-tcp-secret-key".to_string());
        let authenticator = Some(TcpAuthenticator::new(tcp_auth_secret));
        
        let session = Self {
            stream: Arc::new(Mutex::new(stream)),
            addr,
            config,
            validator: CommandValidator::new(),
            state: Arc::new(Mutex::new(ClientState::Connected)),
            authenticator,
        };
        
        // 启动认证流程
        session.handle_initial_authentication().await?;
        
        Ok(session)
    }

    pub async fn connect_with_retry(addr: &str, config: &ClientConfig) -> Result<AsyncTcpStream, Box<dyn std::error::Error + Send + Sync>> {
        let mut retry = 0;
        loop {
            match Self::create_socket_async(addr).await {
                Ok(stream) => {
                    info!("Successfully connected to {}", addr);
                    return Ok(stream);
                }
                Err(e) => {
                    retry += 1;
                    if retry > config.retry_max_attempts {
                        error!("Max retry attempts reached for {}: {}", addr, e);
                        return Err(e.into());
                    }
                    
                    let delay = (config.retry_base_delay_secs.pow(retry)).min(config.retry_max_delay_secs);
                    warn!("Connection failed (attempt {}/{}): {}, retrying in {}s", retry, config.retry_max_attempts, e, delay);
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                }
            }
        }
    }

    pub async fn create_socket_async(addr: &str) -> std::io::Result<AsyncTcpStream> {
        AsyncTcpStream::connect(addr).await
    }

    /// 处理初始认证流程
    async fn handle_initial_authentication(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let tcp_auth_enabled = std::env::var("OPS_TCP_AUTH_ENABLED")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);
        
        if !tcp_auth_enabled {
            info!("TCP authentication disabled, skipping authentication");
            let mut state = self.state.lock().await;
            *state = ClientState::Authenticated;
            return Ok(());
        }
        
        info!("TCP authentication enabled, waiting for challenge");
        
        // 等待服务器的认证质询
        let mut buf = vec![0u8; 4096];
        let n = {
            let mut stream = self.stream.lock().await;
            tokio::time::timeout(
                Duration::from_secs(10),
                stream.read(&mut buf)
            ).await
            .map_err(|_| "Authentication timeout")?
            .map_err(|e| format!("Read error during auth: {}", e))?
        };
        
        if n == 0 {
            return Err("Connection closed during authentication".into());
        }
        
        buf.truncate(n);
        let challenge_data = String::from_utf8_lossy(&buf);
        debug!("Received potential challenge: {}", challenge_data);
        
        // 解析认证质询
        let server_msg: ServerMessage = serde_json::from_slice(&buf)
            .map_err(|e| format!("Failed to parse server message: {}", e))?;
        
        match server_msg {
            ServerMessage::AuthChallenge { nonce, timestamp } => {
                info!("Received authentication challenge");
                {
                    let mut state = self.state.lock().await;
                    *state = ClientState::Authenticating;
                }
                
                // 生成认证响应
                if let Some(ref auth) = self.authenticator {
                    let client_id = get_or_create_client_id(&self.config)?;
                    let response = auth.generate_response(client_id.clone(), nonce, timestamp)?;
                    
                    if let TcpAuthMessage::Response { client_id, nonce, response_hash, timestamp } = response {
                        let auth_msg = ClientMessage::AuthResponse {
                            client_id,
                            nonce,
                            response_hash,
                            timestamp,
                        };
                        
                        // 发送认证响应
                        self.send_message(&auth_msg).await?;
                        
                        // 等待认证结果
                        self.wait_for_auth_result().await?;
                    }
                } else {
                    return Err("Authenticator not available".into());
                }
            }
            ServerMessage::AuthResult { success, message } => {
                if success {
                    info!("Authentication successful: {}", message);
                    let mut state = self.state.lock().await;
                    *state = ClientState::Authenticated;
                } else {
                    error!("Authentication failed: {}", message);
                    let mut state = self.state.lock().await;
                    *state = ClientState::AuthFailed;
                    return Err(format!("Authentication failed: {}", message).into());
                }
            }
        }
        
        Ok(())
    }
    
    /// 等待认证结果
    async fn wait_for_auth_result(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut buf = vec![0u8; 4096];
        let n = {
            let mut stream = self.stream.lock().await;
            tokio::time::timeout(
                Duration::from_secs(10),
                stream.read(&mut buf)
            ).await
            .map_err(|_| "Authentication result timeout")?
            .map_err(|e| format!("Read error during auth result: {}", e))?
        };
        
        if n == 0 {
            return Err("Connection closed while waiting for auth result".into());
        }
        
        buf.truncate(n);
        let server_msg: ServerMessage = serde_json::from_slice(&buf)
            .map_err(|e| format!("Failed to parse auth result: {}", e))?;
        
        match server_msg {
            ServerMessage::AuthResult { success, message } => {
                if success {
                    info!("Authentication successful: {}", message);
                    let mut state = self.state.lock().await;
                    *state = ClientState::Authenticated;
                    Ok(())
                } else {
                    error!("Authentication failed: {}", message);
                    let mut state = self.state.lock().await;
                    *state = ClientState::AuthFailed;
                    Err(format!("Authentication failed: {}", message).into())
                }
            }
            _ => Err("Unexpected message type during authentication".into())
        }
    }
    
    /// 发送消息到服务器
    async fn send_message(&self, message: &ClientMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let json_data = serde_json::to_vec(message)?;
        let mut stream = self.stream.lock().await;
        stream.write_all(&json_data).await?;
        stream.write_all(b"\n").await?;
        stream.flush().await?;
        debug!("Message sent: {} bytes", json_data.len());
        Ok(())
    }
    
    /// 检查是否已认证
    pub async fn is_authenticated(&self) -> bool {
        let state = self.state.lock().await;
        *state == ClientState::Authenticated
    }

    pub async fn create_socket(addr: &str) -> std::io::Result<TcpStream> {
        let socket = Socket::new(Domain::IPV4, Type::STREAM, None)?;
        
        let parsed_addr: SocketAddr = addr.parse()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
        
        socket.connect(&parsed_addr.into())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::ConnectionRefused, e))?;

        // 设置TCP keepalive参数
        let keepalive = TcpKeepalive::new()
            .with_time(Duration::from_secs(10))
            .with_interval(Duration::from_secs(5));
        socket.set_tcp_keepalive(&keepalive)?;

        Ok(socket.into())
    }

    pub async fn send_data(&self, data: &[u8]) -> std::io::Result<()> {
        debug!("Sending data to server, size: {} bytes", data.len());
        let mut guard = self.stream.lock().await;
        
        match guard.write_all(data).await {
            Ok(_) => {
                // 确保数据被发送
                if let Err(e) = guard.flush().await {
                    warn!("Failed to flush data: {}", e);
                }
                debug!("Data sent successfully");
                Ok(())
            },
            Err(e) => {
                warn!("Failed to send data, attempting reconnection: {}", e);
                
                // 尝试重新连接
                match Self::connect_with_retry(&self.addr, &self.config).await {
                    Ok(new_stream) => {
                        *guard = new_stream;
                        info!("Reconnected successfully, resending data");
                        guard.write_all(data).await
                    },
                    Err(reconnect_err) => {
                        error!("Reconnection failed: {}", reconnect_err);
                        Err(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, format!("Reconnection failed: {}", reconnect_err)))
                    }
                }
            }
        }
    }

    pub async fn start_heartbeat(&self) {
        let session = self.clone();
        let client_id = match client::get_or_create_client_id(&session.config) {
            Ok(id) => {
                info!("Starting heartbeat with client ID: {}", id);
                id
            },
            Err(e) => {
                error!("Failed to get client ID: {}", e);
                return;
            }
        };

        // Use tokio::spawn to run async tasks - 心跳任务只发送数据，不接收
        tokio::spawn(async move {
            let mut heartbeat_count = 0u64;
            let mut last_successful_heartbeat = SystemTime::now();
            
            loop {
                heartbeat_count += 1;
                debug!("Starting heartbeat #{}", heartbeat_count);
                
                // 收集系统信息
                let sys_info = HostInfo::new();
                let version_info = version_collector::read_app_versions(&session.config.apps_base_dir);
                
                // 收集应用信息
                let app_collector = AppInfoCollector::new(session.config.apps_base_dir.clone());
                let app_info = app_collector.collect_apps_info();
                
                let current_time = SystemTime::now();

                let client_data = ClientInfo {
                    client_id: client_id.clone(),
                    system_info: sys_info,
                    version_info,
                    app_info,
                    last_seen: current_time,
                };

                // 检查是否已认证
                if !session.is_authenticated().await {
                    debug!("Heartbeat #{}: Skipping, not authenticated yet", heartbeat_count);
                    tokio::time::sleep(Duration::from_secs(session.config.heartbeat_interval_secs)).await;
                    continue;
                }

                // 创建客户端消息格式
                let message = ClientMessage::ClientInfo {
                    client_id: client_data.client_id,
                    system_info: client_data.system_info,
                    version_info: client_data.version_info,
                    app_info: client_data.app_info,
                    last_seen: client_data.last_seen,
                };

                // 发送心跳数据
                match session.send_message(&message).await {
                    Ok(()) => {
                        last_successful_heartbeat = current_time;
                        debug!("Heartbeat #{} sent successfully", heartbeat_count);
                    }
                    Err(e) => {
                        error!("Failed to send heartbeat #{}: {}", heartbeat_count, e);
                        
                        // 检查连接健康状态
                        let time_since_last = current_time.duration_since(last_successful_heartbeat)
                            .unwrap_or_else(|_| Duration::from_secs(0));
                        
                        if time_since_last > Duration::from_secs(30) {
                            warn!("Connection seems unhealthy, last successful heartbeat was {:?} ago", time_since_last);
                        }
                    }
                }

                // 等待下次心跳
                tokio::time::sleep(Duration::from_secs(session.config.heartbeat_interval_secs)).await;
            }
        });
    }

    // 处理服务端消息
    async fn process_server_message(&self, data: &[u8]) {
        let message = String::from_utf8_lossy(data);
        debug!("Processing server message: {}", message);

        // 处理不同类型的消息
        let trimmed_message = message.trim();
        if trimmed_message.starts_with("CMD:") {
            // 新格式: CMD:command_id::command 或 旧格式: CMD:command
            let command_part = trimmed_message.trim_start_matches("CMD:");
            
            if let Some((command_id, command)) = command_part.split_once("::") {
                info!("Received command from server: {} (ID: {})", command, command_id);
                self.handle_command_with_id(command_id.trim(), command.trim()).await;
            } else {
                // 兼容旧格式
                info!("Received command from server (legacy): {}", command_part);
                self.handle_command(command_part.trim()).await;
            }
        } else if trimmed_message.starts_with("BROADCAST::") {
            // 处理广播消息
            let broadcast_content = trimmed_message.trim_start_matches("BROADCAST::");
            info!("Received broadcast message: {}", broadcast_content);
            self.handle_broadcast_message(broadcast_content).await;
        } else if trimmed_message.contains("ACK") {
            debug!("Received ACK from server");
        } else if !trimmed_message.is_empty() {
            info!("Received message from server: {}", trimmed_message);
        }
    }

    pub async fn receive(&self) -> std::io::Result<Vec<u8>> {
        let mut guard = self.stream.lock().await;
        let mut buf = [0; 1024];
        
        // 使用超时避免无限阻塞
        let n = tokio::time::timeout(
            Duration::from_secs(1), 
            guard.read(&mut buf)
        ).await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "Read timeout"))?
        .map_err(|e| {
            debug!("Read error: {}", e);
            e
        })?;

        if n > 0 {
            debug!("Received {} bytes: {:?}", n, String::from_utf8_lossy(&buf[..n]));
            Ok(buf[..n].to_vec())
        } else {
            debug!("Received 0 bytes (connection closed)");
            Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Connection closed"))
        }
    }

    // 在TcpSession结构体中添加消息处理
    pub async fn start_message_listener(&self) {
        let session = self.clone();

        tokio::spawn(async move {
            info!("消息监听器已启动");
            loop {
                debug!("等待接收服务端消息...");
                match session.receive().await {
                    Ok(data) => {
                        if !data.is_empty() {
                            let message = String::from_utf8_lossy(&data);
                            debug!("收到服务器消息: {}", message);

                            // 使用现有的消息处理逻辑
                            session.process_server_message(&data).await;
                        } else {
                            debug!("接收到空数据");
                        }
                    }
                    Err(e) => {
                        match e.kind() {
                            std::io::ErrorKind::TimedOut => {
                                // 超时是正常的，继续监听
                                debug!("等待消息超时，继续监听...");
                            }
                            std::io::ErrorKind::UnexpectedEof => {
                                warn!("连接断开，尝试重新连接...");
                                // 尝试重新连接
                                if let Ok(new_stream) = Self::connect_with_retry(&session.addr, &session.config).await {
                                    let mut guard = session.stream.lock().await;
                                    *guard = new_stream;
                                    info!("重新连接成功");
                                } else {
                                    error!("重新连接失败，等待后重试");
                                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                                }
                            }
                            _ => {
                                error!("接收消息错误: {}, 等待后重试", e);
                                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                            }
                        }
                    }
                }
            }
        });
    }

    // 带命令ID的命令处理 - 会将结果返回给服务端
    async fn handle_command_with_id(&self, command_id: &str, command: &str) {
        info!("Executing command with ID {}: {}", command_id, command);

        // 1. 命令验证
        let sanitized_command = self.validator.sanitize_command(command);
        let validation_result = self.validator.validate(&sanitized_command);
        
        // 2. 记录命令到日志
        if let Err(e) = self.log_command(command).await {
            error!("Failed to log command: {}", e);
        }

        let execution_result = match validation_result {
            ValidationResult::Allowed => {
                info!("Command validation passed: {}", sanitized_command);
                self.execute_command(&sanitized_command).await
            }
            ValidationResult::Blocked { reason } => {
                error!("Command blocked: {} (reason: {})", command, reason);
                Err(format!("命令被阻止: {}", reason).into())
            }
        };

        // 3. 准备结果数据
        let (output, error_output, exit_code) = match execution_result {
            Ok(result) => {
                // 解析结果字符串以提取状态码和输出 - 使用安全的字符串操作
                if let Some(status_marker) = result.find("状态码: ") {
                    // 安全地跳过"状态码: "标记
                    let status_part = &result[status_marker..];
                    if let Some(colon_pos) = status_part.find(": ") {
                        let after_colon = &status_part[colon_pos + 2..];
                        if let Some(status_end) = after_colon.find('\n') {
                            let status_str = &after_colon[..status_end];
                            let exit_code = status_str.parse().unwrap_or(-1);
                            
                            if let Some(stdout_marker) = result.find("标准输出:\n") {
                                let after_stdout_marker = &result[stdout_marker..];
                                if let Some(newline_pos) = after_stdout_marker.find('\n') {
                                    let stdout_part = &after_stdout_marker[newline_pos + 1..];
                                    if let Some(stderr_marker_pos) = stdout_part.find("\n错误输出:\n") {
                                        let stdout = stdout_part[..stderr_marker_pos].to_string();
                                        let stderr_part = &stdout_part[stderr_marker_pos..];
                                        if let Some(stderr_newline) = stderr_part.find('\n') {
                                            let stderr = stderr_part[stderr_newline + 1..].to_string();
                                            (stdout, stderr, exit_code)
                                        } else {
                                            (stdout, String::new(), exit_code)
                                        }
                                    } else {
                                        (stdout_part.to_string(), String::new(), exit_code)
                                    }
                                } else {
                                    (result, String::new(), exit_code)
                                }
                            } else {
                                (result, String::new(), exit_code)
                            }
                        } else {
                            (result, String::new(), 0)
                        }
                    } else {
                        (result, String::new(), 0)
                    }
                } else {
                    (result, String::new(), 0)
                }
            }
            Err(e) => {
                (String::new(), e.to_string(), -1)
            }
        };

        // 4. 构建命令结果并发送回服务端
        let executed_at = SystemTime::now();
        let client_id = self.get_client_id().await.unwrap_or_default();
        
        let command_result = ClientMessage::CommandResponse {
            command_id: command_id.to_string(),
            client_id,
            command: command.to_string(),
            output,
            error_output,
            exit_code,
            executed_at,
        };

        match self.send_message(&command_result).await {
            Ok(()) => {
                info!("Command result sent for command ID: {}", command_id);
            }
            Err(e) => {
                error!("Failed to send command result: {}", e);
            }
        }
    }

    // 处理命令 - 添加安全验证 (兼容旧接口)
    async fn handle_command(&self, command: &str) {
        info!("Received command: {}", command);

        // 1. 命令验证
        let sanitized_command = self.validator.sanitize_command(command);
        match self.validator.validate(&sanitized_command) {
            ValidationResult::Allowed => {
                info!("Command validation passed: {}", sanitized_command);
            }
            ValidationResult::Blocked { reason } => {
                error!("Command blocked: {} (reason: {})", command, reason);
                
                let error_response = format!("命令被阻止: {}", reason);
                if let Err(e) = self.send_data(error_response.as_bytes()).await {
                    error!("Failed to send error response: {}", e);
                }
                return;
            }
        }

        // 2. 记录命令到日志
        if let Err(e) = self.log_command(command).await {
            error!("Failed to log command: {}", e);
        }

        // 3. 执行命令
        match self.execute_command(&sanitized_command).await {
            Ok(response) => {
                info!("Command executed successfully");
                if let Err(e) = self.send_data(response.as_bytes()).await {
                    error!("Failed to send command response: {}", e);
                }
            }
            Err(e) => {
                error!("Command execution failed: {}", e);
                let error_response = format!("命令执行失败: {}", e);
                if let Err(e) = self.send_data(error_response.as_bytes()).await {
                    error!("Failed to send error response: {}", e);
                }
            }
        }
    }

    // 记录命令到日志文件
    async fn log_command(&self, command: &str) -> std::io::Result<()> {
        use std::io::Write;
        
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.command_log_file)?;
        
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let log_entry = format!("[{}] {}\n", timestamp, command);
        
        file.write_all(log_entry.as_bytes())?;
        file.flush()?;
        
        Ok(())
    }

    // 安全地执行命令
    async fn execute_command(&self, command: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!("Executing command: {}", command);

        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stdout.is_empty() {
            info!("Command stdout: {}", stdout.trim());
        }

        if !stderr.is_empty() {
            warn!("Command stderr: {}", stderr.trim());
        }

        let response = format!(
            "命令执行完成\n状态码: {}\n标准输出:\n{}\n错误输出:\n{}",
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
        );

        Ok(response)
    }

    // 处理广播消息并发送系统通知
    async fn handle_broadcast_message(&self, message: &str) {
        info!("Handling broadcast message: {}", message);
        
        // 尝试多种Linux系统通知方法
        let mut notification_sent = false;
        
        // 方法1: 使用 wall 命令发送到所有终端
        match self.send_wall_notification(message).await {
            Ok(_) => {
                info!("Broadcast message sent via wall command");
                notification_sent = true;
            }
            Err(e) => {
                warn!("Failed to send wall notification: {}", e);
            }
        }
        
        // 方法2: 使用 notify-send (桌面环境通知)
        match self.send_desktop_notification(message).await {
            Ok(_) => {
                info!("Broadcast message sent via desktop notification");
                notification_sent = true;
            }
            Err(e) => {
                warn!("Failed to send desktop notification: {}", e);
            }
        }
        
        // 方法3: 写入到系统消息文件
        match self.write_to_motd(message).await {
            Ok(_) => {
                info!("Broadcast message written to motd");
                notification_sent = true;
            }
            Err(e) => {
                warn!("Failed to write to motd: {}", e);
            }
        }
        
        // 方法4: 使用 logger 命令写入系统日志
        match self.send_syslog_notification(message).await {
            Ok(_) => {
                info!("Broadcast message sent to syslog");
                notification_sent = true;
            }
            Err(e) => {
                warn!("Failed to send syslog notification: {}", e);
            }
        }
        
        if !notification_sent {
            error!("Failed to send broadcast message via any notification method");
        } else {
            info!("Broadcast message successfully delivered to system");
        }
    }
    
    // 使用 wall 命令发送到所有登录终端
    async fn send_wall_notification(&self, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let formatted_message = format!("【OPS系统广播】{}", message);
        
        let output = tokio::process::Command::new("wall")
            .arg(&formatted_message)
            .output()
            .await?;
            
        if !output.status.success() {
            return Err(format!("wall command failed with status: {}", output.status).into());
        }
        
        Ok(())
    }
    
    // 使用 notify-send 发送桌面通知
    async fn send_desktop_notification(&self, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let output = tokio::process::Command::new("notify-send")
            .arg("OPS系统广播")
            .arg(message)
            .arg("--urgency=critical")
            .arg("--expire-time=10000") // 10秒后自动消失
            .output()
            .await?;
            
        if !output.status.success() {
            return Err(format!("notify-send command failed with status: {}", output.status).into());
        }
        
        Ok(())
    }
    
    // 写入到 motd 文件 (登录时显示的消息)
    async fn write_to_motd(&self, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use std::io::Write;
        
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let motd_message = format!("\n=== OPS系统广播 [{}] ===\n{}\n===============================\n", timestamp, message);
        
        // 尝试写入到用户的 .motd 文件
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let motd_path = format!("{}/.ops_motd", home_dir);
        
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&motd_path)?;
            
        file.write_all(motd_message.as_bytes())?;
        file.flush()?;
        
        // 设置权限，确保用户可读
        let _ = std::process::Command::new("chmod")
            .arg("644")
            .arg(&motd_path)
            .output();
            
        info!("Broadcast message written to: {}", motd_path);
        Ok(())
    }
    
    // 使用 logger 发送到系统日志
    async fn send_syslog_notification(&self, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let log_message = format!("OPS系统广播: {}", message);
        
        let output = tokio::process::Command::new("logger")
            .arg("-t")
            .arg("ops-client")
            .arg("-p")
            .arg("user.notice")
            .arg(&log_message)
            .output()
            .await?;
            
        if !output.status.success() {
            return Err(format!("logger command failed with status: {}", output.status).into());
        }
        
        Ok(())
    }

    // 获取客户端ID的辅助方法
    async fn get_client_id(&self) -> Result<String, std::io::Error> {
        client::get_or_create_client_id(&self.config)
    }
}

impl Clone for TcpSession {
    fn clone(&self) -> Self {
        Self {
            stream: Arc::clone(&self.stream),
            addr: self.addr.clone(),
            config: self.config.clone(),
            validator: self.validator.clone(),
            state: Arc::clone(&self.state),
            authenticator: self.authenticator.clone(),
        }
    }
}
