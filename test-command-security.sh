#!/bin/bash

echo "=== 测试命令安全验证功能 ==="

# 清理旧进程
pkill -f "ops-server|ops-client" || true
sleep 1

# 启动服务端
echo "启动服务端..."
RUST_LOG=info cargo run --bin ops-server &
SERVER_PID=$!
sleep 3

# 启动客户端
echo "启动客户端..."
RUST_LOG=info cargo run --bin ops-client &
CLIENT_PID=$!
sleep 3

echo "=== 测试预定义命令API ==="
echo "获取预定义命令列表:"
curl -s http://localhost:3000/api/predefined-commands | python3 -m json.tool | head -20

echo ""
echo "=== 检查客户端连接状态 ==="
CLIENT_RESPONSE=$(curl -s http://localhost:3000/api/clients)
CLIENT_ID=$(echo "$CLIENT_RESPONSE" | python3 -c "
import json, sys
try:
    data = json.load(sys.stdin)
    for client_id in data['clients']:
        print(client_id)
        break
except:
    pass
" 2>/dev/null)

if [ -n "$CLIENT_ID" ]; then
    echo "✅ 客户端已连接: $CLIENT_ID"
    
    echo ""
    echo "=== 测试安全命令执行 ==="
    echo "测试允许的命令 (whoami):"
    SAFE_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"whoami\"}" \
        http://localhost:3000/api/send-command)
    echo "响应: $SAFE_RESPONSE"
    
    echo ""
    echo "=== 测试危险命令拦截 ==="
    echo "测试危险命令 (shutdown -h now):"
    DANGEROUS_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"shutdown -h now\"}" \
        http://localhost:3000/api/send-command)
    echo "响应: $DANGEROUS_RESPONSE"
    
    echo ""
    echo "测试未授权命令 (malicious_command):"
    MALICIOUS_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"malicious_command\"}" \
        http://localhost:3000/api/send-command)
    echo "响应: $MALICIOUS_RESPONSE"
    
    echo ""
    echo "测试命令注入 (ls; rm -rf /):"
    INJECTION_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"ls; rm -rf /\"}" \
        http://localhost:3000/api/send-command)
    echo "响应: $INJECTION_RESPONSE"
    
else
    echo "❌ 客户端未连接"
fi

echo ""
echo "=== 清理 ==="
kill $SERVER_PID $CLIENT_PID 2>/dev/null
wait $SERVER_PID $CLIENT_PID 2>/dev/null

echo "安全测试完成"