DROP INDEX IF EXISTS realtime.idx_alert_translation_alert_id;

DROP INDEX IF EXISTS realtime.idx_active_period_alert_id_start_time_end_time;

DROP INDEX IF EXISTS realtime.idx_affected_entity_alert_id_route_id;

DROP INDEX IF EXISTS realtime.idx_affected_entity_unique;

DROP TABLE IF EXISTS realtime.affected_entity;

DROP TABLE IF EXISTS realtime.active_period;

DROP TABLE IF EXISTS realtime.alert_translation;

DROP TABLE IF EXISTS realtime.alert;

DROP TYPE IF EXISTS alert_format;

DROP TYPE IF EXISTS alert_section;