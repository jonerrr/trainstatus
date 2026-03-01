use crate::engines::static_data::StaticController;
use crate::sources::RealtimeAdapter;
use crate::stores::position::PositionStore;
use crate::stores::stop_time::StopTimeStore;
use crate::stores::trip::TripStore;
use std::sync::Arc;
use tokio::time::sleep;
use tracing::error;

pub async fn run(
    // pool: &PgPool,
    trip_store: &TripStore,
    stop_time_store: &StopTimeStore,
    position_store: &PositionStore,
    adapters: Vec<Arc<dyn RealtimeAdapter>>,
    static_controller: StaticController,
) {
    for adapter in adapters {
        let controller = static_controller.clone();
        let trip_store = trip_store.clone();
        let stop_time_store = stop_time_store.clone();
        let position_store = position_store.clone();

        tokio::spawn(async move {
            loop {
                let source = adapter.source();
                if let Err(e) = adapter.run(&controller, &trip_store, &position_store).await {
                    error!("Realtime pipeline error for {:?}: {}", source, e);
                }
                // TODO: use stop time store to save stop times (instead of trip store)
                // then we don't have to add this separate cache population step just for stop times
                // Populate stop times cache after trips are committed
                if let Err(e) = stop_time_store.populate_cache(source).await {
                    error!("Stop time cache populate error for {:?}: {}", source, e);
                }

                sleep(adapter.refresh_interval()).await;
            }
        });
    }
}
