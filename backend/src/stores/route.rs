use super::response::{ResponseCache, StaticResponse};
use crate::models::{route::Route, shape::Shape, source::Source};
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;

#[derive(Clone)]
pub struct RouteStore {
    pg_pool: PgPool,
    pub(crate) responses: Arc<ResponseCache<Route>>,
}

impl RouteStore {
    pub fn new(pg_pool: PgPool) -> Self {
        Self {
            pg_pool,
            responses: Arc::default(),
        }
    }

    pub async fn refresh(&self, source: Source) -> anyhow::Result<()> {
        let _guard = self.responses.refresh_locks[&source].lock().await;
        let data = Self::query_all_on(source, &mut *self.pg_pool.acquire().await?).await?;
        self.responses
            .values
            .replace(source, StaticResponse::new(data)?);
        Ok(())
    }

    pub async fn response(&self, source: Source) -> anyhow::Result<Arc<StaticResponse<Route>>> {
        if let Some(response) = self.responses.values.get(source) {
            return Ok(response);
        }
        let _guard = self.responses.refresh_locks[&source].lock().await;
        if let Some(response) = self.responses.values.get(source) {
            return Ok(response);
        }
        let data = Self::query_all_on(source, &mut *self.pg_pool.acquire().await?).await?;
        Ok(self
            .responses
            .values
            .replace(source, StaticResponse::new(data)?))
    }

    /// Raw DB query for all routes of a source.
    pub(crate) async fn query_all_on(
        source: Source,
        connection: &mut PgConnection,
    ) -> anyhow::Result<Vec<Route>> {
        Ok(sqlx::query_as::<_, Route>(
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
                source = $1
            ORDER BY short_name, id"#,
        )
        .bind(source)
        .fetch_all(connection)
        .await?)
    }

    pub async fn get_all(&self, source: Source) -> anyhow::Result<Vec<Route>> {
        Ok(self.response(source).await?.data.clone())
    }

    /// Persist routes inside the owning static import transaction.
    pub(crate) async fn save_all_on(
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
            .map(|r| serde_json::to_value(&r.data).unwrap())
            .collect();

        sqlx::query!(
            r#"
            INSERT INTO static.route (id, source, long_name, short_name, color, text_color, data)
            SELECT
                u.id,
                u.source,
                u.long_name,
                u.short_name,
                u.color,
                u.text_color,
                u.data
            FROM
                unnest(
                    $1::text[],
                    $2::source_enum[],
                    $3::text[],
                    $4::text[],
                    $5::text[],
                    $6::text[],
                    $7::jsonb[]
                ) AS u(id, source, long_name, short_name, color, text_color, data)
            ON CONFLICT (id, source) DO UPDATE SET
                long_name = EXCLUDED.long_name,
                short_name = EXCLUDED.short_name,
                color = EXCLUDED.color,
                text_color = EXCLUDED.text_color,
                data = EXCLUDED.data
            "#,
            &ids as _,
            &sources,
            &long_names as _,
            &short_names as _,
            &colors as _,
            &text_colors as _,
            &datas as _,
        )
        .execute(connection)
        .await?;

        Ok(())
    }

    pub(crate) async fn save_all_shapes_on(
        connection: &mut PgConnection,
        source: Source,
        shapes: &[Shape],
    ) -> anyhow::Result<()> {
        let ids: Vec<_> = shapes.iter().map(|s| &s.id).collect();
        let sources: Vec<_> = vec![source; shapes.len()];
        let geoms: Vec<_> = shapes.iter().map(|s| s.geom.clone()).collect();
        let datas: Vec<_> = shapes
            .iter()
            .map(|s| serde_json::to_value(&s.data).unwrap())
            .collect();

        sqlx::query!(
            r#"
            INSERT INTO static.shape (id, source, geom, data)
            SELECT id, source, ST_SetSRID(geom, 4326), data FROM UNNEST($1::text[], $2::source_enum[], $3::geometry[], $4::jsonb[])
                AS t(id, source, geom, data)
            ON CONFLICT (id, source) DO UPDATE SET
                geom = EXCLUDED.geom,
                data = EXCLUDED.data
            "#,
            &ids as _,
            &sources,
            &geoms,
            &datas as _,
        )
        .execute(connection)
        .await?;
        Ok(())
    }

    pub(crate) async fn get_all_shapes_on(
        source: Source,
        connection: &mut PgConnection,
    ) -> anyhow::Result<Vec<Shape>> {
        Ok(sqlx::query_as::<_, Shape>(
            "SELECT id, source, geom, data FROM static.shape WHERE source = $1",
        )
        .bind(source)
        .fetch_all(connection)
        .await?)
    }
}
