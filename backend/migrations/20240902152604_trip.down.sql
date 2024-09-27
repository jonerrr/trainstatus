DROP INDEX IF EXISTS idx_trip_created_at;

DROP INDEX IF EXISTS idx_position_updated_at;

-- DROP INDEX IF EXISTS idx_position_trip_id;
DROP INDEX IF EXISTS idx_stop_time_arrival;

DROP TABLE IF EXISTS position;

DROP TABLE IF EXISTS stop_time;

DROP TABLE IF EXISTS trip;

DROP TYPE IF EXISTS status;

DROP TYPE IF EXISTS vehicle_type;