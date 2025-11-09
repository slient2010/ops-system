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
use ops_common::log_rotation::ThreadSafeLogRotator;

#[cfg(test)]
mod tests;

// 设置客户端日志配置
fn setup_logging(config: &ClientConfig) {
    // Use size-based rotation if configured
    let log_directory = config.log_directory.clone();
    let log_rotation_size_mb = config.log_rotation_size_mb;

    // Create a custom writer with size-based rotation
    let client_log_rotator = ThreadSafeLogRotator::new(
        format!("{}/ops-client.log", log_directory),
        log_rotation_size_mb,
        log_directory
    ).expect("Failed to create client log rotator");

    // Create custom writer implementation that uses our log rotator
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

    // Wrap the rotator in writer struct for tracing
    let client_log_writer = NonBlockingLogWriter::new(client_log_rotator);

    // 配置日志层 - 记录到文件
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::sync::Mutex::new(client_log_writer))
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

    // 初始化日志配置
    setup_logging(&config);

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