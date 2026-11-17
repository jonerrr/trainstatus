use crate::operations::identity::RunId;
use crate::operations::owner::select_poll_run_winner;
use crate::operations::source::CollectedVisit;
use crate::operations::types::trip::{NjtBusData, StopTimeData, TripData, TripRun};
use crate::source::Source;
use chrono::{NaiveDate, TimeZone, Utc};

fn trip(
    id: RunId,
    updated_at: chrono::DateTime<Utc>,
    vehicle: &str,
    partition: NaiveDate,
) -> (TripRun, Vec<CollectedVisit>) {
    (
        TripRun {
            id,
            original_id: "dup".into(),
            vehicle_id: vehicle.into(),
            route_id: "1".into(),
            shape_ids: vec![],
            direction: 0,
            created_at: updated_at,
            updated_at,
            partition_date: partition,
            data: TripData::NjtBus(NjtBusData {
                deviation: None,
                headsign: "A".into(),
            }),
        },
        vec![CollectedVisit {
            trip_id: id,
            stop_id: "STOP1".into(),
            stop_sequence: 1,
            arrival: updated_at,
            departure: updated_at,
            data: StopTimeData::NjtBus,
        }],
    )
}

#[test]
fn duplicate_run_id_picks_latest_updated_at_first_wins_on_tie() {
    let id = RunId::scheduled_bus(Source::NjtBus, "dup", Utc::now().date_naive());
    let day = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
    let older = Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap();
    let newer = older + chrono::Duration::minutes(5);

    let winner = select_poll_run_winner(vec![
        trip(id, older, "first-on-tie", day),
        trip(id, older, "second-on-tie", day),
        trip(id, newer, "newer", day),
    ])
    .unwrap();
    assert_eq!(winner.0.vehicle_id, "newer");

    let winner = select_poll_run_winner(vec![
        trip(id, older, "first-on-tie", day),
        trip(id, older, "second-on-tie", day),
    ])
    .unwrap();
    assert_eq!(winner.0.vehicle_id, "first-on-tie");
}
