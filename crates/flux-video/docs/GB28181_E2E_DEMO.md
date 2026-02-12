# GB28181 端到端 Demo（M2 整体验收）

本演示覆盖：**SIP 注册 → INVITE → RTP/PS 入库 → HTTP 回放/截图**。

## 1. 构建

```bash
cd /Volumes/fushilu/workspace/flux-iot/crates/flux-video
cargo build --examples
```

## 2. 启动 Demo 服务

```bash
cargo run --example gb28181_e2e_demo
```

- SIP 监听：`0.0.0.0:5060`
- RTP 监听：`0.0.0.0:9000`
- HTTP API：`0.0.0.0:8081`

## 2.1 使用 GB28181 模拟设备（可选）

若没有真实设备，可启动模拟设备（REGISTER + INVITE 응答 + RTP 推流）：

```bash
cargo run --example gb28181_mock_device -- \
  --device-id 34020000001320000001 \
  --domain 3402000000 \
  --local-ip 127.0.0.1 \
  --local-port 5062 \
  --server-ip 127.0.0.1 \
  --server-port 5060 \
  --ssrc 3402000001
```

说明：
- `--ssrc` 必须与 INVITE 中填写的 `ssrc` 一致
- 模拟设备会在收到 INVITE 后，解析 SDP 中的 RTP 地址并推流

## 3. 接入 GB28181 设备或模拟器

请将设备注册到：
- SIP 服务器地址：`<本机IP>:5060`
- 平台 ID：`34020000002000000001`
- 域：`3402000000`

设备注册成功后，可在服务日志中看到 REGISTER / MESSAGE 等信息。

## 4. 发起 INVITE（开始点播）

执行：

```bash
curl -X POST http://localhost:8081/api/gb28181/invite \
  -H 'Content-Type: application/json' \
  -d '{
    "device_id": "34020000001320000001",
    "channel_id": "34020000001320000001",
    "ssrc": 3402000001,
    "rtp_port": 9000,
    "stream_id": "gb_cam_001"
  }'
```

> 说明：
> - `ssrc` 需要与设备发送的 RTP SSRC 一致。
> - `rtp_port` 必须与设备发送 RTP 的目标端口一致。

## 5. 获取快照

```bash
curl -o snapshot.h264 http://localhost:8081/api/gb28181/streams/gb_cam_001/snapshot
```

快照文件保存为 `snapshot.h264`。

## 6. 存储路径

```
./demo_data/gb28181/storage/<stream_id>/...
./demo_data/gb28181/keyframes/<stream_id>/...
```

## 7. 验收清单

- [ ] SIP REGISTER 成功
- [ ] INVITE 成功返回 Call-ID
- [ ] RTP/PS 数据入库
- [ ] Keyframe 提取成功（snapshot 接口可读）
- [ ] HTTP /health 返回 OK

## 8. 常见问题

### 8.1 无法接收到 RTP
- 检查 `ssrc` 是否匹配
- 检查设备 RTP 目的端口是否为 `9000`
- 检查防火墙/端口占用

### 8.2 snapshot 返回 No keyframe
- 等待 3~5 秒后重试（默认每 2 秒提取）
- 确认设备推流为 H.264
