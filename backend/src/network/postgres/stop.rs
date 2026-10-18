use crate::geometry::Geom;
use crate::network::types::stop::NearbyStop;
use crate::network::types::stop::RouteStop;
use crate::network::types::stop::Stop;
use crate::source::Source;
use sqlx::PgConnection;

// The nested JSON projection needs FromRow; its source column is used only to
// decode the domain's tagged data and is not part of Stop itself.
#[derive(sqlx::FromRow)]
struct StopRow {
    source: Source,
    id: String,
    name: String,
    geom: Geom,
    #[sqlx(json)]
    nearby_stops: Vec<NearbyStop>,
    #[sqlx(json)]
    data: serde_json::Value,
    #[sqlx(json)]
    routes: Vec<RouteStop>,
}

/// Raw DB query for all stops of a source (with embedded nearby stops and route associations).
pub(in crate::network) async fn query_all_on(
    source: Source,
    connection: &mut PgConnection,
) -> anyhow::Result<Vec<Stop>> {
    // Runtime projection: nested route/nearby-stop JSON decodes through FromRow.
    sqlx::query_as::<_, StopRow>(
        r#"SELECT
                s.id,
                s.source,
                s.name,
                s.geom,
                COALESCE(
                    (
                        SELECT jsonb_agg(
                            jsonb_build_object(
                                'stop_id', st.to_stop_id,
                                'source', st.to_stop_source,
                                'distance_m', st.distance_m
                            ) ORDER BY st.distance_m, st.to_stop_source, st.to_stop_id
                        )
                        FROM static.nearby_stop st
                        WHERE st.from_stop_id = s.id
                          AND st.from_stop_source = s.source
                    ),
                    '[]'::jsonb
                ) AS nearby_stops,
                s.data,
                COALESCE(
                    (
                        SELECT jsonb_agg(to_jsonb(rs) || jsonb_build_object('data', rs.data || jsonb_build_object('source', rs.source)) ORDER BY rs.route_id)
                        FROM static.route_stop rs
                        WHERE rs.stop_id = s.id
                          AND rs.source = s.source
                    ),
                    '[]'::jsonb
                ) AS routes
            FROM
                static.stop s
            WHERE
                s.source = $1 AND s.current
            ORDER BY s.id"#,
    )
    .bind(source)
    .fetch_all(connection)
    .await?
    .into_iter()
    // Decode may fail per row; collect propagates the first error.
    .map(|row| {
        Ok(Stop {
            id: row.id,
            name: row.name,
            geom: row.geom,
            nearby_stops: row.nearby_stops,
            data: crate::postgres::decode(row.source, row.data)?,
            routes: row.routes,
        })
    })
    .collect()
}

/// Persist stops inside the owning static import transaction.
pub(in crate::network) async fn save_all_on(
    connection: &mut PgConnection,
    source: Source,
    stops: &[Stop],
) -> anyhow::Result<()> {
    // TODO: probably pass vec instead of slice so we don't need to clone
    let ids: Vec<_> = stops.iter().map(|s| s.id.to_uppercase()).collect();
    let names: Vec<_> = stops.iter().map(|s| &s.name).collect();
    let geoms: Vec<_> = stops.iter().map(|r| r.geom.clone()).collect();
    let datas = stops
        .iter()
        .map(|s| crate::postgres::attributes(&s.data).unwrap())
        .collect::<Vec<_>>();
    // let route_types: Vec<_> = values.iter().map(|s| &s.route_type).collect();

    sqlx::query!(
        r#"
            INSERT INTO static.stop (id, source, name, geom, data)
            SELECT * FROM UNNEST(
                $1::TEXT[],
                $2::source_enum[],
                $3::TEXT[],
                $4::GEOMETRY[],
                $5::JSONB[]
            )
            ON CONFLICT (id, source) DO UPDATE SET
                current = TRUE, name = EXCLUDED.name,
                geom = EXCLUDED.geom,
                data = EXCLUDED.data
            "#,
        &ids,
        &vec![source; stops.len()],
        &names as &[&String],
        &geoms,
        &datas
    )
    .execute(connection)
    .await?;

    Ok(())
}

