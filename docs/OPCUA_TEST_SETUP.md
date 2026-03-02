# OPC UA 测试环境搭建指南

> 日期: 2026-02-23
> 用途: 开发和测试 FLUX IOT 的 OPC UA 功能

---

## 🚀 快速开始

### 1. 启动 OPC UA 测试服务器

```bash
# 使用 Docker 启动 open62541 服务器
docker run -d -p 4840:4840 --name flux-opcua-test open62541/open62541:latest

# 验证服务器运行
docker ps | grep opcua
```

**预期输出**:
```
CONTAINER ID   IMAGE                        STATUS         PORTS
abc123def456   open62541/open62541:latest   Up 2 seconds   0.0.0.0:4840->4840/tcp
```

### 2. 测试连接

```bash
# 运行 FLUX IOT 测试示例
cd /Volumes/fushilu/workspace/flux-iot
cargo run -p flux-opcua --example test_connection
```

---

## 📋 测试服务器信息

### 服务器配置
- **端点 URL**: `opc.tcp://localhost:4840`
- **安全策略**: None（无加密）
- **认证**: Anonymous（匿名）
- **协议**: OPC UA Binary

### 可用节点示例

| 节点 ID | 描述 | 类型 | 权限 |
|---------|------|------|------|
| `ns=0;i=2258` | Server/ServerStatus/CurrentTime | DateTime | 只读 |
| `ns=0;i=2259` | Server/ServerStatus/State | Int32 | 只读 |
| `ns=0;i=2260` | Server/ServerStatus/BuildInfo | Object | 只读 |

---

## 🧪 测试场景

### 场景 1: 基础连接测试

```rust
use flux_opcua::{OpcUaClient, OpcUaConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = OpcUaConfig {
        endpoint_url: "opc.tcp://localhost:4840".to_string(),
        ..Default::default()
    };

    let mut client = OpcUaClient::new(config);
    client.connect().await?;
    
    println!("连接成功！");
    
    client.disconnect().await?;
    Ok(())
}
```

### 场景 2: 读取节点值

```rust
// 读取服务器当前时间
let value = client.read_value("ns=0;i=2258").await?;
println!("服务器时间: {:?}", value);
```

### 场景 3: 写入节点值

```rust
// 写入测试值
let test_value = serde_json::json!(42);
client.write_value("ns=2;s=TestValue", test_value).await?;
```

---

## 🔧 故障排除

### 问题 1: 连接失败

**错误**: `Connection refused`

**解决方案**:
```bash
# 检查服务器是否运行
docker ps | grep opcua

# 如果未运行，启动服务器
docker start flux-opcua-test

# 或重新创建
docker rm flux-opcua-test
docker run -d -p 4840:4840 --name flux-opcua-test open62541/open62541
```

### 问题 2: 端口被占用

**错误**: `port 4840 is already allocated`

**解决方案**:
```bash
# 查找占用端口的进程
lsof -i :4840

# 停止旧容器
docker stop flux-opcua-test
docker rm flux-opcua-test

# 或使用不同端口
docker run -d -p 4841:4840 --name flux-opcua-test open62541/open62541
# 然后修改配置: opc.tcp://localhost:4841
```

### 问题 3: 读取节点失败

**原因**: 当前为简化实现

**说明**:
- FLUX IOT 的 OPC UA 客户端提供了框架
- 完整功能需要参考 `docs/OPCUA_IMPLEMENTATION_GUIDE.md`
- 真实实现需要使用 `opcua` crate 的完整 API

---

## 📊 测试检查清单

运行以下命令验证环境：

```bash
# 1. 检查 Docker
docker --version

# 2. 启动服务器
docker run -d -p 4840:4840 --name flux-opcua-test open62541/open62541

# 3. 验证服务器
docker logs flux-opcua-test

# 4. 测试连接
cargo run -p flux-opcua --example test_connection

# 5. 查看日志
# 应该看到 "连接成功" 的消息
```

---

## 🎯 下一步

### 开发环境
✅ 使用 open62541 测试服务器
✅ 快速验证功能
✅ 无需复杂配置

### 生产环境
当需要连接真实设备时：
1. 参考 `docs/OPCUA_IMPLEMENTATION_GUIDE.md`
2. 配置真实服务器端点
3. 设置安全证书
4. 实现完整的数据类型转换

---

## 🛑 停止测试服务器

```bash
# 停止服务器
docker stop flux-opcua-test

# 删除容器
docker rm flux-opcua-test

# 或一次性操作
docker rm -f flux-opcua-test
```

---

## 📚 相关文档

- `docs/OPCUA_IMPLEMENTATION_GUIDE.md` - 完整实现指南
- `crates/flux-opcua/examples/test_connection.rs` - 测试示例
- [open62541 文档](https://open62541.org/)
- [OPC UA 规范](https://opcfoundation.org/)

---

**测试环境已就绪！** 🎉
