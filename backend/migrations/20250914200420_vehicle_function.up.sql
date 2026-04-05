-- Trigger function to store trip history points from vehicle_position changes
CREATE OR REPLACE FUNCTION realtime.insert_trip_history_point()
RETURNS TRIGGER AS $$
BEGIN
    -- Only process if we have both a trip_id and a point geometry
    IF NEW.trip_id IS NULL OR NEW.geom IS NULL THEN
        RETURN NEW;
    END IF;

    INSERT INTO realtime.trip_history_point (trip_id, geom, recorded_at)
    VALUES (NEW.trip_id, NEW.geom, NEW.updated_at)
    ON CONFLICT DO NOTHING;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create the trigger on vehicle_position
CREATE TRIGGER trg_insert_trip_history_point
    AFTER INSERT OR UPDATE ON realtime.vehicle_position
    FOR EACH ROW
    EXECUTE FUNCTION realtime.insert_trip_history_point();

CREATE OR REPLACE FUNCTION realtime.latest_vehicle_position(z integer, x integer, y integer)
RETURNS bytea AS $$
DECLARE
  mvt bytea;
BEGIN
  SELECT INTO mvt ST_AsMVT(tile, 'latest_vehicle_position', 4096, 'geom') FROM (
    SELECT
      vp.vehicle_id,
      vp.trip_id,
      vp.stop_id,
      vp.data->>'status' as status,
      (vp.data->>'bearing')::float as bearing,
      (vp.data->>'passengers')::int as passengers,
      (vp.data->>'capacity')::int as capacity,
      vp.updated_at,
      ST_AsMVTGeom(
        ST_Transform(vp.geom, 3857),
        ST_TileEnvelope(z, x, y),
        4096, 64, true
      ) AS geom
    FROM realtime.vehicle_position vp
    WHERE vp.updated_at >= (now() - INTERVAL '5 minutes')
      AND vp.geom && ST_Transform(ST_TileEnvelope(z, x, y), 4326)
  ) AS tile
  WHERE geom IS NOT NULL;

  RETURN mvt;
END;
$$ LANGUAGE plpgsql STABLE STRICT PARALLEL SAFE;
