# FLUX IOT - OPC UA 客户端

真实的 OPC UA 客户端实现，支持连接工业设备和 OPC UA 服务器。

---

## 🚀 快速开始

### 1. 启动测试服务器

```bash
docker run -d -p 4840:4840 --name flux-opcua-test open62541/open62541
```

### 2. 运行测试示例

```bash
cargo run -p flux-opcua --example test_real_opcua
```

### 3. 查看输出

```
✅ 连接成功
✅ 读取到真实数据
{
  "value": "2026-02-23T06:24:49.198357+00:00",
  "status": "Good"
}
```

---

## 💻 使用示例

```rust
use flux_opcua::{OpcUaClientReal, OpcUaConfig};

fn main() -> anyhow::Result<()> {
    // 配置
    let config = OpcUaConfig {
        endpoint_url: "opc.tcp://localhost:4840".to_string(),
        ..Default::default()
    };

    // 连接
    let mut client = OpcUaClientReal::new(config);
    client.connect()?;

    // 读取
    let value = client.read_value("ns=0;i=2258")?;
    println!("{}", serde_json::to_string_pretty(&value)?);

    // 写入
    client.write_value("ns=2;s=TestValue", serde_json::json!(42))?;

    // 断开
    client.disconnect()?;

    Ok(())
}
```

---

## 📚 文档

- [真实实现报告](docs/OPCUA_REAL_IMPLEMENTATION.md)
- [实现指南](docs/OPCUA_IMPLEMENTATION_GUIDE.md)
- [测试环境搭建](docs/OPCUA_TEST_SETUP.md)

---

## ✅ 功能特性

- ✅ 真实的 OPC UA 连接
- ✅ 读取节点值
- ✅ 写入节点值
- ✅ 完整的数据类型转换
- ✅ 生产环境就绪

---

**立即开始使用真实的 OPC UA 客户端！** 🎉
