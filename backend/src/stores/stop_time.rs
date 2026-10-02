use crate::models::{source::Source, trip::StopTime};
use crate::realtime::LiveSnapshots;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Clone)]
pub struct StopTimeStore {
    pg_pool: PgPool,
    live_snapshots: LiveSnapshots,
}

impl StopTimeStore {
    pub fn new(pg_pool: PgPool, live_snapshots: LiveSnapshots) -> Self {
        Self {
            pg_pool,
            live_snapshots,
        }
    }

    /// Gets all stop times for a specific source, optionally filtered by a specific time.
    /// Explicit times use retained arrival/departure values, not feed version history,
    /// and bypass the committed live snapshot.
    /// Can also filter by route_ids for bus routes.
    pub async fn get_all(
        &self,
        source: Source,
        at: Option<DateTime<Utc>>,
        route_ids: Option<&[String]>,
    ) -> anyhow::Result<Vec<StopTime>> {
        if let Some(at) = at {
            return self.query_all_stop_times(source, at, route_ids).await;
        }
        let Some(snapshot) = self.live_snapshots.get(source) else {
            return Ok(vec![]);
        };
        let route_ids = route_ids.filter(|routes| !routes.is_empty());
        let selected_trips: std::collections::HashSet<_> = snapshot
            .trips
            .iter()
            .filter(|trip| route_ids.is_none_or(|routes| routes.contains(&trip.route_id)))
            .map(|trip| trip.id)
            .collect();
        let mut stops: Vec<_> = snapshot
            .stop_times
            .iter()
            .filter(|stop| selected_trips.contains(&stop.trip_id))
            .cloned()
            .collect();
        stops.sort_by_key(|stop| stop.arrival);
        Ok(stops)
    }

    /// Internal helper function to query stop times without caching
    async fn query_all_stop_times(
        &self,
        source: Source,
        at: DateTime<Utc>,
        route_ids: Option<&[String]>,
    ) -> anyhow::Result<Vec<StopTime>> {
        let route_ids_vec: Vec<String> = route_ids.map(|r| r.to_vec()).unwrap_or_default();
        let has_route_filter = !route_ids_vec.is_empty();

        Ok(sqlx::query_as::<_, StopTime>(
            r#"
            SELECT
                st.trip_id,
                st.stop_id,
                st.arrival,
                st.departure,
                st.data
            FROM realtime.stop_time st
            INNER JOIN realtime.trip t ON t.id = st.trip_id
            WHERE
                st.source = $1
                AND (
                    st.arrival BETWEEN $2 AND ($2 + INTERVAL '4 hours')
                    OR st.departure BETWEEN $2 AND ($2 + INTERVAL '4 hours')
                )
                AND ($4 = false OR t.route_id = ANY($3))
            ORDER BY st.arrival ASC
            "#,
        )
        .bind(source)
        .bind(at)
        .bind(&route_ids_vec)
        .bind(has_route_filter)
        .fetch_all(&self.pg_pool)
        .await?)
    }
}
