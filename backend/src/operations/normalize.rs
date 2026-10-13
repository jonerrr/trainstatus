//! Shared provider identity rules. Missing or ambiguous identities are isolated.
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};

/// GTFS permits hours beyond 24:00; preserve the service-date day offset.
pub(crate) fn gtfs_start(date: NaiveDate, time: &str) -> Option<DateTime<Utc>> {
    let parts = time
        .split(':')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if parts.len() != 3 || parts[1] >= 60 || parts[2] >= 60 {
        return None;
    }
    let date = date.checked_add_signed(Duration::days(i64::from(parts[0] / 24)))?;
    let time = NaiveTime::from_hms_opt(parts[0] % 24, parts[1], parts[2])?;
    super::types::trip::TripRun::<uuid::Uuid>::created_at(date, time)
}

// NJT sometimes omits stop_sequence. Scheduled occurrence matching is required
// to retain visit identity; rejecting ambiguous repeated stops avoids overwrites.
/// Missing sequence can be recovered only from a unique scheduled occurrence.
/// A repeated stop cannot safely be identified by its position in a partial poll.
pub(crate) fn visit_sequence(
    provided: Option<u32>,
    stop: &str,
    scheduled: &[crate::network::schedule::ScheduledStopTime],
) -> Option<i32> {
    if let Some(sequence) = provided {
        return i32::try_from(sequence).ok();
    }
    let mut candidates = scheduled.iter().filter(|visit| visit.stop_id == stop);
    let candidate = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    Some(i32::from(candidate.stop_sequence))
}
