CREATE TYPE borough AS enum (
    'brooklyn',
    'manhattan',
    'staten_island',
    'queens',
    'bronx'
);

CREATE TABLE IF NOT EXISTS stop (
    id INTEGER PRIMARY KEY,
    name VARCHAR NOT NULL,
    lat REAL NOT NULL,
    lon REAL NOT NULL,
    -- Train fields
    ada BOOLEAN,
    north_headsign VARCHAR,
    south_headsign VARCHAR,
    transfers INTEGER [],
    notes VARCHAR,
    borough borough,
    -- bus fields
    direction VARCHAR
)