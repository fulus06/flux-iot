-- 设备管理数据库表结构
-- 版本: 1.0
-- 日期: 2026-02-22

-- 设备表
CREATE TABLE IF NOT EXISTS devices (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    device_type VARCHAR(50) NOT NULL,
    protocol VARCHAR(50) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'Inactive',
    product_id VARCHAR(64),
    secret TEXT,
    metadata JSONB DEFAULT '{}',
    tags TEXT[] DEFAULT '{}',
    group_id VARCHAR(64),
    location JSONB,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    last_seen TIMESTAMP,
    CONSTRAINT fk_group FOREIGN KEY (group_id) REFERENCES device_groups(id) ON DELETE SET NULL
);

-- 设备表索引
CREATE INDEX IF NOT EXISTS idx_devices_status ON devices(status);
CREATE INDEX IF NOT EXISTS idx_devices_type ON devices(device_type);
CREATE INDEX IF NOT EXISTS idx_devices_protocol ON devices(protocol);
CREATE INDEX IF NOT EXISTS idx_devices_group ON devices(group_id);
CREATE INDEX IF NOT EXISTS idx_devices_tags ON devices USING GIN(tags);
CREATE INDEX IF NOT EXISTS idx_devices_created_at ON devices(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_devices_last_seen ON devices(last_seen DESC);

-- 设备分组表
CREATE TABLE IF NOT EXISTS device_groups (
    id VARCHAR(64) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    parent_id VARCHAR(64),
    path VARCHAR(1024) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_parent FOREIGN KEY (parent_id) REFERENCES device_groups(id) ON DELETE CASCADE
);

-- 分组表索引
CREATE INDEX IF NOT EXISTS idx_groups_parent ON device_groups(parent_id);
CREATE INDEX IF NOT EXISTS idx_groups_path ON device_groups(path);
CREATE INDEX IF NOT EXISTS idx_groups_name ON device_groups(name);

-- 设备状态历史表
CREATE TABLE IF NOT EXISTS device_status_history (
    id BIGSERIAL PRIMARY KEY,
    device_id VARCHAR(64) NOT NULL,
    status VARCHAR(20) NOT NULL,
    timestamp TIMESTAMP NOT NULL DEFAULT NOW(),
    metadata JSONB,
    CONSTRAINT fk_device_status FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

-- 状态历史表索引
CREATE INDEX IF NOT EXISTS idx_status_history_device ON device_status_history(device_id);
CREATE INDEX IF NOT EXISTS idx_status_history_timestamp ON device_status_history(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_status_history_status ON device_status_history(status);

-- 设备指标表（时序数据）
CREATE TABLE IF NOT EXISTS device_metrics (
    id BIGSERIAL PRIMARY KEY,
    device_id VARCHAR(64) NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    metric_value DOUBLE PRECISION NOT NULL,
    unit VARCHAR(20),
    timestamp TIMESTAMP NOT NULL DEFAULT NOW(),
    CONSTRAINT fk_device_metrics FOREIGN KEY (device_id) REFERENCES devices(id) ON DELETE CASCADE
);

-- 指标表索引
CREATE INDEX IF NOT EXISTS idx_metrics_device ON device_metrics(device_id);
CREATE INDEX IF NOT EXISTS idx_metrics_name ON device_metrics(metric_name);
CREATE INDEX IF NOT EXISTS idx_metrics_timestamp ON device_metrics(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_metrics_device_name ON device_metrics(device_id, metric_name);

-- 创建更新时间触发器函数
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

-- 为设备表添加更新时间触发器
DROP TRIGGER IF EXISTS update_devices_updated_at ON devices;
CREATE TRIGGER update_devices_updated_at
    BEFORE UPDATE ON devices
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- 为分组表添加更新时间触发器
DROP TRIGGER IF EXISTS update_device_groups_updated_at ON device_groups;
CREATE TRIGGER update_device_groups_updated_at
    BEFORE UPDATE ON device_groups
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- 插入测试数据（可选）
-- INSERT INTO device_groups (id, name, path) VALUES ('grp_root', '根分组', '/grp_root');
