use crate::{
    models::{
        geom::Geom,
        position::{
            MtaBusPositionData, MtaSubwayPositionData, NjtBusPositionData, PositionData,
            VehiclePosition,
        },
        source::Source,
        stop::StopData,
        trip::{StopTime, Trip},
    },
    static_data::index::{IndexedRoute, IndexedStop, StaticTransitRevision},
    trajectory::{TrajectoryCache, TripSnapshot, snapshot_from_persisted_trip},
};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc};

#[derive(sqlx::FromRow)]
struct HistoricalRoute {
    id: String,
    color: String,
}
#[derive(sqlx::FromRow)]
struct HistoricalStop {
    id: String,
    geom: Geom,
    #[sqlx(json)]
    data: StopData,
}
#[derive(sqlx::FromRow)]
struct HistoricalShape {
    id: String,
    geom: Geom,
}
#[derive(sqlx::FromRow)]
struct HistoricalPosition {
    trip_id: uuid::Uuid,
    geom: Geom,
    recorded_at: DateTime<Utc>,
}

/// Explicit historical requests read retained rows and already-resolved shapes.
/// This store is never involved in live trajectory refreshes or live cache misses.
#[derive(Clone)]
pub struct TrajectoryStore {
    pg_pool: PgPool,
    cache: Arc<TrajectoryCache>,
}
impl TrajectoryStore {
    pub fn new(pg_pool: PgPool, cache: Arc<TrajectoryCache>) -> Self {
        Self { pg_pool, cache }
    }
    pub async fn load_historical_inputs(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<Vec<TripSnapshot>> {
        let trips = sqlx::query_as::<_, Trip>(
            r#"
            SELECT t.* FROM realtime.trip t
            WHERE t.source = $1 AND cardinality(t.shape_ids) > 0 AND EXISTS (
                SELECT 1 FROM realtime.stop_time st WHERE st.trip_id = t.id AND st.source = $1
                AND (st.arrival BETWEEN $2 AND ($2 + INTERVAL '4 hours')
                    OR st.departure BETWEEN $2 AND ($2 + INTERVAL '4 hours'))
            ) ORDER BY t.id
        "#,
        )
        .bind(source)
        .bind(at)
        .fetch_all(&self.pg_pool)
        .await?;
        if trips.is_empty() {
            return Ok(vec![]);
        }
        let ids: Vec<_> = trips.iter().map(|trip| trip.id).collect();
        let route_ids: Vec<_> = trips.iter().map(|trip| trip.route_id.clone()).collect();
        let shape_ids: Vec<_> = trips
            .iter()
            .flat_map(|trip| trip.shape_ids.clone())
            .collect();
        let stop_times = sqlx::query_as::<_, StopTime>("SELECT * FROM realtime.stop_time WHERE source = $1 AND trip_id = ANY($2) ORDER BY trip_id, arrival, departure, stop_id").bind(source).bind(&ids).fetch_all(&self.pg_pool).await?;
        let stop_ids: Vec<_> = stop_times.iter().map(|stop| stop.stop_id.clone()).collect();
        let routes = sqlx::query_as::<_, HistoricalRoute>(
            "SELECT id, color FROM static.route WHERE source = $1 AND id = ANY($2)",
        )
        .bind(source)
        .bind(&route_ids)
        .fetch_all(&self.pg_pool)
        .await?;
        let stops = sqlx::query_as::<_, HistoricalStop>(
            "SELECT id, geom, data FROM static.stop WHERE source = $1 AND id = ANY($2)",
        )
        .bind(source)
        .bind(&stop_ids)
        .fetch_all(&self.pg_pool)
        .await?;
        let shapes = sqlx::query_as::<_, HistoricalShape>(
            "SELECT id, geom FROM static.shape WHERE source = $1 AND id = ANY($2)",
        )
        .bind(source)
        .bind(&shape_ids)
        .fetch_all(&self.pg_pool)
        .await?;
        // History stores geometry and observation time, not source-specific status
        // or occupancy. Do not use the current vehicle row (which may be later).
        let points = sqlx::query_as::<_, HistoricalPosition>(
            r#"
            SELECT DISTINCT ON (trip_id) trip_id, geom, recorded_at FROM realtime.trip_history_point
            WHERE trip_id = ANY($1) AND recorded_at BETWEEN ($2 - INTERVAL '5 minutes') AND $2
            ORDER BY trip_id, recorded_at DESC
        "#,
        )
        .bind(&ids)
        .bind(at)
        .fetch_all(&self.pg_pool)
        .await?;
        let revision = StaticTransitRevision {
            source,
            routes: routes
                .into_iter()
                .map(|route| {
                    (
                        route.id,
                        IndexedRoute {
                            color: route.color,
                            shape_ids: vec![],
                        },
                    )
                })
                .collect(),
            stops: stops
                .into_iter()
                .map(|stop| {
                    (
                        stop.id,
                        IndexedStop {
                            geom: stop.geom,
                            data: stop.data,
                        },
                    )
                })
                .collect(),
            shapes: shapes
                .into_iter()
                .map(|shape| (shape.id, shape.geom))
                .collect(),
            route_stop_shapes: HashMap::new(),
            trip_patterns: HashMap::new(),
            stop_remap: HashMap::new(),
            scheduled_trips: HashMap::new(),
        };
        let mut stops_by_trip: HashMap<_, Vec<_>> = HashMap::new();
        for stop in stop_times {
            stops_by_trip.entry(stop.trip_id).or_default().push(stop);
        }
        let mut points_by_trip: HashMap<_, _> = points
            .into_iter()
            .map(|point| (point.trip_id, point))
            .collect();
        let mut inputs = Vec::new();
        for trip in trips {
            let positions = points_by_trip
                .remove(&trip.id)
                .map(|point| VehiclePosition {
                    trip_id: Some(trip.id),
                    vehicle_id: trip.vehicle_id.clone(),
                    stop_id: None,
                    geom: Some(point.geom),
                    updated_at: point.recorded_at,
                    data: match source {
                        Source::MtaBus => PositionData::MtaBus(MtaBusPositionData {
                            bearing: 0.0,
                            passengers: None,
                            capacity: None,
                            status: None,
                            phase: None,
                        }),
                        Source::MtaSubway => PositionData::MtaSubway(MtaSubwayPositionData {
                            assigned: false,
                            status: None,
                        }),
                        Source::NjtBus => PositionData::NjtBus(NjtBusPositionData {
                            occupancy_status:
                                crate::feed::vehicle_position::OccupancyStatus::NoDataAvailable,
                        }),
                    },
                })
                .into_iter()
                .collect();
            match snapshot_from_persisted_trip(
                &trip,
                stops_by_trip.remove(&trip.id).unwrap_or_default(),
                positions,
                &revision,
                &self.cache,
                at,
            )
            .await
            {
                Ok(input) => inputs.push(input),
                Err(error) => {
                    tracing::debug!(%source, trip_id = %trip.id, %error, "Skipping invalid historical trajectory input")
                }
            }
        }
        Ok(inputs)
    }
}
