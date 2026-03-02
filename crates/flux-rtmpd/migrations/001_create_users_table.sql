-- RTMP 用户表
CREATE TABLE IF NOT EXISTS rtmp_users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    roles TEXT NOT NULL DEFAULT '[]',
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_rtmp_users_username ON rtmp_users(username);
CREATE INDEX IF NOT EXISTS idx_rtmp_users_enabled ON rtmp_users(enabled);

-- 插入默认管理员用户
-- 密码: admin123 (bcrypt hash)
INSERT INTO rtmp_users (id, username, password_hash, roles, enabled, created_at)
VALUES (
    'admin-default',
    'admin',
    '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyYqNqJqGqKm',
    '["admin"]',
    TRUE,
    CURRENT_TIMESTAMP
) ON CONFLICT(username) DO NOTHING;

-- 插入示例操作员用户
-- 密码: op123
INSERT INTO rtmp_users (id, username, password_hash, roles, enabled, created_at)
VALUES (
    'operator-default',
    'operator',
    '$2b$12$8vn/FgIRYQBQqz5t5y5rJeqKqH5nqH5nqH5nqH5nqH5nqH5nqH5nq',
    '["operator"]',
    TRUE,
    CURRENT_TIMESTAMP
) ON CONFLICT(username) DO NOTHING;

-- 插入示例查看者用户
-- 密码: view123
INSERT INTO rtmp_users (id, username, password_hash, roles, enabled, created_at)
VALUES (
    'viewer-default',
    'viewer',
    '$2b$12$9wo/GhJSZRCRrz6u6z6sKfrLrI6orI6orI6orI6orI6orI6orI6or',
    '["viewer"]',
    TRUE,
    CURRENT_TIMESTAMP
) ON CONFLICT(username) DO NOTHING;
