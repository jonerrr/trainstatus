DROP INDEX IF EXISTS realtime.idx_trip_created_at;

DROP INDEX IF EXISTS realtime.idx_stop_time_arrival;

DROP INDEX IF EXISTS realtime.idx_vehicle_position_trip_id;
DROP INDEX IF EXISTS realtime.idx_vehicle_position_gix;
DROP INDEX IF EXISTS realtime.idx_trip_history_point_trip_id_recorded_at_unique;
DROP INDEX IF EXISTS realtime.idx_trip_history_point_gix;

DROP TRIGGER IF EXISTS trg_insert_trip_history_point ON realtime.vehicle_position;
DROP FUNCTION IF EXISTS realtime.insert_trip_history_point();

DROP TABLE IF EXISTS realtime.trip_history_point;
DROP TABLE IF EXISTS realtime.vehicle_position;

DROP TABLE IF EXISTS realtime.stop_time;

DROP TABLE IF EXISTS realtime.trip;
