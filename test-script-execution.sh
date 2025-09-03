#!/bin/bash

echo "=== 测试脚本执行安全机制 ==="

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
    echo "=== 测试允许的脚本执行 ==="
    echo "测试健康检查脚本:"
    SCRIPT_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"/tmp/ops-scripts/health-check.sh\"}" \
        http://localhost:3000/api/send-command)
    echo "响应: $SCRIPT_RESPONSE"
    
    # 等待脚本执行完成
    sleep 5
    
    echo ""
    echo "测试Python磁盘分析脚本:"
    PYTHON_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"/tmp/ops-scripts/disk-usage.py\"}" \
        http://localhost:3000/api/send-command)
    echo "响应: $PYTHON_RESPONSE"
    
    # 等待脚本执行完成
    sleep 5
    
    echo ""
    echo "=== 测试危险脚本阻止 ==="
    echo "测试不在白名单目录的脚本:"
    DANGER_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"/tmp/dangerous-script.sh\"}" \
        http://localhost:3000/api/send-command)
    echo "响应: $DANGER_RESPONSE"
    
    echo ""
    echo "测试路径遍历攻击:"
    TRAVERSAL_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"/tmp/ops-scripts/../../../etc/passwd\"}" \
        http://localhost:3000/api/send-command)
    echo "响应: $TRAVERSAL_RESPONSE"
    
    echo ""
    echo "测试不允许的扩展名:"
    BADEXT_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"/tmp/ops-scripts/script.exe\"}" \
        http://localhost:3000/api/send-command)
    echo "响应: $BADEXT_RESPONSE"
    
    echo ""
    echo "测试相对路径脚本:"
    RELATIVE_RESPONSE=$(curl -s -X POST -H "Content-Type: application/json" \
        -d "{\"client_id\":\"$CLIENT_ID\",\"command\":\"./scripts/test.sh\"}" \
        http://localhost:3000/api/send-command)
    echo "响应: $RELATIVE_RESPONSE"
    
else
    echo "❌ 客户端未连接"
fi

echo ""
echo "=== 清理 ==="
kill $SERVER_PID $CLIENT_PID 2>/dev/null
wait $SERVER_PID $CLIENT_PID 2>/dev/null

echo "脚本执行测试完成"