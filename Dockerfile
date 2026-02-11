# ============================================
# FLUX IOT Platform - Multi-stage Dockerfile
# ============================================

# ============ Stage 1: Builder ============
FROM rust:1.75-slim AS builder

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 复制依赖清单（利用 Docker 缓存）
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY sdk ./sdk

# 构建 release 版本
RUN cargo build --release --bin flux-server

# 验证二进制文件
RUN ls -lh target/release/flux-server && \
    file target/release/flux-server

# ============ Stage 2: Runtime ============
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 创建非 root 用户
RUN useradd -m -u 1000 -s /bin/bash flux && \
    mkdir -p /app/data /app/plugins /app/config /app/certs /app/logs && \
    chown -R flux:flux /app

WORKDIR /app

# 从构建阶段复制二进制文件
COPY --from=builder /app/target/release/flux-server /usr/local/bin/flux-server
RUN chmod +x /usr/local/bin/flux-server

# 复制配置文件模板
COPY config.toml /app/config/config.toml.template

# 设置环境变量
ENV RUST_LOG=info \
    RUST_BACKTRACE=1 \
    DATABASE_URL=postgres://flux:flux@postgres:5432/flux_iot

# 切换到非 root 用户
USER flux

# 暴露端口
EXPOSE 3000 1883 8883 9090

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# 启动应用
CMD ["flux-server"]
