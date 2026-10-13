use super::types::{
    ActivePeriod, AffectedEntity, Alert, AlertCollection, AlertData, AlertTranslation,
};
use crate::source::Source;
use chrono::{DateTime, Utc};
use moka::future::Cache;
use sqlx::PgPool;
use std::time::Duration;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use uuid::Uuid;

const TTL: Duration = Duration::from_secs(30);

#[derive(sqlx::FromRow)]
struct AlertRow {
    id: Uuid,
    original_id: String,
    source: Source,
    #[sqlx(json)]
    data: serde_json::Value,
    #[sqlx(json)]
    translations: Vec<AlertTranslation>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    recorded_at: DateTime<Utc>,
    #[sqlx(json)]
    active_periods: Vec<ActivePeriod>,
    #[sqlx(json)]
    entities: Vec<AffectedEntity>,
}
impl TryFrom<AlertRow> for Alert {
    type Error = anyhow::Error;
    fn try_from(row: AlertRow) -> anyhow::Result<Self> {
        Ok(Self {
            details: super::types::AlertDetails {
                id: row.id,
                original_id: row.original_id,
                translations: row.translations,
                created_at: row.created_at,
                updated_at: row.updated_at,
                recorded_at: row.recorded_at,
                active_periods: row.active_periods,
                entities: row.entities,
            },
            data: crate::persistence::decode::<AlertData>(row.source, row.data)?,
        })
    }
}
#[derive(Clone)]
pub struct Alerts {
    pg_pool: PgPool,
    cache: Cache<Source, Arc<Vec<Alert>>>,
    refresh_locks: Arc<HashMap<Source, Mutex<()>>>,
}

impl Alerts {
    pub fn new(pg_pool: PgPool) -> Self {
        Self {
            pg_pool,
            cache: Cache::builder().max_capacity(3).time_to_live(TTL).build(),
            refresh_locks: Arc::new(
                Source::ALL
                    .into_iter()
                    .map(|s| (s, Mutex::new(())))
                    .collect(),
            ),
        }
    }

