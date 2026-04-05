DROP INDEX IF EXISTS realtime.idx_trip_created_at;

DROP INDEX IF EXISTS realtime.idx_stop_time_arrival;

DROP INDEX IF EXISTS realtime.idx_vehicle_position_trip_id;
DROP INDEX IF EXISTS realtime.idx_vehicle_position_gix;
DROP INDEX IF EXISTS realtime.idx_trip_history_point_trip_id_recorded_at_geom_unique;
DROP INDEX IF EXISTS realtime.idx_trip_history_point_trip_id_recorded_at;
DROP INDEX IF EXISTS realtime.idx_trip_history_point_gix;

DROP TABLE IF EXISTS realtime.trip_history_point;
DROP TABLE IF EXISTS realtime.vehicle_position;

DROP TABLE IF EXISTS realtime.stop_time;

DROP TABLE IF EXISTS realtime.trip;

-- DROP TYPE IF EXISTS status;

-- DROP TYPE IF EXISTS vehicle_type;