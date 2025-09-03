use axum::{ Json, extract::{ State, Path, Query }, http::StatusCode,response::{ Html, IntoResponse, Response }, };
use std::collections::HashMap;
use serde::{ Deserialize, Serialize };
use std::time::{SystemTime, Duration};
use crate::{ ClientInfo, SharedDataHandle };
use crate::command_results::{CommandResult, CommandStatus};
use ops_common::security::{CommandValidator, PredefinedCommand};
use axum::http::header::{SET_COOKIE, HeaderMap};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct ClientResponse {
    pub clients: HashMap<String, ClientInfo>,
}

// 新增：广播消息请求结构体
#[derive(Deserialize)]
pub struct BroadcastMessage {
    pub message: String,
}

// 新增：发送命令请求结构体
#[derive(Deserialize)]
pub struct CommandRequest {
    pub client_id: String,
    pub command: String,
}

// 新增：广播消息处理
pub async fn broadcast_message(
    State(shared_data): State<SharedDataHandle>,
    Json(payload): Json<BroadcastMessage>
) -> Result<String, (StatusCode, String)> {
    // 这里应该实现实际的消息发送逻辑
    println!("广播消息: {}", payload.message);
    shared_data
        .lock().await
        .broadcast_message(&payload.message).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // 实际应用中应该通过某种机制通知所有客户端
    // 比如通过一个消息队列或全局状态保存的客户端连接

    Ok(format!("消息已广播: {}", payload.message))
}

#[derive(Serialize)]
pub struct CommandExecuteResponse {
    pub command_id: String,
    pub message: String,
}

