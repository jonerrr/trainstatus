CREATE TABLE IF NOT EXISTS trip (
    id UUID PRIMARY KEY,
    mta_id VARCHAR NOT NULL,
    vehicle_id VARCHAR NOT NULL,
    route_id VARCHAR NOT NULL REFERENCES route(id),
    direction SMALLINT,
    -- for bus, only date part is from GTFS
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    deviation INTEGER,
    -- train fields
    express BOOLEAN,
    assigned BOOLEAN,
    UNIQUE (mta_id, vehicle_id, created_at, direction)
);

CREATE TABLE IF NOT EXISTS stop_time (
    trip_id UUID REFERENCES trip(id) ON DELETE CASCADE,
    stop_id INTEGER REFERENCES stop(id),
    arrival TIMESTAMP WITH TIME ZONE NOT NULL,
    departure TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (trip_id, stop_id)
);

CREATE TYPE status AS ENUM (
    -- train
    'none',
    'incoming',
    'at_stop',
    'in_transit_to',
    -- bus
    'spooking',
    'layover',
    'no_progress'
);

CREATE TABLE IF NOT EXISTS position (
    vehicle_id VARCHAR PRIMARY KEY,
    mta_id VARCHAR,
    stop_id INTEGER REFERENCES stop(id),
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    status status NOT NULL,
    -- train
    -- trip_id UUID UNIQUE REFERENCES trip(id) ON DELETE CASCADE,
    -- current_stop_sequence SMALLINT,
    -- bus
    lat REAL,
    lon REAL,
    bearing REAL,
    passengers INTEGER,
    capacity INTEGER
);

CREATE INDEX idx_trip_created_at ON trip (created_at);

CREATE INDEX idx_stop_time_arrival ON stop_time (arrival);

CREATE INDEX idx_position_updated_at ON position (updated_at);

-- CREATE INDEX idx_position_trip_id ON position (trip_id);