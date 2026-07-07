use std::collections::{BTreeSet, HashMap, HashSet};

use geo::{CoordsIter, Geometry};
use tracing::{debug, info, warn};

use crate::models::{
    route::RouteData, source::Source, static_dataset::StaticDataset, stop::RouteStop,
};

#[derive(Debug, Clone)]
pub struct ImportReport {
    pub source: Source,
    pub route_count: usize,
    pub stop_count: usize,
    pub route_stop_count: usize,
    pub shape_count: usize,
    pub duplicate_route_ids: Vec<String>,
    pub duplicate_stop_ids: Vec<String>,
    pub duplicate_shape_ids: Vec<String>,
    pub missing_route_references: Vec<String>,
    pub missing_stop_references: Vec<String>,
    pub orphaned_stops: Vec<String>,
    pub invalid_geometries: Vec<String>,
    pub missing_shape_references: Vec<String>,
}

impl Default for ImportReport {
    fn default() -> Self {
        Self {
            source: Source::MtaSubway,
            route_count: 0,
            stop_count: 0,
            route_stop_count: 0,
            shape_count: 0,
            duplicate_route_ids: Vec::new(),
            duplicate_stop_ids: Vec::new(),
            duplicate_shape_ids: Vec::new(),
            missing_route_references: Vec::new(),
            missing_stop_references: Vec::new(),
            orphaned_stops: Vec::new(),
            invalid_geometries: Vec::new(),
            missing_shape_references: Vec::new(),
        }
    }
}

impl ImportReport {
    pub fn generate(dataset: &StaticDataset) -> anyhow::Result<Self> {
        let mut report = Self {
            source: dataset.source,
            route_count: dataset.routes.len(),
            stop_count: dataset.stops.len(),
            route_stop_count: dataset.route_stops.len(),
            shape_count: dataset.shapes.len(),
            ..Default::default()
        };

        if dataset.routes.is_empty() {
            anyhow::bail!("static import for {} produced no routes", dataset.source);
        }
        if dataset.stops.is_empty() {
            anyhow::bail!("static import for {} produced no stops", dataset.source);
        }

        report.duplicate_route_ids = duplicate_normalized_ids(
            dataset.routes.iter().map(|route| route.id.as_str()),
            "routes",
        );
        report.duplicate_stop_ids =
            duplicate_normalized_ids(dataset.stops.iter().map(|stop| stop.id.as_str()), "stops");
        report.duplicate_shape_ids = duplicate_normalized_ids(
            dataset.shapes.iter().map(|shape| shape.id.as_str()),
            "shapes",
        );
        // TODO: should we be uppercasing here? we shouldnt be doing normalization here
        let route_ids = dataset
            .routes
            .iter()
            .map(|route| route.id.to_uppercase())
            .collect::<HashSet<_>>();
        let stop_ids = dataset
            .stops
            .iter()
            .map(|stop| stop.id.to_uppercase())
            .collect::<HashSet<_>>();
        let shape_ids = dataset
            .shapes
            .iter()
            .map(|shape| shape.id.to_uppercase())
            .collect::<HashSet<_>>();

        report.missing_route_references =
            missing_route_references(&dataset.route_stops, &route_ids);
        report.missing_stop_references = missing_stop_references(&dataset.route_stops, &stop_ids);
        report.orphaned_stops = orphaned_stops(&stop_ids, &dataset.route_stops);
        report.invalid_geometries = invalid_geometries(dataset);
        report.missing_shape_references = missing_shape_references(dataset, &shape_ids);

        let mut errors = Vec::new();
        if !report.duplicate_route_ids.is_empty() {
            errors.push(format!(
                "duplicate route ids after normalization: {:?}",
                report.duplicate_route_ids
            ));
        }
        if !report.duplicate_stop_ids.is_empty() {
            errors.push(format!(
                "duplicate stop ids after normalization: {:?}",
                report.duplicate_stop_ids
            ));
        }
        if !report.duplicate_shape_ids.is_empty() {
            errors.push(format!(
                "duplicate shape ids after normalization: {:?}",
                report.duplicate_shape_ids
            ));
        }
        if !report.missing_route_references.is_empty() {
            errors.push(format!(
                "route_stops reference missing routes: {:?}",
                report.missing_route_references
            ));
        }
        if !report.missing_stop_references.is_empty() {
            errors.push(format!(
                "route_stops reference missing stops: {:?}",
                report.missing_stop_references
            ));
        }
        if !report.invalid_geometries.is_empty() {
            errors.push(format!(
                "invalid geometries: {:?}",
                report.invalid_geometries
            ));
        }
        if !errors.is_empty() {
            anyhow::bail!(
                "static import validation failed for {}: {}",
                dataset.source,
                errors.join("; ")
            );
        }

        Ok(report)
    }

