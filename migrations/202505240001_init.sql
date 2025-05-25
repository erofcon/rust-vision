-- 202505240001_init.sql

CREATE TABLE organizations
(
    id         UUID PRIMARY KEY,
    name       VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);


CREATE TABLE camera_presets
(
    id              UUID PRIMARY KEY,
    organization_id UUID                NOT NULL,
    camera_name     VARCHAR(255) UNIQUE NOT NULL,
    location        VARCHAR(255)        NOT NULL,
    detectors       JSONB               NOT NULL,
    regions         JSONB               NOT NULL,
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT fk_camera_presets_organizations FOREIGN KEY (organization_id)
        REFERENCES organizations (id)
        ON DELETE CASCADE
);

CREATE TABLE day_maps
(
    id              UUID PRIMARY KEY,
    organization_id UUID         NOT NULL,
    title           VARCHAR(255) NOT NULL,
    description     TEXT,
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT fk_day_maps_organizations FOREIGN KEY (organization_id)
        REFERENCES organizations (id)
        ON DELETE CASCADE
);


CREATE TABLE day_map_entries
(
    id         UUID PRIMARY KEY,
    day_map_id UUID                     NOT NULL,
    full_name  VARCHAR(255),
    card_id    VARCHAR(255)             NOT NULL,
    entry_time TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT fk_day_map_entries_day_maps FOREIGN KEY (day_map_id)
        REFERENCES day_maps (id)
        ON DELETE CASCADE
);

DO
$$
BEGIN
    IF
NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'processing_status') THEN
CREATE TYPE processing_status AS ENUM ('uploaded', 'waiting', 'processing', 'completed', 'failed', 'cancelled');
END IF;
END$$;


CREATE TABLE processing_jobs
(
    id                UUID PRIMARY KEY,
    organization_id   UUID              NOT NULL,
    day_map_id        UUID              NOT NULL,
    video_folder_path TEXT              NOT NULL,
    status            processing_status NOT NULL,
    created_at        TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT fk_processing_jobs_organizations FOREIGN KEY (organization_id)
        REFERENCES organizations (id)
        ON DELETE CASCADE,

    CONSTRAINT fk_processing_jobs_day_maps FOREIGN KEY (day_map_id)
        REFERENCES day_maps (id)
        ON DELETE CASCADE
);


CREATE TABLE video_jobs
(
    id                 UUID PRIMARY KEY,
    processing_jobs_id UUID                     NOT NULL,
    camera_presets_id  UUID                     NOT NULL,
    video_name         TEXT                     NOT NULL,
    file_path          TEXT                     NOT NULL,
    status             processing_status        NOT NULL,
    start_time         TIMESTAMP WITH TIME ZONE NOT NULL,
    created_at         TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT fk_video_jobs_processing_jobs FOREIGN KEY (processing_jobs_id)
        REFERENCES processing_jobs (id)
        ON DELETE CASCADE,

    CONSTRAINT fk_video_jobs_camera_presets FOREIGN KEY (camera_presets_id)
        REFERENCES camera_presets (id)
        ON DELETE CASCADE
);