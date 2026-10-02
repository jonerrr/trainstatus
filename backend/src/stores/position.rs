use crate::{
    models::{position::VehiclePosition, source::Source},
    realtime::LiveSnapshots,
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Clone)]
pub struct PositionStore {
    pg_pool: PgPool,
    live_snapshots: LiveSnapshots,
}

impl PositionStore {
    pub fn new(pg_pool: PgPool, live_snapshots: LiveSnapshots) -> Self {
        Self {
            pg_pool,
            live_snapshots,
        }
    }

    /// Gets all positions for a specific source, optionally filtered by a specific time.
    /// Explicit times read retained observations; default reads use the committed snapshot.
    pub async fn get_all(
        &self,
        source: Source,
        at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<Vec<VehiclePosition>> {
        if let Some(at) = at {
            return self.query_all_positions(source, at).await;
        }

        Ok(self
            .live_snapshots
            .get(source)
            .map(|snapshot| snapshot.positions.clone())
            .unwrap_or_default())
    }

    /// Internal helper function to query positions without caching
    async fn query_all_positions(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<Vec<VehiclePosition>> {
        Ok(sqlx::query_as::<_, VehiclePosition>(
            r#"
                SELECT
                    p.vehicle_id,
                    p.trip_id,
                    p.stop_id,
                    p.updated_at,
                    p.data,
                    p.geom
                FROM realtime.vehicle_position p
                WHERE
                    p.source = $1
                    AND
                    p.updated_at >= (($2)::timestamp with time zone - INTERVAL '5 minutes')
                ORDER BY p.updated_at DESC"#,
            // TODO: maybe also filter for positions that are on trips that have stop_times in the next 4 hours, but this is a bit more complex since not all positions have trip_id and we want to avoid joining on trip_id if possible
            // WHERE
            //     t.source = $1
            //     AND
            //     t.updated_at >= (($2)::timestamp with time zone - INTERVAL '5 minutes')
            //     AND t.id = ANY(
            //         SELECT t.id
            //         FROM realtime.trip t
            //         LEFT JOIN realtime.stop_time st ON st.trip_id = t.id
            //         WHERE st.arrival BETWEEN $2 AND ($2 + INTERVAL '4 hours')
            //     )
            // ORDER BY t.created_at DESC"#,
        )
        .bind(source)
        .bind(at)
        .fetch_all(&self.pg_pool)
        .await?)
    }
}
