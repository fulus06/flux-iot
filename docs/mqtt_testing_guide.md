# MQTT Broker 测试指南

> **测试日期**: 2026-02-22  
> **版本**: v0.2.0  
> **状态**: 准备测试

---

## 📋 测试清单

### 1. 基础功能测试 ✅

| 测试项 | 测试方法 | 预期结果 |
|--------|---------|---------|
| 单元测试 | `cargo test -p flux-mqtt` | 所有测试通过 |
| 编译检查 | `cargo build -p flux-mqtt` | 编译成功 |
| 示例运行 | `cargo run --example mqtt_server` | 服务器启动 |

### 2. MQTT 协议测试

#### 2.1 连接测试

```bash
# 测试 MQTT 连接 (1883)
mosquitto_sub -h localhost -p 1883 -t "test/#" -v

# 预期: 成功连接
```

#### 2.2 发布/订阅测试

```bash
# 终端 1: 订阅
mosquitto_sub -h localhost -p 1883 -t "test/topic" -v

# 终端 2: 发布
mosquitto_pub -h localhost -p 1883 -t "test/topic" -m "Hello MQTT"

# 预期: 终端 1 收到消息 "Hello MQTT"
```

#### 2.3 QoS 测试

```bash
# QoS 0
mosquitto_pub -h localhost -t "test/qos0" -m "QoS 0 message" -q 0
mosquitto_sub -h localhost -t "test/qos0" -q 0

# QoS 1
mosquitto_pub -h localhost -t "test/qos1" -m "QoS 1 message" -q 1
mosquitto_sub -h localhost -t "test/qos1" -q 1

# QoS 2 (应降级为 QoS 1)
mosquitto_pub -h localhost -t "test/qos2" -m "QoS 2 message" -q 2
mosquitto_sub -h localhost -t "test/qos2" -q 2

# 预期: 所有消息都能正确接收
```

### 3. Retained 消息测试

```bash
# 发布 retained 消息
mosquitto_pub -h localhost -t "sensor/temperature" -m "25.5" -r

# 新订阅者应立即收到
mosquitto_sub -h localhost -t "sensor/temperature" -v

# 预期: 立即收到 "sensor/temperature 25.5"

# 删除 retained 消息（空 payload）
mosquitto_pub -h localhost -t "sensor/temperature" -m "" -r

# 新订阅者不应收到消息
mosquitto_sub -h localhost -t "sensor/temperature" -v

# 预期: 不收到任何消息
```

### 4. 主题通配符测试

#### 4.1 单级通配符 `+`

```bash
# 订阅
mosquitto_sub -h localhost -t "sensor/+/temperature" -v

# 发布匹配的主题
mosquitto_pub -h localhost -t "sensor/room1/temperature" -m "22.0"
mosquitto_pub -h localhost -t "sensor/room2/temperature" -m "23.0"

# 发布不匹配的主题
mosquitto_pub -h localhost -t "sensor/room1/room2/temperature" -m "24.0"

# 预期: 收到前两条消息，不收到第三条
```

#### 4.2 多级通配符 `#`

```bash
# 订阅
mosquitto_sub -h localhost -t "sensor/#" -v

# 发布各种主题
mosquitto_pub -h localhost -t "sensor/temperature" -m "25.0"
mosquitto_pub -h localhost -t "sensor/room1/temperature" -m "26.0"
mosquitto_pub -h localhost -t "sensor/room1/room2/temp" -m "27.0"
mosquitto_pub -h localhost -t "device/temperature" -m "28.0"

# 预期: 收到前三条消息，不收到第四条
```

#### 4.3 组合通配符

```bash
# 订阅
mosquitto_sub -h localhost -t "sensor/+/#" -v

# 发布
mosquitto_pub -h localhost -t "sensor/room1/temperature" -m "22.0"
mosquitto_pub -h localhost -t "sensor/room1/humidity/value" -m "60"
mosquitto_pub -h localhost -t "sensor/temperature" -m "25.0"

# 预期: 收到前两条，不收到第三条（缺少中间层级）
```

### 5. TLS/MQTTS 测试

#### 5.1 生成测试证书

