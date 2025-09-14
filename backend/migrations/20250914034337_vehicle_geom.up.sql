-- compared to the other position table, this one isn't unique on vehicle_id
CREATE TABLE IF NOT EXISTS position_history (
    id SERIAL PRIMARY KEY,
    vehicle_id VARCHAR,
    mta_id VARCHAR,
    stop_id INTEGER REFERENCES stop(id),
    status VARCHAR NOT NULL,
    geom geometry(POINT, 4326),
    bearing REAL,
    passengers INTEGER,
    capacity INTEGER,
    recorded_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);