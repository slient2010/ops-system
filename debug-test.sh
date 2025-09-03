#!/bin/bash

echo "=== 启动调试测试 ==="

# 清理旧的进程
pkill -f "ops-server" || true
pkill -f "ops-client" || true
sleep 1

# 构建项目
echo "构建项目..."
cargo build || exit 1

# 启动服务端（带调试日志）
echo "启动服务端（带调试输出）..."
RUST_LOG=debug cargo run --bin ops-server 2>&1 | tee server.log &
SERVER_PID=$!

sleep 2

# 检查服务端是否启动
if ! netstat -ln | grep -q ":12345.*LISTEN"; then
    echo "❌ 服务端TCP端口未监听"
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

if ! netstat -ln | grep -q ":3000.*LISTEN"; then
    echo "❌ 服务端HTTP端口未监听" 
    kill $SERVER_PID 2>/dev/null
    exit 1
fi

echo "✅ 服务端端口正常"

# 启动客户端（带调试日志）
echo "启动客户端（带调试输出）..."
RUST_LOG=debug timeout 10 cargo run --bin ops-client 2>&1 | tee client.log &
CLIENT_PID=$!

sleep 3

echo ""
echo "=== 检查日志输出 ==="
echo "服务端日志（最近10行）："
tail -10 server.log

echo ""
echo "客户端日志（最近10行）："
tail -10 client.log

echo ""
echo "=== 检查API响应 ==="
curl -s http://localhost:3000/api/clients | python3 -m json.tool 2>/dev/null || curl -s http://localhost:3000/api/clients

echo ""
echo "=== 清理 ==="
kill $SERVER_PID $CLIENT_PID 2>/dev/null
wait $SERVER_PID $CLIENT_PID 2>/dev/null

echo ""
echo "调试测试完成。检查上面的日志输出以诊断问题。"