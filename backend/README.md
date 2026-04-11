# Backend

## Structure

TODO: Discuss backend structure

## Config

| Environment Variable | Usage                                                                                            | Required | Default               |
| -------------------- | ------------------------------------------------------------------------------------------------ | -------- | --------------------- |
| `DATABASE_URL`       | PostgreSQL connection URL used to create the sqlx pool and run migrations on startup.            | Yes      | None                  |
| `REDIS_URL`          | Redis/Valkey connection URL used for cache reads/writes and startup connectivity checks.         | Yes      | None                  |
| `ADDRESS`            | Bind address for the Axum HTTP server listener.                                                  | No       | `127.0.0.1:3055`      |
| `MTA_OBA_API_KEY`    | API key for MTA Bus Time OBA endpoints used by the MTA bus source.                               | Yes      | None                  |
| `NJT_USERNAME`       | NJ Transit API username used to authenticate and fetch access tokens.                            | Yes      | None                  |
| `NJT_PASSWORD`       | NJ Transit API password used to authenticate and fetch access tokens.                            | Yes      | None                  |
| `VALHALLA_CONFIG`    | Path to the Valhalla config file used to initialize Valhalla integration.                        | No       | `/data/valhalla.json` |
| `API_PREFIX`         | Base URL prefix for API routes and docs routes (`/v1`, `/docs`, `/openapi.json`).                | No       | `/api`                |
| `DEBUG_RT_DATA`      | If set (to any value), writes raw realtime payloads and decoded debug output to `./debug_data/`. | No       | Disabled (unset)      |

<!-- | `READ_ONLY`            | If set, the backend will not update any realtime or static data        | No       | -->
<!-- | `FORCE_UPDATE`         | If set, static data will update on startup                             | No       | -->
