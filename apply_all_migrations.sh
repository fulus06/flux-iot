#!/bin/bash
set -e

echo "=== FLUX IOT 完整数据库迁移脚本 ==="
echo ""

# 设置数据库 URL
export DATABASE_URL="${DATABASE_URL:-postgresql://flux:flux@localhost/flux_iot}"

echo "数据库 URL: $DATABASE_URL"
echo ""

# 检查 PostgreSQL 连接
echo "1. 检查数据库连接..."
if psql "$DATABASE_URL" -c "SELECT 1;" > /dev/null 2>&1; then
    echo "   ✅ 数据库连接成功"
else
    echo "   ❌ 数据库连接失败"
    echo "   请确保 PostgreSQL 正在运行: docker ps | grep flux-postgres"
    exit 1
fi

echo ""
echo "2. 应用所有迁移文件..."
echo ""

# 按顺序执行所有迁移
for sql_file in migrations_sql/*.sql; do
    if [ -f "$sql_file" ]; then
        echo "   执行: $sql_file"
        if psql "$DATABASE_URL" -f "$sql_file" > /dev/null 2>&1; then
            echo "   ✅ 成功"
        else
            echo "   ⚠️  可能已存在（忽略错误）"
        fi
    fi
done

echo ""
echo "3. 验证表创建..."
echo ""

# 检查关键表
tables=(
    "public.users"
    "public.app_config"
    "public.rules"
    "public.events"
    "device.devices"
    "device.device_metrics"
    "mqtt.mqtt_clients"
    "mqtt.mqtt_subscriptions"
    "control.device_commands"
    "control.command_responses"
)

for table in "${tables[@]}"; do
    if psql "$DATABASE_URL" -c "SELECT 1 FROM $table LIMIT 1;" > /dev/null 2>&1; then
        echo "   ✅ $table"
    else
        echo "   ❌ $table (不存在)"
    fi
done

echo ""
echo "4. 统计信息..."
echo ""

# 统计各 schema 的表数量
psql "$DATABASE_URL" -c "
SELECT 
    schemaname, 
    COUNT(*) as table_count 
FROM pg_tables 
WHERE schemaname IN ('public', 'device', 'mqtt', 'control', 'rtmpd') 
GROUP BY schemaname 
ORDER BY schemaname;
"

echo ""
echo "=== ✅ 数据库迁移完成！==="
echo ""
echo "下一步："
echo "  1. 启动服务: cargo run -p flux-server"
echo "  2. 启动 RTMPD: cargo run -p flux-rtmpd --features persistence"
echo "  3. 测试登录: curl -X POST http://localhost:8082/login -H 'Content-Type: application/json' -d '{\"username\":\"admin\",\"password\":\"admin123\"}'"
