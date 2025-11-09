use tokio::net::TcpListener;
use std::collections::HashMap;
use std::time::{SystemTime, Duration};
use std::process;
use std::net::SocketAddr;
use tracing::{info, error, warn};
use tracing_subscriber::{
    layer::SubscriberExt, 
    util::SubscriberInitExt, 
    EnvFilter, 
    Layer,
    fmt::writer::MakeWriterExt
};
use tracing_appender::{rolling, non_blocking};

mod web;
mod tcp_services;
mod shared_data_handle;
mod middleware;
mod command_results;

use crate::shared_data_handle::{SharedDataHandle, SharedData};
use crate::tcp_services::handle_socket;
use crate::middleware::AuthConfig;

use ops_common::{ClientInfo, config::ServerConfig};
use ops_common::log_rotation::ThreadSafeLogRotator;

#[cfg(test)]
mod tests;

// 设置日志配置
fn setup_logging(config: &ServerConfig) {
    // Use size-based rotation if configured
    let log_directory = config.log_directory.clone();
    let log_rotation_size_mb = config.log_rotation_size_mb;
    
    // Create custom writers with size-based rotation - these will be used by the web middleware
    let web_log_rotator = ThreadSafeLogRotator::new(
        format!("{}/web.log", log_directory), 
        log_rotation_size_mb, 
        log_directory.clone()
    ).expect("Failed to create web log rotator");
    
    // Create another rotator for the application logs
    let app_log_rotator = ThreadSafeLogRotator::new(
        format!("{}/ops-server.log", log_directory), 
        log_rotation_size_mb, 
        log_directory
    ).expect("Failed to create app log rotator");

    // Create custom writer implementations that use our log rotators
    // We use a thin wrapper that implements Write but try to minimize blocking
    
    struct NonBlockingLogWriter {
        rotator: ThreadSafeLogRotator,
    }

    impl NonBlockingLogWriter {
        fn new(rotator: ThreadSafeLogRotator) -> Self {
            Self { rotator }
        }
    }

    impl std::io::Write for NonBlockingLogWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            // The ThreadSafeLogRotator handles thread safety internally
            self.rotator.write(buf)?;
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    // Create writers for different log types
    let web_log_writer = NonBlockingLogWriter::new(web_log_rotator);
    let app_log_writer = NonBlockingLogWriter::new(app_log_rotator);

    // 配置 web 访问日志层 - 只记录 web_access 目标的日志
    let web_access_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::sync::Mutex::new(web_log_writer))
        .with_target(true)
        .with_level(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .with_ansi(false)
        .with_filter(EnvFilter::new("web_access=info"));

    // 配置应用日志层 - 记录除了 web_access 外的所有日志
    let app_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::sync::Mutex::new(app_log_writer))
        .with_filter(EnvFilter::new("info").add_directive("web_access=off".parse().unwrap()));

    // 配置控制台日志层
    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .with_filter(EnvFilter::from_default_env());

    // 组合所有层
    tracing_subscriber::registry()
        .with(web_access_layer)
        .with(app_layer)
        .with(console_layer)
        .init();
}

// HTTP 服务
async fn launch_http_server(shared_data: SharedDataHandle, config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let auth_config = AuthConfig::new(config.auth_token.clone());
    let (app, session_store) = web::routes::routes(shared_data, auth_config);
    
    // 启动会话清理任务
    let session_cleanup_store = session_store.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await; // 每5分钟清理一次
            session_cleanup_store.cleanup_expired_sessions(std::time::Duration::from_secs(3600)).await; // 1小时超时
        }
    });
    
    let addr = config.http_address();

    info!("HTTP server starting on {}", addr);
    
    let parsed_addr = addr.parse().map_err(|e| {
        error!("Invalid HTTP bind address {}: {}", addr, e);
        e
    })?;

    axum_server::bind(parsed_addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .map_err(|e| {
            error!("HTTP server failed: {}", e);
            e.into()
        })
}

// 自定义异步 Socket 服务
async fn launch_tcp_server(shared_data: SharedDataHandle, config: ServerConfig) -> Result<(), Box<dyn std::error::Error>> {
    let addr = config.tcp_address();
    
    let listener = TcpListener::bind(&addr).await.map_err(|e| {
        error!("Failed to bind TCP server to {}: {}", addr, e);
        e
    })?;
    
    info!("TCP server listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, client_addr)) => {
                info!("New client connection from: {}", client_addr);
                let shared_data = shared_data.clone();
                
                tokio::spawn(async move {
                    if let Err(e) = handle_socket::handle_client_connection(stream, shared_data).await {
                        error!("Client connection error from {}: {}", client_addr, e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 加载配置
    let config = ServerConfig::from_env();
    
    // 初始化日志配置
    setup_logging(&config);
    
    info!("Server starting with config: TCP={}, HTTP={}", config.tcp_address(), config.http_address());

    let shared_data = SharedDataHandle::new(SharedData::new(config.max_connections));
    let cleanup_data = shared_data.clone();
    let socket_data = shared_data.clone();
    let web_data = shared_data.clone();

    // 启动清理任务
    let cleanup_interval = config.cleanup_interval_secs;
    let client_timeout = config.client_timeout_secs;
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(cleanup_interval)).await;
            
            let mut data = cleanup_data.lock().await;
            let now = SystemTime::now();
            let before_count = data.client_data.len();
            
            // 收集需要清理的客户端ID
            let mut expired_clients = Vec::new();
            
            data.client_data.retain(|client_id, client_data| {
                let is_valid = now.duration_since(client_data.last_seen)
                    .map(|duration| duration.as_secs() < client_timeout)
                    .unwrap_or(false);
                
                if !is_valid {
                    info!("Removing expired client: {}", client_id);
                    expired_clients.push(client_id.clone());
                }
                is_valid
            });
            
            // 同步清理连接
            for client_id in &expired_clients {
                data.remove_client_connection(client_id).await;
            }
            
            let after_count = data.client_data.len();
            if before_count != after_count {
                info!("Cleaned up {} expired clients, {} remaining", before_count - after_count, after_count);
            }
        }
    });

    // 同时运行两个服务
    let result = tokio::try_join!(
        launch_http_server(web_data, config.clone()),
        launch_tcp_server(socket_data, config)
    );

    match result {
        Ok(_) => {
            info!("All servers stopped");
            Ok(())
        }
        Err(e) => {
            error!("Server error: {}", e);
            process::exit(1);
        }
    }
}
