#!/bin/bash

# 清理可能存在的临时文件
rm -f /tmp/client_id.txt /tmp/client_commands.log

# 创建必要的目录
mkdir -p /tmp/apps

echo "=== 构建项目 ==="
cargo build --release || exit 1

echo ""
echo "=== 启动服务端 ==="
cargo run --release --bin ops-server &
SERVER_PID=$!
echo "服务端 PID: $SERVER_PID"

sleep 2

echo ""
echo "=== 检查服务端端口 ==="
if ! netstat -ln | grep -q ":12345\|:3000"; then
    echo "服务端端口未正常监听"
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

echo "服务端端口监听正常"

echo ""
echo "=== 启动客户端 ==="
timeout 10 cargo run --release --bin ops-client &
CLIENT_PID=$!
echo "客户端 PID: $CLIENT_PID"

sleep 3

echo ""
echo "=== 检查客户端API ==="
if curl -s --connect-timeout 2 http://localhost:3000/api/clients | grep -q "client_id"; then
    echo "✅ 客户端已成功连接到服务端"
else
    echo "❌ 客户端连接失败"
    echo "服务端API响应:"
    curl -s http://localhost:3000/api/clients || echo "无法连接到API"
fi

echo ""
echo "=== 清理进程 ==="
kill $SERVER_PID $CLIENT_PID 2>/dev/null
wait $SERVER_PID $CLIENT_PID 2>/dev/null

echo "测试完成"