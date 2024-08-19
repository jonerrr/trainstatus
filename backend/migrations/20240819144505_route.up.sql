CREATE TYPE route_type AS ENUM ('train', 'bus');

CREATE TABLE IF NOT EXISTS route (
    id VARCHAR PRIMARY KEY,
    long_name VARCHAR NOT NULL,
    short_name VARCHAR NOT NULL,
    color VARCHAR NOT NULL,
    shuttle BOOLEAN NOT NULL,
    geom JSONB NOT NULL,
    route_type route_type NOT NULL
);