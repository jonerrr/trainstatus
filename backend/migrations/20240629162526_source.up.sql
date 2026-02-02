CREATE SCHEMA IF NOT EXISTS static;
CREATE SCHEMA IF NOT EXISTS realtime;

CREATE TYPE source_enum AS ENUM ('mta_subway', 'mta_bus', 'njt_rail', 'lirr', 'mnr');

CREATE TABLE IF NOT EXISTS source (
    id source_enum PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    -- refresh_interval INTERVAL NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL

    -- data_hash VARCHAR(64) NOT NULL TODO: maybe add etag / hash of data
);