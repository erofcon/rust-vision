-- 20250327_001_create_video_upload.sql
CREATE TABLE video_upload
(
    id         SERIAL PRIMARY KEY,
    url        TEXT NOT NULL,
    status     VARCHAR(50),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
