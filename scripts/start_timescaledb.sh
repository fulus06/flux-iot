#!/bin/bash

# FLUX IOT TimescaleDB 启动脚本

echo "🚀 Starting FLUX IOT TimescaleDB..."

# 检查 Docker 是否运行
if ! docker info > /dev/null 2>&1; then
    echo "❌ Docker is not running. Please start Docker first."
    exit 1
fi

# 停止现有容器（如果存在）
if docker ps -a | grep -q flux-timescaledb; then
    echo "🛑 Stopping existing TimescaleDB container..."
    docker stop flux-timescaledb > /dev/null 2>&1
    docker rm flux-timescaledb > /dev/null 2>&1
fi

# 启动 TimescaleDB
echo "📦 Starting TimescaleDB container..."
docker compose -f docker-compose.timescaledb.yml up -d

# 等待数据库就绪
echo "⏳ Waiting for TimescaleDB to be ready..."
for i in {1..30}; do
    if docker exec flux-timescaledb pg_isready -U postgres > /dev/null 2>&1; then
        echo "✅ TimescaleDB is ready!"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "❌ TimescaleDB failed to start"
        exit 1
    fi
    sleep 1
done

# 显示连接信息
echo ""
echo "📊 TimescaleDB Connection Info:"
echo "  Host: localhost"
echo "  Port: 5432"
echo "  Database: flux_iot"
echo "  Username: postgres"
echo "  Password: postgres"
echo ""
echo "🔗 Connection String:"
echo "  postgresql://postgres:postgres@localhost:5432/flux_iot"
echo ""
echo "📝 Test connection:"
echo "  docker exec -it flux-timescaledb psql -U postgres -d flux_iot"
echo ""
echo "✨ TimescaleDB started successfully!"
