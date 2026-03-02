#!/bin/bash

echo "=== RTMPD 登录测试 ==="
echo ""

# 测试 admin 用户登录
echo "1. 测试 admin 用户登录..."
response=$(curl -s -X POST http://localhost:8082/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "admin123"}')

if echo "$response" | grep -q "token"; then
    echo "✅ Admin 登录成功"
    echo "响应: $response"
    
    # 提取 token
    token=$(echo "$response" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
    echo "Token: ${token:0:50}..."
else
    echo "❌ Admin 登录失败"
    echo "响应: $response"
fi

echo ""

# 测试错误密码
echo "2. 测试错误密码..."
response=$(curl -s -X POST http://localhost:8082/login \
  -H "Content-Type: application/json" \
  -d '{"username": "admin", "password": "wrongpassword"}')

if echo "$response" | grep -q "token"; then
    echo "❌ 错误：错误密码应该被拒绝"
else
    echo "✅ 正确：错误密码被拒绝"
fi

echo ""

# 测试 operator 用户
echo "3. 测试 operator 用户登录..."
response=$(curl -s -X POST http://localhost:8082/login \
  -H "Content-Type: application/json" \
  -d '{"username": "operator", "password": "op123"}')

if echo "$response" | grep -q "token"; then
    echo "✅ Operator 登录成功"
else
    echo "❌ Operator 登录失败"
    echo "响应: $response"
fi

echo ""
echo "=== 测试完成 ==="
