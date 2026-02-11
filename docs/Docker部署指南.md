# FLUX IOT Platform - Docker 部署指南

## 📋 目录

1. [系统要求](#系统要求)
2. [快速开始](#快速开始)
3. [架构说明](#架构说明)
4. [配置说明](#配置说明)
5. [部署步骤](#部署步骤)
6. [服务管理](#服务管理)
7. [监控和日志](#监控和日志)
8. [数据备份与恢复](#数据备份与恢复)
9. [安全加固](#安全加固)
10. [故障排查](#故障排查)
11. [性能优化](#性能优化)

---

## 系统要求

### 硬件要求

| 环境 | CPU | 内存 | 磁盘 | 网络 |
|------|-----|------|------|------|
| **开发环境** | 2 核 | 4 GB | 10 GB | 100 Mbps |
| **生产环境（最小）** | 4 核 | 8 GB | 50 GB | 1 Gbps |
| **生产环境（推荐）** | 8 核 | 16 GB | 100 GB | 10 Gbps |

### 软件要求

- **Docker**: 20.10+ 
- **Docker Compose**: 2.0+
- **操作系统**: Linux (Ubuntu 20.04+, CentOS 8+, Debian 11+) / macOS / Windows (WSL2)

### 端口要求

| 端口 | 服务 | 协议 | 说明 |
|------|------|------|------|
| **80** | Nginx | HTTP | Web 访问入口 |
| **443** | Nginx | HTTPS | 加密 Web 访问（可选） |
| **1883** | MQTT | TCP | MQTT Broker |
| **8883** | MQTT | TCP/TLS | MQTT over TLS |
| **5432** | PostgreSQL | TCP | 数据库（内部） |
| **9090** | Prometheus | HTTP | 监控指标（内部） |
| **3001** | Grafana | HTTP | 可视化面板（通过 Nginx） |

---

## 快速开始

### 一键部署

```bash
# 1. 克隆项目
git clone https://github.com/your-org/flux-iot.git
cd flux-iot

# 2. 启动服务
./scripts/docker-start.sh

# 3. 访问服务
# - API: http://localhost/api/v1
# - Grafana: http://localhost/grafana (admin/admin)
# - Prometheus: http://localhost/prometheus
```

### 验证部署

```bash
# 健康检查
curl http://localhost/health

# 预期输出
{"status":"ok","timestamp":"2026-02-11T09:00:00Z"}

# 查看服务状态
docker-compose ps

# 预期输出
NAME                COMMAND                  SERVICE             STATUS              PORTS
flux-grafana        "/run.sh"                grafana             running             0.0.0.0:3001->3000/tcp
flux-iot            "flux-server"            flux-iot            running (healthy)   
flux-nginx          "/docker-entrypoint.…"   nginx               running             0.0.0.0:80->80/tcp, 0.0.0.0:1883->1883/tcp
flux-postgres       "docker-entrypoint.s…"   postgres            running (healthy)   5432/tcp
flux-prometheus     "/bin/prometheus --c…"   prometheus          running             9090/tcp
```

---

## 架构说明

### 系统架构图

```
┌─────────────────────────────────────────────────────────────┐
│                         Internet                            │
└────────────────────────┬────────────────────────────────────┘
                         │
                    ┌────▼────┐
                    │  Nginx  │ (反向代理 + 负载均衡)
                    │  :80    │
                    └────┬────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
   ┌────▼────┐      ┌────▼────┐     ┌────▼────┐
   │ FLUX    │      │Promethe-│     │ Grafana │
   │ IOT     │◄─────┤  us     │◄────┤  :3000  │
   │ :3000   │      │ :9090   │     └─────────┘
   └────┬────┘      └─────────┘
        │
        │ MQTT :1883/8883
        │
   ┌────▼────────┐
   │ PostgreSQL  │
   │   :5432     │
   └─────────────┘
```

### 容器说明

| 容器 | 镜像 | 作用 | 资源限制 |
|------|------|------|---------|
| **flux-iot** | 自定义构建 | 主应用服务 | 2 CPU / 1GB RAM |
| **postgres** | postgres:16-alpine | 数据库 | 1 CPU / 512MB RAM |
| **nginx** | nginx:1.25-alpine | 反向代理 | 1 CPU / 256MB RAM |
| **prometheus** | prom/prometheus | 监控采集 | 1 CPU / 512MB RAM |
| **grafana** | grafana/grafana | 可视化 | 1 CPU / 512MB RAM |
| **alertmanager** | prom/alertmanager | 告警管理 | 0.5 CPU / 256MB RAM |

### 网络拓扑

```
flux-frontend (bridge)
├── nginx
├── grafana
└── prometheus

flux-backend (bridge, internal)
├── flux-iot
├── postgres
├── prometheus
└── alertmanager
```

---

## 配置说明

### 环境变量配置

创建 `.env` 文件（参考 `.env.example`）：

```bash
# PostgreSQL 配置
POSTGRES_DB=flux_iot
POSTGRES_USER=flux
POSTGRES_PASSWORD=your_strong_password_here  # ⚠️ 必须修改

# Grafana 配置
GRAFANA_ADMIN_USER=admin
GRAFANA_ADMIN_PASSWORD=your_admin_password  # ⚠️ 必须修改

# 应用日志级别
RUST_LOG=info  # trace, debug, info, warn, error
```

### 应用配置

编辑 `config.toml`：

```toml
[server]
host = "0.0.0.0"
port = 3000

[database]
# Docker 环境使用环境变量
# url = "postgres://flux:password@postgres:5432/flux_iot"

[plugins]
directory = "/app/plugins"

[mqtt]
port = 1883
workers = 4
enable_tls = true
tls_cert_path = "/app/certs/server-cert.pem"
tls_key_path = "/app/certs/server-key.pem"

[eventbus]
capacity = 1024

[logging]
level = "info"
```

### Nginx 配置

主配置文件：`nginx/nginx.conf`  
站点配置：`nginx/conf.d/flux-iot.conf`

**关键配置**：
- 限流：100 req/s
- 连接限制：10 并发/IP
- 缓存：100MB
- Gzip 压缩：已启用
- MQTT TCP 代理：1883, 8883

---

## 部署步骤

### 1. 准备工作

```bash
# 克隆项目
git clone https://github.com/your-org/flux-iot.git
cd flux-iot

# 检查 Docker 环境
docker --version
docker-compose --version

# 创建必要目录
mkdir -p data plugins certs logs
```

### 2. 生成 TLS 证书

```bash
# 方式 1: 使用脚本自动生成（开发环境）
./scripts/docker-start.sh  # 会自动生成自签名证书

# 方式 2: 手动生成
cd certs

# 生成 CA
openssl req -x509 -newkey rsa:4096 -days 365 -nodes \
  -keyout ca-key.pem -out ca-cert.pem \
  -subj "/CN=FLUX IOT CA"

# 生成服务器证书
openssl genrsa -out server-key.pem 4096
openssl req -new -key server-key.pem -out server-csr.pem \
  -subj "/CN=localhost"
openssl x509 -req -in server-csr.pem -days 365 \
  -CA ca-cert.pem -CAkey ca-key.pem -CAcreateserial \
  -out server-cert.pem

# 清理
rm server-csr.pem ca-cert.srl
cd ..
```

### 3. 配置环境变量

```bash
# 复制示例配置
cp .env.example .env

# 编辑配置（⚠️ 务必修改密码）
vim .env
```

### 4. 构建和启动

```bash
# 方式 1: 使用脚本（推荐）
./scripts/docker-start.sh

# 方式 2: 手动启动
docker-compose build
docker-compose up -d

# 查看启动日志
docker-compose logs -f
```

### 5. 验证部署

```bash
# 健康检查
curl http://localhost/health

# 测试 API
curl http://localhost/api/v1/rules

# 测试 MQTT
mosquitto_pub -h localhost -p 1883 \
  -t "sensors/temp" -m '{"value": 25.5}'

# 访问 Grafana
open http://localhost/grafana
```

---

## 服务管理

### 启动服务

```bash
# 启动所有服务
docker-compose up -d

# 启动指定服务
docker-compose up -d flux-iot postgres
```

### 停止服务

```bash
# 停止所有服务
./scripts/docker-stop.sh

# 或手动停止
docker-compose down

# 停止并删除数据卷（⚠️ 数据会丢失）
docker-compose down -v
```

### 重启服务

```bash
# 重启所有服务
docker-compose restart

# 重启指定服务
docker-compose restart flux-iot
```

### 查看状态

```bash
# 查看容器状态
docker-compose ps

# 查看资源使用
docker stats

# 查看网络
docker network ls
docker network inspect flux-iot_flux-backend
```

### 进入容器

```bash
# 进入 FLUX IOT 容器
docker-compose exec flux-iot sh

# 进入 PostgreSQL 容器
docker-compose exec postgres psql -U flux flux_iot

# 进入 Nginx 容器
docker-compose exec nginx sh
```

---

## 监控和日志

### 查看日志

```bash
# 查看所有服务日志
docker-compose logs -f

# 查看指定服务日志
./scripts/docker-logs.sh flux-iot 100

# 查看 Nginx 访问日志
docker-compose exec nginx tail -f /var/log/nginx/access.log

# 查看 PostgreSQL 日志
docker-compose logs postgres
```

### Prometheus 监控

访问 Prometheus：`http://localhost/prometheus`

**常用查询**：

```promql
# HTTP 请求速率
rate(http_requests_total[5m])

# 内存使用率
container_memory_usage_bytes{name="flux-iot"} / container_spec_memory_limit_bytes{name="flux-iot"}

# CPU 使用率
rate(container_cpu_usage_seconds_total{name="flux-iot"}[5m])

# MQTT 消息速率
rate(mqtt_messages_received_total[5m])
```

### Grafana 仪表板

访问 Grafana：`http://localhost/grafana`

**默认凭证**：admin / admin

**预置仪表板**：
1. FLUX IOT 系统概览
2. MQTT 消息监控
3. Wasm 插件性能
4. 数据库性能

### 告警配置

编辑 `prometheus/alerts.yml` 添加自定义告警规则。

**示例告警**：
- 服务宕机
- 内存使用超过 85%
- CPU 使用超过 80%
- 磁盘空间不足
- MQTT 连接异常

---

## 数据备份与恢复

### 备份数据

```bash
# 完整备份（数据库 + 插件 + 配置）
./scripts/docker-backup.sh

# 备份文件位置
ls -lh backups/flux-iot-backup-*.tar.gz
```

### 恢复数据

```bash
# 从备份恢复
./scripts/docker-restore.sh backups/flux-iot-backup-20260211_150000.tar.gz

# 重启服务
docker-compose restart
```

### 定期备份（Cron）

```bash
# 编辑 crontab
crontab -e

# 添加每日备份任务（凌晨 2 点）
0 2 * * * cd /path/to/flux-iot && ./scripts/docker-backup.sh >> /var/log/flux-backup.log 2>&1

# 添加备份清理任务（保留 30 天）
0 3 * * * find /path/to/flux-iot/backups -name "*.tar.gz" -mtime +30 -delete
```

---

## 安全加固

### 1. 修改默认密码

```bash
# 修改 .env 文件
POSTGRES_PASSWORD=your_strong_password_here
GRAFANA_ADMIN_PASSWORD=your_admin_password
```

### 2. 启用 HTTPS

编辑 `nginx/conf.d/flux-iot.conf`，取消 HTTPS 配置注释：

```nginx
server {
    listen 443 ssl http2;
    server_name your-domain.com;
    
    ssl_certificate /etc/nginx/certs/server-cert.pem;
    ssl_certificate_key /etc/nginx/certs/server-key.pem;
    # ... 其他配置
}
```

### 3. 限制访问

```nginx
# 限制 Prometheus 访问
location /prometheus/ {
    allow 10.0.0.0/8;
    deny all;
    proxy_pass http://prometheus/;
}
```

### 4. 防火墙配置

```bash
# Ubuntu/Debian
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw allow 1883/tcp
sudo ufw allow 8883/tcp
sudo ufw enable

# CentOS/RHEL
sudo firewall-cmd --permanent --add-port=80/tcp
sudo firewall-cmd --permanent --add-port=443/tcp
sudo firewall-cmd --permanent --add-port=1883/tcp
sudo firewall-cmd --permanent --add-port=8883/tcp
sudo firewall-cmd --reload
```

### 5. 定期更新

```bash
# 更新镜像
docker-compose pull

# 重新构建
docker-compose build --no-cache

# 重启服务
docker-compose up -d
```

---

## 故障排查

### 服务无法启动

```bash
# 1. 查看日志
docker-compose logs flux-iot

# 2. 检查端口占用
sudo netstat -tlnp | grep -E '(80|1883|5432)'

# 3. 检查磁盘空间
df -h

# 4. 检查内存
free -h
```

### 数据库连接失败

```bash
# 1. 检查 PostgreSQL 状态
docker-compose ps postgres

# 2. 测试连接
docker-compose exec postgres psql -U flux -d flux_iot -c "SELECT 1;"

# 3. 查看数据库日志
docker-compose logs postgres

# 4. 重启数据库
docker-compose restart postgres
```

### MQTT 连接问题

```bash
# 1. 测试 MQTT 连接
mosquitto_sub -h localhost -p 1883 -t '#' -v

# 2. 检查证书（TLS）
openssl s_client -connect localhost:8883 -CAfile certs/ca-cert.pem

# 3. 查看 Nginx MQTT 日志
docker-compose exec nginx tail -f /var/log/nginx/mqtt_access.log
```

### 性能问题

```bash
# 1. 查看资源使用
docker stats

# 2. 查看慢查询（PostgreSQL）
docker-compose exec postgres psql -U flux -d flux_iot -c "
SELECT query, calls, total_time, mean_time 
FROM pg_stat_statements 
ORDER BY mean_time DESC 
LIMIT 10;"

# 3. 检查 Wasm 插件性能
curl http://localhost/metrics | grep wasm
```

---

## 性能优化

### 1. 数据库优化

```sql
-- 进入 PostgreSQL
docker-compose exec postgres psql -U flux flux_iot

-- 创建索引
CREATE INDEX idx_events_timestamp ON events(timestamp);
CREATE INDEX idx_rules_enabled ON rules(enabled);

-- 分析表
ANALYZE;

-- 清理
VACUUM ANALYZE;
```

### 2. Nginx 缓存优化

编辑 `nginx/nginx.conf`：

```nginx
# 增加缓存大小
proxy_cache_path /var/cache/nginx levels=1:2 
                 keys_zone=flux_cache:100m 
                 max_size=1g inactive=60m;
```

### 3. 资源限制调整

编辑 `docker-compose.yml`：

```yaml
services:
  flux-iot:
    deploy:
      resources:
        limits:
          cpus: '4'      # 增加 CPU
          memory: 2G     # 增加内存
```

### 4. PostgreSQL 调优

```bash
# 编辑 PostgreSQL 配置
docker-compose exec postgres sh -c "cat >> /var/lib/postgresql/data/postgresql.conf << EOF
shared_buffers = 256MB
effective_cache_size = 1GB
maintenance_work_mem = 64MB
checkpoint_completion_target = 0.9
wal_buffers = 16MB
default_statistics_target = 100
random_page_cost = 1.1
effective_io_concurrency = 200
work_mem = 4MB
min_wal_size = 1GB
max_wal_size = 4GB
EOF"

# 重启 PostgreSQL
docker-compose restart postgres
```

---

## 附录

### A. 常用命令速查

```bash
# 启动
./scripts/docker-start.sh

# 停止
./scripts/docker-stop.sh

# 查看日志
./scripts/docker-logs.sh <service> <lines>

# 备份
./scripts/docker-backup.sh

# 恢复
./scripts/docker-restore.sh <backup-file>

# 重启
docker-compose restart

# 重建
docker-compose up -d --build

# 清理
docker system prune -a
```

### B. 端口映射表

| 主机端口 | 容器端口 | 服务 |
|---------|---------|------|
| 80 | 80 | Nginx HTTP |
| 443 | 443 | Nginx HTTPS |
| 1883 | 1883 | MQTT |
| 8883 | 8883 | MQTT TLS |
| 3001 | 3000 | Grafana |

### C. 数据卷说明

| 卷名 | 挂载点 | 用途 |
|------|--------|------|
| postgres-data | /var/lib/postgresql/data | 数据库数据 |
| flux-plugins | /app/plugins | Wasm 插件 |
| flux-logs | /app/logs | 应用日志 |
| prometheus-data | /prometheus | 监控数据 |
| grafana-data | /var/lib/grafana | 仪表板配置 |

---

**文档版本**: 1.0  
**最后更新**: 2026-02-11  
**维护者**: FLUX IOT Team
