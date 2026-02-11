#!/bin/bash
# ============================================
# FLUX IOT Platform - 日志查看脚本
# ============================================

SERVICE=${1:-flux-iot}
LINES=${2:-100}

# 颜色输出
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}📋 查看服务日志: $SERVICE (最近 $LINES 行)${NC}"
echo ""

# 显示日志
docker-compose logs -f --tail=$LINES $SERVICE
