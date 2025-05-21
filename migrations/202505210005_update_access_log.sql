-- 202505210005_update_access_log.sql

ALTER TABLE access_log
    ADD COLUMN map_of_day_id UUID;


ALTER TABLE access_log
    ALTER COLUMN map_of_day_id SET NOT NULL;


ALTER TABLE access_log
    ADD CONSTRAINT fk_map_of_day
        FOREIGN KEY (map_of_day_id)
            REFERENCES map_of_day (id)
            ON DELETE CASCADE;
