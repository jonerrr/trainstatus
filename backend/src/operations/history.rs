use super::postgres::TripRow;
use crate::geometry::Geom;
use crate::network::index::IndexedStop;
use crate::operations::types::trip::StopVisit;
use crate::operations::types::trip::TripRun;
use crate::source::Source;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::collections::HashMap;

// Private SQL projections read only rendering inputs, including retired identities
// and immutable assets. Current catalog queries omit retired rows and load unrelated
// names, nearby stops and route memberships.
struct RouteColorRow {
    id: String,
    color: String,
}
struct RetainedStopRow {
    id: String,
    source: Source,
    geom: Geom,
    data: serde_json::Value,
}
struct ShapeAssetRow {
    id: String,
    geom: Geom,
}
struct GpsSampleRow {
    trip_id: uuid::Uuid,
    longitude: f64,
    latitude: f64,
    observed_at: DateTime<Utc>,
}

/// Recorded coordinates have no vehicle status or provider payload.
#[derive(Clone, Debug)]
pub struct GpsSample {
    pub observed_at: DateTime<Utc>,
    pub point: geo::Point<f64>,
}
/// Latest retained stop descriptions plus immutable shape assets for selected runs.
/// This is not an archived network revision.
pub struct RetainedGeometry {
    pub source: Source,
    pub routes: HashMap<String, String>,
    pub stops: HashMap<String, IndexedStop>,
    pub shapes: HashMap<String, Geom>,
}
/// All visits for eligible runs, materialized from one consistent database read.
pub struct RetainedService {
    pub trips: Vec<TripRun>,
    pub stop_times: Vec<StopVisit>,
}