```bash
# 创建证书目录
mkdir -p certs
cd certs

# 生成私钥
openssl genrsa -out server.key 2048

# 生成自签名证书
openssl req -new -x509 -key server.key -out server.crt -days 365 \
  -subj "/C=CN/ST=Beijing/L=Beijing/O=FluxIOT/CN=localhost"

# 验证证书
openssl x509 -in server.crt -text -noout
```

#### 5.2 启动 MQTTS 服务器

```bash
# 设置环境变量
export MQTT_TLS_ENABLED=true
export MQTT_CERT_PATH=certs/server.crt
export MQTT_KEY_PATH=certs/server.key

# 启动服务器
cargo run -p flux-mqtt --example mqtt_server

# 预期日志:
# Starting MQTT broker with TLS
# TLS configuration loaded successfully
# MQTTS server configured on port 8883
```

#### 5.3 MQTTS 连接测试

```bash
# 使用 TLS 连接 (8883)
mosquitto_sub -h localhost -p 8883 -t "test/#" \
  --cafile certs/server.crt \
  --insecure

# 发布消息
mosquitto_pub -h localhost -p 8883 -t "test/tls" -m "TLS message" \
  --cafile certs/server.crt \
  --insecure

# 预期: 成功连接和收发消息
```

#### 5.4 同时测试 MQTT 和 MQTTS

```bash
# 终端 1: MQTT 订阅 (1883)
mosquitto_sub -h localhost -p 1883 -t "test/#" -v

# 终端 2: MQTTS 订阅 (8883)
mosquitto_sub -h localhost -p 8883 -t "test/#" -v \
  --cafile certs/server.crt --insecure

# 终端 3: MQTT 发布
mosquitto_pub -h localhost -p 1883 -t "test/both" -m "From MQTT"

# 终端 4: MQTTS 发布
mosquitto_pub -h localhost -p 8883 -t "test/both" -m "From MQTTS" \
  --cafile certs/server.crt --insecure

# 预期: 两个订阅者都能收到两条消息
```

### 6. 性能测试

#### 6.1 并发连接测试

```bash
# 使用 mosquitto_sub 创建多个连接
for i in {1..100}; do
  mosquitto_sub -h localhost -t "test/$i" -v &
done

# 检查连接数
ps aux | grep mosquitto_sub | wc -l

# 预期: 100+ 个连接成功
```

#### 6.2 消息吞吐测试

```bash
# 发布大量消息
for i in {1..1000}; do
  mosquitto_pub -h localhost -t "test/perf" -m "Message $i"
done

# 使用 mosquitto_sub 统计接收
mosquitto_sub -h localhost -t "test/perf" -v | wc -l

# 预期: 接收 1000 条消息
```

### 7. 认证测试

```bash
# 无认证（应成功）
mosquitto_sub -h localhost -t "test/#"

# 带用户名密码（根据实现决定）
mosquitto_sub -h localhost -t "test/#" -u "user" -P "pass"

# 预期: 根据认证器实现决定是否成功
```

### 8. 错误处理测试

#### 8.1 无效主题

```bash
# 发布到无效主题（包含 # 或 +）
mosquitto_pub -h localhost -t "test/#" -m "Invalid"
mosquitto_pub -h localhost -t "test/+" -m "Invalid"

# 预期: 连接被拒绝或忽略
```

#### 8.2 连接断开

```bash
# 订阅
mosquitto_sub -h localhost -t "test/#" -v

# 强制断开（Ctrl+C）
# 重新连接
mosquitto_sub -h localhost -t "test/#" -v

# 预期: 能够重新连接
```

### 9. EventBus 集成测试

```bash
# 启动服务器（会打印 EventBus 消息）
RUST_LOG=debug cargo run -p flux-mqtt --example mqtt_server

# 发布 MQTT 消息
mosquitto_pub -h localhost -t "test/eventbus" -m '{"key":"value"}'

# 预期日志:
# Received message from EventBus: topic="test/eventbus", payload={"key":"value"}
```

---

## 🧪 自动化测试脚本

### test_mqtt.sh

