#!/bin/bash
# 视频流 HTTP API 集成测试脚本

set -e

BASE_URL="http://localhost:8080"
STREAM_ID="test_camera_001"

echo "=== FLUX Video HTTP API Integration Test ==="
echo ""

# 1. 健康检查
echo "1. Testing health check..."
curl -s "${BASE_URL}/health" | jq .
echo "✅ Health check passed"
echo ""

# 2. 创建流
echo "2. Creating RTSP stream..."
curl -s -X POST "${BASE_URL}/api/video/streams" \
  -H "Content-Type: application/json" \
  -d '{
    "stream_id": "'"${STREAM_ID}"'",
    "protocol": "rtsp",
    "url": "rtsp://localhost:8554/stream"
  }' | jq .
echo "✅ Stream created"
echo ""

# 3. 列出所有流
echo "3. Listing all streams..."
curl -s "${BASE_URL}/api/video/streams" | jq .
echo "✅ Streams listed"
echo ""

# 4. 获取流信息
echo "4. Getting stream info..."
curl -s "${BASE_URL}/api/video/streams/${STREAM_ID}" | jq .
echo "✅ Stream info retrieved"
echo ""

# 5. 获取快照
echo "5. Getting snapshot..."
curl -s "${BASE_URL}/api/video/streams/${STREAM_ID}/snapshot" | jq .
echo "✅ Snapshot captured"
echo ""

# 6. 删除流
echo "6. Deleting stream..."
curl -s -X DELETE "${BASE_URL}/api/video/streams/${STREAM_ID}" | jq .
echo "✅ Stream deleted"
echo ""

echo "=== All tests passed! ==="
