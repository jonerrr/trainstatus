use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub struct StopTime {
    pub trip_id: Uuid,
    pub stop_id: i32,
    pub arrival: DateTime<Utc>,
    pub departure: DateTime<Utc>,
}

impl StopTime {
    pub async fn insert(values: Vec<Self>, pool: &PgPool) -> Result<(), sqlx::Error> {
        let trip_ids = values.iter().map(|v| v.trip_id).collect::<Vec<_>>();
        let stop_ids = values.iter().map(|v| v.stop_id).collect::<Vec<_>>();
        let arrivals = values.iter().map(|v| v.arrival).collect::<Vec<_>>();
        let departures = values.iter().map(|v| v.departure).collect::<Vec<_>>();

        sqlx::query!(
            r#"
            INSERT INTO stop_time (trip_id, stop_id, arrival, departure)
            SELECT * FROM UNNEST($1::uuid[], $2::int[], $3::timestamptz[], $4::timestamptz[])
            ON CONFLICT (trip_id, stop_id) DO UPDATE SET arrival = EXCLUDED.arrival, departure = EXCLUDED.departure
            "#,
            &trip_ids,
            &stop_ids,
            &arrivals,
            &departures
        ).execute(pool).await?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum IntoStopTimeError {
    StopId,
    Arrival,
    Departure,
    FakeStop,
    // StopSequence,
}
