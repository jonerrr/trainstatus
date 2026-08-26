use crate::{
    models::{
        geom::Geom,
        source::Source,
        trip::{StopTime, Trip},
    },
    stores::{cache_get, cache_set},
};
use bb8_redis::RedisConnectionManager;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::time::Instant;
use std::{collections::HashMap, time::Duration};
use uuid::Uuid;

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct TrajectoryInputRow {
    pub trip_id: Uuid,
    pub route_id: String,
    pub route_color: String,
    pub direction: i16,
    pub trip_geom: Geom,
    /// The shape ID that was selected for this trip (either from the trip's own
    /// shape_ids or resolved via best-fit from the route's shape list).
    pub shape_id: String,
    pub stop_id: String,
    pub arrival_unix: f64,
    pub departure_unix: f64,
    pub stop_distance_m: f64,
    pub car_count: Option<i32>,
    pub car_length_feet: Option<i32>,
    #[sqlx(json)]
    pub stop_time_data: serde_json::Value,
    #[sqlx(json)]
    pub stop_data: serde_json::Value,
}

const TTL: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct TripStore {
    pg_pool: PgPool,
    redis_pool: bb8::Pool<RedisConnectionManager>,
}

impl TripStore {
    pub fn new(pg_pool: PgPool, redis_pool: bb8::Pool<RedisConnectionManager>) -> Self {
        Self {
            pg_pool,
            redis_pool,
        }
    }

