#!/bin/bash

echo "=== 测试客户端命令接收 ==="

# 清理旧进程
pkill -f "ops-server\|ops-client" || true
sleep 1

# 构建项目
cargo build || exit 1

echo ""
echo "=== 启动服务端 ==="
RUST_LOG=debug cargo run --bin ops-server 2>&1 | tee server.log &
SERVER_PID=$!
sleep 2

echo ""
echo "=== 启动客户端 ==="
RUST_LOG=debug cargo run --bin ops-client 2>&1 | tee client.log &
CLIENT_PID=$!
sleep 3

echo ""
echo "=== 检查客户端是否连接 ==="
CLIENT_RESPONSE=$(curl -s http://localhost:3000/api/clients)
echo "API响应: $CLIENT_RESPONSE"

if echo "$CLIENT_RESPONSE" | grep -q "client_id"; then
    echo "✅ 客户端已连接"
    
    # 提取客户端ID
    CLIENT_ID=$(echo "$CLIENT_RESPONSE" | python3 -c "
import json, sys
data = json.load(sys.stdin)
for client_id in data['clients']:
    print(client_id)
    break
" 2>/dev/null)
    
    if [ -n "$CLIENT_ID" ]; then
        echo "客户端ID: $CLIENT_ID"
        
        echo ""
        echo "=== 发送测试命令 ==="
        COMMAND_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
            -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"whoami\"}" \
            http://localhost:3000/api/send-command)
        
        echo "命令发送响应: $COMMAND_RESPONSE"
        
        # 等待命令执行
        sleep 5
        
        echo ""
        echo "=== 检查日志中的命令接收 ==="
        echo "客户端日志（最近20行）："
        tail -20 client.log | grep -E "(收到|命令|Command)"
        
        echo ""
        echo "服务端日志（最近20行）："
        tail -20 server.log | grep -E "(命令|Command|Raw data)"
    else
        echo "❌ 无法提取客户端ID"
    fi
else
    echo "❌ 客户端未连接"
fi

echo ""
echo "=== 完整日志检查 ==="
echo "客户端启动消息："
grep -E "(消息监听器|message.listener)" client.log || echo "未找到消息监听器启动信息"

echo ""
echo "服务端接收数据："
grep -E "(Raw data|Read.*bytes)" server.log || echo "未找到数据接收信息"

echo ""
echo "=== 清理 ==="
kill $SERVER_PID $CLIENT_PID 2>/dev/null
wait $SERVER_PID $CLIENT_PID 2>/dev/null

echo "测试完成"