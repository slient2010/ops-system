#!/bin/bash

echo "启动广播消息测试..."

# 启动服务器
echo "启动服务器..."
cargo run --bin ops-server &
SERVER_PID=$!
echo "服务器 PID: $SERVER_PID"

sleep 3

# 启动客户端
echo "启动客户端..."
cargo run --bin ops-client &
CLIENT_PID=$!
echo "客户端 PID: $CLIENT_PID"

sleep 5

# 发送广播消息
echo "发送广播消息..."
curl -X POST "http://localhost:3000/api/send-message" \
     -H "Content-Type: application/json" \
     -d '{"message": "测试广播消息: 系统维护将在30分钟后开始，请保存工作！"}' \
     --timeout 10

sleep 3

# 检查是否有通知文件被创建
echo "检查通知文件..."
if [ -f "$HOME/.ops_motd" ]; then
    echo "找到 MOTD 文件:"
    cat "$HOME/.ops_motd"
fi

# 检查系统日志
echo "检查系统日志 (最近3行):"
journalctl -t ops-client --no-pager -n 3 2>/dev/null || echo "无法访问journalctl"

# 清理进程
echo "清理进程..."
kill $CLIENT_PID 2>/dev/null
kill $SERVER_PID 2>/dev/null
wait

echo "测试完成!"