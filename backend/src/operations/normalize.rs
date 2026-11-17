//! Shared provider identity rules. Missing or ambiguous identities are isolated.
use chrono::{DateTime, NaiveDate, Utc};

/// GTFS permits hours beyond 24:00; preserve the service-date day offset.
pub fn gtfs_start(date: NaiveDate, time: &str) -> Option<DateTime<Utc>> {
    let mut parts = time.split(':');
    let hours = parts.next()?.parse::<u32>().ok()?;
    let minutes = parts.next()?.parse::<u32>().ok()?;
    let seconds = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() || minutes >= 60 || seconds >= 60 {
        return None;
    }
    let elapsed = hours
        .checked_mul(3600)?
        .checked_add(minutes.checked_mul(60)?)?
        .checked_add(seconds)?;
    crate::network::expansion::gtfs_datetime(date, elapsed)
}

// NJT sometimes omits stop_sequence. Scheduled occurrence matching is required
// to retain visit identity; rejecting ambiguous repeated stops avoids overwrites.
/// Missing sequence can be recovered only from a unique scheduled occurrence.
/// A repeated stop cannot safely be identified by its position in a partial poll.
pub fn visit_sequence(
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
