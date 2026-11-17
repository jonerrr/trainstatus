use super::*;
use crate::operations::tests::support::TempStore;

fn instant(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(1_700_000_000 + seconds, 0).unwrap()
}

fn sample(seconds: i64) -> GpsSample {
    GpsSample {
        observed_at: instant(seconds),
        point: geo::Point::new(-74.0 + seconds as f64 / 1_000_000.0, 40.7),
    }
}

fn append(store: &GpsStore, series: &mut GpsSeries, start: i64, count: usize) {
    let run = RunId::arbitrary("indexed-gps");
    let current = HashMap::from([(run, Arc::new(series.clone()))]);
    let accepted: Vec<_> = (0..count)
        .map(|offset| (run, sample(start + offset as i64)))
        .collect();
    let seals = store
        .prepare_seals(&current, &HashMap::new(), &accepted)
        .unwrap();
    for (_, sample) in accepted {
        series.insert(sample);
    }
    for seal in seals {
        store.apply_seal(series, seal);
    }
}

#[test]
fn recent_anchor_does_not_read_an_unrelated_corrupt_history_subtree() {
    let context = TempStore::new();
    let store = GpsStore::from_blocks(context.blocks.clone());
    let mut series = GpsSeries::default();
    append(&store, &mut series, 0, PAGE_SAMPLES * NODE_ITEMS);
    let old_node = series.root.as_ref().unwrap().record.clone();
    append(&store, &mut series, 20_000, PAGE_SAMPLES);
    std::fs::remove_file(
        context
            .blocks
            .root()
            .join(format!("{}.trsb", old_node.block)),
    )
    .unwrap();
    // A fresh cache ensures this exercises the on-disk interval index.
    let reader = GpsStore::open(context.blocks.root()).unwrap();
    let at = instant(20_000 + PAGE_SAMPLES as i64 - 1);
    let anchor = reader.latest(&series, at).unwrap().unwrap();
    assert_eq!(anchor.observed_at, at);
    assert_eq!(anchor.point, sample(20_000 + PAGE_SAMPLES as i64 - 1).point);
    let before = reader
        .latest(&series, at - chrono::Duration::microseconds(1))
        .unwrap()
        .unwrap();
    assert_eq!(before.observed_at, at - chrono::Duration::seconds(1));
    assert!(reader.latest(&series, instant(10)).is_err());
}

#[test]
fn old_reader_index_nodes_survive_reclamation_until_the_reader_drops() {
    let context = TempStore::new();
    let store = GpsStore::from_blocks(context.blocks.clone());
    let mut current = GpsSeries::default();
    append(&store, &mut current, 0, PAGE_SAMPLES);
    let old = current.clone();
    let old_node = old.root.as_ref().unwrap().record.clone();
    append(&store, &mut current, PAGE_SAMPLES as i64, PAGE_SAMPLES);
    let mut live = HashSet::new();
    store.live_blocks(&current, &mut live).unwrap();
    store.live_pinned_blocks(&mut live).unwrap();
    context.blocks.collect_garbage(&live).unwrap();
    assert_eq!(
        store.all(&old).unwrap().last().unwrap().observed_at,
        instant(PAGE_SAMPLES as i64 - 1)
    );
    drop(old);
    let mut live = HashSet::new();
    store.live_blocks(&current, &mut live).unwrap();
    store.live_pinned_blocks(&mut live).unwrap();
    context.blocks.collect_garbage(&live).unwrap();
    assert!(
        !context
            .blocks
            .root()
            .join(format!("{}.trsb", old_node.block))
            .exists()
    );
    assert!(context.blocks.read::<Node>(&old_node).is_err());
    assert_eq!(
        store.all(&current).unwrap().last().unwrap().observed_at,
        instant((2 * PAGE_SAMPLES) as i64 - 1)
    );
}

#[test]
fn replay_after_uncertain_checkpoint_keeps_the_newer_unsealed_remainder() {
    let context = TempStore::new();
    let store = GpsStore::from_blocks(context.blocks.clone());
    let run = RunId::arbitrary("uncertain-checkpoint-tail");
    let mut runtime = GpsSeries::default();
    for offset in 1_000..1_100 {
        runtime.insert(sample(offset));
    }
    let mut checkpoint = runtime.clone();
    let flush = store
        .flush_tails(&HashMap::from([(run, Arc::new(checkpoint.clone()))]))
        .unwrap();
    for seal in flush {
        store.apply_seal(&mut checkpoint, seal);
    }
    let path = context.root.join("checkpoint.trst");
    crate::storage::codec::write_atomic(&path, &checkpoint).unwrap();
    // The manifest rename survived, but its error prevented activation in memory.
    let accepted: Vec<_> = (0..PAGE_SAMPLES)
        .map(|offset| (run, sample(offset as i64)))
        .collect();
    let seals = store
        .prepare_seals(
            &HashMap::from([(run, Arc::new(runtime))]),
            &HashMap::new(),
            &accepted,
        )
        .unwrap();
    let mut frame = Vec::new();
    ciborium::ser::into_writer(&seals, &mut frame).unwrap();
    let mut recovered: GpsSeries = crate::storage::codec::read(&path).unwrap();
    store.validate_descriptor(&recovered).unwrap();
    for (_, sample) in accepted {
        recovered.insert(sample);
    }
    for seal in ciborium::de::from_reader::<Vec<GpsSeal>, _>(frame.as_slice()).unwrap() {
        store.apply_seal(&mut recovered, seal);
    }
    let expected: Vec<_> = (0..PAGE_SAMPLES as i64)
        .chain(1_000..1_100)
        .map(instant)
        .collect();
    let actual = store.all(&recovered).unwrap();
    assert_eq!(
        actual
            .iter()
            .map(|sample| sample.observed_at)
            .collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        store
            .latest(&recovered, instant(1_099))
            .unwrap()
            .unwrap()
            .point,
        sample(1_099).point
    );
}
