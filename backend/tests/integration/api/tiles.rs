use crate::support::geometry::ingest_case;
use backend::network::types::stop::{CompassDirection, StopData};
use backend::source::Source;
use prost::Message;

// Decode the fields used by this contract. Wire tags follow the MVT 2.1 schema:
// https://github.com/mapbox/vector-tile-spec/blob/master/2.1/vector_tile.proto
#[derive(Clone, PartialEq, Message)]
struct Tile {
    #[prost(message, repeated, tag = "3")]
    layers: Vec<Layer>,
}
#[derive(Clone, PartialEq, Message)]
struct Layer {
    #[prost(string, tag = "1")]
    name: String,
    #[prost(message, repeated, tag = "2")]
    features: Vec<Feature>,
    #[prost(string, repeated, tag = "3")]
    keys: Vec<String>,
    #[prost(message, repeated, tag = "4")]
    values: Vec<Value>,
}
#[derive(Clone, PartialEq, Message)]
struct Feature {
    #[prost(uint32, repeated, packed = "true", tag = "2")]
    tags: Vec<u32>,
    #[prost(uint32, tag = "3")]
    kind: u32,
    #[prost(uint32, repeated, packed = "true", tag = "4")]
    geometry: Vec<u32>,
}
#[derive(Clone, PartialEq, Message)]
struct Value {
    #[prost(string, optional, tag = "1")]
    text: Option<String>,
    #[prost(bool, optional, tag = "7")]
    boolean: Option<bool>,
}

async fn assert_tile(pool: sqlx::PgPool, source: Source) {
    let _stores = ingest_case(pool.clone(), source).await;
    if source == Source::MtaBus {
        sqlx::query(r#"UPDATE static.route SET data = jsonb_set(data, '{service_types}', '["SHUTTLE"]') WHERE source = $1"#)
            .bind(source).execute(&pool).await.unwrap();
    }
    // This function intentionally uses the database clock's five-minute window.
    sqlx::query("UPDATE realtime.trip SET updated_at = NOW() WHERE source = $1")
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();
    let (tile,): (Vec<u8>,) = sqlx::query_as("SELECT realtime.active_route_shapes(12,1206,1540)")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        !tile.is_empty(),
        "{source} active shapes reach the map tile"
    );
    let decoded = Tile::decode(tile.as_slice()).expect("valid MVT protobuf");
    assert_eq!(decoded.layers.len(), 1);
    let layer = &decoded.layers[0];
    assert_eq!(layer.name, "active_route_shapes");
    assert_eq!(layer.features.len(), 1);
    let feature = &layer.features[0];
    assert_eq!(feature.kind, 2, "line geometry");
    assert!(!feature.geometry.is_empty());
    let property = |key| {
        feature.tags.as_chunks::<2>().0.iter().find_map(|pair| {
            (layer.keys[pair[0] as usize] == key)
                .then(|| layer.values[pair[1] as usize].text.as_deref().unwrap())
        })
    };
    assert_eq!(property("source"), Some(source.as_str()));
    assert_eq!(
        property("route_id"),
        Some(match source {
            Source::MtaSubway => "A",
            Source::MtaBus => "B94",
            Source::NjtBus => "87",
        })
    );
    if source == Source::MtaBus {
        let shuttle = feature.tags.as_chunks::<2>().0.iter().find_map(|pair| {
            (layer.keys[pair[0] as usize] == "shuttle")
                .then_some(layer.values[pair[1] as usize].boolean)
                .flatten()
        });
        assert_eq!(
            shuttle,
            Some(true),
            "the shuttle filter needs a Boolean property"
        );
    }
    sqlx::query("UPDATE realtime.trip SET shape_ids = '{}' WHERE source = $1")
        .bind(source)
        .execute(&pool)
        .await
        .unwrap();
    let (empty,): (Vec<u8>,) = sqlx::query_as("SELECT realtime.active_route_shapes(12,1206,1540)")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        empty.is_empty(),
        "the tile must come from the ingested trip's shapes"
    );
}

