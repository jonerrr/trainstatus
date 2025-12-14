# Train Status - Copilot Instructions

## Project Overview

Real-time MTA subway and bus tracker with a Rust backend (Axum) and SvelteKit frontend. Consumes MTA GTFS-RT and SIRI feeds, stores data in PostgreSQL with PostGIS, caches in Valkey/Redis.

## Architecture

### Backend (`backend/`)

- **Framework**: Axum with utoipa for OpenAPI docs (served at `/api/docs`)
- **Data flow**: GTFS-RT/SIRI feeds → parse → PostgreSQL → Redis cache → API
- **Key modules**:
  - `static_data/` - Routes, stops, shapes (refreshes every 3 days from MTA GTFS)
  - `realtime/` - Trips, alerts, stop times (updates every ~30 seconds)
  - `api/` - HTTP handlers with Redis caching for static data
- **Protobuf**: GTFS-RT protos compiled via `build.rs` into `feed` module

### Frontend (`frontend/`)

- **Framework**: SvelteKit with Svelte 5 runes, TailwindCSS 4
- **Pattern**: Trips, routes, and stops each have `Button.svelte` and `Modal.svelte` components in `src/lib/{Route,Stop,Trip}/`
- **State**: Global reactive stores using Svelte 5's `$state` runes in `*.svelte.ts` files (e.g., `trips.svelte.ts`, `alerts.svelte.ts`)
- **Types**: API types mirrored in `src/lib/static.ts` with type guards (`is_bus`, `is_train`)

## Development

### Setup

```bash
# Uses mise for tool management - installs automatically
# Backend: Rust (system-installed), Frontend: Node/pnpm via mise

# Start PostgreSQL + Valkey (required)
cd backend && podman compose up -d

# Run migrations
cargo install sqlx-cli --no-default-features --features native-tls,postgres
sqlx migrate run

# Backend - requires API_KEY env var for BusTime API
cargo run  # or: mise run dev

# Frontend
cd frontend && pnpm install && pnpm dev
```

### Environment Variables

Backend (in `backend/.env.toml`):

- `DATABASE_URL` - PostgreSQL connection string (required)
- `REDIS_URL` - Valkey/Redis URL (required)
- `API_KEY` - MTA BusTime API key (required)
- `FORCE_UPDATE` - Force static data refresh on startup
- `READ_ONLY` - Skip all data updates (useful for testing API)
- `DEBUG_GTFS` - Save raw GTFS feeds to `./gtfs/` for debugging

## Conventions

### Backend

- Use `sqlx::query!` macro for compile-time checked SQL
- Migrations in `backend/migrations/` with timestamp prefixes
- Database schema uses `static.` namespace for static data tables
- Custom `FromRow` implementations for complex types with PostGIS geometry
- Error handling via `thiserror` with `ServerError` enum in `api/errors.rs`
- Cache static data in Redis; use `AppState::get_from_cache()` which auto-recaches on miss

### Frontend

- Components follow `Button.svelte`/`Modal.svelte` pattern for entities
- Use `.svelte.ts` extension for files with Svelte 5 runes
- Global stores exported as singletons (e.g., `export const alerts = createAlerts()`)
- API calls go through SvelteKit's `fetch` for SSR compatibility
- Type guards distinguish bus vs train data: `is_bus(stop)`, `is_train(stop)`

### API Design

- Endpoints support `?geojson=true` for GeoJSON responses
- Realtime endpoints accept `?at=timestamp` for historical data
- Static data cached with ETags for conditional requests
- Routes: `/api/v1/{routes,stops,trips,stop_times,alerts}`

## Database

- PostgreSQL with PostGIS extension
- Geometry stored as WKB, decoded via `geozero` crate
- Static data in `static.` schema: `route`, `stop`, `route_stop`, `transfer`, `last_update`
- Realtime data: `trip`, `stop_time`, `alert`, `alert_entity`

## Testing

Run the backend in read-only mode to test API without MTA feed updates:

```bash
READ_ONLY=1 cargo run
```
