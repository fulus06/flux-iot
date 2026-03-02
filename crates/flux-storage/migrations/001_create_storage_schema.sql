-- 创建 storage schema
CREATE SCHEMA IF NOT EXISTS storage;

-- 设置搜索路径
SET search_path TO storage, public;

-- 分片元数据表
CREATE TABLE IF NOT EXISTS storage.segment_metadata (
    id BIGSERIAL PRIMARY KEY,
    stream_id VARCHAR(255) NOT NULL,
    segment_id BIGINT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT NOW(),
    
    -- 唯一约束
    UNIQUE(stream_id, segment_id)
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_segment_metadata_stream_id 
    ON storage.segment_metadata(stream_id);

CREATE INDEX IF NOT EXISTS idx_segment_metadata_segment_id 
    ON storage.segment_metadata(segment_id);

CREATE INDEX IF NOT EXISTS idx_segment_metadata_created_at 
    ON storage.segment_metadata(created_at);

-- JSONB 元数据索引（GIN 索引，支持快速查询）
CREATE INDEX IF NOT EXISTS idx_segment_metadata_metadata 
    ON storage.segment_metadata USING GIN (metadata);

-- 更新时间触发器
CREATE OR REPLACE FUNCTION storage.update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_segment_metadata_updated_at 
    BEFORE UPDATE ON storage.segment_metadata
    FOR EACH ROW
    EXECUTE FUNCTION storage.update_updated_at_column();

-- 注释
COMMENT ON SCHEMA storage IS 'flux-storage 元数据存储 schema';
COMMENT ON TABLE storage.segment_metadata IS '分片元数据表';
COMMENT ON COLUMN storage.segment_metadata.stream_id IS '流 ID';
COMMENT ON COLUMN storage.segment_metadata.segment_id IS '分片序号';
COMMENT ON COLUMN storage.segment_metadata.metadata IS '自定义元数据（JSONB）';
COMMENT ON COLUMN storage.segment_metadata.created_at IS '创建时间';
COMMENT ON COLUMN storage.segment_metadata.updated_at IS '更新时间';
