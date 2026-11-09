use crate::{source::Source, utils::source_snapshot::SourceSnapshot};
use chrono::{DateTime, Utc};
use opentelemetry::{
    KeyValue,
    metrics::{Counter, Histogram, Meter},
};
use std::sync::{Arc, OnceLock};

static METRICS: OnceLock<Metrics> = OnceLock::new();
pub(crate) fn metrics() -> Option<&'static Metrics> {
    METRICS.get()
}
pub(super) fn initialize(meter: Meter) {
    let _ = METRICS.set(Metrics::new(meter));
}

const HTTP: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1., 2.5, 5., 7.5, 10.,
];
const OPERATION: &[f64] = &[
    0.01, 0.05, 0.1, 0.25, 0.5, 1., 2.5, 5., 10., 30., 60., 120., 300., 600.,
];
const OBSERVATION: &[f64] = &[1., 5., 15., 30., 60., 120., 300., 600., 1800.];
const GAP: &[f64] = &[1., 5., 10., 25., 50., 100., 250., 500., 1000.];

pub(crate) struct AlertSummary {
    pub count: usize,
    pub committed_at: DateTime<Utc>,
}
pub(crate) struct TrajectorySummary {
    pub plans: usize,
    pub render_units: usize,
    pub committed_at: Option<DateTime<Utc>>,
}

