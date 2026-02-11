# FLUX IOT API 文档

**版本**: v1  
**基础 URL**: `http://localhost:3000`  
**内容类型**: `application/json`

---

## 📋 目录

- [概述](#概述)
- [认证](#认证)
- [端点列表](#端点列表)
  - [健康检查](#健康检查)
  - [事件管理](#事件管理)
  - [规则管理](#规则管理)
- [数据模型](#数据模型)
- [错误处理](#错误处理)
- [示例代码](#示例代码)

---

## 概述

FLUX IOT 提供 RESTful API 用于：
- 发布事件到消息总线
- 管理规则（创建、查询、重载）
- 系统健康检查

所有 API 响应均为 JSON 格式。

---

## 认证

**当前版本**: 无需认证（开发阶段）

**未来版本**: 将支持以下认证方式
- API Key
- JWT Token
- OAuth 2.0

---

## 端点列表

### 健康检查

#### GET /health

检查服务器是否正常运行。

**请求**

```http
GET /health HTTP/1.1
Host: localhost:3000
```

**响应**

```http
HTTP/1.1 200 OK
Content-Type: text/plain

OK
```

**cURL 示例**

```bash
curl http://localhost:3000/health
```

---

### 事件管理

#### POST /api/v1/event

发布事件到消息总线，触发规则引擎处理。

**请求**

```http
POST /api/v1/event HTTP/1.1
Host: localhost:3000
Content-Type: application/json

{
  "topic": "sensors/temperature",
  "payload": {
    "device_id": "sensor001",
    "temperature": 25.5,
    "humidity": 60,
    "timestamp": 1707638400
  }
}
```

**请求参数**

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| topic | string | 是 | 事件主题，建议使用层级结构（如 `sensors/temperature`） |
| payload | object | 是 | 事件数据，任意 JSON 对象 |

**响应**

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "status": "ok",
  "id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**响应字段**

| 字段 | 类型 | 说明 |
|------|------|------|
| status | string | 处理状态，固定为 `"ok"` |
| id | string | 事件唯一 ID (UUID) |

**错误响应**

```http
HTTP/1.1 400 Bad Request
Content-Type: application/json

{
  "error": "Invalid JSON format"
}
```

**cURL 示例**

```bash
curl -X POST http://localhost:3000/api/v1/event \
  -H "Content-Type: application/json" \
  -d '{
    "topic": "sensors/temperature",
    "payload": {
      "device_id": "sensor001",
      "temperature": 25.5
    }
  }'
```

**JavaScript 示例**

```javascript
fetch('http://localhost:3000/api/v1/event', {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
  },
  body: JSON.stringify({
    topic: 'sensors/temperature',
    payload: {
      device_id: 'sensor001',
      temperature: 25.5
    }
  })
})
.then(response => response.json())
.then(data => console.log('Event published:', data.id));
```

---

### 规则管理

#### POST /api/v1/rules

创建新的规则。

**请求**

```http
POST /api/v1/rules HTTP/1.1
Host: localhost:3000
Content-Type: application/json

{
  "name": "high_temperature_alert",
  "script": "if payload.temperature > 30.0 { return true; } else { return false; }"
}
```

**请求参数**

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| name | string | 是 | 规则名称，必须唯一 |
| script | string | 是 | Rhai 脚本代码 |

**Rhai 脚本说明**

脚本中可以访问以下变量：
- `payload`: 事件的 payload 对象
- `topic`: 事件的 topic 字符串
- `state_get(key)`: 获取持久化状态
- `state_set(key, value)`: 设置持久化状态

脚本应返回布尔值：
- `true`: 规则触发
- `false`: 规则不触发

**响应**

```http
HTTP/1.1 201 Created
Content-Type: application/json

{
  "status": "created",
  "name": "high_temperature_alert"
}
```

**错误响应**

```http
HTTP/1.1 400 Bad Request
Content-Type: application/json

{
  "error": "Script compilation failed: Syntax error at line 1"
}
```

**cURL 示例**

```bash
curl -X POST http://localhost:3000/api/v1/rules \
  -H "Content-Type: application/json" \
  -d '{
    "name": "high_temp_alert",
    "script": "if payload.temperature > 30.0 { return true; }"
  }'
```

---

#### GET /api/v1/rules

获取所有已加载的规则列表。

**请求**

```http
GET /api/v1/rules HTTP/1.1
Host: localhost:3000
```

**响应**

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "rules": [
    "high_temperature_alert",
    "low_battery_warning",
    "motion_detection"
  ]
}
```

**响应字段**

| 字段 | 类型 | 说明 |
|------|------|------|
| rules | array | 规则名称列表 |

**cURL 示例**

```bash
curl http://localhost:3000/api/v1/rules
```

---

#### POST /api/v1/rules/reload

从数据库重新加载所有规则。

**请求**

```http
POST /api/v1/rules/reload HTTP/1.1
Host: localhost:3000
```

**响应**

```http
HTTP/1.1 200 OK
Content-Type: application/json

{
  "status": "reloaded",
  "count": 5
}
```

**响应字段**

| 字段 | 类型 | 说明 |
|------|------|------|
| status | string | 固定为 `"reloaded"` |
| count | number | 重新加载的规则数量 |

**cURL 示例**

```bash
curl -X POST http://localhost:3000/api/v1/rules/reload
```

---

## 数据模型

### Event (事件)

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "topic": "sensors/temperature",
  "payload": {
    "device_id": "sensor001",
    "temperature": 25.5,
    "humidity": 60
  },
  "timestamp": 1707638400000
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| id | string | UUID，系统自动生成 |
| topic | string | 事件主题 |
| payload | object | 事件数据，任意 JSON |
| timestamp | number | Unix 时间戳（毫秒），系统自动生成 |

### Rule (规则)

```json
{
  "id": 1,
  "name": "high_temperature_alert",
  "script": "if payload.temperature > 30.0 { return true; }",
  "active": true,
  "created_at": 1707638400000
}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| id | number | 数据库主键 |
| name | string | 规则名称 |
| script | string | Rhai 脚本代码 |
| active | boolean | 是否激活 |
| created_at | number | 创建时间（Unix 时间戳，毫秒） |

---

## 错误处理

### 错误响应格式

```json
{
  "error": "错误描述信息"
}
```

### HTTP 状态码

| 状态码 | 说明 |
|--------|------|
| 200 OK | 请求成功 |
| 201 Created | 资源创建成功 |
| 400 Bad Request | 请求参数错误或格式不正确 |
| 404 Not Found | 资源不存在 |
| 500 Internal Server Error | 服务器内部错误 |

### 常见错误

#### 无效的 JSON

```http
HTTP/1.1 400 Bad Request

{
  "error": "Invalid JSON format"
}
```

#### 脚本编译失败

```http
HTTP/1.1 400 Bad Request

{
  "error": "Script compilation failed: Syntax error at line 1"
}
```

#### 数据库错误

```http
HTTP/1.1 500 Internal Server Error

{
  "error": "Database error: connection failed"
}
```

---

## 示例代码

### Python

```python
import requests
import json

# 发布事件
def publish_event(topic, payload):
    url = 'http://localhost:3000/api/v1/event'
    data = {
        'topic': topic,
        'payload': payload
    }
    response = requests.post(url, json=data)
    return response.json()

# 创建规则
def create_rule(name, script):
    url = 'http://localhost:3000/api/v1/rules'
    data = {
        'name': name,
        'script': script
    }
    response = requests.post(url, json=data)
    return response.json()

# 使用示例
if __name__ == '__main__':
    # 发布温度事件
    result = publish_event('sensors/temperature', {
        'device_id': 'sensor001',
        'temperature': 35.5
    })
    print(f"Event published: {result['id']}")
    
    # 创建高温告警规则
    rule = create_rule(
        'high_temp_alert',
        'if payload.temperature > 30.0 { return true; }'
    )
    print(f"Rule created: {rule['name']}")
```

### Node.js

```javascript
const axios = require('axios');

const BASE_URL = 'http://localhost:3000';

// 发布事件
async function publishEvent(topic, payload) {
  const response = await axios.post(`${BASE_URL}/api/v1/event`, {
    topic,
    payload
  });
  return response.data;
}

// 创建规则
async function createRule(name, script) {
  const response = await axios.post(`${BASE_URL}/api/v1/rules`, {
    name,
    script
  });
  return response.data;
}

// 获取规则列表
async function getRules() {
  const response = await axios.get(`${BASE_URL}/api/v1/rules`);
  return response.data;
}

// 使用示例
(async () => {
  try {
    // 发布事件
    const event = await publishEvent('sensors/temperature', {
      device_id: 'sensor001',
      temperature: 35.5
    });
    console.log('Event published:', event.id);
    
    // 创建规则
    const rule = await createRule(
      'high_temp_alert',
      'if payload.temperature > 30.0 { return true; }'
    );
    console.log('Rule created:', rule.name);
    
    // 获取规则列表
    const rules = await getRules();
    console.log('Active rules:', rules.rules);
  } catch (error) {
    console.error('Error:', error.response?.data || error.message);
  }
})();
```

### Rust

```rust
use reqwest;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    // 发布事件
    let event_response = client
        .post("http://localhost:3000/api/v1/event")
        .json(&json!({
            "topic": "sensors/temperature",
            "payload": {
                "device_id": "sensor001",
                "temperature": 35.5
            }
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    
    println!("Event published: {}", event_response["id"]);
    
    // 创建规则
    let rule_response = client
        .post("http://localhost:3000/api/v1/rules")
        .json(&json!({
            "name": "high_temp_alert",
            "script": "if payload.temperature > 30.0 { return true; }"
        }))
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    
    println!("Rule created: {}", rule_response["name"]);
    
    Ok(())
}
```

---

## 最佳实践

### 1. 事件主题命名

使用层级结构，便于过滤和路由：

```
sensors/temperature
sensors/humidity
devices/gateway001/status
alerts/critical
```

### 2. Payload 设计

包含必要的元数据：

```json
{
  "device_id": "sensor001",
  "timestamp": 1707638400,
  "location": "room_a",
  "data": {
    "temperature": 25.5,
    "humidity": 60
  }
}
```

### 3. 规则脚本

保持简单，复杂逻辑使用 Wasm 插件：

```rhai
// ✅ 好的做法
if payload.temperature > 30.0 {
    return true;
}

// ❌ 避免复杂逻辑
// 复杂的数据处理应该在 Wasm 插件中完成
```

### 4. 错误处理

始终检查 HTTP 状态码和响应：

```javascript
try {
  const response = await fetch(url, options);
  if (!response.ok) {
    const error = await response.json();
    console.error('API Error:', error.error);
  }
} catch (error) {
  console.error('Network Error:', error);
}
```

---

## 更新日志

### v1.0.0 (2026-02-11)

- ✅ 初始版本发布
- ✅ 事件发布 API
- ✅ 规则管理 API
- ✅ 健康检查端点

### 未来计划

- [ ] 认证和授权
- [ ] 分页和过滤
- [ ] WebSocket 实时推送
- [ ] GraphQL 支持
- [ ] API 版本控制

---

## 支持

如有问题或建议，请：
- 提交 Issue: https://github.com/yourusername/flux-iot/issues
- 查看文档: [README](../README.md)
- 联系邮箱: your.email@example.com