#[sqlx::test]
async fn subway_connected_shapes_reach_tiles(pool: sqlx::PgPool) {
    assert_tile(pool, Source::MtaSubway).await;
}
#[sqlx::test]
async fn mta_bus_resolved_shape_reaches_tiles(pool: sqlx::PgPool) {
    assert_tile(pool, Source::MtaBus).await;
}
#[sqlx::test]
async fn njt_bus_resolved_shape_reaches_tiles(pool: sqlx::PgPool) {
    assert_tile(pool, Source::NjtBus).await;
}

#[sqlx::test]
async fn stop_tiles_exclude_retired_identities_and_keep_selection_fields(pool: sqlx::PgPool) {
    let stores = crate::support::test_owners(pool.clone());
    stores
        .network
        .persist(&crate::support::mta_subway_dataset())
        .await
        .unwrap();
    let retired: (String,) =
        sqlx::query_as("SELECT id FROM static.stop WHERE source = 'mta_subway' LIMIT 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query("UPDATE static.stop SET current = FALSE WHERE id = $1")
        .bind(&retired.0)
        .execute(&pool)
        .await
        .unwrap();
    let (bytes,): (Vec<u8>,) = sqlx::query_as("SELECT realtime.stop(0,0,0)")
        .fetch_one(&pool)
        .await
        .unwrap();
    let tile = Tile::decode(bytes.as_slice()).unwrap();
    let layer = &tile.layers[0];
    assert_eq!(layer.name, "stop");
    assert!(!layer.features.is_empty());
    for feature in &layer.features {
        let property = |key| {
            feature.tags.as_chunks::<2>().0.iter().find_map(|pair| {
                (layer.keys[pair[0] as usize] == key)
                    .then(|| layer.values[pair[1] as usize].text.as_deref().unwrap())
            })
        };
        assert_ne!(property("id"), Some(retired.0.as_str()));
        assert_eq!(property("source"), Some("mta_subway"));
        assert!(property("name").is_some());
        assert_eq!(feature.kind, 1);
    }
}

#[sqlx::test]
async fn stop_tiles_supply_supported_filter_properties(pool: sqlx::PgPool) {
    let stores = crate::support::test_owners(pool.clone());
    let mut subway = crate::support::mta_subway_dataset();
    for stop in &mut subway.stops {
        if let StopData::MtaSubway(data) = &mut stop.data {
            data.north_headsign = "Northbound".into();
            data.south_headsign = "Southbound".into();
        }
    }
    stores.network.persist(&subway).await.unwrap();
    let mut bus = crate::support::mta_bus_dataset();
    for stop in &mut bus.stops {
        if let StopData::MtaBus(data) = &mut stop.data {
            data.direction = CompassDirection::N;
        }
    }
    stores.network.persist(&bus).await.unwrap();
    let (bytes,): (Vec<u8>,) = sqlx::query_as("SELECT realtime.stop(0,0,0)")
        .fetch_one(&pool)
        .await
        .unwrap();
    let tile = Tile::decode(bytes.as_slice()).unwrap();
    let layer = &tile.layers[0];
    let mut sources = std::collections::HashSet::new();
    for feature in &layer.features {
        let property = |key| {
            feature.tags.as_chunks::<2>().0.iter().find_map(|pair| {
                (layer.keys[pair[0] as usize] == key)
                    .then_some(layer.values[pair[1] as usize].text.as_deref())
                    .flatten()
            })
        };
        match property("source").unwrap() {
            "mta_subway" => {
                sources.insert("mta_subway");
                assert_eq!(property("north_headsign"), Some("Northbound"));
                assert_eq!(property("south_headsign"), Some("Southbound"));
            }
            "mta_bus" => {
                sources.insert("mta_bus");
                assert_eq!(property("direction"), Some("n"));
            }
            source => panic!("unexpected source {source}"),
        }
    }
    assert_eq!(sources.len(), 2);
}
