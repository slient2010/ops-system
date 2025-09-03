use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{ Mutex, MutexGuard };
use tokio::net::TcpStream;
use std::collections::HashMap;
use crate::ClientInfo;
use crate::command_results::CommandResultsManager;

#[derive(Clone)]
pub struct SharedDataHandle(Arc<Mutex<SharedData>>);

impl SharedDataHandle {
    pub fn new(data: SharedData) -> Self {
        SharedDataHandle(Arc::new(Mutex::new(data)))
    }

    pub fn clone(&self) -> Self {
        SharedDataHandle(Arc::clone(&self.0))
    }

    pub async fn lock(&self) -> MutexGuard<'_, SharedData> {
        self.0.lock().await
    }
}

#[derive(Default)]
pub struct SharedData {
    pub client_data: HashMap<String, ClientInfo>,
    pub client_connections: HashMap<String, Arc<Mutex<TcpStream>>>,
    pub max_connections: usize,
    pub connection_count: usize,
    pub command_results: CommandResultsManager,
}

impl SharedData {
    pub fn new(max_connections: usize) -> Self {
        Self {
            client_data: HashMap::new(),
            client_connections: HashMap::new(),
            max_connections,
            connection_count: 0,
            command_results: CommandResultsManager::new(1000), // 最多存储1000个结果
        }
    }
}

impl SharedData {
    // 添加或更新客户端连接 - 带连接数限制
    pub async fn add_client_connection(
        &mut self,
        client_id: String,
        stream: Arc<Mutex<TcpStream>>
    ) -> Result<(), String> {
        // 检查连接数限制
        if self.connection_count >= self.max_connections && !self.client_connections.contains_key(&client_id) {
            return Err(format!("Maximum connections reached: {}", self.max_connections));
        }

        // 如果是新连接，增加计数
        if !self.client_connections.contains_key(&client_id) {
            self.connection_count += 1;
        }

        self.client_connections.insert(client_id, stream);
        Ok(())
    }

    // 移除客户端连接
    pub async fn remove_client_connection(&mut self, client_id: &str) {
        if self.client_connections.remove(client_id).is_some() {
            self.connection_count = self.connection_count.saturating_sub(1);
        }
    }

    // 广播消息给所有连接的客户端
    pub async fn broadcast_message(
        &self,
        message: &str
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("当前连接数: {}", self.client_connections.len());
        
        // 构建带有消息类型的广播消息
        let broadcast_message = format!("BROADCAST::{}\n", message);
        
        for (id, stream) in &self.client_connections {
            let mut stream = stream.lock().await;
            if let Err(e) = stream.write_all(broadcast_message.as_bytes()).await {
                eprintln!("发送消息到客户端 {} 失败: {}", id, e);
            } else {
                // 确保数据被发送
                if let Err(flush_err) = stream.flush().await {
                    eprintln!("刷新数据到客户端 {} 失败: {}", id, flush_err);
                } else {
                    println!("广播消息已发送到客户端: {}", id);
                }
            }
        }
        Ok(())
    }

    // 发送命令给特定客户端并返回命令ID用于跟踪结果
    pub async fn send_command_to_client(
        &self,
        client_id: &str,
        command: &str
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(stream) = self.client_connections.get(client_id) {
            // 创建命令请求并获取命令ID
            let command_id = self.command_results.create_command(client_id.to_string(), command.to_string()).await;
            
            // 发送带有命令ID的命令
            let command_with_id = format!("CMD:{}::{}\n", command_id, command);
            
            tracing::debug!("Preparing to send command to client {}: {}", client_id, command_with_id.trim());
            
            let mut stream_guard = stream.lock().await;
            match stream_guard.write_all(command_with_id.as_bytes()).await {
                Ok(_) => {
                    // 确保数据被发送
                    if let Err(flush_err) = stream_guard.flush().await {
                        tracing::error!("Failed to flush command to client {}: {}", client_id, flush_err);
                        return Err(flush_err.into());
                    }
                    
                    // 标记命令为执行中
                    self.command_results.mark_executing(&command_id).await;
                    
                    tracing::info!("Command {} sent to client {} successfully", command_id, client_id);
                    Ok(command_id)
                }
                Err(write_err) => {
                    tracing::error!("Failed to write command to client {}: {}", client_id, write_err);
                    Err(write_err.into())
                }
            }
        } else {
            tracing::error!("Client {} not connected", client_id);
            Err("客户端未连接".into())
        }
    }
}
