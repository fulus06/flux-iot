-- FLUX IOT TimescaleDB 初始化脚本

-- 启用 TimescaleDB 扩展
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- 创建设备指标表
CREATE TABLE IF NOT EXISTS device_metrics (
    time TIMESTAMPTZ NOT NULL,
    device_id TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    unit TEXT,
    tags JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- 转换为 Hypertable（自动分区）
SELECT create_hypertable('device_metrics', 'time', if_not_exists => TRUE);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_device_metrics_device_id 
    ON device_metrics (device_id, time DESC);
CREATE INDEX IF NOT EXISTS idx_device_metrics_metric_name 
    ON device_metrics (metric_name, time DESC);
CREATE INDEX IF NOT EXISTS idx_device_metrics_tags 
    ON device_metrics USING GIN (tags);

-- 启用压缩
ALTER TABLE device_metrics SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'device_id,metric_name',
    timescaledb.compress_orderby = 'time DESC'
);

-- 自动压缩策略（压缩 7 天前的数据）
SELECT add_compression_policy('device_metrics', INTERVAL '7 days', if_not_exists => TRUE);

-- 数据保留策略（删除 90 天前的数据）
SELECT add_retention_policy('device_metrics', INTERVAL '90 days', if_not_exists => TRUE);

-- 创建 5 分钟聚合视图
CREATE MATERIALIZED VIEW IF NOT EXISTS device_metrics_5m
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('5 minutes', time) AS bucket,
    device_id,
    metric_name,
    AVG(metric_value) as avg_value,
    MAX(metric_value) as max_value,
    MIN(metric_value) as min_value,
    COUNT(*) as count
FROM device_metrics
GROUP BY bucket, device_id, metric_name
WITH NO DATA;

-- 自动刷新策略
SELECT add_continuous_aggregate_policy('device_metrics_5m',
    start_offset => INTERVAL '1 hour',
    end_offset => INTERVAL '5 minutes',
    schedule_interval => INTERVAL '5 minutes',
    if_not_exists => TRUE
);

-- 创建 1 小时聚合视图
CREATE MATERIALIZED VIEW IF NOT EXISTS device_metrics_1h
WITH (timescaledb.continuous) AS
SELECT 
    time_bucket('1 hour', time) AS bucket,
    device_id,
    metric_name,
    AVG(metric_value) as avg_value,
    MAX(metric_value) as max_value,
    MIN(metric_value) as min_value,
    COUNT(*) as count
FROM device_metrics
GROUP BY bucket, device_id, metric_name
WITH NO DATA;

-- 自动刷新策略
SELECT add_continuous_aggregate_policy('device_metrics_1h',
    start_offset => INTERVAL '1 day',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour',
    if_not_exists => TRUE
);

-- 创建设备日志表
CREATE TABLE IF NOT EXISTS device_logs (
    time TIMESTAMPTZ NOT NULL,
    device_id TEXT NOT NULL,
    log_level TEXT NOT NULL,
    message TEXT NOT NULL,
    source TEXT,
    tags JSONB,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- 转换为 Hypertable
SELECT create_hypertable('device_logs', 'time', if_not_exists => TRUE);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_device_logs_device_id 
    ON device_logs (device_id, time DESC);
CREATE INDEX IF NOT EXISTS idx_device_logs_level 
    ON device_logs (log_level, time DESC);

-- 启用压缩
ALTER TABLE device_logs SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'device_id,log_level'
);

-- 自动压缩策略
SELECT add_compression_policy('device_logs', INTERVAL '3 days', if_not_exists => TRUE);

-- 数据保留策略（删除 30 天前的日志）
SELECT add_retention_policy('device_logs', INTERVAL '30 days', if_not_exists => TRUE);

-- 创建设备事件表
CREATE TABLE IF NOT EXISTS device_events (
    time TIMESTAMPTZ NOT NULL,
    device_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    event_data JSONB NOT NULL,
    severity TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- 转换为 Hypertable
SELECT create_hypertable('device_events', 'time', if_not_exists => TRUE);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_device_events_device_id 
    ON device_events (device_id, time DESC);
CREATE INDEX IF NOT EXISTS idx_device_events_type 
    ON device_events (event_type, time DESC);
CREATE INDEX IF NOT EXISTS idx_device_events_severity 
    ON device_events (severity, time DESC);

-- 启用压缩
ALTER TABLE device_events SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'device_id,event_type'
);

-- 自动压缩策略
SELECT add_compression_policy('device_events', INTERVAL '7 days', if_not_exists => TRUE);

-- 数据保留策略（删除 180 天前的事件）
SELECT add_retention_policy('device_events', INTERVAL '180 days', if_not_exists => TRUE);

-- 打印初始化完成信息
DO $$
BEGIN
    RAISE NOTICE 'TimescaleDB initialized successfully!';
    RAISE NOTICE 'Created tables: device_metrics, device_logs, device_events';
    RAISE NOTICE 'Compression policies: 7 days (metrics), 3 days (logs), 7 days (events)';
    RAISE NOTICE 'Retention policies: 90 days (metrics), 30 days (logs), 180 days (events)';
END $$;
