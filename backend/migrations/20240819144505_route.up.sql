CREATE EXTENSION IF NOT EXISTS postgis;

CREATE TABLE IF NOT EXISTS static.shape (
    id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    geom geometry(LINESTRING, 4326) NOT NULL,
    -- maybe add sequence number to make it easier to reconstruct the shape in order?
    length_meters FLOAT GENERATED ALWAYS AS (ST_Length(geom::geography)) STORED,
    data JSONB NOT NULL,
    PRIMARY KEY (id, source)
);

CREATE TABLE IF NOT EXISTS static.route (
    id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    long_name VARCHAR NOT NULL,
    short_name VARCHAR NOT NULL,
    color VARCHAR(7) NOT NULL,
    text_color VARCHAR(7),
    data JSONB NOT NULL,
    -- geom is now determined by active trips and associated shapes
    -- geom geometry(MULTILINESTRING, 4326),
    -- Need to have source in primary key to avoid conflicts between different sources
    PRIMARY KEY (id, source)
);

-- CREATE INDEX IF NOT EXISTS idx_route_geom ON static.route USING GIST (geom);
CREATE INDEX IF NOT EXISTS idx_shape_geom ON static.shape USING GIST (geom);
