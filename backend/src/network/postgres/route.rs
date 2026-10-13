use super::rows::{RouteRow, ShapeRow};
use crate::network::types::route::Route;
use crate::network::types::shape::Shape;
use crate::source::Source;
use sqlx::PgConnection;

/// Raw DB query for all routes of a source.
pub(in crate::network) async fn query_all_on(
    source: Source,
    connection: &mut PgConnection,
) -> anyhow::Result<Vec<Route>> {
    Ok(sqlx::query_as!(
        RouteRow,
        r#"SELECT
                id,
                source,
                long_name,
                short_name,
                color,
                text_color,
                data
            FROM
                static.route
            WHERE
                source = $1 AND current
            ORDER BY short_name, id"#,
        source
    )
    .fetch_all(connection)
    .await?
    .into_iter()
    .map(TryInto::try_into)
    .collect::<anyhow::Result<Vec<Route>>>()?)
}

/// Persist routes inside the owning static import transaction.
pub(in crate::network) async fn save_all_on(
    connection: &mut PgConnection,
    source: Source,
    routes: &[Route],
) -> anyhow::Result<()> {
    // ensure all ids are uppercase for consistency (and frontend search)
    let ids: Vec<_> = routes.iter().map(|r| r.id.to_uppercase()).collect();
    let sources: Vec<_> = routes.iter().map(|_| source).collect();
    let long_names: Vec<_> = routes.iter().map(|r| &r.long_name).collect();
    let short_names: Vec<_> = routes.iter().map(|r| &r.short_name).collect();
    // Normalize colors to `#RRGGBB` here, the single write path for every
    // source, so the map and DOM never have to reconcile bare vs prefixed hex.
    let colors: Vec<_> = routes
        .iter()
        .map(|r| crate::utils::color::normalize_hex_color(&r.color))
        .collect();
    let text_colors: Vec<_> = routes
        .iter()
        .map(|r| crate::utils::color::normalize_hex_color(&r.text_color))
        .collect();
    let datas: Vec<_> = routes
        .iter()
        .map(|r| crate::persistence::attributes(&r.data).unwrap())
        .collect();

    sqlx::query!(
        r#"
            INSERT INTO static.route(id, source, long_name, short_name, color, text_color, data)
            SELECT
                u.id,
                u.source,
                u.long_name,
                u.short_name,
                u.color,
                u.text_color,
                u.data
            FROM unnest($1::text[], $2::source_enum[], $3::text[], $4::text[], $5::text[], $6::text[], $7::jsonb[]) AS u(id, source, long_name, short_name, color, text_color, data)
            ON CONFLICT(id, source)
            DO UPDATE SET
                current = TRUE,
                long_name = EXCLUDED.long_name,
                short_name = EXCLUDED.short_name,
                color = EXCLUDED.color,
                text_color = EXCLUDED.text_color,
                data = EXCLUDED.data
            "#,
        &ids,
        &sources,
        &long_names as &[&String],
        &short_names as &[&String],
        &colors,
        &text_colors,
        &datas
    )
    .execute(connection)
    .await?;

    Ok(())
}
pub(in crate::network) async fn save_all_shapes_on(
    connection: &mut PgConnection,
    source: Source,
    shapes: &[Shape],
) -> anyhow::Result<()> {
    let ids: Vec<_> = shapes.iter().map(|s| &s.id).collect();
    let geoms: Vec<_> = shapes.iter().map(|s| s.geom.clone()).collect();
    let datas: Vec<_> = shapes
        .iter()
        .map(|s| crate::persistence::attributes(&s.data).unwrap())
        .collect();

    let assets = shapes
        .iter()
        .map(|shape| shape.geom.content_id())
        .collect::<anyhow::Result<Vec<_>>>()?;
    sqlx::query!(
        r#"
            INSERT INTO static.shape_asset(source, id, geom)
            SELECT
                $1,
                id,
                ST_SetSRID(geom, 4326)
            FROM UNNEST($2::text[], $3::geometry[]) AS i(id, geom)
            ON CONFLICT(source, id) DO NOTHING
            "#,
        source,
        &assets,
        &geoms
    )
    .execute(&mut *connection)
    .await?;
    sqlx::query!(
        r#"
            INSERT INTO static.shape AS existing(id, source, asset_id, data)
            SELECT
                id,
                $1,
                asset,
                data
            FROM UNNEST($2::text[], $3::text[], $4::jsonb[]) AS i(id, asset, data)
            ON CONFLICT(id, source)
            DO UPDATE SET
                asset_id = EXCLUDED.asset_id,
                current = TRUE,
                data = EXCLUDED.data
            WHERE(existing.asset_id, existing.current, existing.data) IS DISTINCT FROM(EXCLUDED.asset_id, TRUE, EXCLUDED.data)
            "#,
        source,
        &ids as &[&String],
        &assets,
        &datas
    )
    .execute(&mut *connection)
    .await?;
    Ok(())
}

pub(in crate::network) async fn get_all_shapes_on(
    source: Source,
    connection: &mut PgConnection,
) -> anyhow::Result<Vec<Shape>> {
    Ok(sqlx::query_as!(
        ShapeRow,
        r#"
            SELECT
                s.id,
                s.source,
                a.geom,
                s.data
            FROM static.shape s JOIN static.shape_asset a ON(a.source, a.id) =(s.source, s.asset_id)
            WHERE s.source = $1
            AND s.current
            "#,
        source
    )
    .fetch_all(connection)
    .await?
    .into_iter()
    .map(Into::into)
    .collect::<Vec<Shape>>())
}
