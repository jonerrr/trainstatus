CREATE TABLE IF NOT EXISTS static.stop (
    id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    name VARCHAR NOT NULL,
    -- location_type SMALLINT NOT NULL DEFAULT 0,
    -- parent_stop_id VARCHAR,
    geom geometry(POINT, 4326) NOT NULL,
    data JSONB NOT NULL,

    PRIMARY KEY (id, source)
    -- FOREIGN KEY (parent_stop_id, source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);


CREATE TABLE IF NOT EXISTS static.stop_transfer (
    from_stop_id VARCHAR NOT NULL,
    from_stop_source source_enum NOT NULL,
    to_stop_id VARCHAR NOT NULL,
    to_stop_source source_enum NOT NULL,
    transfer_type SMALLINT NOT NULL,
    min_transfer_time SMALLINT,
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

CREATE INDEX IF NOT EXISTS idx_stop_geom ON static.stop USING GIST (geom);