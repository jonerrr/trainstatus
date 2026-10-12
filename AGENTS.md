# Train Status - Agent Instructions

## Project Overview

Real-time tracker for MTA subway, MTA buses and NJT buses. Rust/Axum backend, SvelteKit frontend, PostgreSQL + PostGIS, Rust snapshots and Moka caches. Sources: `mta_subway` (Helium), `mta_bus` (GTFS-RT/OBA), `njt_bus` (GTFS-RT).

## Architecture

### Backend (`backend/src/`)

**Realtime data flow**: Feeds → `RealtimeSource` → `CollectedSnapshot` → `RealtimeIngestor` transaction → committed `Arc<PersistedSnapshot>` → shared `LiveSnapshots` watch channels → `ServiceStore` / `TrajectoryDeriver` → Axum API. Static imports are transactional; static responses, catalogs and current alerts are cached in process.

**Key modules**:

- `sources/` — `RealtimeSource` collectors plus `AlertsAdapter` and `StaticAdapter` implementations in `mta_subway/`, `mta_bus/` and `njt_bus/`
- `realtime/` — `RealtimeEngine` collects each source independently; `RealtimeIngestor` commits trips, stop times, positions and resolved shapes together before publishing. `LiveSnapshots` shares the latest committed Arc with `ServiceStore` and trajectory workers; watch channels coalesce pending generations. `TrajectoryDeriver` uses the pinned static revision without SQL readback
- `static_data/` — `StaticController`, collected datasets, transactional persistence and pinned static revisions; expanded NJT schedules persist in PostgreSQL and restore on restart
- `alerts/` — Alert polling and shared processing; `AlertStore` persists alerts and coordinates its 30-second local cache
- `passenger/` — Passenger read models. `CatalogStore` builds each source's static catalog (routes, stops, transfers, platform edges) with an ETag. `ServiceStore` builds route-keyed service (trips, vehicle status, columnar calls) once per committed generation, or from retained history for `?at=`
- `stores/` — Route, stop and alert persistence. Static response snapshots pair exact JSON bytes with their ETag; `TrajectoryStore` loads historical trajectory inputs behind `TrajectoryService`
- `api/` — Axum handlers; `AppState` holds all stores; OpenAPI docs at `/api/docs` (via utoipa/scalar)
- `integrations/` — External GTFS-RT/OBA protocol helpers and the Valhalla routing-library wrapper
- `trajectory/` — Live derivation, source builders, calculation, continuity and Arrow encoding; `TrajectoryService` owns live/historical reads and caches. Live snapshots never expire or evict
- `models/` — DB row types; geometry decoded via `geozero` from WKB
- `protos/` — GTFS-RT protobuf compiled in `build.rs` into `crate::feed`

**`StaticController`**: `RealtimeEngine` calls `controller.ensure_updated(source)` before collection. A foreign-key violation triggers one `force_update` and one ingestion retry using the same collected snapshot.

**Live and historical reads**: Live service reflects the last committed source snapshot and never touches SQL. Collection/transaction failures preserve it; a successful empty snapshot clears it; it is `503` until the first commit. Every requested route is present, empty when it has no trips; unknown bus routes are `404`. Explicit `?at=` service selects trips with any retained arrival or departure in the inclusive four-hour window, returns all their calls and no vehicle status; trajectories use their historical store path.

**API routes** (all under `/api/v1/`):

