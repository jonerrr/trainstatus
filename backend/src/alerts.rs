use crate::{
    feed::{self},
    static_data::ROUTES,
    trips::DecodeFeedError,
};
use chrono::{DateTime, Utc};
use prost::Message;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use sqlx::{PgPool, QueryBuilder};
use std::{env::var, time::Duration};
use tokio::{
    fs::{create_dir, write},
    time::sleep,
};
use uuid::Uuid;

pub async fn import(pool: PgPool) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = decode(&pool).await {
                tracing::error!("Failed to decode feed: {:?}", e);
            }
            sleep(Duration::from_secs(10)).await;
        }
    });
}

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

async fn decode(pool: &PgPool) -> Result<(), DecodeFeedError> {
    let data = reqwest::Client::new()
        .get("https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/camsys%2Fall-alerts")
        .send()
        .await?
        .bytes()
        .await?;
    let feed = feed::FeedMessage::decode(data)?;

    if var("DEBUG_GTFS").is_ok() {
        let msgs = format!("{:#?}", feed);
        create_dir("./gtfs").await.ok();
        write("./gtfs/alerts.txt", msgs).await.unwrap();
    }

    let mut in_feed_ids = vec![];
    let mut cloned_ids: Vec<String> = vec![];

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

        if let Some(clone_id) = mercury_alert.clone_id {
            cloned_ids.push(clone_id);
        }

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
        alert.find(pool).await?;
        in_feed_ids.push(alert.id);

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
                let route_id = entity
                    .route_id
                    .clone()
                    .map(|r: String| {
                        if r.ends_with('X') {
                            r[..r.len() - 1].to_string()
                        } else if r == "SS" {
                            "SI".to_owned()
                        } else {
                            r
                        }
                    })
                    .map(|r| {
                        if ROUTES.contains(&r.as_str()) {
                            Some(r)
                        } else {
                            None
                        }
                    })
                    .flatten();

                // check if route_id is in ROUTES, otherwise its a bus route
                // TODO: improve this

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

    // set in_feed to false for alerts not in feed
    sqlx::query!(
        "UPDATE alerts SET in_feed = false WHERE NOT id = ANY($1)",
        &in_feed_ids
    )
    .execute(pool)
    .await?;
    // delete cloned ids
    sqlx::query!("DELETE FROM alerts WHERE mta_id = ANY($1)", &cloned_ids)
        .execute(pool)
        .await?;

    // TODO: Use transaction https://github.com/launchbadge/sqlx/blob/main/examples/postgres/transaction/src/main.rs
    // TODO: figure out what to do about old active periods that are now incorrect. Maybe delete all active periods and entities for alerts we are updating?
    let mut query_builder =
    QueryBuilder::new("INSERT INTO alerts (id, mta_id, alert_type, header_plain, header_html, description_plain, description_html, created_at, updated_at, display_before_active) ");
    query_builder.push_values(alerts, |mut b, active_period| {
        b.push_bind(active_period.id)
            .push_bind(active_period.mta_id)
            .push_bind(active_period.alert_type)
            .push_bind(active_period.header_plain)
            .push_bind(active_period.header_html)
            .push_bind(active_period.description_plain)
            .push_bind(active_period.description_html)
            .push_bind(active_period.created_at)
            .push_bind(active_period.updated_at)
            .push_bind(active_period.display_before_active);
    });
    // TODO: test if this prevents duplicates
    query_builder
        .push("ON CONFLICT (id) DO UPDATE SET alert_type = EXCLUDED.alert_type, header_plain = EXCLUDED.header_plain, header_html = EXCLUDED.header_html, description_plain = EXCLUDED.description_plain, description_html = EXCLUDED.description_html, created_at = EXCLUDED.created_at, updated_at = EXCLUDED.updated_at, display_before_active = EXCLUDED.display_before_active");
    let query = query_builder.build();
    query.execute(pool).await?;

    let mut query_builder =
        QueryBuilder::new("INSERT INTO active_periods (alert_id, start_time, end_time) ");
    query_builder.push_values(active_periods, |mut b, active_period| {
        b.push_bind(active_period.alert_id)
            .push_bind(active_period.start_time)
            .push_bind(active_period.end_time);
    });
    // TODO: test if this prevents duplicates
    query_builder
        .push("ON CONFLICT (alert_id, start_time) DO UPDATE SET end_time = EXCLUDED.end_time");
    let query = query_builder.build();
    query.execute(pool).await?;

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
    query_builder.push(
        "ON CONFLICT (alert_id, route_id, bus_route_id, stop_id) DO UPDATE SET sort_order = EXCLUDED.sort_order",
    );
    let query = query_builder.build();
    query.execute(pool).await?;

    Ok(())
}
