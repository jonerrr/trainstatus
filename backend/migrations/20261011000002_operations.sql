CREATE TABLE IF NOT EXISTS realtime.trip (
    id UUID PRIMARY KEY,
    original_id VARCHAR NOT NULL,
    vehicle_id VARCHAR NOT NULL,
    route_id VARCHAR NOT NULL,
    shape_ids VARCHAR[] NOT NULL,
    source source_enum NOT NULL,
    direction SMALLINT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    data JSONB NOT NULL,

    UNIQUE (source, original_id, created_at, direction),
    FOREIGN KEY (route_id, source) REFERENCES static.route(id, source)
);

CREATE TABLE realtime.run_shape (
    run_id UUID NOT NULL REFERENCES realtime.trip(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    source source_enum NOT NULL,
    provider_shape_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    PRIMARY KEY (run_id, ordinal),
    FOREIGN KEY (source, asset_id) REFERENCES static.shape_asset(source, id)
);

CREATE TABLE IF NOT EXISTS realtime.stop_time (
    trip_id UUID NOT NULL REFERENCES realtime.trip(id) ON DELETE CASCADE,
    stop_sequence INTEGER NOT NULL CHECK (stop_sequence >= 0),
    stop_id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    arrival TIMESTAMP WITH TIME ZONE NOT NULL,
    departure TIMESTAMP WITH TIME ZONE NOT NULL,
    data JSONB NOT NULL,

    PRIMARY KEY (source, trip_id, stop_sequence),
    FOREIGN KEY (stop_id, source) REFERENCES static.stop(id, source)
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
    FOREIGN KEY (stop_id, source) REFERENCES static.stop(id, source)
);

CREATE INDEX idx_vehicle_position_trip_id ON realtime.vehicle_position (trip_id);


-- Compact GPS trail; operations writes samples explicitly with accepted observations.
CREATE TABLE realtime.gps_sample (
    run_id UUID NOT NULL REFERENCES realtime.trip(id) ON DELETE CASCADE,
    observed_at TIMESTAMPTZ NOT NULL,
    longitude DOUBLE PRECISION NOT NULL CHECK (longitude BETWEEN -180 AND 180),
    latitude DOUBLE PRECISION NOT NULL CHECK (latitude BETWEEN -90 AND 90),
    PRIMARY KEY (run_id, observed_at)
);

CREATE INDEX idx_trip_source_route ON realtime.trip (source, route_id);

CREATE INDEX idx_stop_time_source_arrival ON realtime.stop_time (source, arrival);

CREATE INDEX idx_stop_time_source_departure ON realtime.stop_time (source, departure);