    // TODO: use tracing::instrument with source field
    pub fn log_summary(&self) {
        info!(
            source = %self.source,
            routes = self.route_count,
            stops = self.stop_count,
            route_stops = self.route_stop_count,
            shapes = self.shape_count,
            "Validated static import"
        );

        if !self.orphaned_stops.is_empty() {
            debug!(
                source = %self.source,
                orphaned_stop_count = self.orphaned_stops.len(),
                "Static import contains orphaned stops"
            );
        }

        if !self.missing_shape_references.is_empty() {
            warn!(
                source = %self.source,
                missing_shape_reference_count = self.missing_shape_references.len(),
                "Static import contains route shape references without matching shape rows"
            );
        }
    }
}

fn duplicate_normalized_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    label: &'static str,
) -> Vec<String> {
    let mut counts = HashMap::<String, usize>::new();
    for id in ids {
        *counts.entry(id.to_uppercase()).or_default() += 1;
    }

    let duplicates = counts
        .into_iter()
        .filter_map(|(id, count)| (count > 1).then_some(id))
        .collect::<Vec<_>>();

    if !duplicates.is_empty() {
        warn!(
            label,
            duplicate_count = duplicates.len(),
            "Duplicate static ids"
        );
    }

    duplicates
}

fn missing_route_references(route_stops: &[RouteStop], route_ids: &HashSet<String>) -> Vec<String> {
    route_stops
        .iter()
        .filter_map(|rs| {
            let route_id = rs.route_id.to_uppercase();
            (!route_ids.contains(&route_id)).then_some(route_id)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn missing_stop_references(route_stops: &[RouteStop], stop_ids: &HashSet<String>) -> Vec<String> {
    route_stops
        .iter()
        .filter_map(|rs| {
            let stop_id = rs.stop_id.to_uppercase();
            (!stop_ids.contains(&stop_id)).then_some(stop_id)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn orphaned_stops(stop_ids: &HashSet<String>, route_stops: &[RouteStop]) -> Vec<String> {
    let referenced = route_stops
        .iter()
        .map(|rs| rs.stop_id.to_uppercase())
        .collect::<HashSet<_>>();

    stop_ids
        .difference(&referenced)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn invalid_geometries(dataset: &StaticDataset) -> Vec<String> {
    let mut invalid = Vec::new();

    for stop in &dataset.stops {
        if !is_valid_geometry(&stop.geom.0) {
            invalid.push(format!("stop:{}", stop.id));
        }
    }

    for shape in &dataset.shapes {
        if !is_valid_geometry(&shape.geom.0) {
            invalid.push(format!("shape:{}", shape.id));
        }
    }

    invalid
}

fn is_valid_geometry(geometry: &Geometry<f64>) -> bool {
    match geometry {
        Geometry::Point(point) => point.x().is_finite() && point.y().is_finite(),
        Geometry::LineString(line) => {
            line.coords_count() >= 2
                && line
                    .coords_iter()
                    .all(|coord| coord.x.is_finite() && coord.y.is_finite())
        }
        Geometry::MultiLineString(lines) => {
            !lines.0.is_empty()
                && lines
                    .0
                    .iter()
                    .all(|line| is_valid_geometry(&line.clone().into()))
        }
        other => other
            .coords_iter()
            .all(|coord| coord.x.is_finite() && coord.y.is_finite()),
    }
}

fn missing_shape_references(dataset: &StaticDataset, shape_ids: &HashSet<String>) -> Vec<String> {
    dataset
        .routes
        .iter()
        .flat_map(|route| match &route.data {
            RouteData::MtaBus(data) => data
                .shape_ids
                .iter()
                .filter_map(|shape_id| {
                    let normalized = shape_id.to_uppercase();
                    (!shape_ids.contains(&normalized))
                        .then_some(format!("route:{} shape:{}", route.id, shape_id))
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        })
        .collect()
}
