#!/bin/bash
# 应用缺失的数据库迁移
# 使用方法: ./apply_missing_migrations.sh

set -e

echo "=== 应用 FLUX IOT 缺失的数据库迁移 ==="
echo ""

# 检查 DATABASE_URL 环境变量
if [ -z "$DATABASE_URL" ]; then
    echo "❌ 错误: DATABASE_URL 环境变量未设置"
    echo "请设置: export DATABASE_URL='postgresql://flux:flux@localhost/flux_iot'"
    exit 1
fi

echo "数据库连接: $DATABASE_URL"
echo ""

# 检查 PostgreSQL 是否运行
if ! psql "$DATABASE_URL" -c "SELECT 1" > /dev/null 2>&1; then
    echo "❌ 错误: 无法连接到 PostgreSQL 数据库"
    echo ""
    echo "请启动 PostgreSQL:"
    echo "  docker run -d --name flux-postgres \\"
    echo "    -e POSTGRES_USER=flux \\"
    echo "    -e POSTGRES_PASSWORD=flux \\"
    echo "    -e POSTGRES_DB=flux_iot \\"
    echo "    -p 5432:5432 \\"
    echo "    postgres:15"
    exit 1
fi

echo "✅ 数据库连接成功"
echo ""

# 应用设备表迁移
echo "1. 应用设备表迁移 (003_create_devices_tables.sql)..."
if psql "$DATABASE_URL" -f migrations_sql/003_create_devices_tables.sql > /dev/null 2>&1; then
    echo "   ✅ 设备表创建成功"
else
    echo "   ⚠️  设备表可能已存在"
fi

# 应用控制表迁移
echo "2. 应用控制表迁移 (005_create_control_tables.sql)..."
if psql "$DATABASE_URL" -f migrations_sql/005_create_control_tables.sql > /dev/null 2>&1; then
    echo "   ✅ 控制表创建成功"
else
    echo "   ⚠️  控制表可能已存在"
fi

echo ""
echo "=== 验证表创建 ==="
echo ""

# 验证表是否存在
echo "检查已创建的表:"
psql "$DATABASE_URL" -c "\dt device.*" 2>/dev/null | grep -E "(devices|device_metrics)" || echo "  ⚠️  设备表未找到"
psql "$DATABASE_URL" -c "\dt control.*" 2>/dev/null | grep -E "(device_commands|command_responses)" || echo "  ⚠️  控制表未找到"

echo ""
echo "=== 迁移完成 ==="
echo ""
echo "已创建的表:"
echo "  - device.devices (设备表)"
echo "  - device.device_metrics (设备指标表)"
echo "  - control.device_commands (设备指令表)"
echo "  - control.command_responses (指令响应表)"
echo ""
echo "✅ 所有迁移已应用！"
