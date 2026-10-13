-- Current network membership (retained identity rows are deliberately excluded).
SELECT source, count(*) FROM static.route WHERE current GROUP BY source;
SELECT source, count(*) FROM static.stop WHERE current GROUP BY source;

-- Latest retained runs eligible in the inclusive four-hour arrival/departure window.
-- Replace the example source/time when inspecting another retained selection.
EXPLAIN (ANALYZE, BUFFERS)
SELECT t.id FROM realtime.trip t
WHERE t.source = 'mta_bus' AND EXISTS (
    SELECT 1 FROM realtime.stop_time v WHERE v.trip_id = t.id AND v.source = t.source
    AND (v.arrival BETWEEN TIMESTAMPTZ '2026-05-28 20:26:40+00' AND TIMESTAMPTZ '2026-05-29 00:26:40+00'
      OR v.departure BETWEEN TIMESTAMPTZ '2026-05-28 20:26:40+00' AND TIMESTAMPTZ '2026-05-29 00:26:40+00')
);

-- Table/index/TOAST space is physical space, not a count of record versions.
SELECT n.nspname, c.relname, pg_table_size(c.oid) AS table_bytes,
       pg_indexes_size(c.oid) AS index_bytes, pg_total_relation_size(c.oid) AS total_bytes,
       CASE WHEN c.reltoastrelid = 0 THEN 0 ELSE pg_total_relation_size(c.reltoastrelid) END AS toast_bytes
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind = 'r' AND n.nspname IN ('static', 'realtime') ORDER BY n.nspname, c.relname;

-- GPS sample growth is separate from latest-record growth.
SELECT count(*) AS run_count FROM realtime.trip;
SELECT count(*) AS visit_count FROM realtime.stop_time;
SELECT count(*) AS observation_count FROM realtime.vehicle_position;
SELECT count(*) AS sample_count FROM realtime.gps_sample;
SELECT count(*) AS asset_count FROM static.shape_asset;
SELECT count(*) AS shape_reference_count FROM realtime.run_shape;
