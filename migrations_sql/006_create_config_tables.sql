-- 应用配置表
CREATE TABLE IF NOT EXISTS public.app_config (
    id BIGSERIAL PRIMARY KEY,
    content TEXT NOT NULL,
    updated_at BIGINT NOT NULL
);

-- 配置审计表
CREATE TABLE IF NOT EXISTS public.app_config_audit (
    id BIGSERIAL PRIMARY KEY,
    prev_updated_at BIGINT,
    new_updated_at BIGINT NOT NULL,
    prev_hash TEXT,
    new_hash TEXT NOT NULL,
    user_agent TEXT,
    forwarded_for TEXT,
    created_at BIGINT NOT NULL
);

-- 规则表
CREATE TABLE IF NOT EXISTS public.rules (
    id VARCHAR(255) PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    trigger_type VARCHAR(50) NOT NULL,
    trigger_config JSONB,
    script TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 50,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at BIGINT NOT NULL
);

-- 事件表
CREATE TABLE IF NOT EXISTS public.events (
    id BIGSERIAL PRIMARY KEY,
    event_type VARCHAR(100) NOT NULL,
    source VARCHAR(255) NOT NULL,
    payload JSONB,
    timestamp BIGINT NOT NULL
);

-- 创建索引
CREATE INDEX IF NOT EXISTS idx_app_config_audit_created_at ON public.app_config_audit(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_rules_active ON public.rules(active);
CREATE INDEX IF NOT EXISTS idx_events_timestamp ON public.events(timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_events_event_type ON public.events(event_type);
