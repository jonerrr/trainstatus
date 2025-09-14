DROP INDEX IF EXISTS realtime.idx_trip_created_at;

-- DROP INDEX IF EXISTS idx_position_updated_at;

-- DROP INDEX IF EXISTS idx_position_trip_id;
DROP INDEX IF EXISTS realtime.idx_stop_time_arrival;

DROP INDEX IF EXISTS realtime.idx_position_recorded_at;
DROP INDEX IF EXISTS realtime.idx_position_vehicle_id;
DROP INDEX IF EXISTS realtime.idx_position_mta_id;
DROP INDEX IF EXISTS realtime.idx_position_gix;

DROP TABLE IF EXISTS realtime.position;

DROP TABLE IF EXISTS realtime.stop_time;

DROP TABLE IF EXISTS realtime.trip;

-- DROP TYPE IF EXISTS status;

-- DROP TYPE IF EXISTS vehicle_type;