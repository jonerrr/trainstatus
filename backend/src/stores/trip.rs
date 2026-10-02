use crate::{
    models::{source::Source, trip::Trip},
    realtime::LiveSnapshots,
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Clone)]
pub struct TripStore {
    pg_pool: PgPool,
    live_snapshots: LiveSnapshots,
}

impl TripStore {
    pub fn new(pg_pool: PgPool, live_snapshots: LiveSnapshots) -> Self {
        Self {
            pg_pool,
            live_snapshots,
        }
    }

    /// Gets all trips for a specific source, optionally filtered by a specific time.
    /// Explicit times use retained arrival/departure values, not feed version history,
    /// and bypass the committed live snapshot.
    pub async fn get_all(
        &self,
        source: Source,
        at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<Vec<Trip>> {
        if let Some(at) = at {
            return self.query_all_trips(source, at).await;
        }

        Ok(self
            .live_snapshots
            .get(source)
            .map(|snapshot| snapshot.trips.clone())
            .unwrap_or_default())
    }

    /// Internal helper function to query trips without caching
    async fn query_all_trips(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<Vec<Trip>> {
        Ok(sqlx::query_as::<_, Trip>(
            r#"
                SELECT
                    t.id,
                    t.original_id,
                    t.vehicle_id,
                    t.route_id,
                    t.shape_ids,
                    t.source,
                    t.direction,
                    t.created_at,
                    t.updated_at,
                    t.data
                FROM realtime.trip t
                WHERE
                    t.source = $1
                    AND EXISTS (
                        SELECT 1
                        FROM realtime.stop_time st
                        WHERE st.trip_id = t.id
                            AND st.source = $1
                            AND (
                                st.arrival BETWEEN $2 AND ($2 + INTERVAL '4 hours')
                                OR st.departure BETWEEN $2 AND ($2 + INTERVAL '4 hours')
                            )
                    )
                ORDER BY t.created_at DESC"#,
        )
        .bind(source)
        .bind(at)
        .fetch_all(&self.pg_pool)
        .await?)
    }
}
