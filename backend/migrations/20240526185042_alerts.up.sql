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
    display_before_active INTEGER
);

CREATE TABLE IF NOT EXISTS active_periods(
    alert_id UUID NOT NULL REFERENCES alerts(id),
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    end_time TIMESTAMP WITH TIME ZONE
);

CREATE TABLE IF NOT EXISTS affected_entities(
    alert_id UUID NOT NULL REFERENCES alerts(id),
    route_id VARCHAR REFERENCES routes(id),
    stop_id VARCHAR REFERENCES stops(id),
    sort_order INTEGER NOT NULL
);