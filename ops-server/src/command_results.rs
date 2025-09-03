use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, Duration};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult {
    pub command_id: String,
    pub client_id: String,
    pub command: String,
    pub output: String,
    pub error_output: String,
    pub exit_code: i32,
    pub executed_at: SystemTime,
    pub received_at: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CommandStatus {
    Pending,
    Executing,
    Completed(CommandResult),
    Failed(String),
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCommand {
    pub command_id: String,
    pub client_id: String,
    pub command: String,
    pub created_at: SystemTime,
    pub status: CommandStatus,
}

#[derive(Default)]
pub struct CommandResultsManager {
    pending_commands: Arc<RwLock<HashMap<String, PendingCommand>>>,
    completed_results: Arc<RwLock<HashMap<String, CommandResult>>>,
    max_results: usize,
}

impl CommandResultsManager {
    pub fn new(max_results: usize) -> Self {
        Self {
            pending_commands: Arc::new(RwLock::new(HashMap::new())),
            completed_results: Arc::new(RwLock::new(HashMap::new())),
            max_results,
        }
    }

    // 创建一个新的命令请求
    pub async fn create_command(&self, client_id: String, command: String) -> String {
        let command_id = Uuid::new_v4().to_string();
        let pending_command = PendingCommand {
            command_id: command_id.clone(),
            client_id,
            command,
            created_at: SystemTime::now(),
            status: CommandStatus::Pending,
        };

        let mut pending = self.pending_commands.write().await;
        pending.insert(command_id.clone(), pending_command);
        
        tracing::info!("Created command request: {}", command_id);
        command_id
    }

    // 标记命令为执行中
    pub async fn mark_executing(&self, command_id: &str) -> bool {
        let mut pending = self.pending_commands.write().await;
        if let Some(cmd) = pending.get_mut(command_id) {
            cmd.status = CommandStatus::Executing;
            tracing::info!("Command {} marked as executing", command_id);
            true
        } else {
            false
        }
    }

    // 存储命令执行结果
    pub async fn store_result(&self, result: CommandResult) {
        let command_id = result.command_id.clone();
        
        // 从待执行列表中移除
        {
            let mut pending = self.pending_commands.write().await;
            pending.remove(&command_id);
        }

        // 添加到完成结果中
        {
            let mut results = self.completed_results.write().await;
            
            // 如果结果太多，删除最旧的
            if results.len() >= self.max_results {
                // 找到最旧的结果并删除
                if let Some((oldest_id, _)) = results.iter()
                    .min_by_key(|(_, result)| result.received_at) {
                    let oldest_id = oldest_id.clone();
                    results.remove(&oldest_id);
                }
            }
            
            results.insert(command_id.clone(), result);
        }

        tracing::info!("Stored result for command: {}", command_id);
    }

    // 获取命令结果
    pub async fn get_result(&self, command_id: &str) -> Option<CommandResult> {
        let results = self.completed_results.read().await;
        results.get(command_id).cloned()
    }

    // 获取命令状态
    pub async fn get_command_status(&self, command_id: &str) -> Option<CommandStatus> {
        // 首先检查是否在待执行列表中
        {
            let pending = self.pending_commands.read().await;
            if let Some(cmd) = pending.get(command_id) {
                return Some(cmd.status.clone());
            }
        }

        // 然后检查完成的结果
        {
            let results = self.completed_results.read().await;
            if let Some(result) = results.get(command_id) {
                return Some(CommandStatus::Completed(result.clone()));
            }
        }

        None
    }

    // 获取客户端的所有最近结果
    pub async fn get_client_results(&self, client_id: &str, limit: usize) -> Vec<CommandResult> {
        let results = self.completed_results.read().await;
        let mut client_results: Vec<CommandResult> = results.values()
            .filter(|r| r.client_id == client_id)
            .cloned()
            .collect();
        
        // 按接收时间排序，最新的在前
        client_results.sort_by(|a, b| b.received_at.cmp(&a.received_at));
        client_results.truncate(limit);
        client_results
    }

    // 清理过期的待执行命令
    pub async fn cleanup_expired_commands(&self, timeout_duration: Duration) {
        let mut pending = self.pending_commands.write().await;
        let now = SystemTime::now();
        
        let mut expired_ids = Vec::new();
        
        for (id, cmd) in pending.iter_mut() {
            if let Ok(elapsed) = now.duration_since(cmd.created_at) {
                if elapsed > timeout_duration {
                    expired_ids.push(id.clone());
                    cmd.status = CommandStatus::Timeout;
                }
            }
        }

        for id in expired_ids {
            pending.remove(&id);
            tracing::warn!("Command {} timed out and was removed", id);
        }
    }

    // 获取统计信息
    pub async fn get_stats(&self) -> (usize, usize) {
        let pending = self.pending_commands.read().await;
        let completed = self.completed_results.read().await;
        (pending.len(), completed.len())
    }
}