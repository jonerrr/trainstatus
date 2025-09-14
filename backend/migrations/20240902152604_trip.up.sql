CREATE TABLE IF NOT EXISTS realtime.trip (
    id UUID PRIMARY KEY,
    mta_id VARCHAR NOT NULL,
    vehicle_id VARCHAR NOT NULL,
    route_id VARCHAR NOT NULL REFERENCES static.route(id),
    direction SMALLINT,
    -- for bus, only date part is from GTFS
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    deviation INTEGER,
    -- train fields
    express BOOLEAN,
    assigned BOOLEAN,
    UNIQUE (mta_id, vehicle_id, created_at, direction)
);

CREATE TABLE IF NOT EXISTS realtime.stop_time (
    trip_id UUID REFERENCES realtime.trip(id) ON DELETE CASCADE,
    stop_id INTEGER REFERENCES static.stop(id),
    arrival TIMESTAMP WITH TIME ZONE NOT NULL,
    departure TIMESTAMP WITH TIME ZONE NOT NULL,
    scheduled_track VARCHAR,
    actual_track VARCHAR,
    PRIMARY KEY (trip_id, stop_id)
);

-- CREATE TYPE status AS ENUM (
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

-- CREATE TYPE vehicle_type AS ENUM ('train', 'bus');
-- CREATE TABLE IF NOT EXISTS position (
--     vehicle_id VARCHAR PRIMARY KEY,
--     mta_id VARCHAR,
--     stop_id INTEGER REFERENCES stop(id),
--     updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
--     status status NOT NULL,
--     -- train
--     -- trip_id UUID UNIQUE REFERENCES trip(id) ON DELETE CASCADE,
--     -- current_stop_sequence SMALLINT,
--     -- bus
--     -- vehicle_type vehicle_type,
--     lat REAL,
--     lon REAL,
--     bearing REAL,
--     passengers INTEGER,
--     capacity INTEGER
-- );

CREATE TABLE IF NOT EXISTS realtime.position (
    id SERIAL PRIMARY KEY,
    vehicle_id VARCHAR NOT NULL,
    mta_id VARCHAR,
    stop_id INTEGER REFERENCES static.stop(id),
    status VARCHAR,
    bearing REAL,
    passengers INTEGER,
    capacity INTEGER,
    geom geometry(POINT, 4326),
    recorded_at TIMESTAMP WITH TIME ZONE NOT NULL
);

CREATE INDEX idx_position_recorded_at ON realtime.position (recorded_at);
CREATE INDEX idx_position_vehicle_id ON realtime.position (vehicle_id);
CREATE INDEX idx_position_mta_id ON realtime.position (mta_id);
CREATE INDEX idx_position_gix ON realtime.position USING GIST(geom);

CREATE INDEX idx_trip_created_at ON realtime.trip (created_at);

CREATE INDEX idx_stop_time_arrival ON realtime.stop_time (arrival);

-- CREATE INDEX idx_position_updated_at ON realtime.position (updated_at);

-- CREATE INDEX idx_position_trip_id ON realtime.position (trip_id);