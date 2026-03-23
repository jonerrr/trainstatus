-- Function to append a point to an existing linestring
-- If the new point is the same as the last point, skip it to avoid duplicates
CREATE OR REPLACE FUNCTION append_point_to_linestring(
    existing_geom geometry(LINESTRING, 4326),
    new_point geometry
) RETURNS geometry(LINESTRING, 4326) AS $$
DECLARE
    last_point geometry;
BEGIN
    -- Get the last point of the existing linestring
    last_point := ST_EndPoint(existing_geom);

    -- Skip if the new point is the same as the last point (within ~1m tolerance)
    -- TODO: probably just do basic equality check and don't cast to geography
    IF ST_DWithin(new_point::geography, last_point::geography, 1) THEN
        RETURN existing_geom;
    END IF;

    -- Append the new point to the linestring
    RETURN ST_AddPoint(existing_geom, new_point);
END;
$$ LANGUAGE plpgsql IMMUTABLE STRICT PARALLEL SAFE;

-- Trigger function to automatically update trip_geometry when vehicle_position changes
CREATE OR REPLACE FUNCTION realtime.update_trip_geometry()
RETURNS TRIGGER AS $$
DECLARE
    existing_geom geometry;
    new_geom geometry;
BEGIN
    -- Only process if we have both a trip_id and a point geometry
    IF NEW.trip_id IS NULL OR NEW.geom IS NULL THEN
        RETURN NEW;
    END IF;

    -- Check if trip_geometry exists for this trip
    SELECT geom INTO existing_geom
    FROM realtime.trip_geometry
    WHERE trip_id = NEW.trip_id;

    IF existing_geom IS NULL THEN
        -- First point: create a new linestring with two identical points
        -- (PostGIS requires at least 2 points for a valid linestring)
        INSERT INTO realtime.trip_geometry (trip_id, geom, updated_at)
        VALUES (NEW.trip_id, ST_MakeLine(NEW.geom, NEW.geom), NEW.updated_at);
    ELSE
        -- Check if the new point is different from the last point (within ~1m)
        IF NOT ST_DWithin(NEW.geom::geography, ST_EndPoint(existing_geom)::geography, 1) THEN
            -- Append the new point to the existing linestring
            UPDATE realtime.trip_geometry
            SET geom = ST_AddPoint(existing_geom, NEW.geom),
                updated_at = NEW.updated_at
            WHERE trip_id = NEW.trip_id;
        END IF;
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create the trigger on vehicle_position
CREATE TRIGGER trg_update_trip_geometry
    AFTER INSERT OR UPDATE ON realtime.vehicle_position
    FOR EACH ROW
    EXECUTE FUNCTION realtime.update_trip_geometry();

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

-- Function to get trip geometries as MVT tiles
CREATE OR REPLACE FUNCTION realtime.trip_geometries(z integer, x integer, y integer)
RETURNS bytea AS $$
DECLARE
  mvt bytea;
BEGIN
  SELECT INTO mvt ST_AsMVT(tile, 'trip_geometries', 4096, 'geom') FROM (
    SELECT
      tg.trip_id,
      t.route_id,
      t.vehicle_id,
      tg.updated_at,
      ST_AsMVTGeom(
        ST_Transform(tg.geom, 3857),
        ST_TileEnvelope(z, x, y),
        4096, 64, true
      ) AS geom
    FROM realtime.trip_geometry tg
    JOIN realtime.trip t ON t.id = tg.trip_id
    WHERE tg.updated_at >= (now() - INTERVAL '30 minutes')
      AND tg.geom && ST_Transform(ST_TileEnvelope(z, x, y), 4326)
  ) AS tile
  WHERE geom IS NOT NULL;

  RETURN mvt;
END;
$$ LANGUAGE plpgsql STABLE STRICT PARALLEL SAFE;