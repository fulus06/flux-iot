#!/bin/bash
# ============================================
# FLUX IOT Platform - 数据备份脚本
# ============================================

set -e

# 配置
BACKUP_DIR="./backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="flux-iot-backup-${TIMESTAMP}.tar.gz"

# 颜色输出
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

# 创建备份目录
mkdir -p $BACKUP_DIR

echo ""
print_info "开始备份 FLUX IOT 数据..."
echo ""

# 备份 PostgreSQL 数据库
print_info "备份 PostgreSQL 数据库..."
docker-compose exec -T postgres pg_dump -U flux flux_iot > "${BACKUP_DIR}/postgres-${TIMESTAMP}.sql"
print_success "数据库备份完成"

# 备份插件目录
print_info "备份插件目录..."
docker run --rm -v flux-iot_flux-plugins:/data -v $(pwd)/${BACKUP_DIR}:/backup alpine \
    tar czf /backup/plugins-${TIMESTAMP}.tar.gz -C /data .
print_success "插件备份完成"

# 备份配置文件
print_info "备份配置文件..."
tar czf "${BACKUP_DIR}/config-${TIMESTAMP}.tar.gz" config.toml .env 2>/dev/null || true
print_success "配置备份完成"

# 创建完整备份
print_info "创建完整备份归档..."
cd $BACKUP_DIR
tar czf $BACKUP_FILE \
    postgres-${TIMESTAMP}.sql \
    plugins-${TIMESTAMP}.tar.gz \
    config-${TIMESTAMP}.tar.gz 2>/dev/null || true

# 清理临时文件
rm -f postgres-${TIMESTAMP}.sql plugins-${TIMESTAMP}.tar.gz config-${TIMESTAMP}.tar.gz

cd ..

echo ""
print_success "备份完成: ${BACKUP_DIR}/${BACKUP_FILE}"
echo ""
echo "📦 备份文件大小: $(du -h ${BACKUP_DIR}/${BACKUP_FILE} | cut -f1)"
echo ""
