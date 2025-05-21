-- 202505210002_create_access_log.sql

CREATE TYPE event_status AS ENUM ('entrance', 'exit');


CREATE TABLE access_log
(
    id         UUID PRIMARY KEY,
    full_name  VARCHAR(512),
    event_time TIMESTAMP    NOT NULL,
    event      event_status NOT NULL,
    card_id    VARCHAR(512) NOT NULL
);
