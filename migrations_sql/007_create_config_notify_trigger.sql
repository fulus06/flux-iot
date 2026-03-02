-- 创建配置变更通知触发器
-- 用于实现配置热重载功能

-- 创建通知函数
CREATE OR REPLACE FUNCTION notify_config_change()
RETURNS TRIGGER AS $$
DECLARE
    payload JSON;
BEGIN
    -- 构建通知负载
    payload = json_build_object(
        'key', NEW.key,
        'version', NEW.version,
        'operation', TG_OP,
        'timestamp', extract(epoch from now())
    );
    
    -- 发送通知
    PERFORM pg_notify('app_configs_changes', payload::text);
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- 创建触发器（如果不存在）
DROP TRIGGER IF EXISTS config_change_trigger ON config.app_configs;

CREATE TRIGGER config_change_trigger
AFTER INSERT OR UPDATE OR DELETE ON config.app_configs
FOR EACH ROW
EXECUTE FUNCTION notify_config_change();

-- 创建索引以提高查询性能
CREATE INDEX IF NOT EXISTS idx_app_configs_key ON config.app_configs(key);
CREATE INDEX IF NOT EXISTS idx_app_configs_version ON config.app_configs(version DESC);

COMMENT ON FUNCTION notify_config_change() IS '配置变更通知函数 - 用于实现配置热重载';
COMMENT ON TRIGGER config_change_trigger ON config.app_configs IS '配置变更触发器 - 自动发送 NOTIFY';
