use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub tcp_bind_addr: String,
    pub http_bind_addr: String,
    pub tcp_port: u16,
    pub http_port: u16,
    pub cleanup_interval_secs: u64,
    pub client_timeout_secs: u64,
    pub max_connections: usize,
    pub auth_token: Option<String>,
    pub allowed_script_dirs: Vec<String>, // 允许执行脚本的目录
    pub allowed_script_extensions: Vec<String>, // 允许的脚本扩展名
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            tcp_bind_addr: "0.0.0.0".to_string(),
            http_bind_addr: "0.0.0.0".to_string(),
            tcp_port: 12345,
            http_port: 3000,
            cleanup_interval_secs: 10,
            client_timeout_secs: 30,
            max_connections: 1000,
            auth_token: None,
            allowed_script_dirs: vec![
                "/opt/ops-scripts".to_string(),
                "/usr/local/bin/scripts".to_string(),
                "/home/ops/scripts".to_string(),
            ],
            allowed_script_extensions: vec![
                "sh".to_string(),
                "py".to_string(),
                "pl".to_string(),
                "rb".to_string(),
            ],
        }
    }
}

impl ServerConfig {
    pub fn from_env() -> Self {
        Self {
            tcp_bind_addr: env::var("OPS_TCP_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string()),
            http_bind_addr: env::var("OPS_HTTP_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0".to_string()),
            tcp_port: env::var("OPS_TCP_PORT")
                .unwrap_or_else(|_| "12345".to_string())
                .parse()
                .unwrap_or(12345),
            http_port: env::var("OPS_HTTP_PORT")
                .unwrap_or_else(|_| "3000".to_string())
                .parse()
                .unwrap_or(3000),
            cleanup_interval_secs: env::var("OPS_CLEANUP_INTERVAL")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
            client_timeout_secs: env::var("OPS_CLIENT_TIMEOUT")
                .unwrap_or_else(|_| "300".to_string())
                .parse()
                .unwrap_or(300),
            max_connections: env::var("OPS_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "1000".to_string())
                .parse()
                .unwrap_or(1000),
            auth_token: env::var("OPS_AUTH_TOKEN").ok(),
            allowed_script_dirs: env::var("OPS_ALLOWED_SCRIPT_DIRS")
                .unwrap_or_else(|_| "/opt/ops-scripts,/usr/local/bin/scripts,/home/ops/scripts".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            allowed_script_extensions: env::var("OPS_ALLOWED_SCRIPT_EXTENSIONS")
                .unwrap_or_else(|_| "sh,py,pl,rb".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn tcp_address(&self) -> String {
        format!("{}:{}", self.tcp_bind_addr, self.tcp_port)
    }

    pub fn http_address(&self) -> String {
        format!("{}:{}", self.http_bind_addr, self.http_port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub server_host: String,
    pub server_port: u16,
    pub heartbeat_interval_secs: u64,
    pub retry_max_attempts: u32,
    pub retry_base_delay_secs: u64,
    pub retry_max_delay_secs: u64,
    pub client_id_file: String,
    pub apps_base_dir: String,
    pub command_log_file: String,
    pub auth_token: Option<String>,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_host: "127.0.0.1".to_string(),
            server_port: 12345,
            heartbeat_interval_secs: 3,
            retry_max_attempts: 10,
            retry_base_delay_secs: 2,
            retry_max_delay_secs: 60,
            client_id_file: "/tmp/client_id.txt".to_string(),
            apps_base_dir: "/tmp/apps".to_string(),
            command_log_file: "/tmp/client_commands.log".to_string(),
            auth_token: None,
        }
    }
}

impl ClientConfig {
    pub fn from_env() -> Self {
        Self {
            server_host: env::var("OPS_SERVER_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            server_port: env::var("OPS_SERVER_PORT")
                .unwrap_or_else(|_| "12345".to_string())
                .parse()
                .unwrap_or(12345),
            heartbeat_interval_secs: env::var("OPS_HEARTBEAT_INTERVAL")
                .unwrap_or_else(|_| "3".to_string())
                .parse()
                .unwrap_or(3),
            retry_max_attempts: env::var("OPS_RETRY_MAX_ATTEMPTS")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .unwrap_or(10),
            retry_base_delay_secs: env::var("OPS_RETRY_BASE_DELAY")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .unwrap_or(2),
            retry_max_delay_secs: env::var("OPS_RETRY_MAX_DELAY")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
            client_id_file: env::var("OPS_CLIENT_ID_FILE")
                .unwrap_or_else(|_| "/tmp/client_id.txt".to_string()),
            apps_base_dir: env::var("OPS_APPS_BASE_DIR")
                .unwrap_or_else(|_| "/tmp/apps".to_string()),
            command_log_file: env::var("OPS_COMMAND_LOG_FILE")
                .unwrap_or_else(|_| "/tmp/client_commands.log".to_string()),
            auth_token: env::var("OPS_AUTH_TOKEN").ok(),
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server_host, self.server_port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_server_config_defaults() {
        let config = ServerConfig::default();
        assert_eq!(config.tcp_port, 12345);
        assert_eq!(config.http_port, 3000);
    }

    #[test]
    fn test_client_config_from_env() {
        env::set_var("OPS_SERVER_HOST", "test-server");
        env::set_var("OPS_SERVER_PORT", "9999");
        
        let config = ClientConfig::from_env();
        assert_eq!(config.server_host, "test-server");
        assert_eq!(config.server_port, 9999);
        
        // 清理环境变量
        env::remove_var("OPS_SERVER_HOST");
        env::remove_var("OPS_SERVER_PORT");
    }

    #[test]
    fn test_server_addresses() {
        let config = ServerConfig::default();
        assert_eq!(config.tcp_address(), "0.0.0.0:12345");
        assert_eq!(config.http_address(), "0.0.0.0:3000");
    }
}