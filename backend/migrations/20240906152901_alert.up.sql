CREATE TABLE IF NOT EXISTS realtime.alert (
    id SERIAL PRIMARY KEY,
    mta_id VARCHAR NOT NULL,
    alert_type VARCHAR(255) NOT NULL,
    header_html VARCHAR NOT NULL,
    header_plain VARCHAR NOT NULL,
    description_html VARCHAR,
    description_plain VARCHAR,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    last_in_feed TIMESTAMP WITH TIME ZONE NOT NULL,
    display_before_active INTEGER,
    UNIQUE (created_at, alert_type)
);

CREATE TABLE IF NOT EXISTS realtime.active_period (
    alert_id INTEGER NOT NULL REFERENCES realtime.alert(id) ON DELETE CASCADE,
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    end_time TIMESTAMP WITH TIME ZONE,
    PRIMARY KEY (alert_id, start_time)
);

CREATE TABLE IF NOT EXISTS realtime.affected_entity (
    alert_id INTEGER NOT NULL REFERENCES realtime.alert(id) ON DELETE CASCADE,
    route_id VARCHAR REFERENCES static.route(id),
    stop_id INTEGER REFERENCES static.stop(id),
    sort_order INTEGER NOT NULL,
    UNIQUE (
        alert_id,
        route_id,
        stop_id,
        sort_order
    )
);

CREATE INDEX idx_active_period_alert_id_start_time_end_time ON realtime.active_period (alert_id, start_time, end_time);

CREATE INDEX idx_affected_entity_alert_id_route_id ON realtime.affected_entity (alert_id, route_id);