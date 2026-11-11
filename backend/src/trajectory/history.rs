use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::{
    cache::TrajectoryCache,
    snapshot::{GeometryInputs, snapshot_from_geometry},
    types::TripSnapshot,
};
use crate::operations::history::History;
use crate::source::Source;

/// Load retained inputs for the same snapshot builder and trajectory calculator
/// used by live derivation. History supplies GPS anchors instead of current status.
pub async fn load_historical_inputs(
    history: &History,
    cache: &TrajectoryCache,
    source: Source,
    at: DateTime<Utc>,
) -> anyhow::Result<Vec<TripSnapshot>> {
    let retained = history.load_trajectories(source, at).await?;
    let Some(network) = retained.network else {
        return Ok(vec![]);
    };
    let mut visits: HashMap<_, Vec<_>> = HashMap::new();
    for visit in retained.service.stop_times {
        visits.entry(visit.trip_id).or_default().push(visit);
    }
    let mut anchors = retained.anchors;
    let mut inputs = Vec::new();
    for trip in retained.service.trips {
        let Some(color) = network.routes.get(&trip.route_id) else {
            continue;
        };
        let geometry = GeometryInputs {
            source,
            historical: true,
            route_color: color,
            stops: &network.stops,
            shapes: &network.shapes,
        };
        match snapshot_from_geometry(
            &trip,
            visits.remove(&trip.id).unwrap_or_default(),
            vec![],
            &geometry,
            cache,
            at,
        )
        .await
        {
            Ok(mut input) => {
                input.recorded_anchor = anchors.remove(&trip.id);
                inputs.push(input);
            }
            Err(error) => {
                tracing::debug!(
                    {
                        trainstatus.source = source.as_str(),
                        trainstatus.trip.id = %trip.id,
                        error.message = %format_args!("{error:#}"),
                    },
                    "Skipping invalid historical trajectory input"
                );
            }
        }
    }
    Ok(inputs)
}
