# Backend

## Structure

`sources/` fetches and normalizes provider data. `integrations/` wraps external protocols and Valhalla. `static_data/`, `alerts/`, and `realtime/` each own their background lifecycle; static adapters return datasets rather than writing stores directly.

PostgreSQL is the durable store. Static imports commit routes, stops, associations, shapes, NJT schedules, metadata, and their timestamp together, then publish an immutable revision. NJT schedules preserve today/tomorrow expansion and per-entry 48-hour expiration, retaining unexpired earlier service dates for overnight trips. Complete empty imports clear the schedule set. An existing installation imports NJT static data once to initialize schedule persistence; later restarts restore fresh revisions from PostgreSQL.

Routes and stops use local response snapshots with matching JSON bytes and ETags. Proximity-transfer updates refresh every source's stop response. Current alerts use a coordinated 30-second Moka cache; explicit historical reads bypass it. `trajectory/` owns calculation and derivation; `TrajectoryService` hides live publication, historical loading, and evictable computation caches. Authoritative realtime and live trajectory snapshots never evict.

This architecture targets one backend process owning ingestion and serving the API. API and historical response formats remain unchanged.

## Config

| Environment Variable    | Usage                                                                                            | Required | Default                    |
| ----------------------- | ------------------------------------------------------------------------------------------------ | -------- | -------------------------- |
| `DATABASE_URL`          | PostgreSQL connection URL used to create the sqlx pool and run migrations on startup.            | Yes      | None                       |
| `ADDRESS`               | Bind address for the Axum HTTP server listener.                                                  | No       | `127.0.0.1:3055`           |
| `MTA_OBA_API_KEY`       | API key for MTA Bus Time OBA endpoints used by the MTA bus source.                               | Yes      | None                       |
| `NJT_USERNAME`          | NJ Transit API username used to authenticate and fetch access tokens.                            | Yes      | None                       |
| `NJT_PASSWORD`          | NJ Transit API password used to authenticate and fetch access tokens.                            | Yes      | None                       |
| `VALHALLA_TILE_EXTRACT` | Path to Valhalla tiles.tar extract for in-process route snapping (via `valhalla`).               | No       | `/data/valhalla_tiles.tar` |
| `API_PREFIX`            | Base URL prefix for API routes and docs routes (`/v1`, `/docs`, `/openapi.json`).                | No       | `/api`                     |
| `DEBUG_RT_DATA`         | If set (to any value), writes raw realtime payloads and decoded debug output to `./debug_data/`. | No       | Disabled (unset)           |

<!-- | `READ_ONLY`            | If set, the backend will not update any realtime or static data        | No       | -->
<!-- | `FORCE_UPDATE`         | If set, static data will update on startup                             | No       | -->
