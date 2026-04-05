DROP TRIGGER IF EXISTS trg_insert_trip_history_point ON realtime.vehicle_position;
DROP FUNCTION IF EXISTS realtime.insert_trip_history_point();
DROP FUNCTION IF EXISTS realtime.latest_vehicle_position(integer, integer, integer);