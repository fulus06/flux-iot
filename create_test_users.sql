-- 创建测试用户
-- 使用 create_user 示例生成的哈希

-- Admin 用户 (密码: admin123)
INSERT INTO rtmp_users (id, username, password_hash, roles, enabled, created_at)
VALUES (
    'admin-001',
    'admin',
    '$2b$12$N7wiWED8i/vsN7ZT4cU0V.VTRBbJ9E8BIfns9b0dOS7IwC.n.NAFS',
    '["admin"]',
    1,
    datetime('now')
);

-- Operator 用户 (密码: op123)
INSERT INTO rtmp_users (id, username, password_hash, roles, enabled, created_at)
VALUES (
    'operator-001',
    'operator',
    '$2b$12$kSrPMTtFohcoPmcy2jvHge4yIVzAn/pk4eI9IdPgBRIgJekgYf7vy',
    '["operator"]',
    1,
    datetime('now')
);

-- Viewer 用户 (密码: view123)
INSERT INTO rtmp_users (id, username, password_hash, roles, enabled, created_at)
VALUES (
    'viewer-001',
    'viewer',
    '$2b$12$M3tKs/FTd4Om6lsaGVDazO8Z1914DrkGK3g/fhvL5eq6UcIOtkKQ.',
    '["viewer"]',
    1,
    datetime('now')
);
