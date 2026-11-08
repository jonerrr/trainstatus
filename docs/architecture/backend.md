# Backend architecture

Rust/Axum, PostgreSQL + PostGIS, in-memory snapshots and Moka caches. Sources: `mta_subway` (Helium), `mta_bus` (GTFS-RT/OBA), `njt_bus` (GTFS-RT). Testing policy lives in [backend/TESTING.md](../../backend/TESTING.md).

## Data flow

Feeds → `RealtimeSource` → `CollectedSnapshot` (pinned `NetworkRevision`) → `OperationsIngestor` transaction → committed `Arc<SourceSnapshot>` → shared `LiveSnapshots` watch channels → `Operations` / `TrajectoryDeriver` → Axum API.

## Modules (`backend/src/`)

| Module          | Owns                                                                                                                                                              |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `app.rs`        | Construction, owner handles, background worker startup/shutdown. `main.rs` handles HTTP and signals.                                                              |
| `network/`      | Static imports, schedules, transfers, restore, atomic revision/catalog publication. A `NetworkRevision` pairs routing inputs with exact catalog bytes and ETag.   |
| `operations/`   | Source-scoped run keys, run/visit/observation reconciliation, GPS sample writes, committed live membership, passenger indexing, retained-history materialization. |
| `alerts/`       | Source-typed alert aggregates, transactional replacement, current membership, declared-period historical reads, local cache.                                      |
| `trajectory/`   | Source builders, geometry, PCHIP, continuity, Arrow rendering. See [trajectories](trajectories.md).                                                               |
| `sources/`      | `RealtimeSource`, `AlertsAdapter`, `StaticAdapter` per provider.                                                                                                  |
| `api/`          | Request validation, handlers, borrowed domain-to-DTO encoding, OpenAPI. `AppState` holds owner read handles and a readiness check.                                |
| `integrations/` | GTFS-RT/OBA protocol helpers, Valhalla routing.                                                                                                                   |
| `protos/`       | GTFS-RT protobuf compiled in `build.rs` into `crate::feed`; provider enums are translated at collection.                                                          |

`network`, `operations`, `alerts` and `trajectory` own their domain, persistence and reads. Domain aggregates have no SQL row or HTTP behavior. Serializable leaf payloads/enums are shared with API DTOs and carry their OpenAPI schema. SQL rows stay private to their owner.

## API (`/api/v1/`)

- `GET /catalog/{source}`: static catalog, ETag-validated.
- `GET /service/{source}`: subway is whole-source; buses require `?route_ids=`.
- `GET /alerts/{source}`, `GET /trajectories/{source}`.
- Realtime endpoints accept `?at=<unix_timestamp>` for historical reads.

## Network

- Imports serialize transfer work, persist membership/descriptions/schedules and prepare every affected catalog in one transaction. Commit precedes an atomic in-memory revision replacement. Readers pin one revision. Failures keep both SQL and the previous publication.
- `RealtimeEngine` calls `NetworkController::ensure_updated(source)` before collection. A foreign-key failure triggers one forced import and one retry of the same decoded provider batch against the new revision.
- Empty routes/stops are rejected as incomplete imports; an empty schedule set is allowed. Static route/stop identities remain for retained references.
- Geometry is content-addressed per source. Per-run rows store ordered asset references, not geometry. A reused provider shape ID does not rewrite an existing run's accepted asset. Subway leg IDs accompany assets so historical topology keeps station junctions.
- NJT service-date schedules keep their original bounded expiration across rollover and restore. Provider patterns and child-stop remaps stay pinned during enrichment.

## Operations and reads

- Run identity is source, provider run ID, service-date/start timestamp and direction. Vehicle assignment is mutable. Collectors emit stable correlation keys; operations assigns persistent UUIDs. Poll time never supplies identity. Missing NJT dates are recovered only from a unique pinned schedule, otherwise the run is isolated.
- Visits use a nonnegative `INTEGER` sequence. Missing bus sequences are recovered only from a unique scheduled occurrence. Conflicting repeated sequences reject a collection.
- Omission removes live membership, but retained calls remain. Each source's commits serialize; there is one ingestion writer per source/process. Separate backend instances are not coordinated.
- Live reads use the last committed `Arc` and never touch SQL. Failures preserve it; a successful empty collection clears it. Startup returns `503` until the first commit. Requested routes are present even when empty; unknown bus routes return `404`.
- Live `updated_at` is the source commit time, so refetching an old generation does not renew freshness.
- `at` selects the latest retained runs with any arrival or departure in inclusive `[at, at + 4h]`, then loads all their visits in one repeatable-read transaction. Service and trajectories share this selection. Historical output has no vehicle status or freshness timestamp. It is latest-record materialization, not prediction replay. Trajectory construction rejects a stop more than 500 m from the retained path, and invalid inputs are isolated while calls stay readable.
- Observations use microsecond precision and are first-wins for equal times; older ones are rejected. Accepted linked coordinates insert `(run_id, observed_at, longitude, latitude)` in the same transaction. Identical polls add no row versions or samples. Historical anchors use each run's latest sample at or before `at` within five minutes.
- Schema and tests verify there are no stored snapshot, API JSON, Arrow or prediction-version tables.
- Age-based retention is deferred.

## Alerts

A successful collection replaces current membership and each alert's owned translations, periods and affected entities transactionally. Empty polls clear; failures preserve. `AlertCollection` is a discriminated enum (MTA variants carry MTA attributes plus `AlertDetails`; NJT carries details only), so source and payload are derived together. The DTO prefers HTML over plain text, English over other languages, and orders route-bearing entities by priority.

## Database

PostgreSQL with PostGIS. Geometry is WKB decoded by `geozero`. `source_enum` maps to the `Source` Rust enum. Use SQLx compile-time macros where SQL is describable; the metadata workflow is in [backend/TESTING.md](../../backend/TESTING.md). `backend/queries.sql` has schema inspection queries.
