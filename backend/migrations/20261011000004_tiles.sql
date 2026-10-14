CREATE OR REPLACE FUNCTION realtime.active_route_shapes(z integer, x integer, y integer)
RETURNS bytea AS $$
DECLARE
  mvt bytea;
BEGIN
  SELECT INTO mvt ST_AsMVT(tile, 'active_route_shapes', 4096, 'geom') FROM (
    WITH active_trips AS (
      SELECT DISTINCT
        t.id,
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
        ST_LineMerge(ST_MakeLine(s.geom ORDER BY us.ordinal)) AS geom
      FROM active_trips at
      JOIN realtime.run_shape us ON us.run_id = at.id
      JOIN static.shape_asset s ON s.id = us.asset_id AND s.source = at.source
      GROUP BY at.id, at.route_id, at.source, at.shape_ids
    )
    SELECT
      r.id AS id,
      r.id AS route_id,
      tl.source,
      r.short_name,
      r.long_name,
      r.color,
      r.text_color,
      CASE WHEN r.source = 'mta_bus' THEN COALESCE(r.data->'service_types' ? 'SHUTTLE', false) END AS shuttle,
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
                    "text_color": "String",
                    "shuttle": "Boolean"
                }
            }
        ]
    }
    $$::json || '$tj$';
END $do$;


-- Map stops are current membership only. Retained stop identities never leak into
-- tiles, and internal projected geometry/assets are not auto-published by Martin.
CREATE FUNCTION realtime.stop(z integer, x integer, y integer)
RETURNS bytea AS $$
    SELECT COALESCE(ST_AsMVT(tile, 'stop', 4096, 'geom'), ''::bytea)
    FROM (
        SELECT s.id, s.source::text AS source, s.name,
            s.data->>'north_headsign' AS north_headsign,
            s.data->>'south_headsign' AS south_headsign,
            s.data->>'direction' AS direction,
            ST_AsMVTGeom(ST_Transform(s.geom, 3857), ST_TileEnvelope(z, x, y), 4096, 64, true) AS geom
        FROM static.stop s
        WHERE s.current AND s.geom && ST_Transform(ST_TileEnvelope(z, x, y, margin => 64.0 / 4096), 4326)
    ) tile WHERE geom IS NOT NULL;
$$ LANGUAGE sql STABLE STRICT PARALLEL SAFE;
COMMENT ON FUNCTION realtime.stop(integer, integer, integer) IS '{"description":"Current network stops","vector_layers":[{"id":"stop","fields":{"id":"String","source":"String","name":"String","north_headsign":"String","south_headsign":"String","direction":"String"}}]}';
