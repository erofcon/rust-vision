-- 202505210001_create_map_of_day_table.sql


CREATE TABLE map_of_day
(
    id          UUID PRIMARY KEY,
    filename    VARCHAR(512) NOT NULL,
    uploaded_at TIMESTAMP    NOT NULL DEFAULT NOW()
);
