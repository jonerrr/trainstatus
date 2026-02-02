 CREATE TABLE IF NOT EXISTS realtime.trip (
    id UUID PRIMARY KEY,
    original_id VARCHAR NOT NULL,
    vehicle_id VARCHAR NOT NULL,
    route_id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    direction SMALLINT,
    -- for bus, only date part is from GTFS
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    data JSONB NOT NULL,

    -- TODO: what if we created a geom col that is updated by a trigger on position insert?

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

-- CREATE TYPE status_enum AS ENUM (
--     -- train
--     'none',
--     'incoming',
--     'at_stop',
--     'in_transit_to',
--     -- bus
--     'spooking',
--     'layover',
--     'no_progress'
-- );

-- CREATE TYPE vehicle_enum AS ENUM ('train', 'bus');
-- CREATE TABLE IF NOT EXISTS position (
--     vehicle_id VARCHAR PRIMARY KEY,
--     original_id VARCHAR,
--     stop_id VARCHAR REFERENCES static.stop(id),
--     updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
--     data JSONB NOT NULL,
--     -- -- status status NOT NULL,
--     -- bus JSONB,
--     -- train JSONB
--     -- train
--     -- trip_id UUID UNIQUE REFERENCES trip(id) ON DELETE CASCADE,
--     -- current_stop_sequence SMALLINT,
--     -- bus
--     -- -- vehicle_type vehicle_type,
--     -- lat REAL,
--     -- lon REAL,
--     -- bearing REAL,
--     -- passengers INTEGER,
--     -- capacity INTEGER
-- );

CREATE TABLE IF NOT EXISTS realtime.position (
    id UUID PRIMARY KEY,
    vehicle_id VARCHAR NOT NULL,
    original_id VARCHAR,
    stop_id VARCHAR,
    source source_enum NOT NULL,
    data JSONB NOT NULL,
    geom geometry(POINT, 4326),
    recorded_at TIMESTAMP WITH TIME ZONE NOT NULL,

    FOREIGN KEY (stop_id, source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);

CREATE INDEX idx_position_recorded_at ON realtime.position (recorded_at);
CREATE INDEX idx_position_vehicle_id ON realtime.position (vehicle_id);
CREATE INDEX idx_position_original_id ON realtime.position (original_id);
CREATE INDEX idx_position_gix ON realtime.position USING GIST(geom);

CREATE INDEX idx_trip_created_at ON realtime.trip (created_at);

CREATE INDEX idx_stop_time_arrival ON realtime.stop_time (arrival);

-- CREATE INDEX idx_position_updated_at ON realtime.position (updated_at);

-- CREATE INDEX idx_position_trip_id ON realtime.position (trip_id);