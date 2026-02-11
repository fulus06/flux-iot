#!/bin/bash
# ============================================
# FLUX IOT Platform - 数据恢复脚本
# ============================================

set -e

# 检查参数
if [ -z "$1" ]; then
    echo "用法: $0 <备份文件>"
    echo "示例: $0 backups/flux-iot-backup-20260211_150000.tar.gz"
    exit 1
fi

BACKUP_FILE=$1

# 颜色输出
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# 检查备份文件是否存在
if [ ! -f "$BACKUP_FILE" ]; then
    print_error "备份文件不存在: $BACKUP_FILE"
    exit 1
fi

echo ""
print_info "开始恢复 FLUX IOT 数据..."
echo ""

# 解压备份文件
TEMP_DIR=$(mktemp -d)
print_info "解压备份文件..."
tar xzf $BACKUP_FILE -C $TEMP_DIR
print_success "解压完成"

# 恢复 PostgreSQL 数据库
if [ -f "$TEMP_DIR"/postgres-*.sql ]; then
    print_info "恢复 PostgreSQL 数据库..."
    cat "$TEMP_DIR"/postgres-*.sql | docker-compose exec -T postgres psql -U flux flux_iot
    print_success "数据库恢复完成"
fi

# 恢复插件目录
if [ -f "$TEMP_DIR"/plugins-*.tar.gz ]; then
    print_info "恢复插件目录..."
    docker run --rm -v flux-iot_flux-plugins:/data -v $TEMP_DIR:/backup alpine \
        sh -c "cd /data && tar xzf /backup/plugins-*.tar.gz"
    print_success "插件恢复完成"
fi

# 恢复配置文件
if [ -f "$TEMP_DIR"/config-*.tar.gz ]; then
    print_info "恢复配置文件..."
    tar xzf "$TEMP_DIR"/config-*.tar.gz -C . 2>/dev/null || true
    print_success "配置恢复完成"
fi

# 清理临时目录
rm -rf $TEMP_DIR

echo ""
print_success "恢复完成！"
echo ""
print_info "请重启服务: docker-compose restart"
echo ""
