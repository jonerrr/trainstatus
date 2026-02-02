use crate::engines::static_data::StaticController;
use crate::sources::RealtimeAdapter;
use crate::stores::position::PositionStore;
use crate::stores::trip::TripStore;
use std::sync::Arc;
use tokio::time::sleep;
use tracing::error;

pub async fn run(
    // pool: &PgPool,
    trip_store: &TripStore,
    // stop_time_store: &StopTimeStore,
    position_store: &PositionStore,
    adapters: Vec<Arc<dyn RealtimeAdapter>>,
    static_controller: StaticController,
) {
    for adapter in adapters {
        // let pool = pool.clone();
        let controller = static_controller.clone();
        let trip_store = trip_store.clone();
        // let stop_time_store = stop_time_store.clone();
        let position_store = position_store.clone();

        tokio::spawn(async move {
            loop {
                // The adapter implementation (e.g. MtaSubwayAdapter) handles the
                // concrete call to the generic GTFS pipeline internally.
                if let Err(e) = adapter.run(&controller, &trip_store, &position_store).await {
                    error!("Realtime pipeline error for {:?}: {}", adapter.source(), e);
                }

                sleep(adapter.refresh_interval()).await;
            }
        });
    }
}
