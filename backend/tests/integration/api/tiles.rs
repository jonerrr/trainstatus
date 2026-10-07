use crate::support::geometry::ingest_case;
use backend::models::source::Source;
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
}

async fn assert_tile(pool: sqlx::PgPool, source: Source) {
    let _stores = ingest_case(pool.clone(), source).await;
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
        feature.tags.chunks_exact(2).find_map(|pair| {
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
