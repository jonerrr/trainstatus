CREATE TABLE IF NOT EXISTS alert (
    id UUID PRIMARY KEY,
    mta_id VARCHAR NOT NULL,
    alert_type VARCHAR(255) NOT NULL,
    header_html VARCHAR NOT NULL,
    header_plain VARCHAR NOT NULL,
    description_html VARCHAR,
    description_plain VARCHAR,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    last_in_feed TIMESTAMP WITH TIME ZONE NOT NULL,
    display_before_active INTEGER
);

CREATE TABLE IF NOT EXISTS active_period(
    alert_id UUID NOT NULL REFERENCES alert(id) ON DELETE CASCADE,
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    end_time TIMESTAMP WITH TIME ZONE,
    PRIMARY KEY (alert_id, start_time)
);

CREATE TABLE IF NOT EXISTS affected_entity(
    alert_id UUID NOT NULL REFERENCES alert(id) ON DELETE CASCADE,
    route_id VARCHAR REFERENCES route(id),
    stop_id INTEGER REFERENCES stop(id),
    sort_order INTEGER NOT NULL,
    UNIQUE (
        alert_id,
        route_id,
        stop_id,
        sort_order
    )
);

CREATE INDEX idx_active_period_alert_id_start_time_end_time ON active_period (alert_id, start_time, end_time);

CREATE INDEX idx_affected_entity_alert_id_route_id ON affected_entity (alert_id, route_id);