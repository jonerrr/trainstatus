CREATE TABLE IF NOT EXISTS realtime.alert (
    id UUID PRIMARY KEY,
    original_id VARCHAR NOT NULL,
    source source_enum NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL,
    recorded_at TIMESTAMP WITH TIME ZONE NOT NULL,
    data JSONB NOT NULL,
    -- might need some other way to remove dupes
    UNIQUE (created_at, original_id, source)
);

CREATE TYPE alert_section AS ENUM ('header', 'description');
CREATE TYPE alert_format AS ENUM ('plain', 'html');

CREATE TABLE IF NOT EXISTS realtime.alert_translation (
    alert_id UUID NOT NULL REFERENCES realtime.alert(id) ON DELETE CASCADE,
    section alert_section NOT NULL,
    format alert_format NOT NULL,
    language VARCHAR(10) NOT NULL DEFAULT 'en',
    text TEXT NOT NULL,
    PRIMARY KEY (alert_id, section, format, language)
);


CREATE TABLE IF NOT EXISTS realtime.active_period (
    alert_id UUID NOT NULL REFERENCES realtime.alert(id) ON DELETE CASCADE,
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    end_time TIMESTAMP WITH TIME ZONE,
    PRIMARY KEY (alert_id, start_time)
);

CREATE TABLE IF NOT EXISTS realtime.affected_entity (
    alert_id UUID NOT NULL REFERENCES realtime.alert(id) ON DELETE CASCADE,
    route_id VARCHAR,
    stop_id VARCHAR,
    source source_enum NOT NULL,
    sort_order INTEGER NOT NULL,
    FOREIGN KEY (route_id, source) REFERENCES static.route(id, source) ON DELETE CASCADE,
    FOREIGN KEY (stop_id, source) REFERENCES static.stop(id, source) ON DELETE CASCADE
);

-- Use a unique index with COALESCE to treat NULL as a specific value for uniqueness
-- This ensures that multiple rows with NULL stop_id are still considered duplicates
CREATE UNIQUE INDEX idx_affected_entity_unique ON realtime.affected_entity (
    alert_id,
    COALESCE(route_id, ''),
    source,
    COALESCE(stop_id, '')
);

CREATE INDEX idx_alert_translation_alert_id ON realtime.alert_translation (alert_id);

CREATE INDEX idx_active_period_alert_id_start_time_end_time ON realtime.active_period (alert_id, start_time, end_time);

CREATE INDEX idx_affected_entity_alert_id_route_id ON realtime.affected_entity (alert_id, route_id, source);