use crate::{
    feed::{self, alert},
    trips::DecodeFeedError,
};
use chrono::{DateTime, Utc};
use prost::Message;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use sqlx::{PgPool, QueryBuilder};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

// use std::io::Write;

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

impl Alert {
    pub async fn get_existing(&mut self, pool: &PgPool) -> Result<(), sqlx::Error> {
        // let alert = sqlx::query_as!(Self, "SELECT * FROM alerts WHERE (mta_id = $1 OR header_plain = $2) AND created_at::date = current_date", self.mta_id, self.header_plain)
        //     .fetch_optional(pool)
        //     .await?;

        // if let Some(alert) = alert {
        //     self.id = alert.id;
        //     self.alert_type = alert.alert_type;
        //     self.header_plain = alert.header_plain;
        //     self.header_html = alert.header_html;
        //     self.description_plain = alert.description_plain;
        //     self.description_html = alert.description_html;
        //     self.created_at = alert.created_at;
        //     self.updated_at = alert.updated_at;
        //     self.display_before_active = alert.display_before_active;
        // }

        Ok(())
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
    pub stop_id: Option<String>,
    pub sort_order: i32,
}

async fn decode(pool: &PgPool) -> Result<(), DecodeFeedError> {
    let data = reqwest::Client::new()
        .get("https://api-endpoint.mta.info/Dataservice/mtagtfsfeeds/camsys%2Fsubway-alerts")
        .send()
        .await?
        .bytes()
        .await?;
    let feed = feed::FeedMessage::decode(data)?;

    // let mut msgs = Vec::new();
    // write!(msgs, "{:#?}", feed).unwrap();
    // tokio::fs::write("./alerts.txt", msgs).await.unwrap();

    let mut in_feed_ids = vec![];

    for entity in feed.entity {
        let Some(alert) = entity.alert else {
            tracing::warn!("no alert");
            continue;
        };

        let Some(mercury_alert) = alert.mercury_alert else {
            // elevator outages also only have 1 description and header (plain)
            tracing::debug!("No mercury alert (likely elevator outage)");
            continue;
        };

        if let Some(clone_id) = mercury_alert.clone_id {
            tracing::debug!("deleting clone of alert");
            sqlx::query!("DELETE FROM alerts WHERE mta_id = $1", clone_id)
                .execute(pool)
                .await?;
        }

        let mta_id = entity.id;
        // first in vec is plain text, second is html
        let headers = alert
            .header_text
            .unwrap()
            .translation
            .into_iter()
            .map(|h| h.text)
            .collect::<Vec<_>>();
        let descriptions = alert.description_text.map(|d| {
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

        // let id_name = headers[0].clone()
        //     + " "
        //     + &created_at.to_rfc2822()
        //     + " "
        //     + &alert_type
        //     + " "
        //     + &display_before_active.to_string();
        // let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, id_name.as_bytes());

        let id = Uuid::now_v7();
        in_feed_ids.push(id);

        // let alert = Alert {
        //     id,
        //     mta_id,
        //     alert_type,
        //     header_plain: headers[0].clone(),
        //     header_html: headers[1].clone(),
        //     description_plain,
        //     description_html,
        //     created_at,
        //     updated_at,
        //     display_before_active,
        // };

        // delete duplicate alerts
        sqlx::query!(
            "DELETE FROM alerts WHERE description_plain = $1 AND header_plain = $2",
            description_plain,
            headers[0]
        )
        .execute(pool)
        .await?;

        sqlx::query!("
            INSERT INTO alerts (id, mta_id, alert_type, header_plain, header_html, description_plain, description_html, created_at, updated_at, display_before_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            ON CONFLICT (id) DO UPDATE
            SET alert_type = $3, header_plain = $4, header_html = $5, description_plain = $6, description_html = $7, created_at = $8, updated_at = $9, display_before_active = $10
            ",
            id, mta_id, alert_type, headers[0], headers[1], description_plain, description_html, created_at, updated_at, display_before_active
        )
            .execute(pool)
            .await?;

        let active_periods = alert
            .active_period
            .par_iter()
            .map(|ap| {
                let start = DateTime::from_timestamp(ap.start.unwrap() as i64, 0).unwrap();
                let end = ap
                    .end
                    .map(|end| DateTime::from_timestamp(end as i64, 0).unwrap());
                (id, start, end)
            })
            .collect::<Vec<_>>();

        let mut query_builder =
            QueryBuilder::new("INSERT INTO active_periods (alert_id, start_time, end_time) ");
        query_builder.push_values(active_periods, |mut b, active_period| {
            b.push_bind(active_period.0)
                .push_bind(active_period.1)
                .push_bind(active_period.2);
        });
        let query = query_builder.build();
        query.execute(pool).await?;

        let affected_entities = alert
            .informed_entity
            .par_iter()
            .map(|entity| {
                let route_id = entity.route_id.clone().map(|r| {
                    if r.ends_with('X') {
                        r[..r.len() - 1].to_string()
                    } else if r == "SS" {
                        "SI".to_owned()
                    } else {
                        r
                    }
                });
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
                (id, route_id, stop_id, sort_order)
            })
            .collect::<Vec<_>>();

        let mut query_builder = QueryBuilder::new(
            "INSERT INTO affected_entities (alert_id, route_id, stop_id, sort_order) ",
        );
        query_builder.push_values(affected_entities, |mut b, affected_entity| {
            b.push_bind(affected_entity.0)
                .push_bind(affected_entity.1)
                .push_bind(affected_entity.2)
                .push_bind(affected_entity.3);
        });
        let query = query_builder.build();
        query.execute(pool).await?;
    }

    // set in_feed to false for alerts not in feed
    sqlx::query!(
        "UPDATE alerts SET in_feed = false WHERE NOT id = ANY($1)",
        &in_feed_ids
    )
    .execute(pool)
    .await?;

    Ok(())
}
