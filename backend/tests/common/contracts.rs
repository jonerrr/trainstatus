use backend::sources::StaticAdapter;
use backend::models::source::Source;

// This function panics (fails the test) if the baseline contract is violated
pub async fn assert_standard_static_import<A: StaticAdapter>(
    adapter: &A,
    route_store: &RouteStore,
    stop_store: &StopStore,
    // ... other stores
) {
    // 1. Run the import
    adapter.import(route_store, stop_store, ...).await.expect("Import failed");

    let source = adapter.source();
    let routes = route_store.get_all(source).await.unwrap();
    let stops = stop_store.get_all(source).await.unwrap();

    // 2. Standard Assertions
    assert!(!routes.is_empty(), "[{:?}] Must import at least one route", source);
    assert!(!stops.is_empty(), "[{:?}] Must import at least one stop", source);

    // 3. Relational Integrity Checks
    for route in routes {
        // Assert every route has an ID, color, etc.
        assert!(!route.id.is_empty());
    }
}