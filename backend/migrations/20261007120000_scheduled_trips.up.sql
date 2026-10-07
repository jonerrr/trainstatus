CREATE TABLE static.scheduled_trip (
    source source_enum NOT NULL REFERENCES source(id),
    trip_id TEXT NOT NULL,
    service_date TEXT NOT NULL,
    payload JSONB NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (source, trip_id, service_date)
);
ALTER TABLE source ADD COLUMN schedules_initialized BOOLEAN NOT NULL DEFAULT FALSE;
