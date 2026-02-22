# flux-device-api

设备管理 REST API 服务

## 功能特性

- ✅ 设备注册/查询/更新/删除
- ✅ 设备监控（心跳、状态、指标）
- ✅ 设备分组管理
- ✅ RESTful API 设计
- ✅ 完整的错误处理
- ✅ CORS 支持
- ✅ 请求追踪

## API 端点

### 设备管理

```
POST   /api/v1/devices              # 注册设备
GET    /api/v1/devices              # 列出设备
GET    /api/v1/devices/:id          # 获取设备
PUT    /api/v1/devices/:id          # 更新设备
DELETE /api/v1/devices/:id          # 删除设备
GET    /api/v1/devices/stats        # 获取统计
```

### 设备监控

```
POST   /api/v1/devices/:id/heartbeat    # 设备心跳
GET    /api/v1/devices/:id/status       # 获取状态
GET    /api/v1/devices/:id/online       # 检查在线
POST   /api/v1/devices/:id/metrics      # 记录指标
GET    /api/v1/devices/:id/metrics      # 获取指标
```

### 设备分组

```
POST   /api/v1/groups                        # 创建分组
GET    /api/v1/groups                        # 列出分组
GET    /api/v1/groups/:id                    # 获取分组
DELETE /api/v1/groups/:id                    # 删除分组
GET    /api/v1/groups/:id/children           # 获取子分组
GET    /api/v1/groups/:id/devices            # 获取分组设备
POST   /api/v1/groups/:id/devices/:device_id # 添加设备到分组
DELETE /api/v1/groups/:id/devices/:device_id # 从分组移除设备
```

## 使用示例

### 注册设备

```bash
curl -X POST http://localhost:8080/api/v1/devices \
  -H "Content-Type: application/json" \
  -d '{
    "name": "温度传感器01",
    "device_type": "Sensor",
    "protocol": "MQTT",
    "tags": ["temperature", "indoor"]
  }'
```

### 查询设备

```bash
curl http://localhost:8080/api/v1/devices?device_type=Sensor&status=Online
```

### 设备心跳

```bash
curl -X POST http://localhost:8080/api/v1/devices/dev_123/heartbeat
```

### 记录指标

```bash
curl -X POST http://localhost:8080/api/v1/devices/dev_123/metrics \
  -H "Content-Type: application/json" \
  -d '{
    "metric_name": "temperature",
    "metric_value": 25.5,
    "unit": "°C"
  }'
```

## 集成到应用

```rust
use flux_device_api::{create_router, AppState};
use flux_device::DeviceManager;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    // 创建设备管理器
    let device_manager = Arc::new(DeviceManager::new(db, 30, 60));
    
    // 创建 API 状态
    let state = AppState::new(device_manager);
    
    // 创建路由
    let app = create_router(state);
    
    // 启动服务器
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```

## 错误处理

API 返回标准的 HTTP 状态码和 JSON 错误响应：

```json
{
  "error": "Device not found: dev_123",
  "status": 404
}
```

状态码：
- `200 OK` - 成功
- `201 Created` - 创建成功
- `204 No Content` - 删除成功
- `400 Bad Request` - 请求错误
- `404 Not Found` - 资源未找到
- `409 Conflict` - 资源冲突
- `500 Internal Server Error` - 服务器错误
