-- A trip can visit the same stop more than once, so visits are keyed by sequence.
-- Retained rows have no recorded sequence and are discarded rather than backfilled.
TRUNCATE realtime.stop_time;

ALTER TABLE realtime.stop_time DROP CONSTRAINT stop_time_pkey;
ALTER TABLE realtime.stop_time ADD COLUMN stop_sequence INTEGER NOT NULL;
ALTER TABLE realtime.stop_time ADD PRIMARY KEY (source, trip_id, stop_sequence);
