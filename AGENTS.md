# Train Status - Agent Instructions

## Project Overview

Real-time tracker for MTA subway, MTA buses and NJT buses. Rust/Axum backend, SvelteKit frontend, PostgreSQL + PostGIS, Rust snapshots and Moka caches. Sources: `mta_subway` (Helium), `mta_bus` (GTFS-RT/OBA), `njt_bus` (GTFS-RT).

## Architecture

### Backend (`backend/src/`)

**Realtime data flow**: Feeds → `RealtimeSource` → `CollectedSnapshot` with a pinned `NetworkRevision` → `OperationsIngestor` transaction → committed `Arc<SourceSnapshot>` → shared `LiveSnapshots` watch channels → `Operations` / `TrajectoryDeriver` → Axum API.

**Key modules**:

- `app.rs` — Application construction, owner handles and background worker startup/shutdown; `main.rs` handles HTTP and signals
- `network/` — Transactional static imports, schedules, transfers, restore and atomic revision/catalog publication. `NetworkRevision` pairs routing inputs with exact catalog bytes and their ETag
- `operations/` — Stable source-scoped run keys, latest run/visit/observation reconciliation, explicit GPS sample writes, committed live membership, passenger indexing and shared retained-history materialization
- `alerts/` — Source-typed alert aggregates, transactional replacement of owned collections, current membership, declared-period historical reads and the local cache
- `trajectory/` — Source builders, geometry, PCHIP, continuity and Arrow rendering. Live derivation consumes the committed Arc; historical rendering consumes operations' retained inputs and recorded GPS anchors
- `sources/` — `RealtimeSource`, `AlertsAdapter` and `StaticAdapter` implementations for each provider
- `api/` — Request validation, HTTP handlers, borrowed domain-to-DTO encoding and OpenAPI. `AppState` holds owner read handles and a readiness check; private SQL rows live behind owners
- `integrations/` — External GTFS-RT/OBA protocol helpers and Valhalla routing
- `protos/` — GTFS-RT protobuf compiled in `build.rs` into `crate::feed`; provider enums are translated at collection

**Network refresh**: `RealtimeEngine` calls `NetworkController::ensure_updated(source)` before collection. A foreign-key failure triggers one forced import and one retry; network-dependent normalization retries the same decoded provider batch against the new pinned revision.

**Reads**: Live service uses the last committed source snapshot without SQL. Failures preserve it; a successful empty collection clears membership; startup returns `503` until the first commit. Requested routes are present even when empty; unknown bus routes return `404`. Service and trajectories share the historical loader's inclusive four-hour arrival-or-departure selection, with all retained visits and no historical vehicle status.

**API routes** (all under `/api/v1/`):

- Static: `GET /catalog/{source}` (the frontend's static source, ETag-validated)
- Realtime: `GET /service/{source}` (subway: whole source; buses: required `?route_ids=`), `GET /alerts/{source}`, `GET /trajectories/{source}`
- Realtime endpoints accept `?at=<unix_timestamp>` for historical queries

### Frontend (`frontend/src/`)

**Type generation**: TypeScript types are generated from the OpenAPI spec via `@hey-api/openapi-ts` and stored in `frontend/src/lib/client`. After backend contract changes, export with `mise exec -- cargo run --bin openapi` from `backend/`, then run `mise exec -- pnpm openapi-ts --input <exported-json>` from `frontend/`. Export requires no running feeds, frontend or database.

**Data loading**: The layout loads selected sources and `?at=` without waiting for API data. `+layout.svelte` creates one `TransitData` (`src/lib/transit/store.svelte.ts`) and shares it with `setTransitData`; components call `getTransitData()`. Create `catalog`, `route`, `trip`, `stopArrivals`, or `alerts` bindings during component initialization, supplying getters for reactive source/ID/time keys. Read `.current` synchronously inside `$derived` for `{ value, loading, error }`; fetch failures are outcomes, while boundaries handle rendering errors. Each binding's explicit effect owns reader interest and idempotent request acquisition. Native `$state.raw` snapshots carry loaded data; reads never subscribe, create effects, or schedule requests. `peekCatalog`, `loadedTrip`, and `loadedStopArrivals` observe snapshots without acquiring requests. Subway service loads eagerly outside `/map`; bus routes load on demand and batch into shared `?route_ids=` requests. Remounts within the 30-second refresh reuse data. Historical (`at`) entries live in a small LRU protected by active bindings. `decode.ts` indexes columnar service by trip and stop.

**Freshness**: Live data older than 90 seconds since its last successful fetch or source commit triggers the shared warning. Passed calls are dropped and future predictions remain visible. Empty detail views say “No predictions to show”; list rows omit prediction labels when none remain. One “Can’t update data” indicator provides Retry, moving into the dialog while a modal is open. Refresh failures retain loaded values; successful historical data does not age into a live warning. Persisted preferences use cached `LocalStorage.current` snapshots: replace the value to mutate and persist, and use the layout-owned listener for cross-tab updates.

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

- Add new transit sources by implementing `RealtimeSource` + `AlertsAdapter` + `StaticAdapter` in `sources/<name>/`, then registering in `app.rs`, `Source::ALL` and the per-source `LiveSnapshots` registry. Collectors return source-scoped run keys and normalized owner types; `OperationsIngestor` resolves persistent IDs and owns committed publication
- Error handling: `AppError(anyhow::Error)` in `api/` converts to 500; use `?` freely
- Use SQLx compile-time query macros for describable SQL; complex JSON/spatial projections and utility commands may stay runtime queries. When changing checked SQL or schema, follow the metadata workflow in [backend/TESTING.md](backend/TESTING.md).

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

- Automatic historical-age retention remains deferred.

## Maintainability Rules

- Prefer the simplest, cleanest implementation that satisfies the requirements. Avoid unnecessary complexity, indirection, and overengineering.
- Search for existing implementations and conventions before introducing new helpers, types, dependencies, or abstractions.
- Favor maintainability and architectural consistency over minimizing the size of a diff. Larger refactors are acceptable when they meaningfully improve the codebase.
- **Do not preserve backward compatibility unless explicitly required.** Prefer clean breaking changes over compatibility layers, deprecated interfaces, fallback logic, or workarounds.
- Treat development databases and their data as disposable. Prefer updating schemas and resetting the development database over implementing complex migrations or compatibility logic solely to preserve development data. Ask before performing destructive operations.
- When replacing functionality, fully remove obsolete implementations, unused code, and references. Do not leave behind legacy paths or redundant abstractions.
- Avoid speculative future-proofing, unnecessary defensive programming, and abstractions that serve no concrete purpose.
- Follow existing architectural conventions where reasonable, but do not perpetuate poor patterns merely for consistency.
- Do not add functionality, dependencies, or architectural complexity beyond what is requested without discussing it first.
- **Stop and ask for clarification whenever you encounter a meaningful decision, ambiguity, unexpected issue, or trade-off that the user should decide.** Present the available options and their implications rather than silently making assumptions or implementing workarounds.
- Proactively identify opportunities for simplification, refactoring, or architectural improvement. Surface significant findings and recommendations, even when they fall outside the immediate task, but ask before expanding the scope.
- Make routine, low-risk implementation decisions independently. Do not interrupt for trivial details that have an obvious solution.
- Keep comments and documentation accurate and useful. Avoid comments that merely restate what the code does.
