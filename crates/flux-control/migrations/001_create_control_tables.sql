-- 设备指令表
CREATE TABLE IF NOT EXISTS device_commands (
    id VARCHAR(255) PRIMARY KEY,
    device_id VARCHAR(255) NOT NULL,
    command_type VARCHAR(100) NOT NULL,
    params JSONB,
    timeout_seconds INTEGER NOT NULL,
    status VARCHAR(50) NOT NULL,
    created_at TIMESTAMP NOT NULL,
    sent_at TIMESTAMP,
    executed_at TIMESTAMP,
    completed_at TIMESTAMP,
    result JSONB,
    error TEXT
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_device_commands_device_id ON device_commands(device_id);
CREATE INDEX IF NOT EXISTS idx_device_commands_status ON device_commands(status);
CREATE INDEX IF NOT EXISTS idx_device_commands_created_at ON device_commands(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_device_commands_device_status ON device_commands(device_id, status);

-- 指令响应表
CREATE TABLE IF NOT EXISTS command_responses (
    id BIGSERIAL PRIMARY KEY,
    command_id VARCHAR(255) NOT NULL,
    device_id VARCHAR(255) NOT NULL,
    response_data JSONB NOT NULL,
    received_at TIMESTAMP NOT NULL,
    FOREIGN KEY (command_id) REFERENCES device_commands(id) ON DELETE CASCADE
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_command_responses_command_id ON command_responses(command_id);
CREATE INDEX IF NOT EXISTS idx_command_responses_device_id ON command_responses(device_id);
CREATE INDEX IF NOT EXISTS idx_command_responses_received_at ON command_responses(received_at DESC);

-- 注释
COMMENT ON TABLE device_commands IS '设备指令表';
COMMENT ON TABLE command_responses IS '指令响应表';

COMMENT ON COLUMN device_commands.command_type IS '指令类型（reboot/reset/custom等）';
COMMENT ON COLUMN device_commands.params IS '指令参数（JSON格式）';
COMMENT ON COLUMN device_commands.status IS '指令状态（pending/sent/executing/success/failed/timeout/cancelled）';
COMMENT ON COLUMN device_commands.result IS '执行结果（JSON格式）';
