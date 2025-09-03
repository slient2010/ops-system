use std::time::Duration;
use std::process;
use tokio::spawn;
use tracing::{info, error};
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use tracing_appender::{rolling, non_blocking};

mod collection;
mod tcp_services;

use crate::tcp_services::client;
use ops_common::config::ClientConfig;

#[cfg(test)]
mod tests;

// 设置客户端日志配置
fn setup_logging() {
    // 创建客户端日志的文件 appender
    let client_log_file = rolling::daily(".", "ops-client.log");
    let (client_log_writer, client_log_guard) = non_blocking(client_log_file);

    // 配置日志层 - 记录到文件和控制台
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(client_log_writer)
        .with_target(true)
        .with_ansi(false)
        .with_filter(EnvFilter::new("info"));

    // 控制台层
    let console_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_target(false)
        .with_filter(EnvFilter::new("info"));

    // 组合所有层
    tracing_subscriber::registry()
        .with(file_layer)
        .with(console_layer)
        .init();
        
    // 保持守护线程运行
    std::mem::forget(client_log_guard);
}

#[derive(Parser, Debug)]
#[command(name = "ops-client")]
#[command(about = "OPS系统客户端")]
#[command(version = "0.1.0")]
struct Args {
    /// 服务端主机地址
    #[arg(long, short = 'H', help = "服务端主机地址 (默认: 127.0.0.1)")]
    host: Option<String>,

    /// 服务端端口
    #[arg(long, short = 'p', help = "服务端TCP端口 (默认: 12345)")]
    port: Option<u16>,

    /// 配置文件路径
    #[arg(long, short = 'c', help = "配置文件路径 (TOML格式)")]
    config: Option<String>,

    /// 心跳间隔（秒）
    #[arg(long, help = "心跳间隔秒数 (默认: 3)")]
    heartbeat_interval: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 解析命令行参数
    let args = Args::parse();

    // 初始化日志配置
    setup_logging();

    // 加载配置，优先级：命令行参数 > 配置文件 > 环境变量 > 默认值
    let mut config = if let Some(config_path) = &args.config {
        match ClientConfig::from_file(config_path) {
            Ok(config) => {
                info!("Loaded config from file: {}", config_path);
                config
            }
            Err(e) => {
                error!("Failed to load config file {}: {}", config_path, e);
                info!("Falling back to environment variables and defaults");
                ClientConfig::from_env()
            }
        }
    } else {
        ClientConfig::from_env()
    };

    // 命令行参数覆盖配置
    if let Some(host) = args.host {
        config.server_host = host;
    }
    if let Some(port) = args.port {
        config.server_port = port;
    }
    if let Some(interval) = args.heartbeat_interval {
        config.heartbeat_interval_secs = interval;
    }

    info!("Client starting with config: server={}", config.server_address());

    // 创建会话
    let session = match client::TcpSession::new(config).await {
        Ok(session) => session,
        Err(e) => {
            error!("Failed to create TCP session: {}", e);
            process::exit(1);
        }
    };

    // 启动心跳任务
    let heartbeat_session = session.clone();
    spawn(async move {
        heartbeat_session.start_heartbeat().await;
    });

    // 启动命令监听任务
    spawn(async move {
        session.start_message_listener().await;
    });

    // 保持程序运行
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}