    /// Gets all alerts for a specific source, optionally filtered by a specific time.
    /// If a time is specified, alerts are not cached.
    pub async fn get_all(
        &self,
        source: Source,
        at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<Vec<Alert>> {
        if let Some(at) = at {
            return Self::query_all_alerts_on(
                source,
                at,
                false,
                &mut *self.pg_pool.acquire().await?,
            )
            .await;
        }

        if let Some(alerts) = self.cache.get(&source).await {
            return Ok((*alerts).clone());
        }
        // A source's miss and write-through refresh share one lock, so an
        // earlier SQL read cannot overwrite a result from a later commit.
        let cached = {
            let _guard = self.refresh_locks[&source].lock().await;
            match self.cache.get(&source).await {
                Some(alerts) => alerts,
                None => {
                    let alerts = Arc::new(
                        Self::query_all_alerts_on(
                            source,
                            Utc::now(),
                            true,
                            &mut *self.pg_pool.acquire().await?,
                        )
                        .await?,
                    );
                    self.cache.insert(source, alerts.clone()).await;
                    alerts
                }
            }
        };
        Ok((*cached).clone())
    }

    /// Internal helper function to query alerts without caching
    async fn query_all_alerts_on(
        source: Source,
        at: DateTime<Utc>,
        current: bool,
        connection: &mut sqlx::PgConnection,
    ) -> anyhow::Result<Vec<Alert>> {
        // Runtime projection: JSON aggregates decode owned alert collections.
        let rows = sqlx::query_as::<_, AlertRow>(
            r#"
            SELECT
                a.id,
                a.source,
                a.original_id,
                a.created_at,
                a.updated_at,
                a.recorded_at,
                a.data,
                COALESCE(tr.translations, '[]'::json) as translations,
                (
                    SELECT json_agg(json_build_object(
                        'start_time', ap.start_time, 'end_time', ap.end_time
                    ) ORDER BY ap.start_time)
                    FROM realtime.active_period ap WHERE ap.alert_id = a.id
                ) as active_periods,
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
                        WHERE ae.alert_id = a.id
                    ),
                    '[]'::json
                ) as entities
            FROM realtime.alert a
            LEFT JOIN LATERAL (
                SELECT json_agg(t) as translations
                FROM (
                    SELECT section, format, language, text
                    FROM realtime.alert_translation
                    WHERE alert_id = a.id
                    ORDER BY section, format, language
                ) t
            ) tr ON true
            WHERE
                a.source = $1
                AND (NOT $3 OR a.current)
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
        .bind(current)
        .fetch_all(connection)
        .await?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Replace a collection atomically. Its enum variant determines source and
    /// payload together, and children inherit their parent's identity at the write.
    #[tracing::instrument(skip_all, level = "debug")]
    pub async fn save_all(&self, collection: AlertCollection) -> anyhow::Result<()> {
        let (source, alerts) = collection.into_alerts();
        let cloned_mta_ids: Vec<_> = alerts
            .iter()
            .filter_map(|a| match &a.data {
                AlertData::MtaSubway(data) | AlertData::MtaBus(data) => data.clone_id.as_deref(),
                AlertData::NjtBus => None,
            })
            .collect();
        let mut seen = std::collections::HashSet::new();
        let alerts: Vec<_> = alerts
            .iter()
            .filter(|a| !cloned_mta_ids.contains(&a.original_id.as_str()))
            .filter(|a| seen.insert((a.created_at, &a.original_id)))
            .collect();
        let _guard = self.refresh_locks[&source].lock().await;
        let mut tx = self.pg_pool.begin().await?;
        sqlx::query!(
            "UPDATE realtime.alert SET current = FALSE WHERE source = $1 AND current",
            source
        )
        .execute(&mut *tx)
        .await?;

        // Delete cloned alerts (old versions that have been replaced)
        if !cloned_mta_ids.is_empty() {
            sqlx::query!(
                "DELETE FROM realtime.alert WHERE original_id = ANY($1) AND source = $2",
                &cloned_mta_ids as &[&str],
                source
            )
            .execute(&mut *tx)
            .await?;
            tracing::debug!(count = cloned_mta_ids.len(), "Deleted cloned alerts");
        }

        // Insert alerts
        let ids = alerts.iter().map(|a| a.id).collect::<Vec<_>>();
        let original_ids = alerts
            .iter()
            .map(|a| a.original_id.clone())
            .collect::<Vec<_>>();
        let sources = vec![source; alerts.len()];
        let created_ats = alerts.iter().map(|a| a.created_at).collect::<Vec<_>>();
        let updated_ats = alerts.iter().map(|a| a.updated_at).collect::<Vec<_>>();
        let recorded_ats = alerts.iter().map(|a| a.recorded_at).collect::<Vec<_>>();
        let data_jsons = alerts
            .iter()
            .map(|a| crate::persistence::attributes(&a.data))
            .collect::<anyhow::Result<Vec<_>>>()?;

        // Use a CTE to get the actual IDs (either inserted or existing due to conflict)
        // This returns the mapping from (created_at, original_id, source) to the actual id
        let id_rows = sqlx::query!(
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
                    current = TRUE, recorded_at = EXCLUDED.recorded_at,
                    data = EXCLUDED.data
                RETURNING id, created_at, original_id, source
            )
            SELECT i.new_id AS "new_id!", u.id AS "id!"
            FROM input_data i
            JOIN upserted u ON i.created_at = u.created_at
                AND i.original_id = u.original_id
                AND i.source = u.source
            "#,
            &ids,
            &original_ids,
            &sources,
            &created_ats,
            &updated_ats,
            &recorded_ats,
            &data_jsons
        )
        .fetch_all(&mut *tx)
        .await?;

        // Build a mapping from old (generated) IDs to actual (database) IDs
        let id_mapping: std::collections::HashMap<uuid::Uuid, uuid::Uuid> =
            id_rows.into_iter().map(|r| (r.new_id, r.id)).collect();

        let accepted_ids: Vec<_> = id_mapping.values().copied().collect();
        sqlx::query!(
            "DELETE FROM realtime.alert_translation WHERE alert_id = ANY($1)",
            &accepted_ids
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM realtime.active_period WHERE alert_id = ANY($1)",
            &accepted_ids
        )
        .execute(&mut *tx)
        .await?;
        sqlx::query!(
            "DELETE FROM realtime.affected_entity WHERE alert_id = ANY($1)",
            &accepted_ids
        )
        .execute(&mut *tx)
        .await?;
        tracing::debug!(count = alerts.len(), "Inserted alerts");

        let mut seen_translation_keys = std::collections::HashSet::new();
        let translations: Vec<_> = alerts
            .iter()
            .flat_map(|a| {
                let id = id_mapping[&a.id];
                a.translations.iter().map(move |t| (id, t))
            })
            .filter(|(id, t)| seen_translation_keys.insert((*id, t.section, t.format, &t.language)))
            .collect();
        if !translations.is_empty() {
            let alert_ids = translations.iter().map(|(id, _)| *id).collect::<Vec<_>>();
            let sections = translations
                .iter()
                .map(|(_, t)| match t.section {
                    super::types::AlertSection::Header => "header",
                    super::types::AlertSection::Description => "description",
                })
                .collect::<Vec<_>>();
            let formats = translations
                .iter()
                .map(|(_, t)| match t.format {
                    super::types::AlertFormat::Plain => "plain",
                    super::types::AlertFormat::Html => "html",
                })
                .collect::<Vec<_>>();
            let languages = translations
                .iter()
                .map(|(_, t)| &t.language)
                .collect::<Vec<_>>();
            let texts = translations
                .iter()
                .map(|(_, t)| &t.text)
                .collect::<Vec<_>>();
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
                        unnest($2::text[])::alert_section,
                        unnest($3::text[])::alert_format,
                        unnest($4::text[]),
                        unnest($5::text[])
                    ON CONFLICT (alert_id, section, format, language) DO UPDATE SET text = EXCLUDED.text
                    "#,
                &alert_ids,
                &sections as &[&str],
                &formats as &[&str],
                &languages as &[&String],
                &texts as &[&String]
            )
                .execute(&mut *tx)
                .await?;
        }

