# Ops System

一个基于 Rust 的分布式运维监控系统，采用客户端-服务端架构，支持实时系统信息收集、远程命令执行和 Web 管理界面。

## 主要特性

- ✅ **安全的命令执行**: 命令白名单验证，防止恶意命令执行
- ✅ **配置化部署**: 支持环境变量和配置文件
- ✅ **结构化日志**: 使用 tracing 框架，便于调试和监控
- ✅ **认证授权**: 支持 Bearer Token API 认证
- ✅ **连接管理**: 自动重连、连接池和超时处理
- ✅ **健康检查**: 内置健康检查端点
- ✅ **Docker 支持**: 提供完整的容器化部署方案
- ✅ **单元测试**: 全面的测试覆盖

## 快速开始

### 方式一：使用 Docker Compose（推荐）

```bash
# 克隆项目
git clone https://github.com/slient2010/ops-system.git
cd ops-system

# 启动服务（包含1个服务端和2个客户端）
docker-compose up -d

# 查看日志
docker-compose logs -f ops-server

# 访问Web界面
open http://localhost:3000
```

### 方式二：本地开发

1. **安装 Rust**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

2. **克隆代码**
```bash
git clone https://github.com/slient2010/ops-system.git
cd ops-system
```

3. **运行测试**
```bash
cargo test
```

4. **启动服务端**
```bash
cargo run --bin ops-server
```

5. **启动客户端**（另一个终端）
```bash
cargo run --bin ops-client
```

6. **访问 Web 界面**
```bash
open http://localhost:3000
```

## 配置说明

### 环境变量配置

**服务端环境变量：**
```bash
export OPS_TCP_BIND_ADDR=0.0.0.0      # TCP服务绑定地址
export OPS_HTTP_BIND_ADDR=0.0.0.0     # HTTP服务绑定地址
export OPS_TCP_PORT=12345              # TCP服务端口
export OPS_HTTP_PORT=3000              # HTTP服务端口
export OPS_CLEANUP_INTERVAL=60         # 清理过期客户端间隔(秒)
export OPS_CLIENT_TIMEOUT=300          # 客户端超时时间(秒)
export OPS_MAX_CONNECTIONS=1000        # 最大连接数
export OPS_AUTH_TOKEN=your-token-here  # API认证令牌(可选)
```

**客户端环境变量：**
```bash
export OPS_SERVER_HOST=127.0.0.1       # 服务端地址
export OPS_SERVER_PORT=12345            # 服务端端口
export OPS_HEARTBEAT_INTERVAL=3         # 心跳间隔(秒)
export OPS_RETRY_MAX_ATTEMPTS=10        # 最大重试次数
export OPS_CLIENT_ID_FILE=/tmp/client_id.txt    # 客户端ID存储文件
export OPS_APPS_BASE_DIR=/tmp/apps      # 应用版本扫描目录
export OPS_COMMAND_LOG_FILE=/tmp/client_commands.log  # 命令日志文件
export OPS_AUTH_TOKEN=your-token-here   # 认证令牌(如果服务端启用)
```

### 配置文件

复制 `config.example.toml` 为 `config.toml` 并修改相应配置。

## API 接口

### 公开端点
- `GET /` - Web 管理界面
- `GET /health` - 健康检查

### 认证端点（需要 Bearer Token）
- `GET /api/clients` - 获取所有客户端信息
- `POST /api/send-message` - 广播消息到所有客户端
- `POST /api/send-command` - 发送命令到指定客户端

### API 使用示例

```bash
# 健康检查
curl http://localhost:3000/health

# 获取客户端列表（需要认证）
curl -H "Authorization: Bearer your-token" http://localhost:3000/api/clients

# 发送命令（需要认证）
curl -X POST -H "Content-Type: application/json" \
     -H "Authorization: Bearer your-token" \
     -d '{"client_id":"client-uuid","command":"ps aux"}' \
     http://localhost:3000/api/send-command

# 广播消息（需要认证）
curl -X POST -H "Content-Type: application/json" \
     -H "Authorization: Bearer your-token" \
     -d '{"message":"Hello all clients"}' \
     http://localhost:3000/api/send-message
```

## 安全特性

### 命令执行安全
- **命令白名单**: 只允许预定义的安全命令
- **危险模式检测**: 自动阻止包含危险模式的命令
- **命令净化**: 移除潜在的注入字符
- **长度限制**: 限制命令长度防止滥用

### 默认允许的命令
```
ps, ls, pwd, whoami, date, uptime, df, free, top, htop, systemctl, journalctl
```

### 自动阻止的危险模式
```
rm -rf, shutdown, reboot, mkfs, fdisk, dd, curl, wget, nc, bash -i, sudo su, chmod 777
```

## 开发指南

### 项目结构
```
ops-system/
├── ops-server/          # 服务端
│   ├── src/
│   │   ├── main.rs      # 主程序
│   │   ├── web/         # Web API
│   │   ├── tcp_services/ # TCP服务
│   │   ├── middleware.rs # 认证中间件
│   │   └── tests.rs     # 单元测试
│   └── static/          # 静态文件
├── ops-client/          # 客户端
│   ├── src/
│   │   ├── main.rs      # 主程序
│   │   ├── tcp_services/ # TCP客户端
│   │   ├── collection/  # 信息收集
│   │   └── tests.rs     # 单元测试
├── ops-common/          # 共享库
│   ├── src/
│   │   ├── lib.rs       # 数据结构
│   │   ├── config.rs    # 配置管理
│   │   └── security.rs  # 安全验证
└── docker-compose.yml   # 容器编排
```

### 运行测试
```bash
# 运行所有测试
cargo test

# 运行服务端测试
cargo test -p ops-server

# 运行客户端测试
cargo test -p ops-client

# 运行共享库测试
cargo test -p ops-common
```

### 代码检查
```bash
# 代码格式化
cargo fmt

# 静态分析
cargo clippy

# 安全审计
cargo audit
```

## 部署建议

### 生产环境
1. **启用认证**: 设置强密码的 `OPS_AUTH_TOKEN`
2. **使用 TLS**: 建议在负载均衡器层添加 HTTPS
3. **限制网络**: 使用防火墙限制端口访问
4. **监控日志**: 配置日志收集和分析
5. **健康检查**: 配置监控系统检查 `/health` 端点

### 监控指标
- 连接的客户端数量
- 命令执行成功/失败率
- 网络连接状态
- 系统资源使用情况

## 故障排除

### 常见问题

**1. 客户端无法连接服务端**
- 检查网络连通性：`telnet server_ip 12345`
- 检查服务端日志：`docker-compose logs ops-server`
- 验证配置：检查服务端地址和端口配置

**2. 命令被阻止执行**
- 查看客户端日志确认被阻止的原因
- 检查命令是否在白名单中
- 避免使用危险模式的命令

**3. API 认证失败**
- 确认 `Authorization: Bearer token` 格式正确
- 检查令牌是否与服务端配置一致
- 确认令牌没有过期

**4. Docker 容器启动失败**
- 检查端口是否被占用：`netstat -tlnp | grep :3000`
- 查看构建日志：`docker-compose build --no-cache`
- 检查资源限制：确保有足够的内存和磁盘空间

## 贡献指南

1. Fork 项目
2. 创建功能分支：`git checkout -b feature/new-feature`
3. 提交更改：`git commit -am 'Add new feature'`
4. 推送分支：`git push origin feature/new-feature`
5. 创建 Pull Request

## 许可证

[MIT License](LICENSE)