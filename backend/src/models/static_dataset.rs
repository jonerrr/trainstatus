use crate::{
    models::{
        route::Route, shape::Shape, source::Source, static_cache::CachedTrip, stop::RouteStop,
        stop::Stop,
    },
    stores::{route::RouteStore, static_cache::StaticCacheStore, stop::StopStore},
    utils::validation::ImportReport,
};

pub struct StaticDataset {
    pub source: Source,
    pub routes: Vec<Route>,
    pub stops: Vec<Stop>,
    pub route_stops: Vec<RouteStop>,
    pub shapes: Vec<Shape>,
    pub cached_trips: Vec<CachedTrip>,
}
// TODO: use the standardized log_summary method instead of having a separate trace in each source
impl StaticDataset {
    pub fn new(source: Source) -> Self {
        Self {
            source,
            routes: Vec::new(),
            stops: Vec::new(),
            route_stops: Vec::new(),
            shapes: Vec::new(),
            cached_trips: Vec::new(),
        }
    }

    pub fn validate(&self) -> anyhow::Result<ImportReport> {
        ImportReport::generate(self)
    }

    pub async fn persist(
        &self,
        route_store: &RouteStore,
        stop_store: &StopStore,
        static_cache_store: &StaticCacheStore,
    ) -> anyhow::Result<()> {
        let report = self.validate()?;
        report.log_summary();

        route_store.save_all(self.source, &self.routes).await?;
        stop_store.save_all(self.source, &self.stops).await?;

        if !self.route_stops.is_empty() {
            stop_store
                .save_all_route_stops(self.source, &self.route_stops)
                .await?;
        }

        if !self.shapes.is_empty() {
            route_store
                .save_all_shapes(self.source, &self.shapes)
                .await?;
        }

        if !self.cached_trips.is_empty() {
            static_cache_store
                .cache_trips(self.source, &self.cached_trips)
                .await?;
        }

        Ok(())
    }
}
