#!/bin/bash
set -e

echo "=== FLUX IOT PostgreSQL 迁移脚本 ==="
echo ""

# 设置数据库 URL
export DATABASE_URL="postgresql://flux:flux@localhost/flux_iot"

echo "数据库 URL: $DATABASE_URL"
echo ""

# 进入迁移目录并运行
cd "$(dirname "$0")/migration"

echo "开始运行迁移..."
cargo run --bin migration -- up

echo ""
echo "✅ 迁移完成！"
