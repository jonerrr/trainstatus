-- maybe should we instead put bus trips in same table as trains (call it vehicles)
CREATE TABLE IF NOT EXISTS bus_trips (
    -- generate uuid from mta trip id, route id, direction, vehicle
    id UUID PRIMARY KEY,
    mta_id VARCHAR NOT NULL,
    bus_id VARCHAR NOT NULL,
    -- get from start_date, number in id, or first stop update, or lastly when inserted
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    -- 0 = south, 1 = north
    direction SMALLINT NOT NULL,
    -- Delay (in seconds) can be positive (meaning that the vehicle is late) or negative (meaning that the vehicle is ahead of schedule). Delay of 0 means that the vehicle is exactly on time.
    deviation INTEGER NOT NULL,
    route_id VARCHAR NOT NULL REFERENCES routes(id),
    UNIQUE (mta_id, bus_id, created_at, direction)
);

CREATE TABLE IF NOT EXISTS bus_positions (
    trip_id UUID REFERENCES trips(id),
    stop_id VARCHAR NOT NULL REFERENCES stops(id),
    lat REAL NOT NULL,
    lon REAL NOT NULL,
    bearing REAL NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (trip_id)
);

CREATE TABLE IF NOT EXISTS bus_stop_times (
    trip_id UUID REFERENCES bus_trips(id),
    stop_id VARCHAR NOT NULL REFERENCES bus_stops(id),
    arrival TIMESTAMP WITH TIME ZONE NOT NULL,
    departure TIMESTAMP WITH TIME ZONE NOT NULL,
    stop_sequence SMALLINT NOT NULL,
    PRIMARY KEY (trip_id, stop_id)
);