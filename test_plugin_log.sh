#!/bin/bash
# 测试 Wasm 插件日志功能

echo "=== 测试 Wasm 插件多级别日志功能 ==="
echo ""
echo "启动服务器并测试不同的消息..."
echo ""

# 设置日志级别为 debug，以便看到所有级别的日志
export RUST_LOG=debug,wasm_plugin=trace

# 启动服务器（后台运行）
cargo run -p flux-server &
SERVER_PID=$!

# 等待服务器启动
sleep 3

echo ""
echo "=== 测试 1: 空消息（应该触发 WARN）==="
curl -X POST http://127.0.0.1:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d '{"topic":"test","payload":{}}'

sleep 1

echo ""
echo "=== 测试 2: 普通消息（应该触发 INFO 和 DEBUG）==="
curl -X POST http://127.0.0.1:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d '{"topic":"test","payload":{"message":"Hello World"}}'

sleep 1

echo ""
echo "=== 测试 3: 高温警告消息（应该触发 WARN）==="
curl -X POST http://127.0.0.1:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d '{"topic":"sensor/temp","payload":{"temperature":85,"device":"sensor1"}}'

sleep 1

echo ""
echo "=== 测试 4: 临界温度消息（应该触发 ERROR）==="
curl -X POST http://127.0.0.1:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d '{"topic":"sensor/temp","payload":{"temperature":95,"device":"sensor2"}}'

sleep 1

echo ""
echo "=== 测试 5: 大消息（应该触发 WARN）==="
LARGE_MSG=$(python3 -c "print('x' * 2000)")
curl -X POST http://127.0.0.1:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d "{\"topic\":\"test\",\"payload\":{\"data\":\"$LARGE_MSG\"}}"

sleep 1

# 停止服务器
echo ""
echo "=== 停止服务器 ==="
kill $SERVER_PID

echo ""
echo "测试完成！请检查上面的日志输出，应该能看到："
echo "  - TRACE 级别：函数调用和返回"
echo "  - DEBUG 级别：消息接收和处理细节"
echo "  - INFO 级别：正常处理信息"
echo "  - WARN 级别：空消息、大消息、高温警告"
echo "  - ERROR 级别：临界温度告警"
