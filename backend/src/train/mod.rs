// use sqlx::{PgPool, QueryBuilder};
pub mod static_data;
pub mod trips;

// TODO: make some insane proc macro for inserting into db
// pub async fn insert_all<T>(query: &str, values: Vec<T>, pool: &PgPool) -> Result<(), sqlx::Error> {
//     let mut query_builder = QueryBuilder::new(query);

//     query_builder.push_values(values, |mut b| {});

//     Ok(())
// }

// trait Import {
//     fn insert(values: Vec<Self>) -> Result<(), sqlx::Error>
//     where
//         Self: Sized;
// }