        let mut seen_period_keys = std::collections::HashSet::new();
        let periods: Vec<_> = alerts
            .iter()
            .flat_map(|a| {
                let id = id_mapping[&a.id];
                a.active_periods.iter().map(move |p| (id, p))
            })
            .filter(|(id, p)| seen_period_keys.insert((*id, p.start_time)))
            .collect();
        if !periods.is_empty() {
            let alert_ids = periods.iter().map(|(id, _)| *id).collect::<Vec<_>>();
            let start_times = periods
                .iter()
                .map(|(_, p)| p.start_time)
                .collect::<Vec<_>>();
            let end_times = periods.iter().map(|(_, p)| p.end_time).collect::<Vec<_>>();
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
        }

        // Direction stripping can collapse multiple provider entities to one key.
        let mut seen_entity_keys = std::collections::HashSet::new();
        let entities: Vec<_> = alerts
            .iter()
            .flat_map(|a| {
                let id = id_mapping[&a.id];
                a.entities.iter().map(move |e| (id, e))
            })
            .filter(|(id, e)| {
                seen_entity_keys.insert((
                    *id,
                    e.route_id.as_ref().map(|s| s.to_uppercase()),
                    e.stop_id.as_ref().map(|s| s.to_uppercase()),
                ))
            })
            .collect();
        if !entities.is_empty() {
            let alert_ids = entities.iter().map(|(id, _)| *id).collect::<Vec<_>>();
            let route_ids = entities
                .iter()
                .map(|(_, e)| e.route_id.as_ref().map(|s| s.to_uppercase()))
                .collect::<Vec<_>>();
            let sources = vec![source; entities.len()];
            let stop_ids = entities
                .iter()
                .map(|(_, e)| e.stop_id.as_ref().map(|s| s.to_uppercase()))
                .collect::<Vec<_>>();
            let sort_orders = entities
                .iter()
                .map(|(_, e)| e.sort_order)
                .collect::<Vec<_>>();
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
                &sources,
                &stop_ids as &[Option<String>],
                &sort_orders
            )
                .execute(&mut *tx)
                .await?;
        }

        let candidate = Self::query_all_alerts_on(source, Utc::now(), true, &mut tx).await?;
        tx.commit().await?;

        // Populate cache (write-through)
        self.cache.insert(source, Arc::new(candidate)).await;

        Ok(())
    }
}
