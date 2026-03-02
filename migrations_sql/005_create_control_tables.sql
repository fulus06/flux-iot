-- 设备指令表
CREATE TABLE IF NOT EXISTS control.device_commands (
    id VARCHAR(255) PRIMARY KEY,
    device_id VARCHAR(255) NOT NULL,
    command_type VARCHAR(100) NOT NULL,
    params JSONB,
    timeout_seconds INTEGER NOT NULL DEFAULT 30,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    sent_at TIMESTAMP WITH TIME ZONE,
    executed_at TIMESTAMP WITH TIME ZONE,
    completed_at TIMESTAMP WITH TIME ZONE,
    result JSONB,
    error TEXT
);

-- 指令响应表
CREATE TABLE IF NOT EXISTS control.command_responses (
    id BIGSERIAL PRIMARY KEY,
    command_id VARCHAR(255) NOT NULL,
    device_id VARCHAR(255) NOT NULL,
    response_data JSONB NOT NULL,
    received_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (command_id) REFERENCES control.device_commands(id) ON DELETE CASCADE
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_device_commands_device_id ON control.device_commands(device_id);
CREATE INDEX IF NOT EXISTS idx_device_commands_status ON control.device_commands(status);
CREATE INDEX IF NOT EXISTS idx_device_commands_created_at ON control.device_commands(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_command_responses_command_id ON control.command_responses(command_id);
CREATE INDEX IF NOT EXISTS idx_command_responses_device_id ON control.command_responses(device_id);
