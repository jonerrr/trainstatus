use crate::{routes::alerts::Alert as AlertRow, train::static_data::ROUTES};
use bb8_redis::RedisConnectionManager;
use chrono::{DateTime, Utc};
use rayon::prelude::*;
use redis::AsyncCommands;
use sqlx::{PgPool, QueryBuilder};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

pub async fn import(pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = parse_gtfs(&pool).await {
                tracing::error!("Failed to decode feed: {:?}", e);
            }

            let mut alerts = sqlx::query_as!(
                AlertRow,
                "SELECT
                a.id,
                a.alert_type,
                a.header_html,
                a.description_html,
                a.created_at,
                a.updated_at,
                ap.start_time,
                ap.end_time,
                jsonb_agg(DISTINCT jsonb_build_object('bus_route_id',
                ae.bus_route_id,
                'route_id',
                ae.route_id,
                'stop_id',
                ae.stop_id,
                'sort_order',
                ae.sort_order)) AS entities
            FROM
                alerts a
            LEFT JOIN active_periods ap ON
                a.id = ap.alert_id
            LEFT JOIN affected_entities ae ON
                a.id = ae.alert_id
            WHERE
                a.in_feed IS TRUE
                AND (ae.route_id IS NOT NULL OR ae.bus_route_id IS NOT NULL)
                AND ap.start_time < now()
                AND (ap.end_time > now()
                    OR ap.end_time IS NULL)
            GROUP BY
                a.id,
                ap.start_time,
                ap.end_time"
            )
            .fetch_all(&pool)
            .await
            .unwrap();
            alerts.sort_by(|a, b| a.id.cmp(&b.id));
            alerts.dedup_by(|a, b| a.id == b.id);
            let Ok(alerts_str) = serde_json::to_string(&alerts) else {
                tracing::error!("Failed to convert alerts to string");
                continue;
            };
            let mut conn = redis_pool.get().await.unwrap();
            let _: () = conn.set("alerts", alerts_str).await.unwrap();

            sleep(Duration::from_secs(15)).await;
        }
    });
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

// pub enum IntoAlertError {
//     Mercury,
//     MtaId,
//     AlertType,
//     Header,
//     Description,
//     CreatedAt,
//     UpdatedAt,
//     DisplayBeforeActive,
// }

// impl TryFrom<FeedAlert> for Alert {
//     type Error = IntoAlertError;

//     fn try_from(value: FeedAlert) -> Result<Self, Self::Error> {
//         // No mercury alert means elevator outage. They also only have 1 description and header (plain)
//         let mercury_alert = value.mercury_alert.ok_or(IntoAlertError::Mercury)?;
//         // let mta_id =

//         todo!("finish this")
//     }
// }

impl Alert {
    pub async fn find(&mut self, pool: &PgPool) -> Result<bool, sqlx::Error> {
        let res = sqlx::query!(
            "
            SELECT
                id
            FROM
                alerts
            WHERE
                (mta_id = $1
                    OR header_plain = $2
                    OR (description_plain IS NOT NULL
                        AND description_plain = $3))
                AND created_at::date = CURRENT_DATE",
            self.mta_id,
            self.header_plain,
            self.description_plain
        )
        .fetch_optional(pool)
        .await?;

        match res {
            Some(t) => {
                self.id = t.id;
                Ok(true)
            }
            None => Ok(false),
        }
    }
}

#[derive(Debug)]
pub struct ActivePeriod {
    pub alert_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
}

pub struct AffectedEntity {
    pub alert_id: Uuid,
    pub route_id: Option<String>,
    pub bus_route_id: Option<String>,
    pub stop_id: Option<String>,
    pub sort_order: i32,
}