```bash
#!/bin/bash

echo "=== MQTT Broker 自动化测试 ==="

# 1. 单元测试
echo "1. 运行单元测试..."
cargo test -p flux-mqtt
if [ $? -ne 0 ]; then
    echo "❌ 单元测试失败"
    exit 1
fi
echo "✅ 单元测试通过"

# 2. 启动服务器
echo "2. 启动 MQTT 服务器..."
cargo run -p flux-mqtt --example mqtt_server &
SERVER_PID=$!
sleep 3

# 3. 基础连接测试
echo "3. 测试基础连接..."
timeout 2 mosquitto_sub -h localhost -p 1883 -t "test/#" -C 1 &
sleep 1
mosquitto_pub -h localhost -p 1883 -t "test/basic" -m "test"
wait
echo "✅ 基础连接测试通过"

# 4. Retained 消息测试
echo "4. 测试 Retained 消息..."
mosquitto_pub -h localhost -p 1883 -t "test/retained" -m "retained_msg" -r
RESULT=$(timeout 2 mosquitto_sub -h localhost -p 1883 -t "test/retained" -C 1)
if [ "$RESULT" == "retained_msg" ]; then
    echo "✅ Retained 消息测试通过"
else
    echo "❌ Retained 消息测试失败"
fi

# 5. 通配符测试
echo "5. 测试主题通配符..."
timeout 2 mosquitto_sub -h localhost -p 1883 -t "sensor/+/temp" -C 1 &
sleep 1
mosquitto_pub -h localhost -p 1883 -t "sensor/room1/temp" -m "22.0"
wait
echo "✅ 通配符测试通过"

# 清理
echo "清理测试环境..."
kill $SERVER_PID
echo "=== 测试完成 ==="
```

---

## 📊 测试结果记录

### 测试环境

- **操作系统**: macOS / Linux
- **Rust 版本**: 1.75+
- **Mosquitto 版本**: 2.0+

### 测试结果

| 测试项 | 状态 | 备注 |
|--------|------|------|
| 单元测试 | ⏳ 待测试 | - |
| MQTT 连接 | ⏳ 待测试 | - |
| 发布/订阅 | ⏳ 待测试 | - |
| QoS 0/1 | ⏳ 待测试 | - |
| Retained 消息 | ⏳ 待测试 | - |
| 单级通配符 `+` | ⏳ 待测试 | - |
| 多级通配符 `#` | ⏳ 待测试 | - |
| TLS 连接 | ⏳ 待测试 | - |
| MQTTS 发布/订阅 | ⏳ 待测试 | - |
| 并发连接 | ⏳ 待测试 | - |
| EventBus 集成 | ⏳ 待测试 | - |

---

## 🔧 故障排查

### 问题 1: 无法连接到 MQTT 服务器

**症状**: `mosquitto_sub` 连接超时

**解决方案**:
```bash
# 检查服务器是否运行
ps aux | grep mqtt_server

# 检查端口是否监听
netstat -an | grep 1883

# 检查防火墙
sudo ufw allow 1883
```

### 问题 2: TLS 连接失败

**症状**: `Error: A TLS error occurred`

**解决方案**:
```bash
# 验证证书有效性
openssl x509 -in certs/server.crt -text -noout

# 使用 --insecure 跳过证书验证（仅测试）
mosquitto_sub -h localhost -p 8883 -t "test/#" \
  --cafile certs/server.crt --insecure
```

### 问题 3: Retained 消息未保存

**症状**: 新订阅者未收到 retained 消息

**解决方案**:
```bash
# 确保使用 -r 标志
mosquitto_pub -h localhost -t "test/retained" -m "message" -r

# 检查日志
RUST_LOG=debug cargo run -p flux-mqtt --example mqtt_server
```

---

## 📝 测试报告模板

```markdown
# MQTT Broker 测试报告

**测试日期**: YYYY-MM-DD
**测试人员**: [姓名]
**版本**: v0.2.0

## 测试摘要
- 总测试项: X
- 通过: Y
- 失败: Z
- 跳过: W

## 详细结果
[填写详细测试结果]

## 问题列表
[记录发现的问题]

## 建议
[改进建议]
```

---

**维护者**: FLUX IOT Team  
**创建日期**: 2026-02-22  
**状态**: 准备测试
