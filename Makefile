# FLUX IOT Platform - Makefile
# 提供统一的测试、构建、部署命令

.PHONY: help test test-unit test-integration test-e2e test-all coverage fmt lint clean build run docker-build docker-up docker-down

# 默认目标
help:
	@echo "FLUX IOT Platform - Available Commands:"
	@echo ""
	@echo "Testing:"
	@echo "  make test              - Run all tests"
	@echo "  make test-unit         - Run unit tests only"
	@echo "  make test-integration  - Run integration tests only"
	@echo "  make test-e2e          - Run end-to-end tests"
	@echo "  make coverage          - Generate test coverage report"
	@echo ""
	@echo "Code Quality:"
	@echo "  make fmt               - Format code with rustfmt"
	@echo "  make lint              - Run clippy linter"
	@echo "  make audit             - Security audit"
	@echo ""
	@echo "Build & Run:"
	@echo "  make build             - Build all binaries"
	@echo "  make build-release     - Build release binaries"
	@echo "  make run               - Run flux-server"
	@echo "  make clean             - Clean build artifacts"
	@echo ""
	@echo "Docker:"
	@echo "  make docker-build      - Build Docker images"
	@echo "  make docker-up         - Start all services"
	@echo "  make docker-down       - Stop all services"
	@echo "  make docker-logs       - View logs"

# ============================================================================
# 测试命令
# ============================================================================

# 运行所有测试
test: test-unit test-integration

# 单元测试（仅 lib）
test-unit:
	@echo "Running unit tests..."
	DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test \
		cargo test --lib --all-features

# 集成测试（tests/ 目录）
test-integration:
	@echo "Running integration tests..."
	DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test \
		cargo test --test '*' --all-features

# 端到端测试
test-e2e:
	@echo "Running E2E tests..."
	cargo test --test e2e_scenarios --all-features

# 运行所有测试（包括 doc tests）
test-all:
	@echo "Running all tests..."
	cargo test --all-features

# 特定包测试
test-server:
	DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test \
		cargo test -p flux-server --all-features

test-mqtt:
	DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test \
		cargo test -p flux-mqtt --all-features

test-video:
	DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test \
		cargo test -p flux-video --all-features

test-gb28181:
	DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test \
		cargo test -p flux-gb28181d --all-features

test-device:
	DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test \
		cargo test -p flux-device --all-features

test-storage:
	DATABASE_URL=postgres://postgres:postgres@localhost:5432/flux_test \
		cargo test -p flux-storage --all-features

# 测试覆盖率
coverage:
	@echo "Generating test coverage report..."
	cargo tarpaulin --out Html --output-dir coverage --all-features
	@echo "Coverage report generated in coverage/index.html"

# 覆盖率（仅核心模块）
coverage-core:
	cargo tarpaulin --packages flux-server flux-mqtt flux-video flux-storage \
		--out Html --output-dir coverage

# ============================================================================
# 代码质量
# ============================================================================

# 格式化代码
fmt:
	@echo "Formatting code..."
	cargo fmt --all

# 检查格式
fmt-check:
	cargo fmt --all -- --check

# Clippy 检查
lint:
	@echo "Running clippy..."
	cargo clippy --all-targets --all-features -- -D warnings

# 安全审计
audit:
	@echo "Running security audit..."
	cargo audit

# 检查过期依赖
outdated:
	cargo outdated

# ============================================================================
# 构建
# ============================================================================

# 开发构建
build:
	@echo "Building debug binaries..."
	cargo build --all-features

# 发布构建
build-release:
	@echo "Building release binaries..."
	cargo build --release --all-features

# 构建特定二进制
build-server:
	cargo build -p flux-server --release

build-gb28181d:
	cargo build -p flux-gb28181d --release

build-rtmpd:
	cargo build -p flux-rtmpd --release

build-rtspd:
	cargo build -p flux-rtspd --release

# 清理构建产物
clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	rm -rf coverage/

# ============================================================================
# 运行
# ============================================================================

# 运行主服务器
run:
	cargo run -p flux-server -- --config config.toml

# 运行 GB28181 服务
run-gb28181d:
	cargo run -p flux-gb28181d

# 运行 RTMP 服务
run-rtmpd:
	cargo run -p flux-rtmpd

# 运行 RTSP 服务
run-rtspd:
	cargo run -p flux-rtspd

# ============================================================================
# Docker
# ============================================================================

# 构建 Docker 镜像
docker-build:
	@echo "Building Docker images..."
	docker-compose build

# 启动所有服务
docker-up:
	@echo "Starting services..."
	docker-compose up -d

# 停止所有服务
docker-down:
	@echo "Stopping services..."
	docker-compose down

# 查看日志
docker-logs:
	docker-compose logs -f

# 重启服务
docker-restart:
	docker-compose restart

# ============================================================================
# 开发工具
# ============================================================================

# 安装开发工具
install-tools:
	@echo "Installing development tools..."
	cargo install cargo-tarpaulin
	cargo install cargo-audit
	cargo install cargo-outdated
	cargo install cargo-watch
	cargo install cargo-flamegraph

# 监听文件变化自动测试
watch:
	cargo watch -x test

# 监听文件变化自动运行
watch-run:
	cargo watch -x 'run -p flux-server'

# ============================================================================
# 数据库
# ============================================================================

# 初始化数据库
db-init:
	@echo "Initializing database..."
	psql -U postgres -c "CREATE DATABASE flux_iot;"

# 运行迁移
db-migrate:
	@echo "Running migrations..."
	# TODO: 添加迁移工具

# 重置数据库
db-reset:
	@echo "Resetting database..."
	psql -U postgres -c "DROP DATABASE IF EXISTS flux_iot;"
	psql -U postgres -c "CREATE DATABASE flux_iot;"

# ============================================================================
# 性能测试
# ============================================================================

# 运行基准测试
bench:
	cargo bench --all-features

# 性能分析（需要 perf）
profile:
	cargo flamegraph --bin flux-server

# 压力测试（需要 wrk）
stress-test:
	@echo "Running stress test..."
	wrk -t4 -c100 -d30s http://localhost:8080/health

# ============================================================================
# 文档
# ============================================================================

# 生成文档
doc:
	cargo doc --all-features --no-deps --open

# 生成私有项文档
doc-private:
	cargo doc --all-features --document-private-items --no-deps --open

# ============================================================================
# 发布
# ============================================================================

# 检查发布准备
pre-release:
	make fmt-check
	make lint
	make test-all
	make audit

# 打包发布
package:
	cargo package --allow-dirty
