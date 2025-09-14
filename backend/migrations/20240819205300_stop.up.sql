CREATE TYPE borough AS ENUM (
    'brooklyn',
    'manhattan',
    'staten_island',
    'queens',
    'bronx'
);

CREATE TYPE bus_direction AS ENUM (
    'sw',
    's',
    'se',
    'e',
    'w',
    'ne',
    'nw',
    'n',
    'unknown'
);

CREATE TABLE IF NOT EXISTS stop (
    id INTEGER PRIMARY KEY,
    name VARCHAR NOT NULL,
    geom geometry(POINT, 4326) NOT NULL,
    route_type route_type NOT NULL,
    -- train fields
    ada BOOLEAN,
    north_headsign VARCHAR,
    south_headsign VARCHAR,
    -- transfers INTEGER [],
    notes VARCHAR,
    borough borough,
    -- bus fields
    direction bus_direction
);

CREATE TABLE IF NOT EXISTS stop_transfer (
    from_stop_id INTEGER REFERENCES stop(id) ON DELETE CASCADE,
    to_stop_id INTEGER REFERENCES stop(id) ON DELETE CASCADE,
    -- will probably be used to indicate if its from the official dataset or calculated by us
    transfer_type SMALLINT NOT NULL,
    min_transfer_time SMALLINT NOT NULL,
    PRIMARY KEY (from_stop_id, to_stop_id)
);

CREATE TYPE stop_type AS enum (
    'full_time',
    'part_time',
    'late_night',
    'rush_hour_one_direction',
    'rush_hour'
);

CREATE TABLE IF NOT EXISTS route_stop (
    route_id VARCHAR REFERENCES route(id) ON DELETE CASCADE,
    stop_id INTEGER REFERENCES stop(id) ON DELETE CASCADE,
    stop_sequence SMALLINT NOT NULL,
    -- train fields
    stop_type stop_type,
    -- bus fields
    headsign VARCHAR,
    direction SMALLINT,
    PRIMARY KEY (route_id, stop_id)
)