- Static: `GET /catalog/{source}` (the frontend's static source, ETag-validated)
- Realtime: `GET /service/{source}` (subway: whole source; buses: required `?route_ids=`), `GET /alerts/{source}`, `GET /trajectories/{source}`
- Realtime endpoints accept `?at=<unix_timestamp>` for historical queries

### Frontend (`frontend/src/`)

**Type generation**: TypeScript types are generated from the OpenAPI spec via `@hey-api/openapi-ts` and stored in `frontend/src/lib/client`. Run codegen with `pnpm openapi-ts` (in frontend directory) after backend changes. Frontend must be running to generate types.

**Data loading**: The layout loads only selected sources and `?at=`; it never waits for API data. `+layout.svelte` creates one `TransitData` (`src/lib/transit/store.svelte.ts`) and shares it with `setTransitData`; components call `getTransitData()`. Reads such as `catalog`, `route`, `trip`, `stopArrivals` and `alerts` return loaded data synchronously or a promise for `{const x = $derived(await ...)}` inside `<svelte:boundary>`; `loaded(read)` peeks without suspending. Readers subscribe through `createSubscriber`, so mounted readers share one entry and remounts within the 30-second refresh make no request. Subway service loads eagerly as one whole-source entry; bus routes load on demand and batch into shared `?route_ids=` requests. Historical (`at`) entries live in a small LRU. `decode.ts` turns columnar service into `RouteData` indexed by trip and stop.

**Freshness**: Live data older than 90 seconds since its last successful fetch is stale. Passed calls are dropped, future predictions are never hidden, and no remaining calls is "unavailable", never "no service". One global indicator reports failures and retries.

**Map**: `/map` polls its own trajectories and makes no passenger requests until an itinerary is opened.

**Offline**: The service worker caches the shell per deployment and catalogs in a stable `catalogs` cache: saved catalogs are served immediately and revalidated with `If-None-Match`, and replacements are posted to the open app (`replaceCatalog`). Service, alerts and trajectories are never persisted. Offline navigations fall back to the prerendered `/offline` shell, which boots the real route from saved catalogs.

**Component pattern**: Each entity type has `Button.svelte` + `Modal.svelte` in `src/lib/{Route,Stop,Trip}/`. Modal routing uses shallow URL params with the source: `?s=&src=`, `?r=&src=`, and `?t=&r=&src=` (trips carry their route). View transitions use `document.startViewTransition`.

**Reactive state files** use `.svelte.ts` (e.g., `url_params.svelte.ts`, `storage.svelte.ts`, `pins.svelte.ts`). Current time is scoped to each layout through `createCurrentTime`, `setCurrentTime`, and `getCurrentTime`; retrieve the context during component initialization.

## Development Setup

See README.md for detailed development setup instructions, including environment variable configuration and database requirements.

## Conventions

### Pull requests and commits

- Use a Conventional Commit subject for every PR title and commit: `<type>(<scope>): <imperative description>`. Keep the description concise and lowercase after the colon.
- Use the repository's scopes when applicable: `backend`, `frontend`, `database`, `api`, or `deps`. Omit the scope when no single scope fits.
- Use `feat` for user-visible functionality, `fix` for bug fixes, `perf` for performance improvements, and `refactor` for behavior-preserving restructuring. Use `docs`, `test`, `build`, `ci`, or `chore` for changes that do not affect the released runtime.
- Mark breaking changes with `!` before the colon, for example `feat(api)!: replace the trip response format`, and explain the migration in the commit body or `BREAKING CHANGE:` footer.
- Prefer squash merging. The PR title becomes the squash commit subject, so verify that the title is a valid Conventional Commit before merging. Release Please uses `feat` and `fix` commits to determine releases; non-conventional merge titles are ignored.
- Write commit bodies only when context is needed. Wrap paragraphs normally and include issue references or trailers in the footer.

### Backend

- When adding, changing, reviewing, or running backend tests, read [.agents/skills/backend-testing/SKILL.md](.agents/skills/backend-testing/SKILL.md) and follow the linked testing policy.

- Add new transit sources by implementing `RealtimeSource` + `AlertsAdapter` + `StaticAdapter` in `sources/<name>/`, then registering in `main.rs` and the per-source `LiveSnapshots` registry. Collectors return normalized entities; `RealtimeIngestor` owns persistence and committed publication
- Error handling: `AppError(anyhow::Error)` in `api/` converts to 500; use `?` freely

### Frontend

- For Svelte/SvelteKit questions or work under `frontend/`, read [frontend/AGENTS.md](frontend/AGENTS.md) for the Svelte MCP workflow before proceeding.
- When adding, changing, reviewing, or running frontend tests, read [.agents/skills/frontend-testing/SKILL.md](.agents/skills/frontend-testing/SKILL.md) and follow the linked testing policy.
- All API types come from autogenerated client in src/lib/client. — do not manually define API response types
- Narrow source-specific `data` fields (`CatalogRoute`, `CatalogStop`, `ServiceTrip`, `VehicleStatus`) on `data.source`
- Read API data through `TransitData`; it owns fetching, sharing, refresh and retry, so components hold no request state
- The layout's `current_time.value` (obtained with `getCurrentTime()` from `url_params.svelte.ts`) is the `at` argument for `TransitData` reads; `undefined` means live

## Database

- PostgreSQL with PostGIS; geometry as WKB decoded by `geozero`
- `source_enum` postgres enum maps to `Source` Rust enum (`mta_subway`, `mta_bus`, `njt_bus`)
