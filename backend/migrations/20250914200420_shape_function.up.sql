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
    WITH active_trips AS (
      SELECT DISTINCT
        t.route_id,
        t.source,
        t.shape_ids
      FROM realtime.trip t
      WHERE t.updated_at >= (now() - INTERVAL '5 minutes')
        AND array_length(t.shape_ids, 1) > 0
    ),
    trip_lines AS (
      SELECT
        at.route_id,
        at.source,
        ST_LineMerge(ST_MakeLine(s.geom ORDER BY us.ord)) AS geom
      FROM active_trips at
      JOIN LATERAL unnest(at.shape_ids) WITH ORDINALITY AS us(shape_id, ord) ON TRUE
      JOIN static.shape s ON s.id = us.shape_id AND s.source = at.source
      GROUP BY at.route_id, at.source, at.shape_ids
    )
    SELECT
      r.id AS id,
      r.id AS route_id,
      tl.source,
      r.short_name,
      r.long_name,
      r.color,
      r.text_color,
      ST_AsMVTGeom(
        ST_Transform(tl.geom, 3857),
        ST_TileEnvelope(z, x, y),
        4096, 64, true
      ) AS geom
    FROM trip_lines tl
    JOIN static.route r ON r.id = tl.route_id AND r.source = tl.source
    WHERE tl.geom && ST_Transform(ST_TileEnvelope(z, x, y), 4326)
  ) AS tile
  WHERE geom IS NOT NULL;

  RETURN mvt;
END;
$$ LANGUAGE plpgsql STABLE STRICT PARALLEL SAFE;

DO $do$ BEGIN
    EXECUTE 'COMMENT ON FUNCTION realtime.active_route_shapes(integer, integer, integer) IS $tj$' || $$
    {
        "description": "Realtime route shapes built from active trip shape_ids",
        "vector_layers": [
            {
                "id": "active_route_shapes",
                "fields": {
                    "id": "String",
                    "route_id": "String",
                    "source": "String",
                    "short_name": "String",
                    "long_name": "String",
                    "color": "String",
                    "text_color": "String"
                }
            }
        ]
    }
    $$::json || '$tj$';
END $do$;
