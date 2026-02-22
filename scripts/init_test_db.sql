-- FLUX IOT 测试数据库初始化脚本
-- 用于 PostgreSQL

-- 删除已存在的表（如果存在）
DROP TABLE IF EXISTS device_groups CASCADE;
DROP TABLE IF EXISTS devices CASCADE;
DROP TABLE IF EXISTS device_metrics CASCADE;
DROP TABLE IF EXISTS rules CASCADE;
DROP TABLE IF EXISTS events CASCADE;

-- 创建设备表
CREATE TABLE IF NOT EXISTS devices (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    device_type VARCHAR(50) NOT NULL,
    protocol VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'Inactive',
    product_id VARCHAR(255),
    secret VARCHAR(255),
    group_id VARCHAR(255),
    metadata JSONB,
    tags TEXT[],
    location JSONB,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    last_seen BIGINT
);

-- 创建设备组表
CREATE TABLE IF NOT EXISTS device_groups (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    parent_id VARCHAR(255),
    path VARCHAR(1024),
    description TEXT,
    metadata JSONB,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL,
    FOREIGN KEY (parent_id) REFERENCES device_groups(id) ON DELETE CASCADE
);

-- 创建设备指标表
CREATE TABLE IF NOT EXISTS device_metrics (
    id SERIAL PRIMARY KEY,
    device_id VARCHAR(255) NOT NULL,
    metric_name VARCHAR(255) NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    timestamp BIGINT NOT NULL,
    FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

-- 创建规则表
CREATE TABLE IF NOT EXISTS rules (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    script TEXT NOT NULL,
    active BOOLEAN NOT NULL DEFAULT true,
    created_at BIGINT NOT NULL
);

-- 创建事件表
CREATE TABLE IF NOT EXISTS events (
    id SERIAL PRIMARY KEY,
    topic VARCHAR(255) NOT NULL,
    payload JSONB NOT NULL,
    timestamp BIGINT NOT NULL,
    device_id VARCHAR(255),
    processed BOOLEAN NOT NULL DEFAULT false
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_devices_status ON devices(status);
CREATE INDEX IF NOT EXISTS idx_devices_type ON devices(device_type);
CREATE INDEX IF NOT EXISTS idx_devices_group ON devices(group_id);
CREATE INDEX IF NOT EXISTS idx_devices_last_seen ON devices(last_seen);
CREATE INDEX IF NOT EXISTS idx_device_groups_parent ON device_groups(parent_id);
CREATE INDEX IF NOT EXISTS idx_device_metrics_device ON device_metrics(device_id);
CREATE INDEX IF NOT EXISTS idx_device_metrics_timestamp ON device_metrics(timestamp);
CREATE INDEX IF NOT EXISTS idx_events_topic ON events(topic);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON events(timestamp);
CREATE INDEX IF NOT EXISTS idx_events_device ON events(device_id);

-- 添加外键约束
ALTER TABLE devices ADD CONSTRAINT fk_devices_group 
    FOREIGN KEY (group_id) REFERENCES device_groups(id) ON DELETE SET NULL;

COMMENT ON TABLE devices IS '设备表';
COMMENT ON TABLE device_groups IS '设备分组表';
COMMENT ON TABLE device_metrics IS '设备指标表';
COMMENT ON TABLE rules IS '规则表';
COMMENT ON TABLE events IS '事件表';
