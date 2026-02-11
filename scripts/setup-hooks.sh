#!/bin/bash
# 设置 Git Hooks 脚本

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
HOOKS_DIR="$PROJECT_ROOT/.githooks"
GIT_HOOKS_DIR="$PROJECT_ROOT/.git/hooks"

echo "🔧 Setting up Git hooks for FLUX IOT..."

# 检查是否在 Git 仓库中
if [ ! -d "$PROJECT_ROOT/.git" ]; then
    echo "❌ Error: Not a Git repository"
    exit 1
fi

# 创建 .githooks 目录（如果不存在）
mkdir -p "$HOOKS_DIR"

# 配置 Git 使用自定义 hooks 目录
echo "📁 Configuring Git to use custom hooks directory..."
git config core.hooksPath "$HOOKS_DIR"

# 设置 hooks 可执行权限
echo "🔐 Setting executable permissions..."
chmod +x "$HOOKS_DIR/pre-commit"
chmod +x "$HOOKS_DIR/pre-push"

echo "✅ Git hooks setup complete!"
echo ""
echo "Installed hooks:"
echo "  - pre-commit: Format check, Clippy, Tests"
echo "  - pre-push: Full test suite, Release build"
echo ""
echo "💡 To skip hooks temporarily, use: git commit --no-verify"
