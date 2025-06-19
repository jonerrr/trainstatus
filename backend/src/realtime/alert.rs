use super::{ImportError, decode};
use crate::static_data::stop::convert_stop_id;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

// pub const ROUTES: [&str; 26] = [
//     "1", "2", "3", "4", "5", "6", "7", "A", "C", "E", "B", "D", "F", "M", "G", "J", "Z", "L", "N",
//     "Q", "R", "W", "H", "FS", "GS", "SI",
// ];

pub async fn import(pool: &PgPool) -> Result<(), ImportError> {
    let feed = decode(
        "https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/camsys%2Fall-alerts",
        "alerts",
    )
    .await?;

    // This is used to set alerts not in feed to false
    let mut in_feed_ids = vec![];
    // This will be used to delete the cloned alerts
    let mut cloned_mta_ids: Vec<String> = vec![];
    // This is used to delete existing active periods and affected entities for alerts that already exist. Otherwise there may be duplicates
    let mut existing_alert_ids: Vec<Uuid> = vec![];

    let mut alerts: Vec<Alert> = vec![];
    let mut active_periods: Vec<ActivePeriod> = vec![];
    let mut affected_entities: Vec<AffectedEntity> = vec![];

    for entity in feed.entity {
        let Some(feed_alert) = entity.alert else {
            tracing::warn!("no alert");
            continue;
        };

        let Some(mercury_alert) = feed_alert.mercury_alert else {
            // elevator outages also only have 1 description and header (plain)
            tracing::debug!("No mercury alert (likely elevator outage)");
            continue;
        };

        // first in vec is plain text, second is html
        let headers = feed_alert
            .header_text
            .ok_or(ImportError::Parse("Alert header text not found".into()))?
            .translation
            .into_iter()
            .map(|h| h.text)
            .collect::<Vec<_>>();
        let descriptions = feed_alert.description_text.map(|d| {
            d.translation
                .into_iter()
                .map(|d| d.text)
                .collect::<Vec<_>>()
        });
        let (description_plain, description_html) = match descriptions {
            Some(descriptions) => (Some(descriptions[0].clone()), Some(descriptions[1].clone())),
            None => (None, None),
        };

        let created_at = DateTime::from_timestamp(mercury_alert.created_at as i64, 0).ok_or(
            ImportError::Parse("Failed to parse alert created_at timestamp".into()),
        )?;
        let updated_at = DateTime::from_timestamp(mercury_alert.updated_at as i64, 0).ok_or(
            ImportError::Parse("Failed to parse alert updated_at timestamp".into()),
        )?;
        let alert_type = mercury_alert.alert_type;
        let display_before_active = mercury_alert.display_before_active.unwrap_or(0) as i32;

        let mut alert = Alert {
            id: Uuid::now_v7(),
            mta_id: entity.id,
            alert_type,
            header_plain: headers[0].clone(),
            header_html: headers[1].clone(),
            description_plain,
            description_html,
            created_at,
            updated_at,
            display_before_active,
        };

        let alert_exists = alert.find(pool).await?;
        // There could be duplicate alerts in the feed so I need to remove them
        if alert_exists && alerts.iter().any(|a| a.id == alert.id) {
            tracing::debug!("Skipping duplicate alert in feed");
            continue;
        }
        if alert_exists {
            existing_alert_ids.push(alert.id);
        }

        in_feed_ids.push(alert.id);

        // Check if this was cloned. If it was we will remove the old one from the DB
        // TODO: test this
        if let Some(clone_id) = mercury_alert.clone_id {
            cloned_mta_ids.push(clone_id);
            // cloned_ids.push(alert.id);
        }

        let mut alert_active_periods = feed_alert
            .active_period
            .iter()
            .map(|ap| {
                let start = DateTime::from_timestamp(ap.start.unwrap() as i64, 0).unwrap();
                let end = ap
                    .end
                    .map(|end| DateTime::from_timestamp(end as i64, 0).unwrap());
                ActivePeriod {
                    alert_id: alert.id,
                    start_time: start,
                    end_time: end,
                }
            })
            .collect::<Vec<_>>();
        // tracing::info!("active period: {:?}", end - start);

        let mut alert_affected_entities = feed_alert
            .informed_entity
            .iter()
            // Remove all MNR and LIRR alerts
            .filter(|entity| {
                entity.agency_id != Some("MNR".to_string())
                    && entity.agency_id != Some("LI".to_string())
            })
            .map(|entity| {
                let route_id = entity.route_id.clone().and_then(|r| {
                    // Standardize route id
                    if r.ends_with('X') {
                        Some(r[..r.len() - 1].to_string())
                    } else if r == "SS" {
                        Some("SI".to_owned())
                        // X80 is in the alert api but not anywhere else so it causes a foreign key constraint error
                        // see: https://groups.google.com/g/mtadeveloperresources/c/ZFnhNRrTlr8/m/K8oNzFrBAgAJ
                    } else if r == "X80" {
                        None
                    } else {
                        Some(r.to_string())
                    }

                    // Check if it is a train route
                    // if ROUTES.contains(&route_id.as_str()) {
                    //     Some(route_id)
                    // } else {
                    //     None
                    // }
                });

                // If it isn't a train route id, it must be a bus route id
                // let bus_route_id = if route_id.is_none() {
                //     entity.route_id.clone()
                // } else {
                //     None
                // };

                let stop_id = &entity.stop_id.clone().and_then(convert_stop_id);
                let sort_order = match &entity.mercury_entity_selector {
                    Some(entity_selector) => entity_selector
                        .sort_order
                        .split(':')
                        .next_back()
                        .unwrap()
                        .parse()
                        .unwrap(),
                    _ => 0,
                };
                AffectedEntity {
                    alert_id: alert.id,
                    route_id,
                    stop_id: *stop_id,
                    sort_order,
                }
            })
            .collect::<Vec<_>>();

        alerts.push(alert);
        active_periods.append(&mut alert_active_periods);
        affected_entities.append(&mut alert_affected_entities);
    }

    // Remove duplicate alerts and their active periods and affected entities
    // TODO: check if this is required still
    let mut cloned_ids = vec![];
    alerts.retain(|a| {
        if cloned_mta_ids.contains(&a.mta_id) {
            cloned_ids.push(a.id);
            false
        } else {
            true
        }
    });
    active_periods.retain(|a| !cloned_ids.contains(&a.alert_id));
    affected_entities.retain(|a| !cloned_ids.contains(&a.alert_id));

    let mut transaction = pool.begin().await?;

    // set in_feed to false for alerts not in feed
    // sqlx::query!(
    //     "UPDATE alerts SET in_feed = false WHERE NOT id = ANY($1)",
    //     &in_feed_ids
    // )
    // .execute(&mut *transaction)
    // .await?;
    // Set end time for active periods that are no longer active
    sqlx::query!(
        "UPDATE active_period SET end_time = NOW() WHERE alert_id = ANY($1) AND end_time IS NULL AND start_time NOT IN (SELECT start_time FROM active_period WHERE alert_id = ANY($1))",
        &in_feed_ids
    ).execute(&mut *transaction).await?;

    // delete cloned ids
    sqlx::query!("DELETE FROM alert WHERE mta_id = ANY($1)", &cloned_mta_ids)
        .execute(&mut *transaction)
        .await?;

    // Delete existing active periods and affected entities
    sqlx::query!(
        "DELETE FROM affected_entity WHERE alert_id = ANY($1)",
        &existing_alert_ids
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query!(
        "DELETE FROM active_period WHERE alert_id = ANY($1)",
        &existing_alert_ids
    )
    .execute(&mut *transaction)
    .await?;

    let start_time = std::time::Instant::now();
    let alerts_count = alerts.len();
    let active_periods_count = active_periods.len();
    let affected_entities_count = affected_entities.len();

    Alert::insert(alerts, &mut transaction).await?;
    let alerts_duration = start_time.elapsed();
    tracing::debug!("Inserted {} alerts in {:?}", alerts_count, alerts_duration);

    let active_periods_start = std::time::Instant::now();
    ActivePeriod::insert(active_periods, &mut transaction).await?;
    let active_periods_duration = active_periods_start.elapsed();
    tracing::debug!(
        "Inserted {} active periods in {:?}",
        active_periods_count,
        active_periods_duration
    );

    let affected_entities_start = std::time::Instant::now();
    AffectedEntity::insert(affected_entities, &mut transaction).await?;
    let affected_entities_duration = affected_entities_start.elapsed();
    tracing::debug!(
        "Inserted {} affected entities in {:?}",
        affected_entities_count,
        affected_entities_duration
    );

    transaction.commit().await?;

    let total_duration = start_time.elapsed();
    tracing::debug!("Total alert import completed in {:?}", total_duration);

    Ok(())
}

#[derive(Debug)]
pub struct Alert {
    pub id: Uuid,
    pub mta_id: String,
    pub alert_type: String,
    pub header_plain: String,
    pub header_html: String,
    pub description_plain: Option<String>,
    pub description_html: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub display_before_active: i32,
}

impl Alert {
    pub async fn insert(
        values: Vec<Self>,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), sqlx::Error> {
        let ids = values.iter().map(|a| a.id).collect::<Vec<_>>();
        let mta_ids = values.iter().map(|a| a.mta_id.clone()).collect::<Vec<_>>();
        let alert_types = values
            .iter()
            .map(|a| a.alert_type.clone())
            .collect::<Vec<_>>();
        let header_plains = values
            .iter()
            .map(|a| a.header_plain.clone())
            .collect::<Vec<_>>();
        let header_htmls = values
            .iter()
            .map(|a| a.header_html.clone())
            .collect::<Vec<_>>();
        let description_plains = values
            .iter()
            .map(|a| a.description_plain.clone())
            .collect::<Vec<_>>();
        let description_htmls = values
            .iter()
            .map(|a| a.description_html.clone())
            .collect::<Vec<_>>();
        let created_ats = values.iter().map(|a| a.created_at).collect::<Vec<_>>();
        let updated_ats = values.iter().map(|a| a.updated_at).collect::<Vec<_>>();
        let display_before_actives = values
            .iter()
            .map(|a| a.display_before_active)
            .collect::<Vec<_>>();
        // add last_in_feed as utc::now for each alert
        let last_in_feed = values.iter().map(|_| Utc::now()).collect::<Vec<_>>();

        sqlx::query!(r#"
        INSERT INTO alert (
            id,
            mta_id,
            alert_type,
            header_plain,
            header_html,
            description_plain,
            description_html,
            created_at,
            updated_at,
            last_in_feed,
            display_before_active
        )
        SELECT
            unnest($1::uuid[]),
            unnest($2::text[]),
            unnest($3::text[]),
            unnest($4::text[]),
            unnest($5::text[]),
            unnest($6::text[]),
            unnest($7::text[]),
            unnest($8::timestamptz[]),
            unnest($9::timestamptz[]),
            unnest($10::timestamptz[]),
            unnest($11::int[])
        ON CONFLICT (id) DO UPDATE SET alert_type = EXCLUDED.alert_type, header_plain = EXCLUDED.header_plain, header_html = EXCLUDED.header_html, description_plain = EXCLUDED.description_plain, description_html = EXCLUDED.description_html, created_at = EXCLUDED.created_at, updated_at = EXCLUDED.updated_at, last_in_feed = EXCLUDED.last_in_feed, display_before_active = EXCLUDED.display_before_active
        "#,
        &ids,
        &mta_ids,
        &alert_types,
        &header_plains,
        &header_htmls,
        &description_plains as &[Option<String>],
        &description_htmls as &[Option<String>],
        &created_ats,
        &updated_ats,
        &last_in_feed,
        &display_before_actives)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }

    pub async fn get_all(
        pool: &PgPool,
        at: DateTime<Utc>,
        stop_alerts: bool,
        // limit_start_time: bool,
    ) -> Result<serde_json::Value, sqlx::Error> {
        // TODO: maybe if time is specified, dont include alerts where end time is null
        let alerts: (Option<serde_json::Value>,) = sqlx::query_as(
            r#"
            SELECT json_agg(result) FROM
            (SELECT
                a.id,
                a.alert_type,
                a.header_html,
                a.description_html,
                a.created_at,
                a.updated_at,
                ap.start_time,
                ap.end_time,
                json_agg(DISTINCT jsonb_build_object(
                'route_id',
                ae.route_id,
                'stop_id',
                ae.stop_id,
                'sort_order',
                ae.sort_order)) AS entities
            FROM
                alert a
            LEFT JOIN active_period ap ON
                a.id = ap.alert_id
            LEFT JOIN affected_entity ae ON
                a.id = ae.alert_id
            WHERE
                a.last_in_feed >= $1 - INTERVAL '2 minutes' AND
                (ae.route_id IS NOT NULL OR ($2 = FALSE OR ae.stop_id IS NOT NULL))
                AND ap.start_time <= $1
                AND (ap.end_time >= $1
                    OR ap.end_time IS NULL)
            GROUP BY
                a.id,
                ap.start_time,
                ap.end_time
            ) AS result"#,
        )
        .bind(at)
        .bind(stop_alerts)
        // .bind(limit_start_time)
        .fetch_one(pool)
        .await?;

        match alerts.0 {
            Some(value) => Ok(value),
            None => Ok(serde_json::Value::Array(vec![])), // Return an empty array if the result is NULL
        }
    }

    pub async fn find(&mut self, pool: &PgPool) -> Result<bool, sqlx::Error> {
        let res = sqlx::query!(
            "
            SELECT
                a.id, ae.route_id, ae.stop_id
            FROM
                alert a
            LEFT JOIN affected_entity ae ON
                a.id = ae.alert_id
            WHERE
                a.mta_id = $1 OR a.created_at = $2",
            self.mta_id,
            self.created_at
        )
        .fetch_optional(pool)
        .await?;

        // TODO: check if affected entity matches

        match res {
            Some(t) => {
                self.id = t.id;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    // pub async fn cache()
}

// #[derive(Debug)]
pub struct ActivePeriod {
    pub alert_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
}

impl ActivePeriod {
    pub async fn insert(
        values: Vec<Self>,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), sqlx::Error> {
        let alert_ids = values.iter().map(|ap| ap.alert_id).collect::<Vec<_>>();
        let start_times = values.iter().map(|ap| ap.start_time).collect::<Vec<_>>();
        let end_times = values.iter().map(|ap| ap.end_time).collect::<Vec<_>>();

        sqlx::query!(
            r#"
            INSERT INTO active_period (
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
        .execute(&mut **tx)
        .await?;

        Ok(())
    }
}

pub struct AffectedEntity {
    pub alert_id: Uuid,
    pub route_id: Option<String>,
    pub stop_id: Option<i32>,
    pub sort_order: i32,
}

impl AffectedEntity {
    pub async fn insert(
        values: Vec<Self>,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> Result<(), sqlx::Error> {
        let alert_ids = values.iter().map(|ae| ae.alert_id).collect::<Vec<_>>();
        let route_ids = values
            .iter()
            .map(|ae| ae.route_id.clone())
            .collect::<Vec<_>>();
        // let bus_route_ids = values
        //     .iter()
        //     .map(|ae| ae.bus_route_id.clone())
        //     .collect::<Vec<_>>();
        let stop_ids = values.iter().map(|ae| ae.stop_id).collect::<Vec<_>>();
        let sort_orders = values.iter().map(|ae| ae.sort_order).collect::<Vec<_>>();

        // TODO: find some efficient way to log rows that don't get inserted
        sqlx::query!(
            r#"
            INSERT INTO affected_entity (
                alert_id,
                route_id,
                stop_id,
                sort_order
            )
            SELECT
                data.alert_id,
                data.route_id,
                data.stop_id,
                data.sort_order
            FROM (
                SELECT
                    unnest($1::uuid[]) as alert_id,
                    unnest($2::text[]) as route_id,
                    unnest($3::int[]) as stop_id,
                    unnest($4::int[]) as sort_order
            ) data
            LEFT JOIN route r ON data.route_id = r.id
            LEFT JOIN stop s ON data.stop_id = s.id
            WHERE (data.route_id IS NULL OR r.id IS NOT NULL)
            AND (data.stop_id IS NULL OR s.id IS NOT NULL)
            ON CONFLICT DO NOTHING
            "#,
            &alert_ids,
            &route_ids as &[Option<String>],
            &stop_ids as &[Option<i32>],
            &sort_orders
        )
        .execute(&mut **tx)
        .await?;

        Ok(())
    }
}
