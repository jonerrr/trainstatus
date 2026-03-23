-- CREATE TYPE static.borough AS ENUM (
--     'brooklyn',
--     'manhattan',
--     'staten_island',
--     'queens',
--     'bronx'
-- );

-- CREATE TYPE static.compass_direction AS ENUM (
--     'sw',
--     's',
--     'se',
--     'e',
--     'w',
--     'ne',
--     'nw',
--     'n',
--     'unknown'
-- );

CREATE TABLE IF NOT EXISTS static.stop (
    id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    name VARCHAR NOT NULL,
    geom geometry(POINT, 4326) NOT NULL,
    data JSONB NOT NULL,

    PRIMARY KEY (id, source)
);

CREATE TABLE IF NOT EXISTS static.stop_transfer (
    from_stop_id VARCHAR NOT NULL,
    from_stop_source source_enum NOT NULL,
    to_stop_id VARCHAR NOT NULL,
    to_stop_source source_enum NOT NULL,
    -- will probably be used to indicate if its from the official dataset or calculated by us
    transfer_type SMALLINT NOT NULL,
    min_transfer_time SMALLINT,
    PRIMARY KEY (from_stop_id, from_stop_source, to_stop_id, to_stop_source),
    FOREIGN KEY (from_stop_id, from_stop_source) REFERENCES static.stop(id, source) ON DELETE CASCADE,
    FOREIGN KEY (to_stop_id, to_stop_source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);


-- CREATE TYPE static.stop_type AS enum (
--     'full_time',
--     'part_time',
--     'late_night',
--     'rush_hour_one_direction',
--     'rush_hour'
-- );

CREATE TABLE IF NOT EXISTS static.route_stop (
    route_id VARCHAR NOT NULL,
    stop_id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    --  REFERENCES static.stop(id) ON DELETE CASCADE,
    stop_sequence SMALLINT NOT NULL,

    data JSONB NOT NULL,

    PRIMARY KEY (route_id, source, stop_id),
    FOREIGN KEY (route_id, source) REFERENCES static.route(id, source) ON DELETE CASCADE,
    FOREIGN KEY (stop_id, source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_stop_geom ON static.stop USING GIST (geom);