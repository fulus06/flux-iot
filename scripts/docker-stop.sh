#!/bin/bash
# ============================================
# FLUX IOT Platform - Docker 停止脚本
# ============================================

set -e

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

# 停止服务
stop_services() {
    print_info "停止 FLUX IOT 服务..."
    docker-compose down
    print_success "服务已停止"
}

# 显示清理选项
show_cleanup_options() {
    echo ""
    echo "💡 清理选项:"
    echo "  - 删除数据卷:      docker-compose down -v"
    echo "  - 删除镜像:        docker-compose down --rmi all"
    echo "  - 完全清理:        docker-compose down -v --rmi all"
    echo ""
}

# 主函数
main() {
    echo ""
    echo "🛑 FLUX IOT Platform - 停止服务"
    echo ""
    
    stop_services
    show_cleanup_options
}

main
