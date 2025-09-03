use axum::{
    extract::State,
    http::StatusCode,
    middleware::Next,
    response::Response,
    body::Body,
    http::Request,
    extract::ConnectInfo,
    http::HeaderMap,
};
use std::net::SocketAddr;
use tracing::{warn, debug, info};
use ops_common::security::validate_auth_header;
use crate::shared_data_handle::SharedDataHandle;
use crate::web::handlers::SessionStore;

#[derive(Clone)]
pub struct AuthConfig {
    pub token: Option<String>,
    pub enabled: bool,
    pub session_store: Option<SessionStore>,
}

impl AuthConfig {
    pub fn new(token: Option<String>) -> Self {
        Self {
            enabled: token.is_some(),
            token,
            session_store: None,
        }
    }

    pub fn with_session_store(mut self, session_store: SessionStore) -> Self {
        self.session_store = Some(session_store);
        self
    }
}

pub async fn auth_middleware(
    State(auth_config): State<AuthConfig>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let headers = request.headers();
    
    // 首先尝试基于Session的认证
    if let Some(session_store) = &auth_config.session_store {
        if let Some(session_id) = extract_session_from_headers(headers) {
            // 检查Session是否有效（1小时内）
            if session_store.is_session_valid(&session_id, std::time::Duration::from_secs(3600)).await {
                debug!("Session authentication successful");
                return Ok(next.run(request).await);
            }
        }
    }
    
    // 回退到基于Token的认证（如果启用）
    if auth_config.enabled {
        let expected_token = auth_config.token.as_ref().unwrap();

        // 检查 Authorization header
        let auth_header = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|header| header.to_str().ok());

        match auth_header {
            Some(header) => {
                if validate_auth_header(header, expected_token) {
                    debug!("Token authentication successful");
                    return Ok(next.run(request).await);
                } else {
                    warn!("Authentication failed: invalid token");
                }
            }
            None => {
                warn!("Authentication failed: missing credentials");
            }
        }
    }
    
    // 所有认证方法都失败
    Err(StatusCode::UNAUTHORIZED)
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

// CORS 中间件
pub async fn cors_middleware(
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let mut response = next.run(request).await;
    
    let headers = response.headers_mut();
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN,
        "*".parse().unwrap(),
    );
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_METHODS,
        "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap(),
    );
    headers.insert(
        axum::http::header::ACCESS_CONTROL_ALLOW_HEADERS,
        "Content-Type, Authorization".parse().unwrap(),
    );

    Ok(response)
}

// Web 请求日志中间件
pub async fn web_logging_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let start_time = std::time::Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();
    let headers = request.headers().clone();
    
    // 获取 User-Agent
    let user_agent = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");
    
    // 获取 Referer
    let referer = headers
        .get(axum::http::header::REFERER)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");
    
    // 获取 Content-Type
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("-");
    
    // 获取请求体大小
    let content_length = headers
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("0");

    // 执行请求
    let response = next.run(request).await;
    
    let duration = start_time.elapsed();
    let status = response.status();
    
    // 记录 web 访问日志到专用的 web.log
    info!(
        target: "web_access",
        "{} - [{}] \"{} {} {:?}\" {} {} \"{}\" \"{}\" \"{}\" {:.3}ms",
        addr.ip(),
        chrono::Utc::now().format("%d/%b/%Y:%H:%M:%S %z"),
        method,
        uri.path_and_query().map(|p| p.as_str()).unwrap_or("/"),
        version,
        status.as_u16(),
        content_length,
        referer,
        user_agent,
        content_type,
        duration.as_secs_f64() * 1000.0
    );
    
    Ok(response)
}