CREATE EXTENSION IF NOT EXISTS postgis;

CREATE TYPE static.route_type AS ENUM ('train', 'bus');

CREATE TABLE IF NOT EXISTS static.route (
    id VARCHAR PRIMARY KEY,
    long_name VARCHAR NOT NULL,
    short_name VARCHAR NOT NULL,
    color VARCHAR NOT NULL,
    shuttle BOOLEAN NOT NULL,
    geom geometry(MULTILINESTRING, 4326),
    route_type static.route_type NOT NULL
);