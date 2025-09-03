# OPS系统完整使用手册

## 目录

1. [系统概述](#系统概述)
2. [快速开始](#快速开始)
3. [客户端配置](#客户端配置)
4. [服务端配置](#服务端配置)
5. [Web界面使用](#web界面使用)
6. [命令安全机制](#命令安全机制)
7. [脚本执行功能](#脚本执行功能)
8. [API接口文档](#api接口文档)
9. [故障排除](#故障排除)
10. [最佳实践](#最佳实践)

---

## 系统概述

OPS系统是一个分布式运维管理平台，支持远程命令执行、系统监控和脚本自动化。系统采用Rust开发，具有高性能、高安全性的特点。

### 核心功能

- 🔐 **安全命令执行** - 基于白名单的命令验证机制
- 📜 **脚本自动化** - 支持在指定目录执行安全脚本
- 🌐 **Web管理界面** - 直观的用户操作界面
- 📊 **实时监控** - 客户端状态和系统信息监控
- 🔄 **心跳机制** - 自动维护客户端连接状态
- 🛡️ **安全防护** - 多层安全验证和攻击防护

### 系统架构

```
┌─────────────────┐    HTTP/TCP    ┌─────────────────┐
│                 │ ──────────────▶ │                 │
│   Web Browser   │                │   OPS Server    │
│                 │ ◀────────────── │                 │
└─────────────────┘                └─────────────────┘
                                           │
                                           │ TCP
                                           ▼
                                   ┌─────────────────┐
                                   │                 │
                                   │   OPS Client    │
                                   │                 │
                                   └─────────────────┘
```

---

## 快速开始

### 环境要求

- Rust 1.70+
- 操作系统：Linux/macOS/Windows
- 网络：TCP端口12345（客户端连接），HTTP端口3000（Web界面）

### 编译安装

```bash
# 克隆项目
git clone <repository-url>
cd ops-system

# 编译项目
cargo build --release

# 或使用开发模式
cargo build
```

### 快速启动

**1. 启动服务端**
```bash
cargo run --bin ops-server
```

**2. 启动客户端**
```bash
cargo run --bin ops-client
```

**3. 访问Web界面**
```
http://localhost:3000
```

---

## 客户端配置

客户端支持多种配置方式，优先级：命令行参数 > 配置文件 > 环境变量 > 默认值

### 命令行参数配置

```bash
# 指定服务端地址和端口
cargo run --bin ops-client -- --host 192.168.1.100 --port 12345

# 设置心跳间隔
cargo run --bin ops-client -- --heartbeat-interval 5

# 使用配置文件
cargo run --bin ops-client -- --config client-config.toml

# 查看所有选项
cargo run --bin ops-client -- --help
```

### 配置文件方式

创建 `client-config.toml`：

```toml
server_host = "192.168.1.100"
server_port = 12345
heartbeat_interval_secs = 5
retry_max_attempts = 10
retry_base_delay_secs = 2
retry_max_delay_secs = 60
client_id_file = "/tmp/client_id.txt"
apps_base_dir = "/tmp/apps"
command_log_file = "/tmp/client_commands.log"
auth_token = "your-secret-token"  # 可选
```

### 环境变量配置

```bash
export OPS_SERVER_HOST="192.168.1.100"
export OPS_SERVER_PORT="12345"
export OPS_HEARTBEAT_INTERVAL="5"
export OPS_AUTH_TOKEN="your-secret-token"

cargo run --bin ops-client
```

### 支持的配置选项

| 配置项 | 命令行参数 | 环境变量 | 默认值 | 说明 |
|-------|------------|----------|--------|------|
| 服务端地址 | `--host` | `OPS_SERVER_HOST` | `127.0.0.1` | 服务端IP地址 |
| 服务端端口 | `--port` | `OPS_SERVER_PORT` | `12345` | TCP连接端口 |
| 心跳间隔 | `--heartbeat-interval` | `OPS_HEARTBEAT_INTERVAL` | `3` | 心跳间隔秒数 |
| 配置文件 | `--config` | - | - | TOML配置文件路径 |

---

## 服务端配置

### 基础配置

创建 `server-config.toml`：

```toml
# 网络配置
tcp_bind_addr = "0.0.0.0"
tcp_port = 12345
http_bind_addr = "0.0.0.0"
http_port = 3000

# 系统配置
cleanup_interval_secs = 60
client_timeout_secs = 300
max_connections = 1000

# 安全配置
auth_token = "your-secret-token"  # 可选

# 脚本安全配置
allowed_script_dirs = [
    "/opt/ops-scripts",
    "/usr/local/bin/scripts",
    "/home/ops/scripts"
]
allowed_script_extensions = ["sh", "py", "pl", "rb"]
```

### 环境变量配置

```bash
export OPS_TCP_PORT="12345"
export OPS_HTTP_PORT="3000"
export OPS_AUTH_TOKEN="your-secret-token"
export OPS_ALLOWED_SCRIPT_DIRS="/opt/ops-scripts,/usr/local/scripts"
export OPS_ALLOWED_SCRIPT_EXTENSIONS="sh,py,pl,rb"
```

---

## Web界面使用

### 访问方式

默认地址：`http://localhost:3000`

### 主要功能

#### 1. 系统状态监控
- 在线客户端数量
- 最后更新时间
- 服务器运行状态

#### 2. 命令执行

**预定义命令模式：**
1. 选择"预定义命令"
2. 从类别中选择命令类型
3. 选择具体的命令
4. 查看命令描述
5. 点击"执行命令"

**自定义命令模式：**
1. 选择"自定义命令"
2. 输入要执行的命令
3. 系统自动进行安全验证
4. 点击"执行命令"

#### 3. 脚本执行
1. 选择"预定义命令"
2. 筛选"脚本执行"类别
3. 选择要执行的脚本
4. 查看脚本说明
5. 执行脚本并查看结果

#### 4. 客户端管理
- 查看所有连接的客户端
- 显示客户端系统信息
- 监控客户端状态

#### 5. 命令历史
- 查看最近执行的命令
- 显示执行结果和状态
- 支持历史记录搜索

---

## 命令安全机制

### 安全验证层级

1. **命令白名单验证**
2. **危险模式检测**
3. **命令注入防护**
4. **脚本路径安全验证**

### 允许的安全命令

#### 系统信息类
- `ps`, `whoami`, `id`, `hostname`, `uname`, `date`, `uptime`

#### 资源监控类
- `free`, `df`, `top`, `htop`, `iostat`, `vmstat`, `sar`, `mpstat`

#### 网络信息类
- `netstat`, `ss`, `ip`, `ifconfig`, `ping`

#### 文件查看类
- `ls`, `cat`, `head`, `tail`, `less`, `more`, `grep`, `find`

#### 服务管理类（只读）
- `systemctl status`, `journalctl`, `service`

#### 环境信息类
- `env`, `history`, `which`, `whereis`

### 危险命令防护

系统自动阻止以下危险操作：

#### 系统控制类
- `shutdown`, `reboot`, `halt`, `poweroff`
- `init 0`, `init 6`, `systemctl poweroff/reboot`

#### 文件操作类
- `rm -rf`, `rmdir`, `mkfs`, `fdisk`
- `dd if=`, `dd of=`, `format`

#### 权限提升类
- `sudo su`, `su -`, `passwd`, `usermod`
- `chmod 777`, `chown root`

#### 网络下载类
- `curl`, `wget`, `ftp`, `sftp`, `rsync`

#### 脚本执行类
- `bash -i`, `sh -i`, `eval`, `exec`
- `python -c`, `perl -e`, `ruby -e`

#### 进程控制类
- `kill -9`, `killall`, `pkill`
- `systemctl start/stop/restart`

#### 命令注入防护
- `;`, `&&`, `||`, `` ` ``, `$(`, `&`

---

## 脚本执行功能

### 安全特性

#### 目录白名单控制
系统只允许执行位于预定义目录中的脚本：
- `/opt/ops-scripts` - 生产运维脚本
- `/usr/local/bin/scripts` - 本地脚本
- `/home/ops/scripts` - 用户脚本

#### 扩展名验证
允许的脚本类型：
- `.sh` - Shell脚本
- `.py` - Python脚本
- `.pl` - Perl脚本
- `.rb` - Ruby脚本

#### 路径安全验证
- 只允许绝对路径
- 防止路径遍历攻击（`../`, `./`）
- 拒绝相对路径

### 脚本示例

**健康检查脚本** (`/opt/ops-scripts/health-check.sh`)：

```bash
#!/bin/bash

echo "=== 系统健康检查报告 ==="
echo "检查时间: $(date)"
echo ""

echo "== 系统负载 =="
uptime

echo ""
echo "== 内存使用 =="
free -h

echo ""
echo "== 磁盘空间 =="
df -h | head -5

echo ""
echo "=== 检查完成 ==="
```

**磁盘分析脚本** (`/opt/ops-scripts/disk-usage.py`)：

```python
#!/usr/bin/env python3

import os
import shutil

def analyze_disk():
    total, used, free = shutil.disk_usage("/")
    print(f"磁盘使用率: {(used/total*100):.1f}%")
    print(f"可用空间: {free//(1024**3):.1f} GB")

if __name__ == "__main__":
    analyze_disk()
```

### 配置脚本目录

```bash
# 环境变量方式
export OPS_ALLOWED_SCRIPT_DIRS="/opt/scripts,/home/admin/scripts"

# 配置文件方式
allowed_script_dirs = ["/opt/scripts", "/home/admin/scripts"]
```

---

## API接口文档

### 基础端点

| 端点 | 方法 | 说明 |
|------|------|------|
| `/` | GET | Web界面 |
| `/health` | GET | 健康检查 |
| `/api/clients` | GET | 获取客户端列表 |
| `/api/predefined-commands` | GET | 获取预定义命令 |

### 命令执行

**发送命令：**
```http
POST /api/send-command
Content-Type: application/json

{
  "client_id": "client-uuid",
  "command": "ps aux"
}
```

**响应：**
```json
{
  "command_id": "command-uuid",
  "message": "命令已发送到客户端"
}
```

**获取执行结果：**
```http
GET /api/command-result?command_id=command-uuid
```

**响应：**
```json
{
  "status": "completed",
  "command": "ps aux",
  "output": "命令输出内容",
  "error_output": "",
  "exit_code": 0,
  "executed_at": "2025-08-29T12:00:00Z"
}
```

### 脚本执行

**执行脚本：**
```http
POST /api/send-command
Content-Type: application/json

{
  "client_id": "client-uuid", 
  "command": "/opt/ops-scripts/health-check.sh"
}
```

### 客户端历史

**获取客户端命令历史：**
```http
GET /api/client-history?client_id=client-uuid&limit=20
```

---

## 故障排除

### 常见问题

#### Q: 客户端无法连接到服务端
A: 检查以下几点：
1. 服务端是否正常启动
2. 网络连接是否正常
3. 端口是否被防火墙阻止
4. 配置的服务端地址和端口是否正确

#### Q: 命令执行被拒绝
A: 可能原因：
1. 命令不在白名单中
2. 命令包含危险模式
3. 脚本不在允许的目录中
4. 脚本扩展名不被支持

#### Q: Web界面无法访问
A: 检查：
1. 服务端HTTP服务是否启动
2. 端口3000是否被占用
3. 防火墙是否允许HTTP访问

#### Q: 脚本执行失败
A: 检查：
1. 脚本是否在允许的目录中
2. 脚本是否有执行权限
3. 脚本路径是否为绝对路径
4. 脚本扩展名是否被允许

### 日志调试

**启用详细日志：**
```bash
# 服务端调试日志
RUST_LOG=debug cargo run --bin ops-server

# 客户端调试日志  
RUST_LOG=debug cargo run --bin ops-client
```

**日志级别说明：**
- `error` - 只显示错误信息
- `warn` - 显示警告和错误
- `info` - 显示一般信息（默认）
- `debug` - 显示详细调试信息
- `trace` - 显示所有信息

---

## 最佳实践

### 安全建议

1. **使用认证令牌**
   ```bash
   export OPS_AUTH_TOKEN="your-secure-random-token"
   ```

2. **限制网络访问**
   - 使用防火墙限制访问IP
   - 在内网环境中部署

3. **定期更新脚本**
   - 定期检查和更新脚本内容
   - 移除不再需要的脚本

4. **监控日志**
   - 定期检查系统日志
   - 关注异常的命令执行

### 部署建议

1. **生产环境配置**
   ```toml
   # 服务端配置
   tcp_bind_addr = "0.0.0.0"
   http_bind_addr = "127.0.0.1"  # 只允许本地访问
   auth_token = "production-secret-token"
   max_connections = 100
   ```

2. **脚本管理**
   ```bash
   # 创建专用脚本目录
   sudo mkdir -p /opt/ops-scripts
   sudo chown ops:ops /opt/ops-scripts
   sudo chmod 755 /opt/ops-scripts
   ```

3. **自动启动**
   ```bash
   # 使用systemd管理服务
   sudo systemctl enable ops-server
   sudo systemctl enable ops-client
   ```

4. **监控和告警**
   - 监控服务状态
   - 设置异常告警
   - 定期备份配置

### 性能优化

1. **调整连接数限制**
   ```toml
   max_connections = 500
   ```

2. **优化心跳间隔**
   ```toml
   heartbeat_interval_secs = 10
   ```

3. **清理历史记录**
   ```toml
   cleanup_interval_secs = 300
   ```

---

## 相关文档

- [脚本安全执行详细文档](SCRIPT_SECURITY.md)
- [客户端配置文档](CLIENT_CONFIG.md)
- [架构设计文档](代码架构文档.md)
- [优化建议文档](优化建议.md)

---

## 技术支持

如有问题或建议，请通过以下方式联系：

- GitHub Issues
- 技术支持邮箱
- 内部技术群组

---

*最后更新：2025-08-29*