CREATE TABLE IF NOT EXISTS trips (
    -- probably should be a different type, no need for UUID
    id UUID PRIMARY KEY,
    mta_id VARCHAR NOT NULL,
    train_id VARCHAR NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    assigned BOOLEAN NOT NULL,
    -- 0 = south, 1 = north
    direction SMALLINT NOT NULL,
    route_id VARCHAR NOT NULL REFERENCES routes(id),
    express BOOLEAN NOT NULL,
    UNIQUE (mta_id, train_id, created_at, direction)
);

CREATE TABLE IF NOT EXISTS positions (
    trip_id UUID REFERENCES trips(id),
    stop_id VARCHAR NOT NULL REFERENCES stops(id),
    -- 0 = incoming, 1 = at stop, 2 = in transit to
    train_status SMALLINT,
    current_stop_sequence SMALLINT,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (trip_id)
);

CREATE TABLE IF NOT EXISTS stop_times (
    trip_id UUID REFERENCES trips(id),
    stop_id VARCHAR NOT NULL REFERENCES stops(id),
    arrival TIMESTAMP WITH TIME ZONE NOT NULL,
    departure TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (trip_id, stop_id)
);