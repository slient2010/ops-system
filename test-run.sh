#!/bin/bash

# 测试服务端和客户端运行脚本

echo "=== 构建项目 ==="
cargo build --workspace || {
    echo "构建失败"
    exit 1
}

echo ""
echo "=== 启动服务端（后台运行）==="
cargo run --bin ops-server &
SERVER_PID=$!
echo "服务端 PID: $SERVER_PID"

# 等待服务端启动
sleep 3

echo ""
echo "=== 启动客户端（后台运行）==="
cargo run --bin ops-client &
CLIENT_PID=$!
echo "客户端 PID: $CLIENT_PID"

# 等待连接建立
sleep 3

echo ""
echo "=== 测试Web接口 ==="
echo "检查客户端列表:"
curl -s http://localhost:3000/api/clients | jq . || echo "请求失败或没有安装jq"

echo ""
echo "=== 系统运行中，按 Ctrl+C 停止 ==="

# 等待用户中断
trap "echo '停止系统...'; kill $SERVER_PID $CLIENT_PID 2>/dev/null; exit" INT

# 保持脚本运行
while true; do
    sleep 1
done