use super::*;
use crate::network::dataset::NetworkDataset;
use crate::operations::types::position::{
    NjtBusPositionData, OccupancyStatus, PositionData, VehicleObservation,
};
use crate::storage::codec::read;

#[test]
fn blocked_source_resumes_only_after_checkpoint_storage_is_repaired() {
    let root =
        std::env::temp_dir().join(format!("trainstatus-maintenance-{}", uuid::Uuid::new_v4()));
    let owner = OperationsOwner::open(Storage::open(&root).unwrap(), NetworkIndex::new()).unwrap();
    let source = Source::NjtBus;
    let revision = Arc::new(NetworkRevision::from_dataset(&NetworkDataset::new(source)));
    let observed_at = Utc::now().trunc_subsecs(6);
    let poll = |occupancy_status| CollectedSnapshot {
        source,
        network_revision: Some(revision.clone()),
        renormalize: None,
        trips: vec![],
        positions: vec![VehicleObservation {
            vehicle_id: "bus".into(),
            trip_id: None,
            stop_id: None,
            updated_at: observed_at,
            geom: None,
            data: PositionData::NjtBus(NjtBusPositionData { occupancy_status }),
        }],
    };
    owner.commit_blocking(poll(OccupancyStatus::Empty)).unwrap();
    let (obstruction, before) = {
        let mut writer = owner.sources[source.index()].lock().unwrap();
        let meta: checkpoint::SourceCheckpointMeta =
            read(&writer.checkpoint_dir.join(checkpoint::MANIFEST_NAME)).unwrap();
        let obstruction = writer
            .checkpoint_dir
            .join("generations")
            .join((meta.generation + 1).to_string());
        std::fs::write(&obstruction, b"obstruct next checkpoint directory").unwrap();
        writer.maintenance_blocked = true;
        (obstruction, writer.journal.next_record_id())
    };
    let failure = owner
        .commit_blocking(poll(OccupancyStatus::Full))
        .err()
        .expect("blocked admission must fail while checkpoint storage is unavailable");
    assert!(
        failure
            .downcast_ref::<OperationsMaintenancePending>()
            .is_some()
    );
    {
        let writer = owner.sources[source.index()].lock().unwrap();
        assert_eq!(writer.journal.next_record_id(), before);
        assert_eq!(
            writer
                .active
                .positions()
                .into_iter()
                .find(|p| p.vehicle_id == "bus")
                .unwrap()
                .data,
            PositionData::NjtBus(NjtBusPositionData {
                occupancy_status: OccupancyStatus::Empty
            })
        );
    }
    std::fs::remove_file(obstruction).unwrap();
    let accepted = owner
        .commit_blocking(poll(OccupancyStatus::Full))
        .unwrap()
        .0;
    assert_eq!(
        accepted.positions[0].data,
        PositionData::NjtBus(NjtBusPositionData {
            occupancy_status: OccupancyStatus::Full
        })
    );
    drop(owner);
    let reopened =
        OperationsOwner::open(Storage::open(&root).unwrap(), NetworkIndex::new()).unwrap();
    assert_eq!(
        reopened.sources[source.index()]
            .lock()
            .unwrap()
            .active
            .positions()
            .into_iter()
            .find(|p| p.vehicle_id == "bus")
            .unwrap()
            .data,
        PositionData::NjtBus(NjtBusPositionData {
            occupancy_status: OccupancyStatus::Full
        })
    );
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}
