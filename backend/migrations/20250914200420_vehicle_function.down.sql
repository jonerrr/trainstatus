DROP TRIGGER IF EXISTS trg_update_trip_geometry ON realtime.vehicle_position;
DROP FUNCTION IF EXISTS realtime.update_trip_geometry();
DROP FUNCTION IF EXISTS realtime.trip_geometries(integer, integer, integer);
DROP FUNCTION IF EXISTS realtime.latest_vehicle_position(integer, integer, integer);
DROP FUNCTION IF EXISTS append_point_to_linestring(geometry, geometry);