-- Trip patterns and the NJT child-stop remap live only on the in-memory
-- static index. Persist them beside the source timestamp so a restart can
-- rebuild that index from Postgres while the static data is still fresh.
ALTER TABLE source
    ADD COLUMN IF NOT EXISTS trip_patterns JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS stop_remap JSONB NOT NULL DEFAULT '{}'::jsonb;
