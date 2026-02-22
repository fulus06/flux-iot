#!/bin/bash
# FLUX IOT 测试数据库初始化脚本

set -e

# 颜色输出
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== FLUX IOT 测试数据库初始化 ===${NC}"

# 数据库配置
DB_USER=${DB_USER:-postgres}
DB_PASSWORD=${DB_PASSWORD:-postgres}
DB_HOST=${DB_HOST:-localhost}
DB_PORT=${DB_PORT:-5432}
DB_NAME=${DB_NAME:-flux_test}

export PGPASSWORD=$DB_PASSWORD

echo -e "${YELLOW}数据库配置:${NC}"
echo "  用户: $DB_USER"
echo "  主机: $DB_HOST:$DB_PORT"
echo "  数据库: $DB_NAME"
echo ""

# 检查 PostgreSQL 是否运行
echo -e "${YELLOW}1. 检查 PostgreSQL 服务...${NC}"
if ! pg_isready -h $DB_HOST -p $DB_PORT -U $DB_USER > /dev/null 2>&1; then
    echo -e "${RED}错误: PostgreSQL 服务未运行${NC}"
    echo "请先启动 PostgreSQL 服务"
    exit 1
fi
echo -e "${GREEN}✓ PostgreSQL 服务正常${NC}"

# 检查数据库是否存在
echo -e "${YELLOW}2. 检查数据库是否存在...${NC}"
if psql -h $DB_HOST -p $DB_PORT -U $DB_USER -lqt | cut -d \| -f 1 | grep -qw $DB_NAME; then
    echo -e "${YELLOW}数据库 $DB_NAME 已存在，是否删除并重建? (y/N)${NC}"
    read -r response
    if [[ "$response" =~ ^([yY][eE][sS]|[yY])$ ]]; then
        echo "删除现有数据库..."
        psql -h $DB_HOST -p $DB_PORT -U $DB_USER -c "DROP DATABASE IF EXISTS $DB_NAME;"
        echo -e "${GREEN}✓ 数据库已删除${NC}"
    else
        echo "保留现有数据库，仅更新表结构"
    fi
fi

# 创建数据库（如果不存在）
echo -e "${YELLOW}3. 创建数据库...${NC}"
psql -h $DB_HOST -p $DB_PORT -U $DB_USER -c "CREATE DATABASE $DB_NAME;" 2>/dev/null || echo "数据库已存在"
echo -e "${GREEN}✓ 数据库准备就绪${NC}"

# 初始化表结构
echo -e "${YELLOW}4. 初始化表结构...${NC}"
if [ -f "scripts/init_test_db.sql" ]; then
    psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -f scripts/init_test_db.sql > /dev/null
    echo -e "${GREEN}✓ 表结构初始化完成${NC}"
else
    echo -e "${RED}错误: 找不到 scripts/init_test_db.sql${NC}"
    exit 1
fi

# 验证表是否创建成功
echo -e "${YELLOW}5. 验证表结构...${NC}"
TABLES=$(psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema='public';")
echo "  创建的表数量: $TABLES"

if [ "$TABLES" -gt 0 ]; then
    echo -e "${GREEN}✓ 表结构验证成功${NC}"
    echo ""
    echo "已创建的表:"
    psql -h $DB_HOST -p $DB_PORT -U $DB_USER -d $DB_NAME -c "\dt"
else
    echo -e "${RED}错误: 表创建失败${NC}"
    exit 1
fi

# 输出连接字符串
echo ""
echo -e "${GREEN}=== 初始化完成 ===${NC}"
echo ""
echo "数据库连接字符串:"
echo -e "${YELLOW}DATABASE_URL=postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME${NC}"
echo ""
echo "运行测试:"
echo "  make test"
echo "  或"
echo "  DATABASE_URL=postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME cargo test"
echo ""

unset PGPASSWORD
