 CREATE TABLE IF NOT EXISTS realtime.trip (
    id UUID PRIMARY KEY,
    original_id VARCHAR NOT NULL,
    vehicle_id VARCHAR NOT NULL,
    route_id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    direction SMALLINT,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    data JSONB NOT NULL,

    UNIQUE (original_id, vehicle_id, created_at, direction),
    FOREIGN KEY (route_id, source) REFERENCES static.route(id, source) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS realtime.stop_time (
    trip_id UUID REFERENCES realtime.trip(id) ON DELETE CASCADE,
    stop_id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    arrival TIMESTAMP WITH TIME ZONE NOT NULL,
    departure TIMESTAMP WITH TIME ZONE NOT NULL,
    data JSONB NOT NULL,

    PRIMARY KEY (trip_id, stop_id, source),
    FOREIGN KEY (stop_id, source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);

-- Current vehicle position (upserted, no history - just latest state)
CREATE TABLE IF NOT EXISTS realtime.vehicle_position (
    vehicle_id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    trip_id UUID REFERENCES realtime.trip(id) ON DELETE SET NULL,
    stop_id VARCHAR,
    geom geometry(POINT, 4326),
    data JSONB NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (vehicle_id, source),
    FOREIGN KEY (stop_id, source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);

CREATE INDEX idx_vehicle_position_trip_id ON realtime.vehicle_position (trip_id);
CREATE INDEX idx_vehicle_position_gix ON realtime.vehicle_position USING GIST(geom);

-- Trip geometry (LineString that accumulates position points over trip lifetime)
-- Points are appended when new positions come in
CREATE TABLE IF NOT EXISTS realtime.trip_geometry (
    trip_id UUID PRIMARY KEY REFERENCES realtime.trip(id) ON DELETE CASCADE,
    geom geometry(LINESTRING, 4326) NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

CREATE INDEX idx_trip_geometry_gix ON realtime.trip_geometry USING GIST(geom);

CREATE INDEX idx_trip_created_at ON realtime.trip (created_at);

CREATE INDEX idx_stop_time_arrival ON realtime.stop_time (arrival);

-- CREATE INDEX idx_position_updated_at ON realtime.position (updated_at);

-- CREATE INDEX idx_position_trip_id ON realtime.position (trip_id);