CREATE TABLE IF NOT EXISTS bus_stops (
    id INTEGER PRIMARY KEY,
    name VARCHAR NOT NULL,
    direction VARCHAR NOT NULL,
    lat REAL NOT NULL,
    lon REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS bus_route_stops (
    route_id VARCHAR REFERENCES bus_routes(id),
    stop_id INTEGER REFERENCES bus_stops(id),
    stop_sequence INTEGER NOT NULL,
    headsign VARCHAR NOT NULL,
    -- can be 1 or 0
    -- each route has two directions with separate stops
    direction INTEGER NOT NULL,
    PRIMARY KEY (route_id, stop_id, stop_sequence)
);

-- CREATE TABLE IF NOT EXISTS bus_stop_groups (
--     route_id VARCHAR REFERENCES bus_routes(id),
--     -- same direction as the route stops
--     direction INTEGER NOT NULL,
--     headsign VARCHAR NOT NULL,
--     PRIMARY KEY (route_id, direction)
-- );