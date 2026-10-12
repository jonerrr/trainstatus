TRUNCATE realtime.stop_time;

ALTER TABLE realtime.stop_time DROP CONSTRAINT stop_time_pkey;
ALTER TABLE realtime.stop_time DROP COLUMN stop_sequence;
ALTER TABLE realtime.stop_time ADD PRIMARY KEY (trip_id, stop_id, source);