pub(crate) struct Metrics {
    meter: Meter,
    pub operation: Histogram<f64>,
    changed: Counter<u64>,
    pub static_recovery: Counter<u64>,
    observation_age: Histogram<f64>,
    pub trajectory_inputs: Counter<u64>,
    pub continuity_discards: Counter<u64>,
    pub knots_removed: Counter<u64>,
    pub anchor_gap: Histogram<f64>,
    pub server_duration: Histogram<f64>,
    pub client_duration: Histogram<f64>,
    source_attrs: [[KeyValue; 1]; 3],
    alerts: Arc<SourceSnapshot<AlertSummary>>,
    trajectories: Arc<SourceSnapshot<TrajectorySummary>>,
    // The SDK owns observable callbacks; only their registration needs a guard.
    state: OnceLock<()>,
}
fn histogram(
    meter: &Meter,
    name: &'static str,
    unit: &'static str,
    boundaries: &[f64],
) -> Histogram<f64> {
    meter
        .f64_histogram(name)
        .with_unit(unit)
        .with_boundaries(boundaries.to_vec())
        .build()
}
fn source_attrs() -> [[KeyValue; 1]; 3] {
    Source::ALL.map(|source| [KeyValue::new("trainstatus.source", source.as_str())])
}
fn age(now: DateTime<Utc>, committed: DateTime<Utc>) -> f64 {
    (now - committed)
        .num_microseconds()
        .unwrap_or(i64::MAX)
        .max(0) as f64
        / 1_000_000.
}
impl Metrics {
    pub(super) fn new(meter: Meter) -> Self {
        let alerts = Arc::new(SourceSnapshot::<AlertSummary>::new());
        let trajectories = Arc::new(SourceSnapshot::<TrajectorySummary>::new());
        let alert_store = alerts.clone();
        let attrs = source_attrs();
        meter
            .u64_observable_gauge("trainstatus.alerts.entities")
            .with_unit("{alert}")
            .with_callback(move |observer| {
                for source in Source::ALL {
                    if let Some(summary) = alert_store.get(source) {
                        observer.observe(summary.count as u64, &attrs[source.index()]);
                    }
                }
            })
            .build();
        let alert_store = alerts.clone();
        let attrs = source_attrs();
        meter
            .f64_observable_gauge("trainstatus.alerts.snapshot.age")
            .with_unit("s")
            .with_callback(move |observer| {
                let now = Utc::now();
                for source in Source::ALL {
                    if let Some(summary) = alert_store.get(source) {
                        observer.observe(age(now, summary.committed_at), &attrs[source.index()]);
                    }
                }
            })
            .build();
        let trajectory_store = trajectories.clone();
        let attrs = source_attrs();
        meter
            .u64_observable_gauge("trainstatus.trajectory.entities")
            .with_unit("{entity}")
            .with_callback(move |observer| {
                for source in Source::ALL {
                    if let Some(summary) = trajectory_store.get(source) {
                        for (entity, count) in [
                            ("plan", summary.plans),
                            ("render_unit", summary.render_units),
                        ] {
                            observer.observe(
                                count as u64,
                                &[
                                    attrs[source.index()][0].clone(),
                                    KeyValue::new("trainstatus.entity", entity),
                                ],
                            );
                        }
                    }
                }
            })
            .build();
        let trajectory_store = trajectories.clone();
        let attrs = source_attrs();
        meter
            .f64_observable_gauge("trainstatus.trajectory.snapshot.age")
            .with_unit("s")
            .with_callback(move |observer| {
                let now = Utc::now();
                for source in Source::ALL {
                    if let Some(summary) = trajectory_store.get(source)
                        && let Some(committed) = summary.committed_at
                    {
                        observer.observe(age(now, committed), &attrs[source.index()]);
                    }
                }
            })
            .build();
        Self {
            operation: histogram(&meter, "trainstatus.operation.duration", "s", OPERATION),
            changed: meter
                .u64_counter("trainstatus.realtime.changed")
                .with_unit("{entity}")
                .build(),
            static_recovery: meter
                .u64_counter("trainstatus.realtime.static_recovery")
                .with_unit("{recovery}")
                .build(),
            observation_age: histogram(
                &meter,
                "trainstatus.live.vehicle_observation.age",
                "s",
                OBSERVATION,
            ),
            trajectory_inputs: meter
                .u64_counter("trainstatus.trajectory.inputs")
                .with_unit("{trip}")
                .build(),
            continuity_discards: meter
                .u64_counter("trainstatus.trajectory.continuity.discards")
                .with_unit("{discard}")
                .build(),
            knots_removed: meter
                .u64_counter("trainstatus.trajectory.knots.removed")
                .with_unit("{knot}")
                .build(),
            anchor_gap: histogram(&meter, "trainstatus.trajectory.live_anchor.gap", "m", GAP),
            server_duration: histogram(&meter, "http.server.request.duration", "s", HTTP),
            client_duration: histogram(&meter, "http.client.request.duration", "s", HTTP),
            meter,
            source_attrs: source_attrs(),
            alerts,
            trajectories,
            state: OnceLock::new(),
        }
    }
    pub(crate) fn attrs(&self, source: Source) -> &[KeyValue; 1] {
        &self.source_attrs[source.index()]
    }
    pub(crate) fn committed(&self, snapshot: &crate::operations::SourceSnapshot) {
        let source = self.source_attrs[snapshot.source.index()][0].clone();
        let changes = snapshot.changes;
        for (entity, count) in [
            ("trip", changes.trips),
            ("stop_time", changes.stop_times),
            ("vehicle_observation", changes.positions),
            ("resolved_shape", changes.resolved_shapes),
        ] {
            self.changed.add(
                count as u64,
                &[source.clone(), KeyValue::new("trainstatus.entity", entity)],
            );
        }
        for observation in &snapshot.positions {
            self.observation_age.record(
                age(snapshot.committed_at, observation.updated_at),
                self.attrs(snapshot.source),
            );
        }
    }
    pub(crate) fn alerts_published(&self, source: Source, count: usize) {
        self.alerts.replace(
            source,
            AlertSummary {
                count,
                committed_at: Utc::now(),
            },
        );
    }
    pub(crate) fn trajectory_published(&self, source: Source, summary: TrajectorySummary) {
        self.trajectories.replace(source, summary);
    }
    pub(crate) fn continuity(
        &self,
        source: Source,
        stats: &crate::trajectory::continuity::ContinuityStatsBatch,
    ) {
        let attr = self.attrs(source)[0].clone();
        for (reason, count) in [
            ("stale_prev_state", stats.stale_prev_state_discarded),
            ("backward_gap", stats.backward_gap_discarded),
            ("forward_jump", stats.forward_jump_discarded),
        ] {
            self.continuity_discards.add(
                count as u64,
                &[attr.clone(), KeyValue::new("trainstatus.reason", reason)],
            );
        }
        for (reason, count) in [
            ("pre_anchor", stats.pre_anchor_knots_dropped),
            ("backtracking", stats.backtracking_knots_removed),
        ] {
            self.knots_removed.add(
                count,
                &[attr.clone(), KeyValue::new("trainstatus.reason", reason)],
            );
        }
    }
    fn register_state(
        &self,
        pool: sqlx::PgPool,
        live: crate::operations::LiveSnapshots,
        network: crate::network::index::NetworkIndex,
    ) {
        self.state.get_or_init(|| {
            let attrs = source_attrs();
            let ready_live = live.clone();
            self.meter
                .u64_observable_gauge("trainstatus.live.ready")
                .with_unit("1")
                .with_callback(move |observer| {
                    for source in Source::ALL {
                        observer.observe(
                            u64::from(ready_live.get(source).is_some()),
                            &attrs[source.index()],
                        );
                    }
                })
                .build();
            let attrs = source_attrs();
            let count_live = live.clone();
            self.meter
                .u64_observable_gauge("trainstatus.live.entities")
                .with_unit("{entity}")
                .with_callback(move |observer| {
                    for source in Source::ALL {
                        if let Some(snapshot) = count_live.get(source) {
                            let located = snapshot
                                .positions
                                .iter()
                                .filter(|p| p.geom.is_some())
                                .count();
                            let linked = snapshot
                                .positions
                                .iter()
                                .filter(|p| p.trip_id.is_some())
                                .count();
                            for (entity, count) in [
                                ("trip", snapshot.trips.len()),
                                ("stop_time", snapshot.stop_times.len()),
                                ("vehicle_observation", snapshot.positions.len()),
                                ("located_vehicle", located),
                                ("linked_vehicle", linked),
                            ] {
                                observer.observe(
                                    count as u64,
                                    &[
                                        attrs[source.index()][0].clone(),
                                        KeyValue::new("trainstatus.entity", entity),
                                    ],
                                );
                            }
                        }
                    }
                })
                .build();
            let attrs = source_attrs();
            self.meter
                .f64_observable_gauge("trainstatus.live.snapshot.age")
                .with_unit("s")
                .with_callback(move |observer| {
                    let now = Utc::now();
                    for source in Source::ALL {
                        if let Some(snapshot) = live.get(source) {
                            observer
                                .observe(age(now, snapshot.committed_at), &attrs[source.index()]);
                        }
                    }
                })
                .build();
            let attrs = source_attrs();
            self.meter
                .u64_observable_gauge("trainstatus.static.entities")
                .with_unit("{entity}")
                .with_callback(move |observer| {
                    for source in Source::ALL {
                        if let Some(revision) = network.get(source) {
                            for (entity, count) in [
                                ("route", revision.routes.len()),
                                ("stop", revision.stops.len()),
                                ("shape", revision.shapes.len()),
                            ] {
                                observer.observe(
                                    count as u64,
                                    &[
                                        attrs[source.index()][0].clone(),
                                        KeyValue::new("trainstatus.entity", entity),
                                    ],
                                );
                            }
                        }
                    }
                })
                .build();
            let pool_attrs = [
                KeyValue::new("db.system.name", "postgresql"),
                KeyValue::new("db.client.connection.pool.name", "main"),
            ];
            self.meter
                .i64_observable_up_down_counter("db.client.connection.count")
                .with_unit("{connection}")
                .with_callback(move |observer| {
                    let size = pool.size() as usize;
                    let idle = pool.num_idle();
                    for (state, count) in [("idle", idle), ("used", size.saturating_sub(idle))] {
                        observer.observe(
                            count as i64,
                            &[
                                pool_attrs[0].clone(),
                                pool_attrs[1].clone(),
                                KeyValue::new("db.client.connection.state", state),
                            ],
                        );
                    }
                })
                .build();
        });
    }
}
pub fn register_state(
    pool: sqlx::PgPool,
    live: crate::operations::LiveSnapshots,
    network: crate::network::index::NetworkIndex,
) {
    if let Some(metrics) = metrics() {
        metrics.register_state(pool, live, network);
    }
}
