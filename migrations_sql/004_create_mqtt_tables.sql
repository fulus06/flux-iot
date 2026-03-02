-- MQTT 客户端表
CREATE TABLE IF NOT EXISTS mqtt.mqtt_clients (
    client_id VARCHAR(255) PRIMARY KEY,
    username VARCHAR(255),
    password_hash VARCHAR(255),
    connected BOOLEAN NOT NULL DEFAULT FALSE,
    connected_at TIMESTAMP WITH TIME ZONE,
    disconnected_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- MQTT 订阅表
CREATE TABLE IF NOT EXISTS mqtt.mqtt_subscriptions (
    id BIGSERIAL PRIMARY KEY,
    client_id VARCHAR(255) NOT NULL,
    topic VARCHAR(255) NOT NULL,
    qos SMALLINT NOT NULL DEFAULT 0,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (client_id) REFERENCES mqtt.mqtt_clients(client_id) ON DELETE CASCADE
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_mqtt_clients_connected ON mqtt.mqtt_clients(connected);
CREATE INDEX IF NOT EXISTS idx_mqtt_subscriptions_client_id ON mqtt.mqtt_subscriptions(client_id);
CREATE INDEX IF NOT EXISTS idx_mqtt_subscriptions_topic ON mqtt.mqtt_subscriptions(topic);
