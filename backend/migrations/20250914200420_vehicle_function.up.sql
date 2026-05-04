-- CREATE OR REPLACE FUNCTION realtime.latest_vehicle_position(z integer, x integer, y integer)
-- RETURNS bytea AS $$
-- DECLARE
--   mvt bytea;
-- BEGIN
--   SELECT INTO mvt ST_AsMVT(tile, 'latest_vehicle_position', 4096, 'geom') FROM (
--     SELECT
--       vp.vehicle_id,
--       vp.trip_id,
--       vp.stop_id,
--       vp.data->>'status' as status,
--       (vp.data->>'bearing')::float as bearing,
--       (vp.data->>'passengers')::int as passengers,
--       (vp.data->>'capacity')::int as capacity,
--       vp.updated_at,
--       ST_AsMVTGeom(
--         ST_Transform(vp.geom, 3857),
--         ST_TileEnvelope(z, x, y),
--         4096, 64, true
--       ) AS geom
--     FROM realtime.vehicle_position vp
--     WHERE vp.updated_at >= (now() - INTERVAL '5 minutes')
--       AND vp.geom && ST_Transform(ST_TileEnvelope(z, x, y), 4326)
--   ) AS tile
--   WHERE geom IS NOT NULL;

--   RETURN mvt;
-- END;
-- $$ LANGUAGE plpgsql STABLE STRICT PARALLEL SAFE;

CREATE OR REPLACE FUNCTION realtime.active_route_shapes(z integer, x integer, y integer)
RETURNS bytea AS $$
DECLARE
  mvt bytea;
BEGIN
  SELECT INTO mvt ST_AsMVT(tile, 'active_route_shapes', 4096, 'geom') FROM (
    SELECT
      r.id AS route_id,
      r.short_name,
      r.long_name,
      r.color,
      r.text_color,
      active_shapes.shape_id,
      ST_AsMVTGeom(
        ST_Transform(s.geom, 3857),
        ST_TileEnvelope(z, x, y),
        4096, 64, true
      ) AS geom
    FROM (
      SELECT DISTINCT
        t.route_id,
        t.source,
        unnest(t.shape_ids) AS shape_id
      FROM realtime.trip t
      WHERE t.updated_at >= (now() - INTERVAL '5 minutes')
    ) active_shapes
    JOIN static.shape s ON s.id = active_shapes.shape_id AND s.source = active_shapes.source
    JOIN static.route r ON r.id = active_shapes.route_id AND r.source = active_shapes.source
    WHERE s.geom && ST_Transform(ST_TileEnvelope(z, x, y), 4326)
  ) AS tile
  WHERE geom IS NOT NULL;

  RETURN mvt;
END;
$$ LANGUAGE plpgsql STABLE STRICT PARALLEL SAFE;
