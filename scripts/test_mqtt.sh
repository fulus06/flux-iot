#!/bin/bash

# MQTT Broker 自动化测试脚本
# 使用方法: ./scripts/test_mqtt.sh

set -e

echo "================================================"
echo "  MQTT Broker 自动化测试"
echo "================================================"
echo ""

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 测试结果统计
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# 测试函数
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo -n "[$TOTAL_TESTS] $test_name ... "
    
    if eval "$test_command" > /dev/null 2>&1; then
        echo -e "${GREEN}✓ PASSED${NC}"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        return 0
    else
        echo -e "${RED}✗ FAILED${NC}"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        return 1
    fi
}

# 1. 单元测试
echo "=== 阶段 1: 单元测试 ==="
echo ""

run_test "编译检查" "cargo build -p flux-mqtt --quiet"
run_test "单元测试" "cargo test -p flux-mqtt --lib --quiet"
run_test "集成测试" "cargo test -p flux-mqtt --test integration_test --quiet"
run_test "文档测试" "cargo test -p flux-mqtt --doc --quiet"

echo ""

# 2. 代码质量检查
echo "=== 阶段 2: 代码质量检查 ==="
echo ""

run_test "Clippy 检查" "cargo clippy -p flux-mqtt --quiet"
run_test "格式检查" "cargo fmt -p flux-mqtt -- --check"

echo ""

# 3. 示例编译
echo "=== 阶段 3: 示例编译 ==="
echo ""

run_test "示例服务器编译" "cargo build -p flux-mqtt --example mqtt_server --quiet"

echo ""

# 4. 性能测试（可选）
echo "=== 阶段 4: 性能测试 ==="
echo ""

# 检查是否有 mosquitto 客户端
if command -v mosquitto_pub &> /dev/null; then
    echo -e "${YELLOW}检测到 mosquitto 客户端，可以进行实际测试${NC}"
    echo -e "${YELLOW}提示: 运行 'cargo run -p flux-mqtt --example mqtt_server' 启动服务器${NC}"
else
    echo -e "${YELLOW}未检测到 mosquitto 客户端，跳过实际连接测试${NC}"
    echo -e "${YELLOW}安装: brew install mosquitto (macOS) 或 apt-get install mosquitto-clients (Linux)${NC}"
fi

echo ""

# 5. 测试总结
echo "================================================"
echo "  测试总结"
echo "================================================"
echo ""
echo "总测试数: $TOTAL_TESTS"
echo -e "通过: ${GREEN}$PASSED_TESTS${NC}"
echo -e "失败: ${RED}$FAILED_TESTS${NC}"
echo ""

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}✓ 所有测试通过！${NC}"
    exit 0
else
    echo -e "${RED}✗ 有 $FAILED_TESTS 个测试失败${NC}"
    exit 1
fi