    /// Gets all trips for a specific source, optionally filtered by a specific time.
    /// If a time is specified, trips are not cached.
    pub async fn get_all(
        &self,
        source: Source,
        at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<Vec<Trip>> {
        if let Some(at) = at {
            return self.query_all_trips(source, at).await;
        }

        let key = format!("trips:{}", source.as_str());
        if let Some(cached) = cache_get::<Vec<Trip>>(&self.redis_pool, &key).await {
            return Ok(cached);
        }
        self.query_all_trips(source, Utc::now()).await
    }

    /// Populate the trips Redis cache by re-querying from DB.
    async fn populate_cache(&self, source: Source) -> anyhow::Result<()> {
        let key = format!("trips:{}", source.as_str());
        let trips = self.query_all_trips(source, Utc::now()).await?;
        cache_set(&self.redis_pool, &key, &trips, TTL).await
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
                    AND
                    t.updated_at >= (($2)::timestamp with time zone - INTERVAL '5 minutes')
                    AND t.id = ANY(
                        SELECT t.id
                        FROM realtime.trip t
                        LEFT JOIN realtime.stop_time st ON st.trip_id = t.id
                        WHERE st.arrival BETWEEN $2 AND ($2 + INTERVAL '4 hours')
                    )
                ORDER BY t.created_at DESC"#,
        )
        .bind(source)
        .bind(at)
        .fetch_all(&self.pg_pool)
        .await?)
    }

    /// Return PostGIS-prepared trajectory inputs for active trips.
    ///
    /// This pushes per-trip shape assembly and stop projection to SQL so the
    /// handler can focus on interpolation and response shaping.
    ///
    /// For sources where GTFS-RT provides `shape_ids` on the trip (e.g. MTA Subway),
    /// those are used directly. For sources that do not (e.g. MTA Bus), we fall back
    /// to picking the best-fitting shape from the route's stored shape_ids
    /// (populated at static import time from the Helium infrastructure API).
    /// "Best" = the shape with the smallest average ST_Distance to the trip's stops.
    pub async fn get_trajectory_inputs(
        &self,
        source: Source,
        at: DateTime<Utc>,
        route_ids: Option<&[String]>,
    ) -> anyhow::Result<Vec<TrajectoryInputRow>> {
        let route_ids_vec: Vec<String> = route_ids.map(|r| r.to_vec()).unwrap_or_default();
        let has_route_filter = !route_ids_vec.is_empty();

        Ok(sqlx::query_as::<_, TrajectoryInputRow>(
            r#"
            WITH filtered_trips AS (
                SELECT
                    t.id,
                    t.route_id,
                    t.direction,
                    t.source,
                    t.shape_ids,
                    (t.data->'consist'->>'car_count')::int AS car_count,
                    (t.data->'consist'->>'car_length_feet')::int AS car_length_feet
                FROM realtime.trip t
                WHERE
                    t.source = $1
                    AND t.updated_at >= (($2)::timestamp with time zone - INTERVAL '5 minutes')
                    AND t.id = ANY(
                        SELECT t2.id
                        FROM realtime.trip t2
                        LEFT JOIN realtime.stop_time st2 ON st2.trip_id = t2.id
                        WHERE st2.arrival BETWEEN $2 AND ($2 + INTERVAL '4 hours')
                    )
                    AND ($4 = false OR t.route_id = ANY($3))
            ),
            -- Determine effective shape_ids per trip:
            -- If the trip itself has shape_ids (e.g. subway), use those directly.
            -- Otherwise fall back to the route-level shape_ids stored in static.route.data.
            trip_shape_candidates AS (
                SELECT
                    ft.id AS trip_id,
                    ft.route_id,
                    ft.direction,
                    ft.car_count,
                    ft.car_length_feet,
                    r.color AS route_color,
                    r.source AS route_source,
                    COALESCE(array_length(ft.shape_ids, 1), 0) > 0 AS has_own_shape_ids,
                    CASE
                        WHEN COALESCE(array_length(ft.shape_ids, 1), 0) > 0
                            THEN ft.shape_ids
                        ELSE
                            ARRAY(
                                SELECT jsonb_array_elements_text(r.data->'shape_ids')
                            )
                    END AS effective_shape_ids
                FROM filtered_trips ft
                JOIN static.route r ON r.id = ft.route_id AND r.source = ft.source
            ),
            -- Trips with their own shape_ids (subway) report one segment per
            -- station-to-station leg, in travel order
            merged_shape AS (
                SELECT
                    tsc.trip_id,
                    tsc.route_id,
                    tsc.direction,
                    tsc.car_count,
                    tsc.car_length_feet,
                    tsc.route_color,
                    array_to_string(tsc.effective_shape_ids, ',') AS shape_id,
                    ST_MakeLine(sh.geom ORDER BY sid.ord) AS shape_geom
                FROM trip_shape_candidates tsc
                JOIN LATERAL UNNEST(tsc.effective_shape_ids) WITH ORDINALITY AS sid(id, ord) ON TRUE
                JOIN static.shape sh ON sh.id = sid.id AND sh.source = tsc.route_source
                WHERE tsc.has_own_shape_ids
                GROUP BY tsc.trip_id, tsc.route_id, tsc.direction, tsc.car_count,
                         tsc.car_length_feet, tsc.route_color, tsc.effective_shape_ids
            ),
            -- Trips using route-level shapes (no shape_ids of their own, e.g. bus) have
            -- several candidate whole-route shapes and no realtime signal for which one
            -- they're on. For each candidate, count how many of the trip's actual stops
            -- Helium's static data says that shape serves (static.route_stop.data) — an
            -- exact identity signal, not a geometric guess.
            candidate_shape_hits AS (
                SELECT
                    tsc.trip_id,
                    sid AS shape_id,
                    count(*) AS hits
                FROM trip_shape_candidates tsc
                JOIN realtime.stop_time st
                    ON st.trip_id = tsc.trip_id AND st.source = tsc.route_source
                JOIN static.route_stop rs
                    ON rs.route_id = tsc.route_id AND rs.stop_id = st.stop_id AND rs.source = tsc.route_source
                CROSS JOIN LATERAL jsonb_array_elements_text(COALESCE(rs.data->'shape_ids', '[]'::jsonb)) AS sid
                WHERE NOT tsc.has_own_shape_ids
                GROUP BY tsc.trip_id, sid
            ),
            -- Score every candidate whole-route shape by its confirmed stop-membership
            -- hit count (0 when the shape serves none of the trip's stops).
            shape_candidate_scores AS (
                SELECT
                    tsc.trip_id,
                    sid AS shape_id,
                    COALESCE(csh.hits, 0) AS hits
                FROM trip_shape_candidates tsc
                JOIN LATERAL UNNEST(tsc.effective_shape_ids) AS sid ON TRUE
                LEFT JOIN candidate_shape_hits csh
                    ON csh.trip_id = tsc.trip_id AND csh.shape_id = sid
                WHERE NOT tsc.has_own_shape_ids
            ),
            -- Primary selector: the candidate with the most stop-membership hits,
            -- ties broken deterministically by shape_id. This is cheap — no geometry.
            hit_best_shape AS (
                SELECT DISTINCT ON (trip_id)
                    trip_id, shape_id, hits
                FROM shape_candidate_scores
                ORDER BY trip_id, hits DESC, shape_id
            ),
            -- Geometric fallback for the rare trip whose stops give no hit signal at
            -- all (e.g. stops/routes predating the route_stop shape data). Only these
            -- trips pay the ST_Distance cost, so a cold cache no longer computes
            -- distances over full-route linestrings for every candidate of every trip
            -- (which took minutes and could OOM the DB). Planar distance is fine here —
            -- it is only a relative tie-break, not a reported metric.
            dist_best_shape AS (
                SELECT DISTINCT ON (tsc.trip_id)
                    tsc.trip_id, sh.id AS shape_id
                FROM trip_shape_candidates tsc
                JOIN LATERAL UNNEST(tsc.effective_shape_ids) AS sid ON TRUE
                JOIN static.shape sh ON sh.id = sid AND sh.source = tsc.route_source
                JOIN realtime.stop_time st
                    ON st.trip_id = tsc.trip_id AND st.source = tsc.route_source
                JOIN static.stop s ON s.id = st.stop_id AND s.source = tsc.route_source
                WHERE NOT tsc.has_own_shape_ids
                  AND st.arrival BETWEEN $2 AND ($2 + INTERVAL '4 hours')
                  AND tsc.trip_id IN (SELECT trip_id FROM hit_best_shape WHERE hits = 0)
                GROUP BY tsc.trip_id, sh.id
                ORDER BY tsc.trip_id, AVG(ST_Distance(sh.geom, s.geom)) ASC
            ),
            chosen_shape AS (
                SELECT trip_id, shape_id FROM hit_best_shape WHERE hits > 0
                UNION ALL
                SELECT trip_id, shape_id FROM dist_best_shape
            ),
            best_shape AS (
                SELECT
                    tsc.trip_id,
                    tsc.route_id,
                    tsc.direction,
                    tsc.car_count,
                    tsc.car_length_feet,
                    tsc.route_color,
                    sh.id AS shape_id,
                    sh.geom AS shape_geom
                FROM chosen_shape cs
                JOIN trip_shape_candidates tsc ON tsc.trip_id = cs.trip_id
                JOIN static.shape sh ON sh.id = cs.shape_id AND sh.source = tsc.route_source
                WHERE NOT tsc.has_own_shape_ids
            ),
            trip_lines AS (
                SELECT trip_id, route_id, direction, car_count, car_length_feet,
                       route_color, shape_id, shape_geom AS trip_geom
                FROM merged_shape
                WHERE GeometryType(shape_geom) = 'LINESTRING'
                  AND ST_NPoints(shape_geom) >= 2
                UNION ALL
                SELECT trip_id, route_id, direction, car_count, car_length_feet,
                       route_color, shape_id, shape_geom AS trip_geom
                FROM best_shape
                WHERE GeometryType(shape_geom) = 'LINESTRING'
                  AND ST_NPoints(shape_geom) >= 2
            )
            SELECT
                tl.trip_id,
                tl.route_id,
                tl.route_color,
                tl.direction,
                tl.trip_geom,
                tl.shape_id,
                st.stop_id,
                EXTRACT(EPOCH FROM st.arrival)::double precision AS arrival_unix,
                EXTRACT(EPOCH FROM st.departure)::double precision AS departure_unix,
                (
                    ST_LineLocatePoint(tl.trip_geom, s.geom)
                    * ST_Length(tl.trip_geom::geography)
                )::double precision AS stop_distance_m
                ,tl.car_count,
                tl.car_length_feet,
                st.data AS stop_time_data,
                s.data AS stop_data
            FROM trip_lines tl
            JOIN realtime.stop_time st ON st.trip_id = tl.trip_id
            JOIN static.stop s ON s.id = st.stop_id AND s.source = $1
            WHERE
                st.source = $1
                AND st.arrival BETWEEN $2 AND ($2 + INTERVAL '4 hours')
            ORDER BY tl.trip_id, st.arrival
            "#,
        )
        .bind(source)
        .bind(at)
        .bind(&route_ids_vec)
        .bind(has_route_filter)
        .fetch_all(&self.pg_pool)
        .await?)
    }

    /// Bulk insert trips with their stop times so we can remap to the correct trip IDs.
    /// Returns a map of input_id -> actual_id for callers that need to reference the saved trips.
    #[tracing::instrument(level = "debug", skip(self, data), fields(source = %source, count = data.len()))]
    pub async fn save_all(
        &self,
        source: Source,
        data: &[(Trip, Vec<StopTime>)],
    ) -> anyhow::Result<HashMap<Uuid, Uuid>> {
        let start = Instant::now();

        if data.is_empty() {
            tracing::debug!("No trips to insert");
            return Ok(HashMap::new());
        }

        // TODO: remove this once we figure out why this is happening
        // Deduplicate trips based on unique constraint: (original_id, vehicle_id, created_at, direction)
        let mut unique_trips = HashMap::new();
        for (trip, sts) in data {
            let key = (
                trip.original_id.clone(),
                trip.vehicle_id.clone(),
                trip.created_at,
                trip.direction,
            );
            // Keep the one with the latest updated_at
            unique_trips
                .entry(key)
                .and_modify(
                    |(existing_trip, existing_sts): &mut (Trip, Vec<StopTime>)| {
                        if trip.updated_at > existing_trip.updated_at {
                            *existing_trip = trip.clone();
                            *existing_sts = sts.clone();
                        }
                    },
                )
                .or_insert_with(|| (trip.clone(), sts.clone()));
        }

        let deduped_data: Vec<_> = unique_trips.into_values().collect();

        // Prepare vectors for bulk insert
        let input_ids: Vec<Uuid> = deduped_data.iter().map(|(t, _)| t.id).collect();
        // TODO: maybe make original and vehicle id uppercase for consistency
        let original_ids: Vec<String> = deduped_data
            .iter()
            .map(|(t, _)| t.original_id.clone())
            .collect();
        let vehicle_ids: Vec<String> = deduped_data
            .iter()
            .map(|(t, _)| t.vehicle_id.clone())
            .collect();
        let route_ids: Vec<String> = deduped_data
            .iter()
            .map(|(t, _)| t.route_id.to_uppercase())
            .collect();
        let shape_ids_json: Vec<serde_json::Value> = deduped_data
            .iter()
            .map(|(t, _)| serde_json::to_value(&t.shape_ids).unwrap())
            .collect();
        let sources: Vec<Source> = vec![source; deduped_data.len()];
        let directions: Vec<i16> = deduped_data.iter().map(|(t, _)| t.direction).collect();
        let created_ats: Vec<DateTime<Utc>> =
            deduped_data.iter().map(|(t, _)| t.created_at).collect();
        let updated_ats: Vec<DateTime<Utc>> =
            deduped_data.iter().map(|(t, _)| t.updated_at).collect();
        let trip_data: Vec<serde_json::Value> = deduped_data
            .iter()
            .map(|(t, _)| serde_json::to_value(&t.data).unwrap())
            .collect();

        // TODO: add back inserting one a time so if one fails we can still insert the rest (or ensure fk violation static data retry works)
        // Bulk insert/upsert trips and get mapping from input_id -> actual_id
        // We use a CTE to map the input_id (which we generated) to the actual_id (from DB)
        // matching on the unique constraint columns.
        // Note: shape_ids are passed as jsonb and converted to varchar[] in SQL
        // because UNNEST doesn't handle array-of-arrays well.
        let records  = sqlx::query!(
            r#"
            WITH input_rows AS (
                SELECT
                    t.input_id,
                    t.original_id,
                    t.vehicle_id,
                    t.route_id,
                    ARRAY(SELECT jsonb_array_elements_text(t.shape_ids_json))::varchar[] AS shape_ids,
                    t.source,
                    t.direction,
                    t.created_at,
                    t.updated_at,
                    t.data
                FROM UNNEST(
                    $1::uuid[], $2::text[], $3::text[], $4::text[], $5::jsonb[], $6::source_enum[],
                    $7::smallint[], $8::timestamptz[], $9::timestamptz[], $10::jsonb[]
                ) AS t(input_id, original_id, vehicle_id, route_id, shape_ids_json, source, direction, created_at, updated_at, data)
            ),
            inserted_rows AS (
                INSERT INTO realtime.trip (id, original_id, vehicle_id, route_id, shape_ids, source, direction, created_at, updated_at, data)
                SELECT input_id, original_id, vehicle_id, route_id, shape_ids, source, direction, created_at, updated_at, data
                FROM input_rows
                 ON CONFLICT (original_id, vehicle_id, created_at, direction) DO UPDATE SET
                    data = EXCLUDED.data,
                    updated_at = EXCLUDED.updated_at,
                    shape_ids = CASE
                        WHEN EXCLUDED.shape_ids IS NOT NULL AND array_length(EXCLUDED.shape_ids, 1) > 0
                        THEN EXCLUDED.shape_ids
                        ELSE realtime.trip.shape_ids
                    END
                RETURNING id, original_id, vehicle_id, created_at, direction
            )
            SELECT
                inserted_rows.id AS actual_id,
                input_rows.input_id
            FROM inserted_rows
            JOIN input_rows ON
                inserted_rows.original_id = input_rows.original_id AND
                inserted_rows.vehicle_id = input_rows.vehicle_id AND
                inserted_rows.created_at = input_rows.created_at AND
                inserted_rows.direction = input_rows.direction
            "#,
            &input_ids,
            &original_ids,
            &vehicle_ids,
            &route_ids,
            &shape_ids_json,
            &sources as &[Source],
            &directions as &[i16],
            &created_ats,
            &updated_ats,
            &trip_data
        )
        .fetch_all(&self.pg_pool)
        .await?;

        // Create a map for quick lookup
        let id_map: HashMap<Uuid, Uuid> = records
            .into_iter()
            .filter_map(|r| r.input_id.map(|id| (id, r.actual_id)))
            .collect();

        // Prepare stop times for bulk insert
        let mut st_trip_ids = Vec::new();
        let mut st_stop_ids = Vec::new();
        let mut st_arrivals = Vec::new();
        let mut st_departures = Vec::new();
        let mut st_data = Vec::new();

        let mut seen_stop_times = std::collections::HashSet::new();
        let mut duplicate_count = 0usize;

        for (trip, sts) in deduped_data {
            if let Some(&actual_id) = id_map.get(&trip.id) {
                for st in sts {
                    let stop_id = st.stop_id.to_uppercase();
                    let key = (actual_id, stop_id.clone());
                    if !seen_stop_times.insert(key) {
                        duplicate_count += 1;
                        continue;
                    }
                    st_trip_ids.push(actual_id);
                    st_stop_ids.push(stop_id);
                    st_arrivals.push(st.arrival);
                    st_departures.push(st.departure);
                    st_data.push(serde_json::to_value(&st.data).unwrap());
                }
            }
        }

        // TODO: figure out why there are dupes once in a while??!!!!
        if duplicate_count > 0 {
            tracing::warn!(duplicate_count, "Deduplicated stop times before insert");
        }
        // im inserting the stop times in the same functions as trips since the stop time struct doesn't have a trip_id field
        // might want to return the mapping or put this in a separate function later (so it can cache and invalidate that cache)
        if !st_trip_ids.is_empty() {
            sqlx::query!(
                r#"
                INSERT INTO realtime.stop_time (trip_id, stop_id, source, arrival, departure, data)
                SELECT * FROM UNNEST($1::uuid[], $2::text[], $3::source_enum[], $4::timestamptz[], $5::timestamptz[], $6::jsonb[])
                ON CONFLICT (trip_id, stop_id, source) DO UPDATE SET
                    arrival = EXCLUDED.arrival,
                    departure = EXCLUDED.departure,
                    data = EXCLUDED.data
                "#,
                &st_trip_ids,
                &st_stop_ids,
                &vec![source; st_trip_ids.len()] as _,
                &st_arrivals,
                &st_departures,
                &st_data
            )
            .execute(&self.pg_pool)
            .await?;
            // TODO: invalidate stop time cache if we have one
        }

        let elapsed = start.elapsed();

        tracing::info!(
            "Inserted {} trips and {} stop times successfully in {:.2}s",
            data.len(),
            st_trip_ids.len(),
            elapsed.as_secs_f64()
        );

        // Populate trips cache (write-through)
        self.populate_cache(source).await?;

        // might want to insert positions from here instead of returning the map
        Ok(id_map)
    }

    // TODO: improve this. i don't like how the shape resolution is separate from the main insert also maybe this should be using moka?
    /// Cache resolved shape IDs on trips that don't have shape IDs set.
    pub async fn update_resolved_shapes(&self, shapes: &[(Uuid, String)]) -> anyhow::Result<()> {
        if shapes.is_empty() {
            return Ok(());
        }

        let mut trip_ids = Vec::with_capacity(shapes.len());
        let mut shape_ids = Vec::with_capacity(shapes.len());

        for (trip_id, shape_id) in shapes {
            trip_ids.push(*trip_id);
            shape_ids.push(shape_id.clone());
        }

        sqlx::query!(
            r#"
            UPDATE realtime.trip AS t
            SET shape_ids = ARRAY[u.shape_id]
            FROM UNNEST($1::uuid[], $2::text[]) AS u(trip_id, shape_id)
            WHERE t.id = u.trip_id
              AND (t.shape_ids IS NULL OR array_length(t.shape_ids, 1) IS NULL OR array_length(t.shape_ids, 1) = 0)
            "#,
            &trip_ids,
            &shape_ids,
        )
        .execute(&self.pg_pool)
        .await?;

        Ok(())
    }
}
