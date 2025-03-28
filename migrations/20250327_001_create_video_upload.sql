-- 20250328_001_create_video_upload.sql
CREATE TYPE video_status AS ENUM ('uploaded', 'processing', 'completed', 'failed');

-- 20250328_001_create_videos_table.sql
CREATE TABLE videos
(
    id          UUID PRIMARY KEY,
    title       VARCHAR(255) NOT NULL,
    description TEXT,
    file_path   VARCHAR(512) NOT NULL,
    created_at  TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    status      video_status NOT NULL
);