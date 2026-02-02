CREATE EXTENSION IF NOT EXISTS postgis;

CREATE TABLE IF NOT EXISTS static.route (
    id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    long_name VARCHAR NOT NULL,
    short_name VARCHAR NOT NULL,
    color VARCHAR(7) NOT NULL,
    data JSONB NOT NULL,
    geom geometry(MULTILINESTRING, 4326),
    -- Need to have source in primary key to avoid conflicts between different sources
    PRIMARY KEY (id, source)
);

CREATE INDEX IF NOT EXISTS idx_route_geom ON static.route USING GIST (geom);