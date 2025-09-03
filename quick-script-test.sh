#!/bin/bash

echo "=== 快速脚本安全测试 ==="

# 清理旧进程
pkill -f "ops-server|ops-client" || true
sleep 1

echo "启动服务端..."
RUST_LOG=debug cargo run --bin ops-server 2>&1 | tee server.log &
SERVER_PID=$!
sleep 2

echo "启动客户端..."  
RUST_LOG=debug cargo run --bin ops-client 2>&1 | tee client.log &
CLIENT_PID=$!
sleep 3

echo "检查连接状态..."
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
    echo "客户端已连接: $CLIENT_ID"
    
    echo ""
    echo "=== 测试1: 允许的脚本 ==="
    curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"/tmp/ops-scripts/health-check.sh\"}" \
        http://localhost:3000/api/send-command
    sleep 3
    
    echo ""
    echo "=== 测试2: 危险路径 ==="  
    curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"/tmp/dangerous.sh\"}" \
        http://localhost:3000/api/send-command
    sleep 2
    
    echo ""
    echo "=== 测试3: 路径遍历 ==="
    curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"/tmp/ops-scripts/../dangerous.sh\"}" \
        http://localhost:3000/api/send-command
    sleep 2
        
    echo ""
    echo "=== 客户端脚本验证日志 ==="
    echo "客户端最近日志:"
    tail -15 client.log | grep -E "(Command.*passed|Command.*blocked|validation|脚本)"
    
else
    echo "❌ 客户端未连接"
fi

echo ""
echo "=== 清理 ==="
kill $SERVER_PID $CLIENT_PID 2>/dev/null
wait $SERVER_PID $CLIENT_PID 2>/dev/null

echo "测试完成"