async fn parse_gtfs(pool: &PgPool) -> Result<(), DecodeFeedError> {
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
            .unwrap()
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

        let created_at = DateTime::from_timestamp(mercury_alert.created_at as i64, 0).unwrap();
        let updated_at = DateTime::from_timestamp(mercury_alert.updated_at as i64, 0).unwrap();
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
            .par_iter()
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

        let mut alert_affected_entities = feed_alert
            .informed_entity
            .par_iter()
            // Remove all MNR and LIRR alerts
            .filter(|entity| {
                entity.agency_id != Some("MNR".to_string())
                    && entity.agency_id != Some("LI".to_string())
            })
            .map(|entity| {
                let route_id = entity.route_id.as_ref().and_then(|r| {
                    // Standardize route id
                    let route_id = if r.ends_with('X') {
                        r[..r.len() - 1].to_string()
                    } else if r == "SS" {
                        "SI".to_owned()
                    } else {
                        r.to_string()
                    };

                    // Check if it is a train route
                    if ROUTES.contains(&route_id.as_str()) {
                        Some(route_id)
                    } else {
                        None
                    }
                });

                // If it isn't a train route id, it must be a bus route id
                let bus_route_id = if route_id.is_none() {
                    entity.route_id.clone()
                } else {
                    None
                };

                let stop_id = &entity.stop_id;
                let sort_order = match &entity.mercury_entity_selector {
                    Some(entity_selector) => entity_selector
                        .sort_order
                        .split(':')
                        .last()
                        .unwrap()
                        .parse()
                        .unwrap(),
                    _ => 0,
                };
                AffectedEntity {
                    alert_id: alert.id,
                    route_id,
                    bus_route_id,
                    stop_id: stop_id.clone(),
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

    // TODO: remove old active periods and affected entities
    let mut transaction = pool.begin().await?;

    // set in_feed to false for alerts not in feed
    sqlx::query!(
        "UPDATE alerts SET in_feed = false WHERE NOT id = ANY($1)",
        &in_feed_ids
    )
    .execute(&mut *transaction)
    .await?;
    // delete cloned ids
    sqlx::query!("DELETE FROM alerts WHERE mta_id = ANY($1)", &cloned_mta_ids)
        .execute(&mut *transaction)
        .await?;

    // Delete existing active periods and affected entities
    sqlx::query!(
        "DELETE FROM affected_entities WHERE alert_id = ANY($1)",
        &existing_alert_ids
    )
    .execute(&mut *transaction)
    .await?;
    sqlx::query!(
        "DELETE FROM active_periods WHERE alert_id = ANY($1)",
        &existing_alert_ids
    )
    .execute(&mut *transaction)
    .await?;

    let mut query_builder =
    QueryBuilder::new("INSERT INTO alerts (id, mta_id, alert_type, header_plain, header_html, description_plain, description_html, created_at, updated_at, display_before_active) ");
    query_builder.push_values(alerts, |mut b, alert| {
        b.push_bind(alert.id)
            .push_bind(alert.mta_id)
            .push_bind(alert.alert_type)
            .push_bind(alert.header_plain)
            .push_bind(alert.header_html)
            .push_bind(alert.description_plain)
            .push_bind(alert.description_html)
            .push_bind(alert.created_at)
            .push_bind(alert.updated_at)
            .push_bind(alert.display_before_active);
    });
    query_builder
        .push("ON CONFLICT (id) DO UPDATE SET alert_type = EXCLUDED.alert_type, header_plain = EXCLUDED.header_plain, header_html = EXCLUDED.header_html, description_plain = EXCLUDED.description_plain, description_html = EXCLUDED.description_html, created_at = EXCLUDED.created_at, updated_at = EXCLUDED.updated_at, display_before_active = EXCLUDED.display_before_active");
    let query = query_builder.build();
    query.execute(&mut *transaction).await?;

    let mut query_builder =
        QueryBuilder::new("INSERT INTO active_periods (alert_id, start_time, end_time) ");
    query_builder.push_values(active_periods, |mut b, active_period| {
        b.push_bind(active_period.alert_id)
            .push_bind(active_period.start_time)
            .push_bind(active_period.end_time);
    });
    query_builder
        .push("ON CONFLICT (alert_id, start_time) DO UPDATE SET end_time = EXCLUDED.end_time");
    let query = query_builder.build();
    query.execute(&mut *transaction).await?;

    let mut query_builder = QueryBuilder::new(
        "INSERT INTO affected_entities (alert_id, route_id, bus_route_id, stop_id, sort_order) ",
    );
    query_builder.push_values(affected_entities, |mut b, affected_entity| {
        b.push_bind(affected_entity.alert_id)
            .push_bind(affected_entity.route_id)
            .push_bind(affected_entity.bus_route_id)
            .push_bind(affected_entity.stop_id)
            .push_bind(affected_entity.sort_order);
    });
    // query_builder.push(
    //     "ON CONFLICT (alert_id, route_id, bus_route_id, stop_id, sort_order) DO UPDATE SET sort_order = EXCLUDED.sort_order",
    // );
    query_builder.push("ON CONFLICT DO NOTHING");
    let query = query_builder.build();
    query.execute(&mut *transaction).await?;

    transaction.commit().await?;

    Ok(())
}
