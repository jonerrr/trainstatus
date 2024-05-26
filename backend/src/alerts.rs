use std::time::Duration;

use sqlx::PgPool;
use tokio::time::sleep;

pub async fn import(pool: PgPool) {
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(10)).await;
        }
    });
}
