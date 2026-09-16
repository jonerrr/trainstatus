CREATE TABLE IF NOT EXISTS realtime.trip (
    id UUID PRIMARY KEY,
    original_id VARCHAR NOT NULL,
    vehicle_id VARCHAR NOT NULL,
    route_id VARCHAR NOT NULL,
    shape_ids VARCHAR[] NOT NULL,
    source source_enum NOT NULL,
    direction SMALLINT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    data JSONB NOT NULL,

    UNIQUE (original_id, vehicle_id, created_at, direction),
    FOREIGN KEY (route_id, source) REFERENCES static.route(id, source) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS realtime.stop_time (
    trip_id UUID REFERENCES realtime.trip(id) ON DELETE CASCADE,
    stop_id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    arrival TIMESTAMP WITH TIME ZONE NOT NULL,
    departure TIMESTAMP WITH TIME ZONE NOT NULL,
    data JSONB NOT NULL,

    PRIMARY KEY (trip_id, stop_id, source),
    FOREIGN KEY (stop_id, source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);

-- Current vehicle position (upserted, no history - just latest state)
CREATE TABLE IF NOT EXISTS realtime.vehicle_position (
    vehicle_id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    trip_id UUID REFERENCES realtime.trip(id) ON DELETE SET NULL,
    stop_id VARCHAR,
    geom geometry(POINT, 4326),
    data JSONB NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    PRIMARY KEY (vehicle_id, source),
    FOREIGN KEY (stop_id, source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);

CREATE INDEX idx_vehicle_position_trip_id ON realtime.vehicle_position (trip_id);
-- TODO: might not need this index
CREATE INDEX idx_vehicle_position_gix ON realtime.vehicle_position USING GIST(geom);

-- Trip history points
CREATE TABLE IF NOT EXISTS realtime.trip_history_point (
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    trip_id UUID NOT NULL REFERENCES realtime.trip(id) ON DELETE CASCADE,
    geom geometry(POINT, 4326) NOT NULL,
    recorded_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Trigger function to store trip history points from vehicle_position changes
CREATE OR REPLACE FUNCTION realtime.insert_trip_history_point()
RETURNS TRIGGER AS $$
BEGIN
    -- Only process if we have both a trip_id and a point geometry
    IF NEW.trip_id IS NULL OR NEW.geom IS NULL THEN
        RETURN NEW;
    END IF;

    -- If this is an UPDATE, check if the data actually changed
    IF TG_OP = 'UPDATE' THEN
        -- If the timestamp is the exact same, don't even attempt the insert.
        -- This saves Postgres from having to fire the ON CONFLICT logic.
        IF NEW.updated_at = OLD.updated_at THEN
            RETURN NEW;
        END IF;
    END IF;

    -- 3. Perform the insert using the explicit unique constraint
    INSERT INTO realtime.trip_history_point (trip_id, geom, recorded_at)
    VALUES (NEW.trip_id, NEW.geom, NEW.updated_at)
    ON CONFLICT (trip_id, recorded_at) DO NOTHING;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create the trigger on vehicle_position
CREATE TRIGGER trg_insert_trip_history_point
    AFTER INSERT OR UPDATE ON realtime.vehicle_position
    FOR EACH ROW
    EXECUTE FUNCTION realtime.insert_trip_history_point();

CREATE UNIQUE INDEX idx_trip_history_point_trip_id_recorded_at_unique
    ON realtime.trip_history_point (trip_id, recorded_at);

CREATE INDEX idx_trip_history_point_gix ON realtime.trip_history_point USING GIST(geom);

CREATE INDEX idx_trip_created_at ON realtime.trip (created_at);

CREATE INDEX idx_stop_time_arrival ON realtime.stop_time (arrival);
