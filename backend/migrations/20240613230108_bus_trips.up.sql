-- maybe should we instead put bus trips in same table as trains (call it vehicles)
CREATE TABLE IF NOT EXISTS bus_trips (
    id UUID PRIMARY KEY,
    mta_id VARCHAR NOT NULL,
    vehicle_id INTEGER NOT NULL,
    start_date DATE NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    -- 0 = south, 1 = north
    direction SMALLINT NOT NULL,
    -- Delay (in seconds) can be positive (meaning that the vehicle is late) or negative (meaning that the vehicle is ahead of schedule)
    deviation INTEGER,
    route_id VARCHAR NOT NULL REFERENCES bus_routes(id),
    UNIQUE (mta_id, vehicle_id, start_date, direction)
);

CREATE TABLE IF NOT EXISTS bus_positions (
    vehicle_id INTEGER PRIMARY KEY,
    stop_id INTEGER NOT NULL REFERENCES bus_stops(id),
    mta_id VARCHAR,
    lat REAL NOT NULL,
    lon REAL NOT NULL,
    bearing REAL NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    -- SIRI STUFF
    progress_status VARCHAR,
    passengers INTEGER,
    capacity INTEGER
);

CREATE TABLE IF NOT EXISTS bus_stop_times (
    trip_id UUID REFERENCES bus_trips(id) ON DELETE CASCADE,
    stop_id INTEGER NOT NULL REFERENCES bus_stops(id),
    arrival TIMESTAMP WITH TIME ZONE NOT NULL,
    departure TIMESTAMP WITH TIME ZONE NOT NULL,
    stop_sequence SMALLINT NOT NULL,
    PRIMARY KEY (trip_id, stop_id)
);

CREATE INDEX idx_bus_trips_created_at ON bus_trips (created_at);

CREATE INDEX idx_bus_positions_updated_at ON bus_positions (updated_at);

CREATE INDEX idx_bus_stop_times_arrival ON bus_stop_times (arrival);