# Backend

## Structure

`sources/` decodes providers into owner domain types; `integrations/` wraps GTFS-RT/OBA and Valhalla. `network/` owns imports, restore, schedules, transfers and catalog revisions. `operations/` owns stable run identity, visits, observations, explicit GPS samples, atomic ingestion and shared retained-history materialization. `alerts/` owns notices and current/retained applicability. `trajectory/` owns geometry, interpolation, continuity and Arrow output.

PostgreSQL is durable latest-record storage. Live reads use immutable committed source snapshots shared by `Arc`; only explicit historical reads access retained SQL rows. Network revisions pair routing inputs with exact catalog JSON/ETags and publish all affected catalogs together. Historical trajectories use retained shape assets and eligible recorded GPS anchors. No realtime generations, prediction versions or encoded API/map artifacts are persisted.

SQLx macros check describable persistence queries at compile time using the committed `.sqlx/` cache and `sqlx.toml` type overrides. Complex aggregate/spatial projections remain runtime queries. See [testing and metadata commands](TESTING.md).

`api/dto/` defines wire representations and borrowed encoders; domain types and private SQL rows are separate. `app.rs` constructs owners, starts workers and stops them during shutdown. This architecture assumes one ingestion writer per source in one process.

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
