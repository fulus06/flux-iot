# FLUX IOT 部署和运维指南

本指南介绍如何在生产环境中部署和运维 FLUX IOT 平台。

---

## 📋 目录

- [系统要求](#系统要求)
- [部署方式](#部署方式)
- [配置管理](#配置管理)
- [数据库设置](#数据库设置)
- [监控和日志](#监控和日志)
- [性能优化](#性能优化)
- [安全加固](#安全加固)
- [备份和恢复](#备份和恢复)
- [故障排查](#故障排查)

---

## 系统要求

### 最低配置

| 组件 | 要求 |
|------|------|
| CPU | 2 核 |
| 内存 | 2GB |
| 磁盘 | 10GB |
| 操作系统 | Linux (Ubuntu 20.04+, CentOS 8+) |
| Rust | 1.75+ |

### 推荐配置

| 组件 | 要求 |
|------|------|
| CPU | 4 核+ |
| 内存 | 8GB+ |
| 磁盘 | 50GB+ SSD |
| 操作系统 | Linux (Ubuntu 22.04 LTS) |
| 数据库 | PostgreSQL 14+ |

### 网络要求

- HTTP 端口: 3000 (可配置)
- MQTT 端口: 1883 (可配置)
- 数据库端口: 5432 (PostgreSQL) 或 3306 (MySQL)

---

## 部署方式

### 方式 1: 二进制部署（推荐）

#### 1. 编译 Release 版本

```bash
# 克隆仓库
git clone https://github.com/yourusername/flux-iot.git
cd flux-iot

# 编译 Release 版本
cargo build --release

# 编译插件
cargo build --target wasm32-unknown-unknown --release \
  --manifest-path plugins/dummy_plugin/Cargo.toml
```

#### 2. 准备部署目录

```bash
# 创建部署目录
sudo mkdir -p /opt/flux-iot
sudo mkdir -p /opt/flux-iot/plugins
sudo mkdir -p /opt/flux-iot/data
sudo mkdir -p /var/log/flux-iot

# 复制文件
sudo cp target/release/flux-server /opt/flux-iot/
sudo cp target/wasm32-unknown-unknown/release/*.wasm /opt/flux-iot/plugins/
sudo cp config.toml /opt/flux-iot/
```

#### 3. 创建配置文件

```bash
sudo nano /opt/flux-iot/config.toml
```

```toml
[server]
host = "0.0.0.0"
port = 3000

[database]
url = "sqlite:///opt/flux-iot/data/flux.db"
# 或使用 PostgreSQL
# url = "postgres://flux:password@localhost/flux_iot"

[plugins]
directory = "/opt/flux-iot/plugins"
```

#### 4. 创建 systemd 服务

```bash
sudo nano /etc/systemd/system/flux-iot.service
```

```ini
[Unit]
Description=FLUX IOT Platform
After=network.target

[Service]
Type=simple
User=flux-iot
Group=flux-iot
WorkingDirectory=/opt/flux-iot
ExecStart=/opt/flux-iot/flux-server --config /opt/flux-iot/config.toml
Restart=always
RestartSec=10

# 环境变量
Environment="RUST_LOG=info,flux_server=debug"
Environment="RUST_BACKTRACE=1"

# 安全设置
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/flux-iot/data /var/log/flux-iot

# 资源限制
LimitNOFILE=65535
LimitNPROC=4096

[Install]
WantedBy=multi-user.target
```

#### 5. 创建用户和设置权限

```bash
# 创建用户
sudo useradd -r -s /bin/false flux-iot

# 设置权限
sudo chown -R flux-iot:flux-iot /opt/flux-iot
sudo chown -R flux-iot:flux-iot /var/log/flux-iot
sudo chmod 755 /opt/flux-iot/flux-server
```

#### 6. 启动服务

```bash
# 重载 systemd
sudo systemctl daemon-reload

# 启动服务
sudo systemctl start flux-iot

# 查看状态
sudo systemctl status flux-iot

# 设置开机自启
sudo systemctl enable flux-iot

# 查看日志
sudo journalctl -u flux-iot -f
```

---

### 方式 2: Docker 部署

#### 1. 创建 Dockerfile

```dockerfile
# Dockerfile
FROM rust:1.75 as builder

WORKDIR /app

# 复制源代码
COPY . .

# 编译 Release 版本
RUN cargo build --release

# 编译插件
RUN rustup target add wasm32-unknown-unknown && \
    cargo build --target wasm32-unknown-unknown --release \
      --manifest-path plugins/dummy_plugin/Cargo.toml

# 运行时镜像
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# 创建用户
RUN useradd -r -s /bin/false flux-iot

# 创建目录
RUN mkdir -p /opt/flux-iot/plugins /opt/flux-iot/data

# 复制二进制文件
COPY --from=builder /app/target/release/flux-server /opt/flux-iot/
COPY --from=builder /app/target/wasm32-unknown-unknown/release/*.wasm /opt/flux-iot/plugins/
COPY config.toml /opt/flux-iot/

# 设置权限
RUN chown -R flux-iot:flux-iot /opt/flux-iot

# 切换用户
USER flux-iot

# 工作目录
WORKDIR /opt/flux-iot

# 暴露端口
EXPOSE 3000 1883

# 环境变量
ENV RUST_LOG=info

# 启动命令
CMD ["/opt/flux-iot/flux-server", "--config", "/opt/flux-iot/config.toml"]
```

#### 2. 创建 docker-compose.yml

```yaml
version: '3.8'

services:
  flux-iot:
    build: .
    container_name: flux-iot
    ports:
      - "3000:3000"
      - "1883:1883"
    volumes:
      - ./data:/opt/flux-iot/data
      - ./plugins:/opt/flux-iot/plugins
      - ./config.toml:/opt/flux-iot/config.toml:ro
    environment:
      - RUST_LOG=info,flux_server=debug
    restart: unless-stopped
    networks:
      - flux-network

  # PostgreSQL (可选)
  postgres:
    image: postgres:14
    container_name: flux-postgres
    environment:
      POSTGRES_DB: flux_iot
      POSTGRES_USER: flux
      POSTGRES_PASSWORD: your_password_here
    volumes:
      - postgres_data:/var/lib/postgresql/data
    networks:
      - flux-network
    restart: unless-stopped

volumes:
  postgres_data:

networks:
  flux-network:
    driver: bridge
```

#### 3. 构建和运行

```bash
# 构建镜像
docker-compose build

# 启动服务
docker-compose up -d

# 查看日志
docker-compose logs -f flux-iot

# 停止服务
docker-compose down
```

---

## 配置管理

### 配置文件结构

```toml
# config.toml

[server]
host = "0.0.0.0"        # 监听地址
port = 3000             # HTTP 端口

[database]
url = "sqlite://flux.db"  # 数据库连接字符串
# PostgreSQL 示例:
# url = "postgres://user:password@localhost/flux_iot"

[plugins]
directory = "plugins"   # 插件目录
```

### 环境变量

可以通过环境变量覆盖配置：

```bash
export FLUX_SERVER_HOST="0.0.0.0"
export FLUX_SERVER_PORT="3000"
export FLUX_DATABASE_URL="postgres://localhost/flux_iot"
export FLUX_PLUGINS_DIR="/opt/flux-iot/plugins"
```

### 日志配置

```bash
# 日志级别
export RUST_LOG=info                    # 全局 info
export RUST_LOG=debug                   # 全局 debug
export RUST_LOG=flux_server=debug       # 只有 flux_server debug
export RUST_LOG=info,wasm_plugin=trace  # 组合配置
```

---

## 数据库设置

### SQLite (开发/小规模)

```toml
[database]
url = "sqlite://flux.db"
```

**优点**:
- 无需额外安装
- 配置简单
- 适合开发和测试

**缺点**:
- 并发性能有限
- 不适合大规模部署

### PostgreSQL (生产推荐)

#### 1. 安装 PostgreSQL

```bash
# Ubuntu/Debian
sudo apt-get install postgresql postgresql-contrib

# CentOS/RHEL
sudo yum install postgresql-server postgresql-contrib
```

#### 2. 创建数据库和用户

```sql
-- 连接到 PostgreSQL
sudo -u postgres psql

-- 创建用户
CREATE USER flux WITH PASSWORD 'your_secure_password';

-- 创建数据库
CREATE DATABASE flux_iot OWNER flux;

-- 授权
GRANT ALL PRIVILEGES ON DATABASE flux_iot TO flux;
```

#### 3. 配置连接

```toml
[database]
url = "postgres://flux:your_secure_password@localhost/flux_iot"
```

#### 4. 性能优化

编辑 `/etc/postgresql/14/main/postgresql.conf`:

```ini
# 连接设置
max_connections = 100

# 内存设置
shared_buffers = 256MB
effective_cache_size = 1GB
work_mem = 16MB

# 检查点设置
checkpoint_completion_target = 0.9
wal_buffers = 16MB

# 查询优化
random_page_cost = 1.1  # SSD
```

---

## 监控和日志

### 日志管理

#### 1. 使用 journalctl

```bash
# 查看实时日志
sudo journalctl -u flux-iot -f

# 查看最近 100 行
sudo journalctl -u flux-iot -n 100

# 查看特定时间范围
sudo journalctl -u flux-iot --since "2026-02-10" --until "2026-02-11"

# 导出日志
sudo journalctl -u flux-iot > flux-iot.log
```

#### 2. 日志轮转

创建 `/etc/logrotate.d/flux-iot`:

```
/var/log/flux-iot/*.log {
    daily
    rotate 7
    compress
    delaycompress
    missingok
    notifempty
    create 0640 flux-iot flux-iot
    sharedscripts
    postrotate
        systemctl reload flux-iot > /dev/null 2>&1 || true
    endscript
}
```

### 性能监控

#### 1. 系统资源

```bash
# CPU 和内存使用
top -p $(pgrep flux-server)

# 详细资源统计
htop

# 网络连接
netstat -tulpn | grep flux-server
```

#### 2. 应用指标

```bash
# 健康检查
curl http://localhost:3000/health

# 规则列表
curl http://localhost:3000/api/v1/rules
```

### Prometheus 集成（未来）

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'flux-iot'
    static_configs:
      - targets: ['localhost:9090']
```

---

## 性能优化

### 1. 系统级优化

```bash
# 增加文件描述符限制
sudo nano /etc/security/limits.conf
```

```
flux-iot soft nofile 65535
flux-iot hard nofile 65535
```

### 2. 网络优化

```bash
# 调整 TCP 参数
sudo sysctl -w net.core.somaxconn=4096
sudo sysctl -w net.ipv4.tcp_max_syn_backlog=4096
```

### 3. 应用优化

- 使用 Release 构建
- 启用 LTO (Link Time Optimization)
- 使用连接池
- 合理配置 EventBus 容量

---

## 安全加固

### 1. 防火墙配置

```bash
# UFW (Ubuntu)
sudo ufw allow 3000/tcp
sudo ufw allow 1883/tcp
sudo ufw enable

# firewalld (CentOS)
sudo firewall-cmd --permanent --add-port=3000/tcp
sudo firewall-cmd --permanent --add-port=1883/tcp
sudo firewall-cmd --reload
```

### 2. SSL/TLS 配置

使用 Nginx 作为反向代理：

```nginx
server {
    listen 443 ssl http2;
    server_name iot.example.com;

    ssl_certificate /etc/letsencrypt/live/iot.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/iot.example.com/privkey.pem;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### 3. 访问控制

- 使用强密码
- 限制数据库访问
- 定期更新依赖

---

## 备份和恢复

### SQLite 备份

```bash
# 备份
sqlite3 /opt/flux-iot/data/flux.db ".backup /backup/flux-$(date +%Y%m%d).db"

# 恢复
sqlite3 /opt/flux-iot/data/flux.db ".restore /backup/flux-20260210.db"
```

### PostgreSQL 备份

```bash
# 备份
pg_dump -U flux flux_iot > flux-$(date +%Y%m%d).sql

# 恢复
psql -U flux flux_iot < flux-20260210.sql
```

### 自动备份脚本

```bash
#!/bin/bash
# /opt/flux-iot/backup.sh

BACKUP_DIR="/backup/flux-iot"
DATE=$(date +%Y%m%d_%H%M%S)

mkdir -p $BACKUP_DIR

# 备份数据库
sqlite3 /opt/flux-iot/data/flux.db ".backup $BACKUP_DIR/flux-$DATE.db"

# 备份配置
cp /opt/flux-iot/config.toml $BACKUP_DIR/config-$DATE.toml

# 删除 7 天前的备份
find $BACKUP_DIR -name "flux-*.db" -mtime +7 -delete

echo "Backup completed: $BACKUP_DIR/flux-$DATE.db"
```

添加到 crontab:

```bash
# 每天凌晨 2 点备份
0 2 * * * /opt/flux-iot/backup.sh
```

---

## 故障排查

### 常见问题

#### 1. 服务无法启动

```bash
# 查看详细日志
sudo journalctl -u flux-iot -n 100 --no-pager

# 检查配置文件
/opt/flux-iot/flux-server --config /opt/flux-iot/config.toml

# 检查端口占用
sudo lsof -i:3000
sudo lsof -i:1883
```

#### 2. 数据库连接失败

```bash
# 测试数据库连接
psql -U flux -h localhost flux_iot

# 检查 PostgreSQL 状态
sudo systemctl status postgresql
```

#### 3. 插件加载失败

```bash
# 检查插件目录
ls -l /opt/flux-iot/plugins/

# 检查插件权限
sudo chmod 644 /opt/flux-iot/plugins/*.wasm

# 查看插件加载日志
sudo journalctl -u flux-iot | grep -i plugin
```

#### 4. 性能问题

```bash
# 检查 CPU 使用
top -p $(pgrep flux-server)

# 检查内存使用
ps aux | grep flux-server

# 检查数据库性能
# PostgreSQL
SELECT * FROM pg_stat_activity;
```

### 调试模式

```bash
# 启用详细日志
export RUST_LOG=trace
export RUST_BACKTRACE=full

# 重启服务
sudo systemctl restart flux-iot
```

---

## 升级指南

### 1. 备份数据

```bash
# 备份数据库
/opt/flux-iot/backup.sh

# 备份配置
cp /opt/flux-iot/config.toml /backup/config.toml.bak
```

### 2. 停止服务

```bash
sudo systemctl stop flux-iot
```

### 3. 更新二进制文件

```bash
# 下载新版本
cd /tmp
git clone https://github.com/yourusername/flux-iot.git
cd flux-iot
cargo build --release

# 替换二进制
sudo cp target/release/flux-server /opt/flux-iot/flux-server.new
sudo mv /opt/flux-iot/flux-server /opt/flux-iot/flux-server.old
sudo mv /opt/flux-iot/flux-server.new /opt/flux-iot/flux-server
```

### 4. 启动服务

```bash
sudo systemctl start flux-iot
sudo systemctl status flux-iot
```

### 5. 验证

```bash
curl http://localhost:3000/health
```

---

## 最佳实践

1. **定期备份**: 每天自动备份数据库
2. **监控告警**: 设置资源使用告警
3. **日志管理**: 定期清理旧日志
4. **安全更新**: 及时更新依赖和系统补丁
5. **容量规划**: 监控磁盘和内存使用趋势
6. **文档维护**: 记录配置变更和故障处理

---

## 支持

如有问题，请：
- 查看 [FAQ](../README.md#常见问题)
- 提交 Issue: https://github.com/yourusername/flux-iot/issues
- 联系邮箱: your.email@example.com
