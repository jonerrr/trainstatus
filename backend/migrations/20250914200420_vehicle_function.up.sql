CREATE OR REPLACE FUNCTION realtime.latest_vehicle_position(z integer, x integer, y integer)
RETURNS bytea AS $$
DECLARE
  mvt bytea;
BEGIN
  SELECT INTO mvt ST_AsMVT(tile, 'latest_vehicle_position', 4096, 'geom') FROM (
    SELECT DISTINCT ON (vehicle_id)
      vehicle_id,
      original_id,
      stop_id,
      status,
      bearing,
      passengers,
      capacity,
      recorded_at,
      ST_AsMVTGeom(
        ST_Transform(geom, 3857),
        ST_TileEnvelope(z, x, y),
        4096, 64, true
      ) AS geom
    FROM realtime.position
    WHERE recorded_at >= (now() - INTERVAL '5 minutes')
      AND geom && ST_Transform(ST_TileEnvelope(z, x, y), 4326)
    ORDER BY vehicle_id, recorded_at DESC
  ) AS tile
  WHERE geom IS NOT NULL;

  RETURN mvt;
END;
$$ LANGUAGE plpgsql STABLE STRICT PARALLEL SAFE;

-- CREATE OR REPLACE FUNCTION realtime.points_to_multilinestring(
--     z integer,
--     x integer,
--     y integer,
--     query_params json
-- )  
-- RETURNS bytea AS $$
-- DECLARE
--   mvt bytea;
--   target_id text;
-- BEGIN
--   -- Extract the ID from query parameters
--   target_id := query_params->>'id';  
    
--   SELECT INTO mvt ST_AsMVT(tile, 'multilinestring_layer', 4096, 'geom') FROM (  
--     SELECT  
--       target_id as feature_id,  
--       ST_AsMVTGeom(  
--           ST_Transform(  
--               ST_MakeLine(  
--                   ST_Transform(geom, 3857) ORDER BY some_order_column  
--               ),   
--               3857  
--           ),  
--           ST_TileEnvelope(z, x, y),  
--           4096, 64, true  
--       ) AS geom  
--     FROM your_points_table  
--     WHERE your_id_column = target_id  
--       AND geom && ST_Transform(ST_TileEnvelope(z, x, y), 4326)  
--     GROUP BY target_id  
--   ) as tile WHERE geom IS NOT NULL;  
  
--   RETURN mvt;  
-- END  
-- $$ LANGUAGE plpgsql IMMUTABLE STRICT PARALLEL SAFE;