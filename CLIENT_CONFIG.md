# OPS客户端配置说明

OPS客户端支持多种方式配置服务端地址和端口，优先级从高到低：

## 1. 命令行参数（最高优先级）

```bash
# 指定服务端地址和端口
cargo run --bin ops-client -- --host 192.168.1.100 --port 12345

# 或使用短选项
cargo run --bin ops-client -- -H 192.168.1.100 -p 12345

# 设置心跳间隔
cargo run --bin ops-client -- --host 192.168.1.100 --heartbeat-interval 5

# 查看帮助
cargo run --bin ops-client -- --help
```

## 2. 配置文件

创建TOML格式配置文件：

```toml
# client-config.toml
server_host = "192.168.1.100"
server_port = 12345
heartbeat_interval_secs = 5
retry_max_attempts = 10
retry_base_delay_secs = 2
retry_max_delay_secs = 60
client_id_file = "/tmp/client_id.txt"
apps_base_dir = "/opt/apps"
command_log_file = "/var/log/client_commands.log"
auth_token = "your-secret-token"  # 可选
```

然后使用配置文件启动：

```bash
cargo run --bin ops-client -- --config client-config.toml
```

## 3. 环境变量

```bash
export OPS_SERVER_HOST="192.168.1.100"
export OPS_SERVER_PORT="12345"
export OPS_HEARTBEAT_INTERVAL="5"
export OPS_AUTH_TOKEN="your-secret-token"

cargo run --bin ops-client
```

## 4. 默认值（最低优先级）

如果没有任何配置，客户端使用以下默认值：
- 服务端地址：`127.0.0.1:12345`
- 心跳间隔：3秒
- 重试次数：10次
- 客户端ID文件：`/tmp/client_id.txt`

## 支持的环境变量

| 环境变量 | 描述 | 默认值 |
|---------|------|--------|
| `OPS_SERVER_HOST` | 服务端主机地址 | `127.0.0.1` |
| `OPS_SERVER_PORT` | 服务端TCP端口 | `12345` |
| `OPS_HEARTBEAT_INTERVAL` | 心跳间隔（秒） | `3` |
| `OPS_RETRY_MAX_ATTEMPTS` | 最大重试次数 | `10` |
| `OPS_RETRY_BASE_DELAY` | 基础重试延迟（秒） | `2` |
| `OPS_RETRY_MAX_DELAY` | 最大重试延迟（秒） | `60` |
| `OPS_CLIENT_ID_FILE` | 客户端ID文件路径 | `/tmp/client_id.txt` |
| `OPS_APPS_BASE_DIR` | 应用程序目录 | `/tmp/apps` |
| `OPS_COMMAND_LOG_FILE` | 命令日志文件 | `/tmp/client_commands.log` |
| `OPS_AUTH_TOKEN` | 认证令牌 | 无 |

## 混合配置示例

命令行参数会覆盖配置文件和环境变量：

```bash
# 使用配置文件，但覆盖主机地址
cargo run --bin ops-client -- --config client-config.toml --host 10.0.0.50

# 使用环境变量，但覆盖端口
export OPS_SERVER_HOST="192.168.1.100"
cargo run --bin ops-client -- --port 9999
```

## 验证配置

客户端启动时会显示实际使用的配置：

```
[INFO] Client starting with config: server=192.168.1.100:12345
```