-- 202505220001_init.sql

CREATE TABLE organizations
(
    id         UUID PRIMARY KEY,
    name       VARCHAR(255) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);


CREATE TABLE camera_presets
(
    id              UUID PRIMARY KEY,
    organization_id UUID         NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    camera_name     VARCHAR(255) NOT NULL,
    location        VARCHAR(255) NOT NULL,
    detectors       JSONB        NOT NULL,
    regions         JSONB        NOT NULL,
    created_at      TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);