/// Passenger inputs plus the geometry needed to render their trajectories.
pub struct RetainedTrajectories {
    pub service: RetainedService,
    pub network: Option<RetainedGeometry>,
    pub anchors: HashMap<uuid::Uuid, GpsSample>,
}
#[derive(Clone)]
pub struct History {
    pg_pool: PgPool,
}
impl History {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
    }
    pub async fn load(
        &self,
        source: Source,
        at: DateTime<Utc>,
        route_ids: Option<&[String]>,
    ) -> anyhow::Result<RetainedService> {
        let mut tx = self.begin_read().await?;
        let service = Self::load_service_on(&mut tx, source, at, route_ids).await?;
        tx.commit().await?;
        Ok(service)
    }

    async fn begin_read(&self) -> anyhow::Result<sqlx::Transaction<'_, sqlx::Postgres>> {
        let mut tx = self.pg_pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }

    async fn load_service_on(
        tx: &mut sqlx::PgConnection,
        source: Source,
        at: DateTime<Utc>,
        route_ids: Option<&[String]>,
    ) -> anyhow::Result<RetainedService> {
        let trips = sqlx::query_as!(
            TripRow,
            r#"
            SELECT
                t.*
            FROM realtime.trip t
            WHERE t.source = $1
            AND($3::text[] IS NULL OR t.route_id = ANY($3))
            AND EXISTS(SELECT 1 FROM realtime.stop_time st WHERE st.trip_id = t.id AND st.source = $1 AND(st.arrival BETWEEN $2 AND($2 + INTERVAL '4 hours') OR st.departure BETWEEN $2 AND($2 + INTERVAL '4 hours')))
            ORDER BY t.id
            "#,
            source,
            at,
            route_ids
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(TryInto::try_into)
        .collect::<anyhow::Result<Vec<TripRun>>>()?;
        let ids: Vec<_> = trips.iter().map(|trip| trip.id).collect();
        let stop_times = sqlx::query!(
            r#"
            SELECT
                *
            FROM realtime.stop_time
            WHERE source = $1
            AND trip_id = ANY($2)
            ORDER BY trip_id, stop_sequence
            "#,
            source,
            &ids
        )
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|row| {
            Ok(StopVisit {
                trip_id: row.trip_id,
                stop_sequence: row.stop_sequence,
                stop_id: row.stop_id,
                arrival: row.arrival,
                departure: row.departure,
                data: crate::postgres::decode(row.source, row.data)?,
            })
        })
        .collect::<anyhow::Result<Vec<StopVisit>>>()?;
        Ok(RetainedService { trips, stop_times })
    }

    pub async fn load_trajectories(
        &self,
        source: Source,
        at: DateTime<Utc>,
    ) -> anyhow::Result<RetainedTrajectories> {
        let mut tx = self.begin_read().await?;
        let mut service = Self::load_service_on(&mut tx, source, at, None).await?;
        if service.trips.is_empty() {
            tx.commit().await?;
            return Ok(RetainedTrajectories {
                service,
                network: None,
                anchors: HashMap::new(),
            });
        }
        let trips = &mut service.trips;
        let ids: Vec<_> = trips.iter().map(|trip| trip.id).collect();
        let route_ids: Vec<_> = trips.iter().map(|trip| trip.route_id.clone()).collect();
        let refs = sqlx::query!(
            r#"
            SELECT
                run_id,
                provider_shape_id,
                asset_id
            FROM realtime.run_shape
            WHERE run_id = ANY($1)
            ORDER BY run_id, ordinal
            "#,
            &ids
        )
        .fetch_all(&mut *tx)
        .await?;
        let mut shapes_by_run: HashMap<_, Vec<_>> = HashMap::new();
        let mut aliases = HashMap::new();
        for row in refs {
            let alias = format!("{}@{}", row.provider_shape_id, row.asset_id);
            aliases.insert(alias.clone(), row.asset_id);
            shapes_by_run.entry(row.run_id).or_default().push(alias);
        }
        for trip in trips.iter_mut() {
            trip.shape_ids = shapes_by_run.remove(&trip.id).unwrap_or_default();
        }
        let shape_ids: Vec<_> = aliases.values().cloned().collect();
        let stop_ids: Vec<_> = service
            .stop_times
            .iter()
            .map(|stop| stop.stop_id.clone())
            .collect();
        let routes = sqlx::query_as!(
            RouteColorRow,
            "SELECT id, color FROM static.route WHERE source = $1 AND id = ANY($2)",
            source,
            &route_ids
        )
        .fetch_all(&mut *tx)
        .await?;
        let stops = sqlx::query_as!(
            RetainedStopRow,
            "SELECT id, source, geom, data FROM static.stop WHERE source = $1 AND id = ANY($2)",
            source,
            &stop_ids
        )
        .fetch_all(&mut *tx)
        .await?;
        let shapes = sqlx::query_as!(
            ShapeAssetRow,
            "SELECT id, geom FROM static.shape_asset WHERE source = $1 AND id = ANY($2)",
            source,
            &shape_ids
        )
        .fetch_all(&mut *tx)
        .await?;
        // History stores geometry and observation time, not source-specific status
        // or occupancy. Do not use the current vehicle row (which may be later).
        let points = sqlx::query_as!(
            GpsSampleRow,
            r#"
            SELECT
                DISTINCT ON(run_id) run_id AS trip_id,
                longitude,
                latitude,
                observed_at
            FROM realtime.gps_sample
            WHERE run_id = ANY($1)
            AND observed_at BETWEEN($2::timestamptz - INTERVAL '5 minutes')
            AND $2
            ORDER BY run_id, observed_at DESC
            "#,
            &ids,
            at
        )
        .fetch_all(&mut *tx)
        .await?;
        let revision = RetainedGeometry {
            source,
            routes: routes
                .into_iter()
                .map(|route| (route.id, route.color))
                .collect(),
            stops: stops
                .into_iter()
                .map(|stop| {
                    Ok((
                        stop.id,
                        IndexedStop {
                            geom: stop.geom,
                            data: crate::postgres::decode(stop.source, stop.data)?,
                        },
                    ))
                })
                .collect::<anyhow::Result<_>>()?,
            shapes: {
                let assets: HashMap<_, _> = shapes
                    .into_iter()
                    .map(|shape| (shape.id, shape.geom))
                    .collect();
                aliases
                    .into_iter()
                    .filter_map(|(alias, asset)| {
                        assets.get(&asset).cloned().map(|geom| (alias, geom))
                    })
                    .collect()
            },
        };
        tx.commit().await?;
        Ok(RetainedTrajectories {
            service,
            network: Some(revision),
            anchors: points
                .into_iter()
                .map(|p| {
                    (
                        p.trip_id,
                        GpsSample {
                            observed_at: p.observed_at,
                            point: geo::Point::new(p.longitude, p.latitude),
                        },
                    )
                })
                .collect(),
        })
    }
}
