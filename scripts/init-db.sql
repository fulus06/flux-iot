-- FLUX IOT PostgreSQL 初始化脚本

-- 创建扩展
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";

-- 创建数据库（如果不存在）
-- 注意：这个脚本在 docker-entrypoint-initdb.d 中运行时，数据库已经创建

-- 设置时区
SET timezone = 'UTC';

-- 创建索引优化查询
-- 这些索引会在应用启动时由 SeaORM 自动创建
-- 这里仅作为参考

COMMENT ON DATABASE flux_iot IS 'FLUX IOT Platform Database';

-- 授予权限
GRANT ALL PRIVILEGES ON DATABASE flux_iot TO flux;

-- 创建性能监控视图
CREATE OR REPLACE VIEW pg_stat_activity_summary AS
SELECT 
    datname,
    state,
    COUNT(*) as connection_count,
    MAX(EXTRACT(EPOCH FROM (now() - query_start))) as max_query_duration
FROM pg_stat_activity
WHERE datname = 'flux_iot'
GROUP BY datname, state;

-- 日志记录
DO $$
BEGIN
    RAISE NOTICE 'FLUX IOT database initialized successfully';
END $$;
