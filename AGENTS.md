# Train Status - Agent Instructions

## Project Overview

Real-time tracker for MTA subway, MTA buses and NJT buses. Rust/Axum backend, SvelteKit frontend, PostgreSQL + PostGIS, Rust snapshots and Moka caches. Sources: `mta_subway` (Helium), `mta_bus` (GTFS-RT/OBA), `njt_bus` (GTFS-RT).

## Architecture

### Backend (`backend/src/`)

**Realtime data flow**: Feeds → `RealtimeSource` → `CollectedSnapshot` → `RealtimeIngestor` transaction → committed `Arc<PersistedSnapshot>` → shared `LiveSnapshots` watch channels → live stores / `TrajectoryDeriver` → Axum API. Static imports are transactional; static responses and current alerts are cached in process.

**Key modules**:

- `sources/` — `RealtimeSource` collectors plus `AlertsAdapter` and `StaticAdapter` implementations in `mta_subway/`, `mta_bus/` and `njt_bus/`
- `realtime/` — `RealtimeEngine` collects each source independently; `RealtimeIngestor` commits trips, stop times, positions and resolved shapes together before publishing. `LiveSnapshots` shares the latest committed Arc with stores and trajectory workers; watch channels coalesce pending generations. `TrajectoryDeriver` uses the pinned static revision without SQL readback
- `static_data/` — `StaticController`, collected datasets, transactional persistence and pinned static revisions; expanded NJT schedules persist in PostgreSQL and restore on restart
- `alerts/` — Alert polling and shared processing; `AlertStore` persists alerts and coordinates its 30-second local cache
- `stores/` — Default trip, stop-time and position reads use `LiveSnapshots`; their explicit historical reads use PostgreSQL. Static response snapshots pair exact JSON bytes with their ETag; `TrajectoryStore` loads historical trajectory inputs behind `TrajectoryService`
- `api/` — Axum handlers; `AppState` holds all stores; OpenAPI docs at `/api/docs` (via utoipa/scalar)
- `integrations/` — External GTFS-RT/OBA protocol helpers and the Valhalla routing-library wrapper
- `trajectory/` — Live derivation, source builders, calculation, continuity and Arrow encoding; `TrajectoryService` owns live/historical reads and caches. Live snapshots never expire or evict
- `models/` — DB row types; geometry decoded via `geozero` from WKB
- `protos/` — GTFS-RT protobuf compiled in `build.rs` into `crate::feed`

**`StaticController`**: `RealtimeEngine` calls `controller.ensure_updated(source)` before collection. A foreign-key violation triggers one `force_update` and one ingestion retry using the same collected snapshot.

**Live and historical reads**: Default trips, stop times and positions reflect the last committed source snapshot, with route filters applied within its membership. Collection/transaction failures preserve it; a successful empty snapshot clears it; startup is empty until the first commit. These live reads use no retained-history fallback. Explicit `?at=` trips and stop times use retained final arrival-or-departure values in the inclusive four-hour window, without a trip update-time gate; positions and trajectories use their historical store paths.

**API routes** (all under `/api/v1/`):

- Static: `GET /routes/{source}`, `GET /stops/{source}`
- Realtime: `GET /trips/{source}`, `GET /stop_times/{source}`, `GET /positions/{source}`, `GET /alerts/{source}`
- All realtime endpoints accept `?at=<unix_timestamp>` for historical queries

### Frontend (`frontend/src/`)

**Type generation**: TypeScript types are generated from the OpenAPI spec via `@hey-api/openapi-ts` and stored in `frontend/src/lib/client`. Run codegen with `pnpm openapi-ts` (in frontend directory) after backend changes. Frontend must be running to generate types.

**Data loading pattern**:

1. `+layout.ts` SSR-fetches all sources in parallel and returns initial indexed data
2. `+layout.svelte` creates `LiveResource<T>` instances from initial data and sets Svelte contexts
3. Components consume typed context via `trip_context.getSource(source)`, `stop_time_context.getSource(source)`, etc.

**`LiveResource<T>`** (`src/lib/resources/index.svelte.ts`): Manages periodic polling + AbortController. Data stored as reactive `SvelteMap` keyed by entity ID.

**Source-discriminated types**: Entity `data` fields vary by source. Use `TypedTrip<S>`, `TypedVehiclePosition<S>`, `TypedStopTime<S>` generics. `source_info` config in `index.svelte.ts` defines per-source refresh intervals and UI metadata.

**StopTimes** are double-indexed: `by_trip_id` and `by_stop_id` (`StopTimeResource<S>`).

**Component pattern**: Each entity type has `Button.svelte` + `Modal.svelte` in `src/lib/{Route,Stop,Trip}/`. Modal routing uses shallow URL params (`?s=`, `?r=`, `?t=`). View transitions use `document.startViewTransition`.

**Global state files** use `.svelte.ts` extension (e.g., `util.svelte.ts` for `current_time`, `storage.svelte.ts`, `pins.svelte.ts`).

## Development Setup

See README.md for detailed development setup instructions, including environment variable configuration and database requirements.

## Conventions

### Backend

- When adding, changing, reviewing, or running backend tests, read [.agents/skills/backend-testing/SKILL.md](.agents/skills/backend-testing/SKILL.md) and follow the linked testing policy.

- Add new transit sources by implementing `RealtimeSource` + `AlertsAdapter` + `StaticAdapter` in `sources/<name>/`, then registering in `main.rs` and the per-source `LiveSnapshots` registry. Collectors return normalized entities; `RealtimeIngestor` owns persistence and committed publication
- Error handling: `AppError(anyhow::Error)` in `api/` converts to 500; use `?` freely

### Frontend

- For Svelte/SvelteKit questions or work under `frontend/`, read [frontend/AGENTS.md](frontend/AGENTS.md) for the Svelte MCP workflow before proceeding.
- When adding, changing, reviewing, or running frontend tests, read [.agents/skills/frontend-testing/SKILL.md](.agents/skills/frontend-testing/SKILL.md) and follow the linked testing policy.
- All API types come from autogenerated client in src/lib/client. — do not manually define API response types
- Use `TypedTrip<S>` / `TypedVehiclePosition<S>` etc. when source is known; use base `Trip` / `VehiclePosition` when source is unknown
- `current_time.value` (from `url_params.svelte.ts`) drives `?at=` queries; updating it auto-refreshes all `LiveResource` instances via `$effect`

## Database

- PostgreSQL with PostGIS; geometry as WKB decoded by `geozero`
- `source_enum` postgres enum maps to `Source` Rust enum (`mta_subway`, `mta_bus`, `njt_bus`)
