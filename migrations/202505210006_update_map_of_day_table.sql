-- 202505210006_update_map_of_day_table.sql

ALTER TABLE map_of_day
    RENAME COLUMN uploaded_at TO created_at;