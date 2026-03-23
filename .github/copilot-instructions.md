# Train Status - Copilot Instructions

## Project Overview

Real-time MTA subway and bus tracker. Rust/Axum backend, SvelteKit frontend, PostgreSQL + PostGIS, Valkey/Redis cache. Sources: `mta_subway` (GTFS-RT), `mta_bus` (OBA/SIRI).

## Architecture

### Backend (`backend/src/`)

**Data flow**: MTA feeds → `sources/` adapters → PostgreSQL → Redis read-through cache → `stores/` → Axum API

**Key modules**:

- `sources/` — Trait-based adapters (`RealtimeAdapter`, `AlertsAdapter`, `StaticAdapter`) with implementations in `mta_subway/` and `mta_bus/`
- `engines/` — Spawns background tokio tasks: `static_data` (manages static import lifecycle via `StaticController`), `realtime`, `alerts`
- `stores/` — Typed store structs (`TripStore`, `RouteStore`, etc.) wrapping PgPool + Redis; use `stores::read_through()` for cache-aside reads
- `api/` — Axum handlers; `AppState` holds all stores; OpenAPI docs at `/api/docs` (via utoipa/scalar)
- `integrations/` — Shared GTFS-RT/OBA parsing helpers
- `models/` — DB row types; geometry decoded via `geozero` from WKB
- `protos/` — GTFS-RT protobuf compiled in `build.rs` into `crate::feed`

**`StaticController`**: Realtime engines call `controller.ensure_updated(source)` before processing to avoid FK errors. `force_update` re-imports if a FK violation occurs.

**API routes** (all under `/api/v1/`):

- Static: `GET /routes/{source}`, `GET /stops/{source}`
- Realtime: `GET /trips/{source}`, `GET /stop_times/{source}`, `GET /positions/{source}`, `GET /alerts/{source}`
- All realtime endpoints accept `?at=<unix_timestamp>` for historical queries

### Frontend (`frontend/src/`)

**Type generation**: `client/` package generates TypeScript types from the OpenAPI spec via `@hey-api/openapi-ts`. Run codegen with `pnpm --filter @trainstatus/client openapi-ts` after backend changes.

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

```bash
# Start PostgreSQL + Valkey
cd backend && podman compose up -d

# Backend (requires env vars in backend/.env.toml)
cd backend && cargo run

# Frontend
cd frontend && pnpm install && pnpm dev

# Regenerate API client types (after backend OpenAPI changes)
pnpm --filter @trainstatus/client openapi-ts
```

**Backend env vars** (`backend/.env.toml`):

- `DATABASE_URL`, `REDIS_URL` — required
- `MTA_OBA_API_KEY` — MTA BusTime API key (required)
- `DEBUG_RT_DATA` — saves raw feed protobufs to `backend/gtfs/`

## Conventions

### Backend

- Always use `sqlx::query_as::<_, ModelType>(...)` (not the `query!` macro) — models use custom `FromRow` impls for PostGIS geometry via `geozero`
- Migrations in `backend/migrations/` with timestamp prefixes
- Add new transit sources by implementing `RealtimeAdapter` + `AlertsAdapter` + `StaticAdapter` traits in a new `sources/<name>/` module, then registering in `main.rs`
- Error handling: `AppError(anyhow::Error)` in `api/` converts to 500; use `?` freely

### Frontend

- All API types come from `@trainstatus/client` — do not manually define API response types
- Use `TypedTrip<S>` / `TypedVehiclePosition<S>` etc. when source is known; use base `Trip` / `VehiclePosition` when source is unknown
- `current_time.value` (from `url_params.svelte.ts`) drives `?at=` queries; updating it auto-refreshes all `LiveResource` instances via `$effect`

## Database

- PostgreSQL with PostGIS; geometry as WKB decoded by `geozero`
- `source_enum` postgres enum maps to `Source` Rust enum (`mta_subway`, `mta_bus`)
