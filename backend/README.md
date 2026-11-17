# Backend

## Structure

`sources/` decodes providers into owner domain types; `integrations/` wraps GTFS-RT/OBA and Valhalla. `network/` owns imports, restore, schedules, transfers and catalog revisions. `operations/` owns stable run identity, visits, observations, explicit GPS samples, atomic ingestion and shared retained-history materialization. `alerts/` owns notices and current/retained applicability. `trajectory/` owns geometry, interpolation, continuity and Arrow output.

Local SSD storage under `TRAINSTATUS_DATA_DIR` is the durable tier. Live reads use immutable committed source snapshots shared by `Arc`; historical reads load retained records from the filesystem store. Network revisions pair routing inputs with exact catalog JSON/ETags and publish all affected catalogs together. Historical trajectories use retained shape assets and eligible recorded GPS anchors. No realtime generations, prediction versions or encoded API/map artifacts are persisted.

Vehicle observations with the same provider timestamp may update passenger/status metadata and run linkage. Coordinates retain the first accepted fix at that timestamp; only a newer report replaces them. Recorded GPS samples remain first-wins per run and timestamp.

`api/dto/` defines wire representations and borrowed encoders; domain types and private persisted shapes are separate. `app.rs` constructs owners, starts workers and stops them during shutdown. This architecture assumes one ingestion writer per source in one process.

## Config

| Environment Variable    | Usage                                                                                            | Required | Default                    |
| ----------------------- | ------------------------------------------------------------------------------------------------ | -------- | -------------------------- |
| `TRAINSTATUS_DATA_DIR`  | Root directory for durable local storage (operations, network, alerts, and journals).            | No       | `data`                     |
| `ADDRESS`               | Bind address for the Axum HTTP server listener.                                                  | No       | `127.0.0.1:3055`           |
| `MTA_OBA_API_KEY`       | API key for MTA Bus Time OBA endpoints used by the MTA bus source.                               | Yes      | None                       |
| `NJT_USERNAME`          | NJ Transit API username used to authenticate and fetch access tokens.                            | Yes      | None                       |
| `NJT_PASSWORD`          | NJ Transit API password used to authenticate and fetch access tokens.                            | Yes      | None                       |
| `VALHALLA_TILE_EXTRACT` | Path to Valhalla tiles.tar extract for in-process route snapping (via `valhalla`).               | No       | `/data/valhalla_tiles.tar` |
| `API_PREFIX`            | Base URL prefix for API routes and docs routes (`/v1`, `/docs`, `/openapi.json`).                | No       | `/api`                     |
| `DEBUG_RT_DATA`         | If set (to any value), writes raw realtime payloads and decoded debug output to `./debug_data/`. | No       | Disabled (unset)           |

<!-- | `READ_ONLY`            | If set, the backend will not update any realtime or static data        | No       | -->
<!-- | `FORCE_UPDATE`         | If set, static data will update on startup                             | No       | -->