/// Persist associations inside the owning static import transaction.
pub(in crate::network) async fn save_all_route_stops_on(
    connection: &mut PgConnection,
    source: Source,
    route_stops: &[RouteStop],
) -> anyhow::Result<()> {
    let route_ids: Vec<_> = route_stops
        .iter()
        .map(|rs| rs.route_id.to_uppercase())
        .collect();
    let stop_ids: Vec<_> = route_stops
        .iter()
        .map(|rs| rs.stop_id.to_uppercase())
        .collect();
    let stop_sequences: Vec<i16> = route_stops.iter().map(|rs| rs.stop_sequence).collect();
    let datas: Vec<serde_json::Value> = route_stops
        .iter()
        .map(|rs| crate::postgres::attributes(&rs.data).unwrap())
        .collect();

    sqlx::query!(
        r#"
            INSERT INTO static.route_stop (route_id, source, stop_id, stop_sequence, data)
            SELECT * FROM UNNEST(
                $1::TEXT[],
                $2::source_enum[],
                $3::TEXT[],
                $4::SMALLINT[],
                $5::JSONB[]
            )
            ON CONFLICT (route_id, source, stop_id) DO UPDATE SET
                stop_sequence = EXCLUDED.stop_sequence,
                data = EXCLUDED.data
            "#,
        &route_ids,
        &vec![source; route_stops.len()],
        &stop_ids,
        &stop_sequences,
        &datas
    )
    .execute(connection)
    .await?;

    Ok(())
}

/// Recompute source-scoped nearby boarding candidates inside the import transaction.
/// Distance bounds candidate discovery; route/direction selection belongs to readers.
pub(in crate::network) async fn compute_nearby_stops(
    connection: &mut PgConnection,
    source: Source,
) -> anyhow::Result<()> {
    let start = std::time::Instant::now();
    // Drive the spatial join with the importing source. Eligibility is evaluated
    // independently for each target: an alighting-only origin is still useful.
    let candidates = sqlx::query_as::<_, (String, Source, String, Source, f64)>(
        r#"
        WITH pairs AS (
            SELECT a.id AS a_id, a.source AS a_source,
                   b.id AS b_id, b.source AS b_source,
                   ST_Distance(a.projected_geom, b.projected_geom) AS distance_m,
                   (a.source != 'mta_bus' OR a.data->>'is_boardable' = 'true')
                     AND EXISTS (
                       SELECT 1 FROM static.route_stop rs
                       JOIN static.route r ON r.id = rs.route_id AND r.source = rs.source AND r.current
                       WHERE rs.stop_id = a.id AND rs.source = a.source
                     ) AS a_boardable,
                   (b.source != 'mta_bus' OR b.data->>'is_boardable' = 'true')
                     AND EXISTS (
                       SELECT 1 FROM static.route_stop rs
                       JOIN static.route r ON r.id = rs.route_id AND r.source = rs.source AND r.current
                       WHERE rs.stop_id = b.id AND rs.source = b.source
                     ) AS b_boardable
            FROM static.stop a
            JOIN static.stop b ON b.current
              AND (a.id, a.source) != (b.id, b.source)
              AND ST_DWithin(a.projected_geom, b.projected_geom, 150.0)
            WHERE a.source = $1 AND a.current
        )
        SELECT a_id, a_source, b_id, b_source, distance_m FROM pairs WHERE b_boardable
        UNION ALL
        SELECT b_id, b_source, a_id, a_source, distance_m FROM pairs
        WHERE b_source != $1 AND a_boardable
        "#,
    ).bind(source).fetch_all(&mut *connection).await?;

    sqlx::query!(
        "DELETE FROM static.nearby_stop WHERE from_stop_source = $1 OR to_stop_source = $1",
        source
    )
    .execute(&mut *connection)
    .await?;

    for chunk in candidates.chunks(10_000) {
        let from_ids: Vec<_> = chunk.iter().map(|c| c.0.as_str()).collect();
        let from_sources: Vec<_> = chunk.iter().map(|c| c.1).collect();
        let to_ids: Vec<_> = chunk.iter().map(|c| c.2.as_str()).collect();
        let to_sources: Vec<_> = chunk.iter().map(|c| c.3).collect();
        let distances: Vec<_> = chunk.iter().map(|c| c.4).collect();
        sqlx::query!(
            r#"INSERT INTO static.nearby_stop
                (from_stop_id, from_stop_source, to_stop_id, to_stop_source, distance_m)
               SELECT * FROM UNNEST(
                 $1::text[], $2::source_enum[], $3::text[], $4::source_enum[], $5::float8[]
               )"#,
            &from_ids as &[&str],
            &from_sources,
            &to_ids as &[&str],
            &to_sources,
            &distances
        )
        .execute(&mut *connection)
        .await?;
    }
    tracing::info!(
        ?source,
        candidates = candidates.len(),
        elapsed_ms = start.elapsed().as_millis() as u64,
        "Recomputed nearby boarding stops"
    );
    Ok(())
}
