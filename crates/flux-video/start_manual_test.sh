#!/bin/bash
# FLUX Video 人工验证测试 - 一键启动脚本

set -e

echo "╔════════════════════════════════════════════════════════════╗"
echo "║   🎥 FLUX Video 人工验证测试                              ║"
echo "║   屏幕捕获 → RTSP推流 → flux-video → Web播放器            ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# 颜色定义
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 清理函数
cleanup() {
    echo ""
    echo -e "${YELLOW}🧹 清理测试环境...${NC}"
    
    if [ ! -z "$SERVER_PID" ]; then
        kill $SERVER_PID 2>/dev/null || true
        echo "   ✓ 已停止 video_server (PID: $SERVER_PID)"
    fi
    
    if [ ! -z "$STREAMER_PID" ]; then
        kill $STREAMER_PID 2>/dev/null || true
        echo "   ✓ 已停止 screen_capture_streamer (PID: $STREAMER_PID)"
    fi
    
    echo ""
    echo -e "${GREEN}✅ 测试环境已清理${NC}"
    exit 0
}

# 捕获 Ctrl+C
trap cleanup INT TERM

# 检查是否在正确的目录
if [ ! -f "Cargo.toml" ]; then
    echo "❌ 错误: 请在 flux-video 目录下运行此脚本"
    exit 1
fi

echo -e "${BLUE}📦 步骤 1/5: 编译示例程序...${NC}"
cargo build --examples --quiet
echo -e "${GREEN}   ✓ 编译完成${NC}"
echo ""

echo -e "${BLUE}🚀 步骤 2/5: 启动 flux-video 服务器...${NC}"
cargo run --example video_server > /tmp/flux_video_server.log 2>&1 &
SERVER_PID=$!
echo "   ✓ 服务器已启动 (PID: $SERVER_PID)"
echo "   ✓ 日志文件: /tmp/flux_video_server.log"
sleep 3

# 检查服务器是否启动成功
if ! curl -s http://localhost:8080/health > /dev/null 2>&1; then
    echo "   ❌ 服务器启动失败，请查看日志"
    cat /tmp/flux_video_server.log
    cleanup
fi
echo -e "${GREEN}   ✓ 服务器运行正常${NC}"
echo ""

echo -e "${BLUE}📡 步骤 3/5: 启动屏幕捕获推流器...${NC}"
cargo run --example screen_capture_streamer > /tmp/flux_screen_streamer.log 2>&1 &
STREAMER_PID=$!
echo "   ✓ 推流器已启动 (PID: $STREAMER_PID)"
echo "   ✓ 日志文件: /tmp/flux_screen_streamer.log"
echo "   ✓ 推流地址: rtsp://127.0.0.1:8554/screen"
sleep 2
echo -e "${GREEN}   ✓ 推流器运行正常${NC}"
echo ""

echo -e "${BLUE}🔗 步骤 4/5: 创建流连接...${NC}"
RESPONSE=$(curl -s -X POST http://localhost:8080/api/video/streams \
  -H 'Content-Type: application/json' \
  -d '{
    "stream_id": "screen_capture",
    "protocol": "rtsp",
    "url": "rtsp://127.0.0.1:8554/screen"
  }')

if echo "$RESPONSE" | grep -q "success"; then
    echo -e "${GREEN}   ✓ 流连接创建成功${NC}"
else
    echo "   ⚠️  流连接可能失败，但可以在 Web 播放器中重试"
fi
echo ""

echo -e "${BLUE}🌐 步骤 5/5: 打开 Web 播放器...${NC}"
echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║  📺 请在浏览器中打开以下地址:                             ║"
echo "║                                                            ║"
echo "║  http://localhost:8080/player.html?stream=screen_capture  ║"
echo "║                                                            ║"
echo "║  或访问首页:                                               ║"
echo "║  http://localhost:8080/                                   ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# 尝试自动打开浏览器
if command -v open > /dev/null 2>&1; then
    # macOS
    echo "🚀 正在打开浏览器..."
    open "http://localhost:8080/player.html?stream=screen_capture"
elif command -v xdg-open > /dev/null 2>&1; then
    # Linux
    echo "🚀 正在打开浏览器..."
    xdg-open "http://localhost:8080/player.html?stream=screen_capture"
else
    echo "💡 请手动在浏览器中打开上述地址"
fi

echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║  ✅ 测试环境已就绪！                                       ║"
echo "║                                                            ║"
echo "║  📋 验证步骤:                                              ║"
echo "║  1. 在 Web 播放器中点击 '▶️ 连接流' 按钮                  ║"
echo "║  2. 观察统计数据是否实时更新                               ║"
echo "║  3. 查看日志区域是否有关键帧接收记录                       ║"
echo "║  4. 尝试点击 '📸 截图' 按钮                               ║"
echo "║                                                            ║"
echo "║  📊 查看实时日志:                                          ║"
echo "║  tail -f /tmp/flux_video_server.log                       ║"
echo "║  tail -f /tmp/flux_screen_streamer.log                    ║"
echo "║                                                            ║"
echo "║  🛑 按 Ctrl+C 停止测试                                     ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# 保持运行
echo "⏳ 测试环境运行中... (按 Ctrl+C 停止)"
echo ""

# 每 10 秒显示一次状态
while true; do
    sleep 10
    
    # 检查进程是否还在运行
    if ! kill -0 $SERVER_PID 2>/dev/null; then
        echo "❌ 服务器进程已停止"
        cleanup
    fi
    
    if ! kill -0 $STREAMER_PID 2>/dev/null; then
        echo "❌ 推流器进程已停止"
        cleanup
    fi
    
    # 显示简单的状态
    echo -n "."
done
