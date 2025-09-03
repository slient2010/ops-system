# Ops System 端到端测试指南

## 功能概述

现在系统已经实现了两个核心功能：

1. **Web 命令执行**: 用户可以通过 Web 界面选择客户端并执行命令，实时获取执行结果
2. **客户端心跳**: 客户端定期发送系统信息给服务端，保持连接状态

## 测试步骤

### 1. 启动服务端

```bash
# 方式1: 使用 Docker Compose (推荐)
docker-compose up -d ops-server

# 方式2: 本地运行
cargo run --bin ops-server
```

服务端启动后会监听：
- HTTP: `http://localhost:3000`
- TCP: `tcp://localhost:12345`

### 2. 启动客户端

```bash
# 方式1: 使用 Docker Compose
docker-compose up -d ops-client-1 ops-client-2

# 方式2: 本地运行
cargo run --bin ops-client
```

### 3. 验证功能

#### 3.1 检查 Web 界面
1. 打开浏览器访问: `http://localhost:3000`
2. 应该能看到：
   - 系统状态显示在线客户端数量
   - 客户端列表显示连接的客户端信息
   - 命令执行区域

#### 3.2 测试命令执行
1. 在"选择客户端"下拉框中选择一个客户端
2. 在命令输入框中输入测试命令，例如：
   - `ps aux` - 查看进程
   - `ls -la` - 列出文件
   - `whoami` - 查看当前用户
   - `uptime` - 查看系统运行时间
3. 点击"执行命令"按钮
4. 观察页面变化：
   - 按钮变为"执行中..."状态
   - 显示"正在发送命令到客户端..."
   - 开始轮询等待结果
   - 最终显示命令执行结果

#### 3.3 验证安全机制
尝试执行被阻止的命令：
- `rm -rf /tmp/test` - 应该被阻止
- `shutdown now` - 应该被阻止
- `curl http://example.com` - 应该被阻止

#### 3.4 检查客户端心跳
1. 观察Web界面的客户端列表，应该每3秒更新一次
2. 关闭一个客户端，5分钟后应该从列表中消失
3. 重新启动客户端，应该重新出现在列表中

## 预期结果

### 成功指标：
1. ✅ Web界面正确显示在线客户端
2. ✅ 能够选择客户端并发送命令
3. ✅ 命令执行结果正确返回到Web界面
4. ✅ 危险命令被安全机制阻止
5. ✅ 客户端信息定期更新
6. ✅ 断线重连机制正常工作

### 日志验证：

**服务端日志应该包含：**
```
INFO ops_server: Server starting with config: TCP=0.0.0.0:12345, HTTP=0.0.0.0:3000
INFO ops_server: HTTP server starting on 0.0.0.0:3000
INFO ops_server: TCP server listening on 0.0.0.0:12345
INFO handle_socket: Received client info from: xxx (ID: client-uuid)
INFO handle_socket: Received command response from client: command_id=xxx, exit_code=0
```

**客户端日志应该包含：**
```
INFO ops_client: Client starting with config: server=127.0.0.1:12345
INFO client: Starting heartbeat with client ID: client-uuid
INFO client: Successfully connected to 127.0.0.1:12345
INFO client: Heartbeat #1 sent successfully
INFO client: Received command from server: ps aux (ID: command-uuid)
INFO client: Command validation passed: ps aux
INFO client: Command executed successfully
INFO client: Command result sent for command ID: command-uuid
```

## 故障排除

### 常见问题：

1. **客户端无法连接服务端**
   - 检查服务端是否启动：`netstat -tlnp | grep 12345`
   - 检查防火墙设置
   - 验证客户端配置的服务端地址

2. **Web界面没有客户端显示**
   - 检查客户端日志是否有连接成功消息
   - 检查服务端日志是否收到客户端信息
   - 等待客户端心跳周期（默认3秒）

3. **命令执行没有结果**
   - 检查客户端日志确认命令是否被接收
   - 验证命令是否在白名单中
   - 检查客户端到服务端的网络连接

4. **命令被阻止**
   - 查看客户端日志中的阻止原因
   - 确认命令在允许列表中：`ps, ls, pwd, whoami, date, uptime, df, free, top, htop, systemctl, journalctl`

## 性能测试

### 压力测试建议：
1. 启动多个客户端实例（10-50个）
2. 并发执行多个命令
3. 监控服务端内存和CPU使用情况
4. 检查命令响应时间

### 预期性能指标：
- 命令执行延迟 < 2秒
- 心跳数据传输延迟 < 100ms
- 服务端内存使用 < 100MB（100个客户端）
- CPU使用率 < 10%（正常负载）

## API 测试

可以使用 curl 直接测试 API：

```bash
# 获取客户端列表
curl http://localhost:3000/api/clients

# 发送命令（如果启用了认证，需要添加 -H "Authorization: Bearer token"）
curl -X POST -H "Content-Type: application/json" \
     -d '{"client_id":"client-uuid","command":"whoami"}' \
     http://localhost:3000/api/send-command

# 获取命令结果
curl "http://localhost:3000/api/command-result?command_id=command-uuid"
```

## 下一步改进

基于测试结果，可以考虑的改进点：
1. 添加实时WebSocket连接减少轮询
2. 实现命令执行历史持久化
3. 添加用户权限管理
4. 支持文件传输功能
5. 添加系统监控图表
6. 实现客户端分组管理