CREATE TABLE IF NOT EXISTS trips (
    -- probably should be a different type, no need for UUID
    id UUID PRIMARY KEY,
    mta_trip_id VARCHAR NOT NULL,
    train_id VARCHAR NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    assigned BOOLEAN NOT NULL,
    -- 0 = south, 1 = north
    direction SMALLINT NOT NULL,
    route_id VARCHAR NOT NULL REFERENCES routes(id),
    -- headsign VARCHAR REFERENCES headsigns(id),
    UNIQUE (mta_trip_id, train_id, created_at, direction)
);

CREATE TABLE IF NOT EXISTS positions (
    trip_id UUID REFERENCES trips(id),
    stop_id VARCHAR NOT NULL REFERENCES stops(id),
    train_status SMALLINT NOT NULL,
    current_stop_sequence SMALLINT,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (trip_id)
);

CREATE TABLE IF NOT EXISTS stop_times (
    trip_id UUID REFERENCES trips(id),
    stop_id VARCHAR NOT NULL REFERENCES stops(id),
    arrival TIMESTAMP WITH TIME ZONE,
    departure TIMESTAMP WITH TIME ZONE,
    PRIMARY KEY (trip_id, stop_id)
);