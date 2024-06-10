CREATE TABLE IF NOT EXISTS bus_routes (
    id VARCHAR PRIMARY KEY,
    long_name VARCHAR NOT NULL,
    short_name VARCHAR NOT NULL,
    color VARCHAR NOT NULL,
    -- 1 = bus, 2 = shuttle
    route_type SMALLINT NOT NULL
);