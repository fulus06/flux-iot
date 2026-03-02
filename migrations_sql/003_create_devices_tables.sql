-- 设备表
CREATE TABLE IF NOT EXISTS device.devices (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    device_type VARCHAR(100) NOT NULL,
    protocol VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'Inactive',
    metadata JSONB,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE,
    last_seen_at TIMESTAMP WITH TIME ZONE
);

-- 设备指标表
CREATE TABLE IF NOT EXISTS device.device_metrics (
    id BIGSERIAL PRIMARY KEY,
    device_id VARCHAR(255) NOT NULL,
    metric_name VARCHAR(100) NOT NULL,
    value DOUBLE PRECISION NOT NULL,
    timestamp TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (device_id) REFERENCES device.devices(id) ON DELETE CASCADE
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_devices_status ON device.devices(status);
CREATE INDEX IF NOT EXISTS idx_devices_type ON device.devices(device_type);
CREATE INDEX IF NOT EXISTS idx_device_metrics_device_id ON device.device_metrics(device_id);
CREATE INDEX IF NOT EXISTS idx_device_metrics_timestamp ON device.device_metrics(timestamp DESC);
