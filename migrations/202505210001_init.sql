-- 202505210001_init.sql

CREATE TYPE video_status AS ENUM ('uploaded', 'waiting', 'processing', 'completed', 'failed', 'cancelled');
CREATE TYPE event_status AS ENUM ('entrance', 'exit');


CREATE TABLE videos
(
    id          UUID PRIMARY KEY,
    title       VARCHAR(255) NOT NULL,
    description TEXT,
    file_path   TEXT         NOT NULL,
    created_at  TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    status      video_status NOT NULL
);


CREATE TABLE map_of_day
(
    id          UUID PRIMARY KEY,
    file_name   VARCHAR(512) NOT NULL,
    description TEXT,
    created_at  TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);



CREATE TABLE access_log
(
    id            UUID PRIMARY KEY,
    full_name     VARCHAR(512),
    event_time    TIMESTAMP WITH TIME ZONE    NOT NULL,
    event         event_status NOT NULL,
    card_id       VARCHAR(512) NOT NULL,
    map_of_day_id UUID         NOT NULL,

    CONSTRAINT fk_access_log_map_of_day
        FOREIGN KEY (map_of_day_id)
            REFERENCES map_of_day (id)
            ON DELETE CASCADE
);




