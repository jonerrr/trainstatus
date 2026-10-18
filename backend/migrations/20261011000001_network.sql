CREATE SCHEMA IF NOT EXISTS static;
CREATE SCHEMA IF NOT EXISTS realtime;

CREATE TYPE source_enum AS ENUM ('mta_subway', 'mta_bus', 'njt_bus');

CREATE TABLE IF NOT EXISTS source (
    id source_enum PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    trip_patterns JSONB NOT NULL DEFAULT '{}'::jsonb,
    stop_remap JSONB NOT NULL DEFAULT '{}'::jsonb
);
CREATE EXTENSION IF NOT EXISTS postgis;

CREATE TABLE static.shape_asset (
    source source_enum NOT NULL,
    id TEXT NOT NULL,
    geom geometry(LINESTRING, 4326) NOT NULL,
    length_meters DOUBLE PRECISION GENERATED ALWAYS AS (ST_Length(geom::geography)) STORED,
    PRIMARY KEY (source, id)
);
CREATE TABLE static.shape (
    source source_enum NOT NULL,
    id VARCHAR NOT NULL,
    asset_id TEXT NOT NULL,
    current BOOLEAN NOT NULL DEFAULT TRUE,
    data JSONB NOT NULL,
    PRIMARY KEY (id, source),
    FOREIGN KEY (source, asset_id) REFERENCES static.shape_asset(source, id)
);

CREATE TABLE IF NOT EXISTS static.route (
    current BOOLEAN NOT NULL DEFAULT TRUE,
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


CREATE TABLE IF NOT EXISTS static.stop (
    current BOOLEAN NOT NULL DEFAULT TRUE,
    id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    name VARCHAR NOT NULL,
    -- location_type SMALLINT NOT NULL DEFAULT 0,
    -- parent_stop_id VARCHAR,
    geom geometry(POINT, 4326) NOT NULL,
    projected_geom geometry(POINT, 6538) GENERATED ALWAYS AS (ST_Transform(geom, 6538)) STORED,
    data JSONB NOT NULL,

    PRIMARY KEY (id, source)
    -- FOREIGN KEY (parent_stop_id, source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);


CREATE TABLE IF NOT EXISTS static.nearby_stop (
    from_stop_id VARCHAR NOT NULL,
    from_stop_source source_enum NOT NULL,
    to_stop_id VARCHAR NOT NULL,
    to_stop_source source_enum NOT NULL,
    distance_m DOUBLE PRECISION NOT NULL CHECK (distance_m >= 0),
    PRIMARY KEY (from_stop_id, from_stop_source, to_stop_id, to_stop_source),
    FOREIGN KEY (from_stop_id, from_stop_source) REFERENCES static.stop(id, source) ON DELETE CASCADE,
    FOREIGN KEY (to_stop_id, to_stop_source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS static.route_stop (
    route_id VARCHAR NOT NULL,
    stop_id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    stop_sequence SMALLINT NOT NULL,

    data JSONB NOT NULL,

    PRIMARY KEY (route_id, source, stop_id),
    FOREIGN KEY (route_id, source) REFERENCES static.route(id, source) ON DELETE CASCADE,
    FOREIGN KEY (stop_id, source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);



-- Membership and target-eligibility lookups are keyed on stop and source.
CREATE INDEX IF NOT EXISTS idx_route_stop_stop ON static.route_stop (stop_id, source);
CREATE INDEX idx_stop_projected_geom ON static.stop USING GIST (projected_geom) WHERE current;

CREATE TABLE static.scheduled_trip (
    source source_enum NOT NULL REFERENCES source(id),
    trip_id TEXT NOT NULL,
    service_date TEXT NOT NULL,
    payload JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (source, trip_id, service_date)
);



CREATE INDEX idx_current_stop_geom ON static.stop USING GIST (geom) WHERE current;
