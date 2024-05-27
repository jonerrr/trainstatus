CREATE TABLE IF NOT EXISTS alerts (
    id UUID PRIMARY KEY,
    mta_id VARCHAR NOT NULL,
    alert_type VARCHAR(255) NOT NULL,
    header_html VARCHAR NOT NULL,
    header_plain VARCHAR NOT NULL,
    description_html VARCHAR,
    description_plain VARCHAR,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    display_before_active INTEGER,
    in_feed BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE IF NOT EXISTS active_periods(
    alert_id UUID NOT NULL REFERENCES alerts(id) ON DELETE CASCADE,
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    end_time TIMESTAMP WITH TIME ZONE
);

CREATE TABLE IF NOT EXISTS affected_entities(
    alert_id UUID NOT NULL REFERENCES alerts(id) ON DELETE CASCADE,
    route_id VARCHAR REFERENCES routes(id),
    stop_id VARCHAR REFERENCES stops(id),
    sort_order INTEGER NOT NULL
);

CREATE INDEX idx_alerts_id ON alerts (id);

CREATE INDEX idx_active_periods_alert_id_start_time_end_time ON active_periods (alert_id, start_time, end_time);

CREATE INDEX idx_affected_entities_alert_id_route_id ON affected_entities (alert_id, route_id);