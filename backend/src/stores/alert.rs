use crate::models::{
    alert::{ActivePeriod, AffectedEntity, Alert, AlertTranslation},
    source::Source,
};
use crate::stores::read_through;
use bb8_redis::RedisConnectionManager;
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Duration;
use utoipa::ToSchema;
use uuid::Uuid;

/// API response for alerts - flattened with all related data
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiAlert {
    pub id: Uuid,
    pub original_id: String,
    #[schema(example = "Boarding Change")]
    // TODO: correct examples for different sources
    /// Alert type, if planned it will start with "Planned"
    pub alert_type: String,
    /// Alert header in HTML format
    pub header_html: String,
    /// Alert description in HTML format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description_html: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Start time of alert (earliest active period)
    pub start_time: DateTime<Utc>,
    /// End time of alert. If null, there is no end time yet.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<DateTime<Utc>>,
    /// Entities affected by alert
    pub entities: Vec<ApiAlertEntity>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
pub struct ApiAlertEntity {
    /// Affected route ID
    #[schema(example = "A")]
    pub route_id: String,
    /// The priority of the alert for the entity in ascending order
    pub sort_order: i32,
    /// Affected stop ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_id: Option<String>,
}

/// Raw row from alert query for sqlx mapping
#[derive(sqlx::FromRow)]
struct AlertRow {
    id: Uuid,
    original_id: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    data: serde_json::Value,
    // Aggregated fields from JOINs
    header_html: Option<String>,
    description_html: Option<String>,
    start_time: Option<DateTime<Utc>>,
    end_time: Option<DateTime<Utc>>,
    // JSON aggregated entities
    entities: serde_json::Value,
}

#[derive(Clone)]
pub struct AlertStore {
    pg_pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
}

impl AlertStore {
    pub fn new(pg_pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) -> Self {
        Self {
            pg_pool,
            redis_pool,
        }
    }

    /// Gets all alerts for a specific source, optionally filtered by a specific time.
    /// If a time is specified, alerts are not cached.
    /// Returns alerts formatted for the API with flattened translations and entities.
    pub async fn get_all(
        &self,
        source: Source,
        at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<Vec<ApiAlert>> {
        if let Some(at) = at {
            return self.query_all_alerts(source, at).await;
        }

        let key = format!("alerts:{}", source.as_str());
        read_through(&self.redis_pool, &key, Duration::from_secs(30), || async {
            self.query_all_alerts(source, Utc::now()).await
        })
        .await
    }

    /// Internal helper function to query alerts without caching
    async fn query_all_alerts(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<Vec<ApiAlert>> {
        // Get alerts with all related data in a single query using JOINs and aggregation
        let rows = sqlx::query_as::<_, AlertRow>(
            r#"
            SELECT
                a.id,
                a.original_id,
                a.created_at,
                a.updated_at,
                a.data,
                -- Get HTML header translation
                (
                    SELECT t.text
                    FROM realtime.alert_translation t
                    WHERE t.alert_id = a.id
                        AND t.section = 'header'
                        AND t.format = 'html'
                        AND t.language = 'en'
                    LIMIT 1
                ) as header_html,
                -- Get HTML description translation
                (
                    SELECT t.text
                    FROM realtime.alert_translation t
                    WHERE t.alert_id = a.id
                        AND t.section = 'description'
                        AND t.format = 'html'
                        AND t.language = 'en'
                    LIMIT 1
                ) as description_html,
                -- Get earliest start time from active periods
                (
                    SELECT MIN(ap.start_time)
                    FROM realtime.active_period ap
                    WHERE ap.alert_id = a.id
                ) as start_time,
                -- Get latest end time from active periods (NULL if any period has no end)
                (
                    SELECT
                        CASE
                            WHEN EXISTS (
                                SELECT 1 FROM realtime.active_period ap
                                WHERE ap.alert_id = a.id AND ap.end_time IS NULL
                            ) THEN NULL
                            ELSE MAX(ap.end_time)
                        END
                    FROM realtime.active_period ap
                    WHERE ap.alert_id = a.id
                ) as end_time,
                -- Aggregate entities as JSON array
                COALESCE(
                    (
                        SELECT json_agg(
                            json_build_object(
                                'route_id', ae.route_id,
                                'sort_order', ae.sort_order,
                                'stop_id', ae.stop_id
                            )
                            ORDER BY ae.sort_order
                        )
                        FROM realtime.affected_entity ae
                        WHERE ae.alert_id = a.id AND ae.route_id IS NOT NULL
                    ),
                    '[]'::json
                ) as entities
            FROM realtime.alert a
            WHERE
                a.source = $1
                AND EXISTS (
                    SELECT 1 FROM realtime.active_period ap
                    WHERE ap.alert_id = a.id
                        AND ap.start_time <= $2
                        AND (ap.end_time IS NULL OR ap.end_time >= $2)
                )
            ORDER BY a.updated_at DESC
            "#,
        )
        .bind(source)
        .bind(at)
        .fetch_all(&self.pg_pool)
        .await?;

        // Transform rows into API response format
        let alerts = rows
            .into_iter()
            .filter_map(|row| {
                // Extract alert_type from data JSON
                let alert_type = row
                    .data
                    .get("alert_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                // Parse entities from JSON
                let entities: Vec<ApiAlertEntity> =
                    serde_json::from_value(row.entities).unwrap_or_default();

                Some(ApiAlert {
                    id: row.id,
                    original_id: row.original_id,
                    alert_type,
                    header_html: row.header_html?,
                    description_html: row.description_html,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                    start_time: row.start_time?,
                    end_time: row.end_time,
                    entities,
                })
            })
            .collect();

        Ok(alerts)
    }

    // TODO: fix affected entities being duplicated. It seems like it creates an affected entity for every active period, even if its the same entity
    /// Bulk insert alerts with their related translations, active periods and affected entities
    /// This should be called with all collections together to maintain consistency
    #[tracing::instrument(skip(self, alerts, translations, active_periods, affected_entities, cloned_mta_ids), fields(
        source = %source.as_str(),
        alerts_count = alerts.len(),
        translations_count = translations.len(),
        active_periods_count = active_periods.len(),
        affected_entities_count = affected_entities.len()
    ), level = "debug")]
    pub async fn save_all(
        &self,
        source: Source,
        alerts: &[Alert],
        translations: &[AlertTranslation],
        active_periods: &[ActivePeriod],
        affected_entities: &[AffectedEntity],
        cloned_mta_ids: &[String],
    ) -> anyhow::Result<()> {
        if alerts.is_empty() {
            tracing::debug!("No alerts to insert");
            return Ok(());
        }

        // Use a transaction for consistency
        let mut tx = self.pg_pool.begin().await?;

        // Delete cloned alerts (old versions that have been replaced)
        if !cloned_mta_ids.is_empty() {
            sqlx::query!(
                "DELETE FROM realtime.alert WHERE original_id = ANY($1) AND source = $2",
                cloned_mta_ids,
                source as Source
            )
            .execute(&mut *tx)
            .await?;
            tracing::debug!("Deleted {} cloned alerts", cloned_mta_ids.len());
        }

        // let test_alert = alerts
        //     .iter()
        //     .find(|a| a.original_id == "lmm:planned_work:23514")
        //     .cloned();
        // if let Some(test_alert) = test_alert {
        //     dbg!(&test_alert);
        //     let test_aes = affected_entities
        //         .iter()
        //         .filter(|ae| ae.alert_id == test_alert.id)
        //         .cloned()
        //         .collect::<Vec<_>>();
        //     dbg!(&test_aes);
        // }

        // Insert alerts
        let ids = alerts.iter().map(|a| a.id).collect::<Vec<_>>();
        let original_ids = alerts
            .iter()
            .map(|a| a.original_id.clone())
            .collect::<Vec<_>>();
        let sources = alerts.iter().map(|a| a.source).collect::<Vec<_>>();
        let created_ats = alerts.iter().map(|a| a.created_at).collect::<Vec<_>>();
        let updated_ats = alerts.iter().map(|a| a.updated_at).collect::<Vec<_>>();
        let recorded_ats = alerts.iter().map(|a| a.recorded_at).collect::<Vec<_>>();
        let data_jsons = alerts
            .iter()
            .map(|a| serde_json::to_value(&a.data).unwrap())
            .collect::<Vec<_>>();

        // Use a CTE to get the actual IDs (either inserted or existing due to conflict)
        // This returns the mapping from (created_at, original_id, source) to the actual id
        let actual_ids = sqlx::query_scalar!(
            r#"
            WITH input_data AS (
                SELECT
                    unnest($1::uuid[]) as new_id,
                    unnest($2::text[]) as original_id,
                    unnest($3::source_enum[]) as source,
                    unnest($4::timestamptz[]) as created_at,
                    unnest($5::timestamptz[]) as updated_at,
                    unnest($6::timestamptz[]) as recorded_at,
                    unnest($7::jsonb[]) as data
            ),
            upserted AS (
                INSERT INTO realtime.alert (
                    id,
                    original_id,
                    source,
                    created_at,
                    updated_at,
                    recorded_at,
                    data
                )
                SELECT
                    new_id,
                    original_id,
                    source,
                    created_at,
                    updated_at,
                    recorded_at,
                    data
                FROM input_data
                ON CONFLICT (created_at, original_id, source) DO UPDATE SET
                    updated_at = EXCLUDED.updated_at,
                    recorded_at = EXCLUDED.recorded_at,
                    data = EXCLUDED.data
                RETURNING id, created_at, original_id, source
            )
            SELECT u.id
            FROM input_data i
            JOIN upserted u ON i.created_at = u.created_at
                AND i.original_id = u.original_id
                AND i.source = u.source
            "#,
            &ids,
            &original_ids,
            &sources as _,
            &created_ats,
            &updated_ats,
            &recorded_ats,
            &data_jsons
        )
        .fetch_all(&mut *tx)
        .await?;

        // Build a mapping from old (generated) IDs to actual (database) IDs
        let id_mapping: std::collections::HashMap<uuid::Uuid, uuid::Uuid> =
            ids.into_iter().zip(actual_ids).collect();

        tracing::debug!("Inserted {} alerts", alerts.len());

        // Insert translations
        if !translations.is_empty() {
            let alert_ids = translations
                .iter()
                .filter_map(|t| id_mapping.get(&t.alert_id).copied())
                .collect::<Vec<_>>();
            let filtered_translations: Vec<_> = translations
                .iter()
                .filter(|t| id_mapping.contains_key(&t.alert_id))
                .collect();
            let sections = filtered_translations
                .iter()
                .map(|t| t.section)
                .collect::<Vec<_>>();
            let formats = filtered_translations
                .iter()
                .map(|t| t.format)
                .collect::<Vec<_>>();
            let languages = filtered_translations
                .iter()
                .map(|t| t.language.clone())
                .collect::<Vec<_>>();
            let texts = filtered_translations
                .iter()
                .map(|t| t.text.clone())
                .collect::<Vec<_>>();

            if !alert_ids.is_empty() {
                sqlx::query!(
                    r#"
                    INSERT INTO realtime.alert_translation (
                        alert_id,
                        section,
                        format,
                        language,
                        text
                    )
                    SELECT
                        unnest($1::uuid[]),
                        unnest($2::alert_section[]),
                        unnest($3::alert_format[]),
                        unnest($4::text[]),
                        unnest($5::text[])
                    ON CONFLICT (alert_id, section, format, language) DO UPDATE SET text = EXCLUDED.text
                    "#,
                    &alert_ids,
                    &sections as _,
                    &formats as _,
                    &languages,
                    &texts
                )
                .execute(&mut *tx)
                .await?;

                tracing::debug!("Inserted {} translations", filtered_translations.len());
            }
        }

        // Insert active periods
        if !active_periods.is_empty() {
            let alert_ids = active_periods
                .iter()
                .filter_map(|ap| id_mapping.get(&ap.alert_id).copied())
                .collect::<Vec<_>>();
            let filtered_periods: Vec<_> = active_periods
                .iter()
                .filter(|ap| id_mapping.contains_key(&ap.alert_id))
                .collect();
            let start_times = filtered_periods
                .iter()
                .map(|ap| ap.start_time)
                .collect::<Vec<_>>();
            let end_times = filtered_periods
                .iter()
                .map(|ap| ap.end_time)
                .collect::<Vec<_>>();

            if !alert_ids.is_empty() {
                sqlx::query!(
                    r#"
                    INSERT INTO realtime.active_period (
                        alert_id,
                        start_time,
                        end_time
                    )
                    SELECT
                        unnest($1::uuid[]),
                        unnest($2::timestamptz[]),
                        unnest($3::timestamptz[])
                    ON CONFLICT (alert_id, start_time) DO UPDATE SET end_time = EXCLUDED.end_time
                    "#,
                    &alert_ids,
                    &start_times,
                    &end_times as &[Option<DateTime<Utc>>]
                )
                .execute(&mut *tx)
                .await?;

                tracing::debug!("Inserted {} active periods", filtered_periods.len());
            }
        }

        // Insert affected entities
        if !affected_entities.is_empty() {
            let alert_ids = affected_entities
                .iter()
                .filter_map(|ae| id_mapping.get(&ae.alert_id).copied())
                .collect::<Vec<_>>();
            let filtered_entities: Vec<_> = affected_entities
                .iter()
                .filter(|ae| id_mapping.contains_key(&ae.alert_id))
                .collect();
            let route_ids = filtered_entities
                .iter()
                .map(|ae| ae.route_id.clone())
                .collect::<Vec<_>>();
            let sources = filtered_entities
                .iter()
                .map(|ae| ae.source)
                .collect::<Vec<_>>();
            let stop_ids = filtered_entities
                .iter()
                .map(|ae| ae.stop_id.clone())
                .collect::<Vec<_>>();
            let sort_orders = filtered_entities
                .iter()
                .map(|ae| ae.sort_order)
                .collect::<Vec<_>>();

            if !alert_ids.is_empty() {
                sqlx::query!(
                    r#"
                    INSERT INTO realtime.affected_entity (
                        alert_id,
                        route_id,
                        source,
                        stop_id,
                        sort_order
                    )
                    SELECT
                        data.alert_id,
                        data.route_id,
                        data.source,
                        data.stop_id,
                        data.sort_order
                    FROM (
                        SELECT
                            unnest($1::uuid[]) as alert_id,
                            unnest($2::text[]) as route_id,
                            unnest($3::source_enum[]) as source,
                            unnest($4::text[]) as stop_id,
                            unnest($5::int[]) as sort_order
                    ) data
                    LEFT JOIN static.route r ON data.route_id = r.id AND data.source = r.source
                    LEFT JOIN static.stop s ON data.stop_id = s.id AND data.source = s.source
                    WHERE (data.route_id IS NULL OR r.id IS NOT NULL)
                    AND (data.stop_id IS NULL OR s.id IS NOT NULL)
                    ON CONFLICT (alert_id, COALESCE(route_id, ''), source, COALESCE(stop_id, '')) DO UPDATE SET sort_order = EXCLUDED.sort_order
                    "#,
                    &alert_ids,
                    &route_ids as &[Option<String>],
                    &sources as _,
                    &stop_ids as &[Option<String>],
                    &sort_orders
                )
                .execute(&mut *tx)
                .await?;

                tracing::debug!("Inserted {} affected entities", filtered_entities.len());
            }
        }

        // Commit transaction
        tx.commit().await?;

        // TODO: probably set end time for non-active alerts here

        // Invalidate Cache
        let key = format!("alerts:{}", source.as_str());
        let mut conn = self.redis_pool.get().await?;
        let _: redis::RedisResult<()> = conn.del(&key).await;

        Ok(())
    }
}