// 发送命令给特定客户端
pub async fn send_command(
    State(shared_data): State<SharedDataHandle>,
    Json(payload): Json<CommandRequest>
) -> Result<Json<CommandExecuteResponse>, (StatusCode, String)> {
    tracing::info!("Received command request: client_id={}, command={}", payload.client_id, payload.command);

    match shared_data
        .lock()
        .await
        .send_command_to_client(&payload.client_id, &payload.command)
        .await
    {
        Ok(command_id) => {
            Ok(Json(CommandExecuteResponse {
                command_id,
                message: format!("命令已发送到客户端 {}", payload.client_id),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to send command: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

// 获取命令执行结果
#[derive(Deserialize)]
pub struct CommandStatusQuery {
    pub command_id: String,
}

pub async fn get_command_result(
    State(shared_data): State<SharedDataHandle>,
    Query(params): Query<CommandStatusQuery>
) -> Result<Json<CommandStatus>, (StatusCode, String)> {
    let data = shared_data.lock().await;
    
    match data.command_results.get_command_status(&params.command_id).await {
        Some(status) => Ok(Json(status)),
        None => Err((StatusCode::NOT_FOUND, "Command not found".to_string())),
    }
}

// 获取客户端的命令历史
#[derive(Deserialize)]
pub struct ClientHistoryQuery {
    pub client_id: String,
    pub limit: Option<usize>,
}

pub async fn get_client_command_history(
    State(shared_data): State<SharedDataHandle>,
    Query(params): Query<ClientHistoryQuery>
) -> Result<Json<Vec<CommandResult>>, (StatusCode, String)> {
    let data = shared_data.lock().await;
    let limit = params.limit.unwrap_or(20);
    
    let results = data.command_results.get_client_results(&params.client_id, limit).await;
    Ok(Json(results))
}

// 列出所有客户端 - 优化版本
pub async fn list_clients(
    State(shared_data): State<SharedDataHandle>
) -> Result<Json<ClientResponse>, (StatusCode, String)> {
    let data = shared_data.lock().await;
    
    // 限制返回的客户端数量，避免大量数据传输
    const MAX_CLIENTS: usize = 100;
    
    let clients: HashMap<String, ClientInfo> = data.client_data
        .iter()
        .take(MAX_CLIENTS)
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    
    if data.client_data.len() > MAX_CLIENTS {
        tracing::warn!("Truncated client list from {} to {} entries", data.client_data.len(), MAX_CLIENTS);
    }
    
    Ok(Json(ClientResponse { clients }))
}

// 返回前端页面 index.html
pub async fn index() -> impl IntoResponse {
    let html = include_str!("../../static/index.html");
    Html(html).into_response()
}

// 健康检查端点
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: SystemTime,
    pub clients_count: usize,
    pub uptime_seconds: u64,
}

pub async fn health_check(
    State(shared_data): State<SharedDataHandle>
) -> Json<HealthResponse> {
    let data = shared_data.lock().await;
    let clients_count = data.client_data.len();
    
    Json(HealthResponse {
        status: "healthy".to_string(),
        timestamp: SystemTime::now(),
        clients_count,
        uptime_seconds: 0, // TODO: 实现实际的运行时间跟踪
    })
}

// 获取预定义安全命令列表
pub async fn get_predefined_commands() -> Json<Vec<PredefinedCommand>> {
    Json(CommandValidator::get_predefined_commands())
}

// 服务管理相关的结构体
#[derive(Deserialize)]
pub struct ServiceManagementRequest {
    pub client_id: String,
    pub app_name: String,
    pub action: ServiceAction,
}

#[derive(Deserialize)]
pub enum ServiceAction {
    Start,
    Stop,
    Restart,
    Status,
}

#[derive(Deserialize)]
pub struct UpdateRequest {
    pub client_id: String,
    pub app_name: String,
    pub version: String,
}

// 服务管理端点
pub async fn manage_service(
    State(shared_data): State<SharedDataHandle>,
    Json(payload): Json<ServiceManagementRequest>
) -> Result<Json<CommandExecuteResponse>, (StatusCode, String)> {
    let command = match payload.action {
        ServiceAction::Start => {
            // 启动服务：执行应用目录下的脚本文件
            format!("cd /tmp/apps/{} && bash {}.sh start", payload.app_name, payload.app_name)
        },
        ServiceAction::Stop => {
            // 停止服务：杀死PID文件中的进程
            format!("cd /tmp/apps/{} && if [ -f {}.pid ]; then kill $(cat {}.pid) && rm -f {}.pid; else echo 'Service is not running'; fi", payload.app_name, payload.app_name, payload.app_name, payload.app_name)
        },
        ServiceAction::Restart => {
            // 重启服务：先停止再启动
            format!("cd /tmp/apps/{} && (if [ -f {}.pid ]; then kill $(cat {}.pid) && rm -f {}.pid; fi) && sleep 1 && bash {}.sh start", payload.app_name, payload.app_name, payload.app_name, payload.app_name, payload.app_name)
        },
        ServiceAction::Status => {
            // 检查状态：查看PID文件和进程状态
            format!("cd /tmp/apps/{} && if [ -f {}.pid ]; then pid=$(cat {}.pid); if ps -p $pid > /dev/null 2>&1; then echo 'Service is running (PID: '$pid')'; else echo 'PID file exists but process is not running'; fi; else echo 'Service is not running'; fi", payload.app_name, payload.app_name, payload.app_name)
        },
    };

    match shared_data
        .lock()
        .await
        .send_command_to_client(&payload.client_id, &command)
        .await
    {
        Ok(command_id) => {
            Ok(Json(CommandExecuteResponse {
                command_id,
                message: format!("服务管理命令已发送到客户端 {}", payload.client_id),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to send service management command: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

// 应用更新端点
pub async fn update_app(
    State(shared_data): State<SharedDataHandle>,
    Json(payload): Json<UpdateRequest>
) -> Result<Json<CommandExecuteResponse>, (StatusCode, String)> {
    let command = format!("cd /tmp/apps/{} && bash {}.sh update {}", payload.app_name, payload.app_name, payload.version);

    match shared_data
        .lock()
        .await
        .send_command_to_client(&payload.client_id, &command)
        .await
    {
        Ok(command_id) => {
            Ok(Json(CommandExecuteResponse {
                command_id,
                message: format!("应用更新命令已发送到客户端 {}", payload.client_id),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to send update command: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

// 获取所有客户端的应用信息
#[derive(Serialize)]
pub struct AppInfoResponse {
    pub client_apps: HashMap<String, ClientAppInfo>,
}

#[derive(Serialize)]
pub struct ClientAppInfo {
    pub client_id: String,
    pub hostname: String,
    pub apps: Vec<ops_common::AppInfo>,
}

pub async fn get_apps_info(
    State(shared_data): State<SharedDataHandle>
) -> Json<AppInfoResponse> {
    let data = shared_data.lock().await;
    let mut client_apps = HashMap::new();
    
    for (client_id, client_info) in &data.client_data {
        let client_app_info = ClientAppInfo {
            client_id: client_id.clone(),
            hostname: client_info.system_info.hostname.clone(),
            apps: client_info.app_info.clone(),
        };
        client_apps.insert(client_id.clone(), client_app_info);
    }
    
    Json(AppInfoResponse { client_apps })
}

// 获取特定客户端的应用信息
#[derive(Deserialize)]
pub struct ClientIdQuery {
    pub client_id: String,
}

pub async fn get_client_apps_info(
    State(shared_data): State<SharedDataHandle>,
    Query(query): Query<ClientIdQuery>,
) -> Result<Json<Vec<ops_common::AppInfo>>, StatusCode> {
    let data = shared_data.lock().await;
    
    match data.client_data.get(&query.client_id) {
        Some(client_info) => Ok(Json(client_info.app_info.clone())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// 用户认证相关结构体
#[derive(Clone)]
pub struct SessionStore {
    sessions: Arc<RwLock<HashMap<String, SessionData>>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_session(&self, user_id: String) -> String {
        let session_id = Uuid::new_v4().to_string();
        let session_data = SessionData {
            user_id,
            created_at: SystemTime::now(),
            last_accessed: SystemTime::now(),
        };
        
        self.sessions.write().await.insert(session_id.clone(), session_data);
        session_id
    }

    pub async fn get_session(&self, session_id: &str) -> Option<SessionData> {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            // 更新最后访问时间
            session.last_accessed = SystemTime::now();
            Some(session.clone())
        } else {
            None
        }
    }

    pub async fn remove_session(&self, session_id: &str) -> bool {
        self.sessions.write().await.remove(session_id).is_some()
    }

    // 清理过期的会话（可选）
    pub async fn cleanup_expired_sessions(&self, max_age: Duration) {
        let now = SystemTime::now();
        let mut sessions = self.sessions.write().await;
        let before_count = sessions.len();
        sessions.retain(|_, session| {
            if let Ok(elapsed) = now.duration_since(session.last_accessed) {
                elapsed < max_age
            } else {
                false
            }
        });
        let after_count = sessions.len();
        if before_count != after_count {
            tracing::info!("Cleaned up {} expired sessions", before_count - after_count);
        }
    }
    
    // 检查会话是否存在且有效
    pub async fn is_session_valid(&self, session_id: &str, max_age: Duration) -> bool {
        if let Some(session) = self.sessions.read().await.get(session_id) {
            let now = SystemTime::now();
            if let Ok(elapsed) = now.duration_since(session.last_accessed) {
                elapsed < max_age
            } else {
                false
            }
        } else {
            false
        }
    }
}

#[derive(Clone)]
pub struct SessionData {
    pub user_id: String,
    pub created_at: SystemTime,
    pub last_accessed: SystemTime,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    pub session_id: Option<String>,
}

// 用户认证配置（简单示例，实际应该从配置文件读取）
pub struct UserAuth {
    pub username: String,
    pub password: String,
}

impl Default for UserAuth {
    fn default() -> Self {
        Self {
            username: "admin".to_string(),
            password: "admin123".to_string(),  // 实际应该是加密的密码
        }
    }
}

// 登录端点
pub async fn login(
    State(session_store): State<SessionStore>,
    Json(payload): Json<LoginRequest>,
) -> Result<(HeaderMap, Json<LoginResponse>), (StatusCode, String)> {
    let user_auth = UserAuth::default();
    
    // 简单的用户名密码验证
    if payload.username == user_auth.username && payload.password == user_auth.password {
        // 创建会话
        let session_id = session_store.create_session(payload.username.clone()).await;
        
        // 设置 HTTP-only Cookie - 1小时有效期
        let mut headers = HeaderMap::new();
        // 在开发环境中移除Secure标志，因为我们使用HTTP
        let is_dev = std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "development";
        let cookie_value = if is_dev {
            format!(
                "session_id={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=3600", 
                session_id
            )
        } else {
            format!(
                "session_id={}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=3600", 
                session_id
            )
        };
        headers.insert(SET_COOKIE, cookie_value.parse().unwrap());
        
        Ok((headers, Json(LoginResponse {
            success: true,
            message: "登录成功".to_string(),
            session_id: Some(session_id),
        })))
    } else {
        Ok((HeaderMap::new(), Json(LoginResponse {
            success: false,
            message: "用户名或密码错误".to_string(),
            session_id: None,
        })))
    }
}

// 登出端点
pub async fn logout(
    State(session_store): State<SessionStore>,
    headers: HeaderMap,
) -> Result<(HeaderMap, Json<LoginResponse>), (StatusCode, String)> {
    // 从 Cookie 中获取 session_id
    if let Some(session_id) = extract_session_from_headers(&headers) {
        session_store.remove_session(&session_id).await;
    }
    
    // 清除 Cookie
    let mut response_headers = HeaderMap::new();
    // 在开发环境中移除Secure标志
    let is_dev = std::env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string()) == "development";
    let cookie_value = if is_dev {
        "session_id=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0"
    } else {
        "session_id=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0"
    };
    response_headers.insert(SET_COOKIE, cookie_value.parse().unwrap());
    
    Ok((response_headers, Json(LoginResponse {
        success: true,
        message: "登出成功".to_string(),
        session_id: None,
    })))
}

// 检查认证状态端点
pub async fn check_auth(
    State(session_store): State<SessionStore>,
    headers: HeaderMap,
) -> Json<LoginResponse> {
    const SESSION_TIMEOUT: Duration = Duration::from_secs(3600); // 1小时会话超时
    
    if let Some(session_id) = extract_session_from_headers(&headers) {
        // 检查会话是否有效且未过期
        if session_store.is_session_valid(&session_id, SESSION_TIMEOUT).await {
            // 更新最后访问时间（延长会话）
            if let Some(session) = session_store.get_session(&session_id).await {
                return Json(LoginResponse {
                    success: true,
                    message: "已认证".to_string(),
                    session_id: Some(session_id),
                });
            }
        } else {
            // 会话已过期，清理它
            session_store.remove_session(&session_id).await;
        }
    }
    
    Json(LoginResponse {
        success: false,
        message: "未认证或会话已过期".to_string(),
        session_id: None,
    })
}

// 从请求头中提取会话ID的辅助函数
fn extract_session_from_headers(headers: &HeaderMap) -> Option<String> {
    headers.get("cookie")
        .and_then(|header| header.to_str().ok())
        .and_then(|cookie_str| {
            cookie_str.split(';')
                .find_map(|cookie| {
                    let parts: Vec<&str> = cookie.trim().split('=').collect();
                    if parts.len() == 2 && parts[0] == "session_id" {
                        Some(parts[1].to_string())
                    } else {
                        None
                    }
                })
        })
}