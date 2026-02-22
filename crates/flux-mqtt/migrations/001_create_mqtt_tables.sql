-- MQTT 持久化会话表

-- 会话表
CREATE TABLE IF NOT EXISTS mqtt_sessions (
    client_id VARCHAR(255) PRIMARY KEY,
    clean_session BOOLEAN NOT NULL DEFAULT false,
    subscriptions TEXT,  -- JSON 格式存储订阅信息
    will_topic VARCHAR(255),
    will_payload BYTEA,
    will_qos SMALLINT,
    will_retained BOOLEAN,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMP
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_mqtt_sessions_last_seen ON mqtt_sessions(last_seen);
CREATE INDEX IF NOT EXISTS idx_mqtt_sessions_expires_at ON mqtt_sessions(expires_at);

-- 离线消息表
CREATE TABLE IF NOT EXISTS mqtt_offline_messages (
    id BIGSERIAL PRIMARY KEY,
    client_id VARCHAR(255) NOT NULL,
    topic VARCHAR(255) NOT NULL,
    payload BYTEA NOT NULL,
    qos SMALLINT NOT NULL,
    retained BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_mqtt_offline_messages_client_id ON mqtt_offline_messages(client_id);
CREATE INDEX IF NOT EXISTS idx_mqtt_offline_messages_created_at ON mqtt_offline_messages(created_at);

-- Retained 消息表（持久化）
CREATE TABLE IF NOT EXISTS mqtt_retained_messages (
    topic VARCHAR(255) PRIMARY KEY,
    payload BYTEA NOT NULL,
    qos SMALLINT NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- ACL 规则表
CREATE TABLE IF NOT EXISTS mqtt_acl_rules (
    id BIGSERIAL PRIMARY KEY,
    client_id VARCHAR(255),
    username VARCHAR(255),
    topic_pattern VARCHAR(255) NOT NULL,
    action VARCHAR(20) NOT NULL,  -- publish, subscribe, both
    permission VARCHAR(10) NOT NULL,  -- allow, deny
    priority INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_mqtt_acl_rules_client_id ON mqtt_acl_rules(client_id);
CREATE INDEX IF NOT EXISTS idx_mqtt_acl_rules_username ON mqtt_acl_rules(username);
CREATE INDEX IF NOT EXISTS idx_mqtt_acl_rules_priority ON mqtt_acl_rules(priority DESC);

-- 注释
COMMENT ON TABLE mqtt_sessions IS 'MQTT 客户端会话信息';
COMMENT ON TABLE mqtt_offline_messages IS 'MQTT 离线消息队列';
COMMENT ON TABLE mqtt_retained_messages IS 'MQTT Retained 消息持久化';
COMMENT ON TABLE mqtt_acl_rules IS 'MQTT 访问控制规则';
