#!/bin/bash
# 测试 Wasm 插件在 Rule Worker 主流程中的集成

echo "=========================================="
echo "  测试 Wasm 插件集成到 Rule Worker"
echo "=========================================="
echo ""

# 设置日志级别，显示所有插件和规则相关的日志
export RUST_LOG=info,flux_server::worker=debug,wasm_plugin=debug

echo "1. 启动 FLUX IOT 服务器（后台运行）..."
cargo run -p flux-server > /tmp/flux-server.log 2>&1 &
SERVER_PID=$!

echo "   服务器 PID: $SERVER_PID"
echo "   等待服务器启动..."
sleep 5

# 检查服务器是否成功启动
if ! kill -0 $SERVER_PID 2>/dev/null; then
    echo "❌ 服务器启动失败！"
    cat /tmp/flux-server.log
    exit 1
fi

echo "✅ 服务器启动成功"
echo ""

echo "2. 发送测试消息到 EventBus..."
echo ""

# 测试 1: 普通 JSON 消息
echo "📤 测试 1: 发送普通 JSON 消息"
curl -s -X POST http://127.0.0.1:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d '{
    "topic": "test/sensor",
    "payload": {
      "device_id": "sensor001",
      "temperature": 25.5,
      "humidity": 60
    }
  }' | jq .

sleep 2

# 测试 2: 包含温度告警的消息
echo ""
echo "📤 测试 2: 发送高温告警消息（应触发插件 WARN 日志）"
curl -s -X POST http://127.0.0.1:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d '{
    "topic": "sensor/temperature",
    "payload": {
      "device_id": "sensor002",
      "temperature": 85,
      "location": "server_room"
    }
  }' | jq .

sleep 2

# 测试 3: 临界温度消息
echo ""
echo "📤 测试 3: 发送临界温度消息（应触发插件 ERROR 日志）"
curl -s -X POST http://127.0.0.1:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d '{
    "topic": "sensor/critical",
    "payload": {
      "device_id": "sensor003",
      "temperature": 95,
      "alert": true
    }
  }' | jq .

sleep 2

# 测试 4: 空消息
echo ""
echo "📤 测试 4: 发送空消息（应触发插件 WARN 日志）"
curl -s -X POST http://127.0.0.1:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d '{
    "topic": "test/empty",
    "payload": {}
  }' | jq .

sleep 2

echo ""
echo "=========================================="
echo "3. 停止服务器并查看日志"
echo "=========================================="

kill $SERVER_PID
wait $SERVER_PID 2>/dev/null

echo ""
echo "📋 服务器日志（最后 100 行）："
echo "=========================================="
tail -100 /tmp/flux-server.log | grep -E "(Plugin|plugin|RULE|Worker|wasm_plugin)" --color=always

echo ""
echo "=========================================="
echo "✅ 测试完成！"
echo "=========================================="
echo ""
echo "预期结果："
echo "  ✓ 每条消息都应该被 dummy_plugin 处理"
echo "  ✓ 插件应该输出多级别日志（trace/debug/info/warn/error）"
echo "  ✓ 插件返回消息长度作为处理结果"
echo "  ✓ 规则引擎在插件处理后执行"
echo ""
echo "检查要点："
echo "  1. 日志中是否有 'Plugin dummy_plugin processed message'"
echo "  2. 日志中是否有来自 wasm_plugin target 的多级别日志"
echo "  3. 高温消息是否触发 WARN 级别日志"
echo "  4. 临界温度是否触发 ERROR 级别日志"
echo "  5. 空消息是否触发 WARN 日志"
