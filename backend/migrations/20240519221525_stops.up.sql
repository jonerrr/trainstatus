CREATE TABLE IF NOT EXISTS stops (
    id VARCHAR PRIMARY KEY,
    name VARCHAR NOT NULL,
    ada BOOLEAN NOT NULL,
    notes VARCHAR,
    borough VARCHAR NOT NULL,
    north_headsign VARCHAR NOT NULL,
    south_headsign VARCHAR NOT NULL
);

CREATE TABLE IF NOT EXISTS route_stops (
    route_id VARCHAR REFERENCES routes(id),
    stop_id VARCHAR REFERENCES stops(id),
    stop_sequence SMALLINT NOT NULL,
    stop_type SMALLINT NOT NULL,
    PRIMARY KEY (route_id, stop_id, stop_sequence)
)