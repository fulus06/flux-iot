#!/bin/bash
# ============================================
# FLUX IOT Platform - Docker 启动脚本
# ============================================

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 打印带颜色的消息
print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

# 检查 Docker 是否运行
check_docker() {
    print_info "检查 Docker 环境..."
    if ! docker info > /dev/null 2>&1; then
        print_error "Docker 未运行，请先启动 Docker"
        exit 1
    fi
    print_success "Docker 运行正常"
}

# 检查 Docker Compose 版本
check_docker_compose() {
    print_info "检查 Docker Compose..."
    if ! command -v docker-compose &> /dev/null; then
        print_error "Docker Compose 未安装"
        exit 1
    fi
    print_success "Docker Compose 已安装: $(docker-compose --version)"
}

# 创建必要的目录
create_directories() {
    print_info "创建必要的目录..."
    mkdir -p data plugins certs logs
    mkdir -p nginx/conf.d
    mkdir -p prometheus grafana/dashboards grafana/datasources
    print_success "目录创建完成"
}

# 生成自签名证书（如果不存在）
generate_certs() {
    if [ ! -f "certs/server-cert.pem" ]; then
        print_info "生成自签名证书..."
        
        # 生成 CA
        openssl req -x509 -newkey rsa:4096 -days 365 -nodes \
            -keyout certs/ca-key.pem -out certs/ca-cert.pem \
            -subj "/CN=FLUX IOT CA" 2>/dev/null
        
        # 生成服务器证书
        openssl genrsa -out certs/server-key.pem 4096 2>/dev/null
        openssl req -new -key certs/server-key.pem -out certs/server-csr.pem \
            -subj "/CN=localhost" 2>/dev/null
        openssl x509 -req -in certs/server-csr.pem -days 365 \
            -CA certs/ca-cert.pem -CAkey certs/ca-key.pem -CAcreateserial \
            -out certs/server-cert.pem 2>/dev/null
        
        # 清理临时文件
        rm -f certs/server-csr.pem certs/ca-cert.srl
        
        print_success "证书生成完成"
    else
        print_info "证书已存在，跳过生成"
    fi
}

# 创建 .env 文件（如果不存在）
create_env_file() {
    if [ ! -f ".env" ]; then
        print_info "创建 .env 配置文件..."
        cat > .env << EOF
# FLUX IOT Platform 环境变量配置

# PostgreSQL
POSTGRES_PASSWORD=flux_secret_2026
POSTGRES_DB=flux_iot
POSTGRES_USER=flux

# Grafana
GRAFANA_ADMIN_USER=admin
GRAFANA_ADMIN_PASSWORD=admin

# 应用配置
RUST_LOG=info
EOF
        print_success ".env 文件创建完成"
        print_warning "请修改 .env 文件中的密码！"
    else
        print_info ".env 文件已存在"
    fi
}

# 拉取最新镜像
pull_images() {
    print_info "拉取 Docker 镜像..."
    docker-compose pull
    print_success "镜像拉取完成"
}

# 构建应用镜像
build_app() {
    print_info "构建 FLUX IOT 应用镜像..."
    docker-compose build --no-cache flux-iot
    print_success "应用镜像构建完成"
}

# 启动服务
start_services() {
    print_info "启动服务..."
    docker-compose up -d
    print_success "服务启动完成"
}

# 等待服务就绪
wait_for_services() {
    print_info "等待服务就绪..."
    
    local max_attempts=30
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        if curl -f http://localhost/health > /dev/null 2>&1; then
            print_success "服务已就绪"
            return 0
        fi
        
        attempt=$((attempt + 1))
        echo -n "."
        sleep 2
    done
    
    echo ""
    print_error "服务启动超时，请检查日志: docker-compose logs"
    return 1
}

# 显示服务信息
show_services() {
    echo ""
    echo "=========================================="
    echo "🚀 FLUX IOT Platform 已启动"
    echo "=========================================="
    echo ""
    echo "📊 服务访问地址:"
    echo "  - HTTP API:        http://localhost/api/v1"
    echo "  - 健康检查:        http://localhost/health"
    echo "  - MQTT Broker:     mqtt://localhost:1883"
    echo "  - MQTT over TLS:   mqtts://localhost:8883"
    echo "  - Prometheus:      http://localhost/prometheus"
    echo "  - Grafana:         http://localhost/grafana (admin/admin)"
    echo "  - Metrics:         http://localhost/metrics"
    echo ""
    echo "📋 常用命令:"
    echo "  - 查看日志:        docker-compose logs -f"
    echo "  - 停止服务:        ./scripts/docker-stop.sh"
    echo "  - 重启服务:        docker-compose restart"
    echo "  - 查看状态:        docker-compose ps"
    echo ""
    echo "🔐 默认凭证:"
    echo "  - Grafana:         admin / admin"
    echo "  - PostgreSQL:      flux / flux_secret_2026"
    echo ""
    print_warning "请及时修改默认密码！"
    echo "=========================================="
}

# 主函数
main() {
    echo ""
    echo "🚀 FLUX IOT Platform - Docker 部署"
    echo ""
    
    check_docker
    check_docker_compose
    create_directories
    generate_certs
    create_env_file
    pull_images
    build_app
    start_services
    
    if wait_for_services; then
        show_services
    else
        print_error "服务启动失败"
        exit 1
    fi
}

# 执行主函数
main
