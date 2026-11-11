# Backend architecture

Rust/Axum, PostgreSQL + PostGIS, in-memory snapshots and Moka caches. Sources: `mta_subway` (Helium), `mta_bus` (GTFS-RT/OBA), `njt_bus` (GTFS-RT). Testing policy lives in [backend/TESTING.md](../../backend/TESTING.md).

## Data flow

Feeds → `RealtimeSource` → `CollectedSnapshot` (pinned `NetworkRevision`) → `OperationsIngestor` transaction → committed `Arc<SourceSnapshot>` → shared `LiveSnapshots` watch channels → `Operations` / `TrajectoryDeriver` → Axum API.

## Modules (`backend/src/`)

| Module          | Owns                                                                                                                                                              |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `app.rs`        | Construction, production HTTP assembly, owner handles, background worker startup/shutdown. `main.rs` handles process lifetime and signals.                        |
| `network/`      | Static imports, schedules, transfers, restore, atomic revision/catalog publication. A `NetworkRevision` pairs routing inputs with exact catalog bytes and ETag.   |
| `operations/`   | Source-scoped run keys, run/visit/observation reconciliation, GPS sample writes, committed live membership, passenger indexing, retained-history materialization. |
| `alerts/`       | Source-typed alert aggregates, transactional replacement, current membership, declared-period historical reads, local cache.                                      |
| `trajectory/`   | Source builders, geometry, PCHIP, continuity, Arrow rendering. See [trajectories](trajectories.md).                                                               |
| `sources/`      | `RealtimeSource`, `AlertsAdapter`, `StaticAdapter` per provider.                                                                                                  |
| `api/`          | Request validation, handlers, borrowed domain-to-DTO encoding, OpenAPI. `AppState` holds owner read handles and a readiness check.                                |
| `integrations/` | GTFS-RT/OBA protocol helpers, Valhalla routing.                                                                                                                   |
| `telemetry/`    | Optional OTLP lifecycle, finite operation/HTTP spans, bounded instruments, committed-state callbacks and publication summaries.                                   |
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

## Optional telemetry

`telemetry::init` installs stdout logging (`RUST_LOG`) before application construction. OTLP `http/protobuf` export for traces, metrics and logs is runtime opt-in: it starts only when `OTEL_EXPORTER_OTLP_ENDPOINT` or a signal-specific endpoint is set. Disable signals with `OTEL_{TRACES,METRICS,LOGS}_EXPORTER=none` or everything with `OTEL_SDK_DISABLED=true`. Invalid configuration disables only the affected signal, and exporter availability never gates readiness. Generic endpoints get `/v1/{traces,metrics,logs}` appended; signal-specific endpoints are full URLs. Standard OTEL resource, sampler, header, timeout and interval settings apply; defaults are 60-second cumulative metrics and parent-based always-on sampling. `service.version` defaults to the crate version, `service.instance.id` is a process UUID, and the deployment environment is never inferred.

Application code is vendor-neutral and reads no `DD_*` settings. SDK and lifecycle diagnostics go to stdout only, avoiding recursive export failures. Providers use dedicated threads with blocking HTTP export and are drained off Tokio workers on shutdown (best effort; panics and SIGKILL excluded).

**Logs.** OTLP logs carry backend events only and inherit only `trainstatus.source` and `trainstatus.operation` from enclosing spans via `TracingSpanAttributes::allowlist`, even when trace export is disabled.

**Spans.** Finite operation spans and duration histograms cover realtime poll/collect/ingest, static imports, alert attempts and live trajectory refreshes. `app::http_service` owns the production router; observability wraps compression, error handling, buffering and rate limiting, and path normalization wraps routing. Server span names and `http.route` use matched templates (unmatched routes omit the attribute), unknown methods use `_OTHER`, and only 5xx sets server error status. `SourcePath` in `api/extract.rs` records the validated source on the server span. Incoming traces extract W3C trace context only; none is injected into provider requests. `integrations::http::fetch` measures send plus body decoding and strips URLs from errors, so paths, queries and credentials never become attributes.

**Metrics.** Initialized once, after the metric exporter succeeds. Live/static gauges read published owners without SQL; alert and trajectory gauges keep tiny post-publication summaries. Ages are computed at collection time, so failed polls cannot renew them. Successful empty publications report zeros; never-published state stays absent (except `trainstatus.live.ready=0`). Domain instruments use only `trainstatus.source` plus the bounded dimensions below; durations and ages are seconds.

| Instrument                                   | Type/unit                              | Dimensions / semantics                                                          |
| -------------------------------------------- | -------------------------------------- | ------------------------------------------------------------------------------- |
| `trainstatus.operation.duration`             | Histogram `s`                          | `operation`, `outcome` (success/error); one finished attempt                    |
| `trainstatus.realtime.changed`               | Counter `{entity}`                     | `entity` (trip/stop_time/vehicle_observation/resolved_shape); committed changes |
| `trainstatus.realtime.static_recovery`       | Counter `{recovery}`                   | Forced import/retry starts                                                      |
| `trainstatus.live.ready`                     | ObservableGauge `1`                    | First committed snapshot, including empty                                       |
| `trainstatus.live.entities`                  | ObservableGauge `{entity}`             | `entity` (trip/stop_time/vehicle_observation/located_vehicle/linked_vehicle)    |
| `trainstatus.live.snapshot.age`              | ObservableGauge `s`                    | Since live commit                                                               |
| `trainstatus.live.vehicle_observation.age`   | Histogram `s`                          | Accepted observation age at each commit, clamped to zero                        |
| `trainstatus.static.entities`                | ObservableGauge `{entity}`             | `entity` (route/stop/shape); published revision                                 |
| `trainstatus.alerts.entities`                | ObservableGauge `{alert}`              | Saved membership, not active-now candidates                                     |
| `trainstatus.alerts.snapshot.age`            | ObservableGauge `s`                    | Since successful alert save                                                     |
| `trainstatus.trajectory.entities`            | ObservableGauge `{entity}`             | `entity` (plan/render_unit); live hot snapshot                                  |
| `trainstatus.trajectory.snapshot.age`        | ObservableGauge `s`                    | Since input commit                                                              |
| `trainstatus.trajectory.inputs`              | Counter `{trip}`                       | `outcome`; per-trip derivation results                                          |
| `trainstatus.trajectory.continuity.discards` | Counter `{discard}`                    | `reason` (stale_prev_state/backward_gap/forward_jump)                           |
| `trainstatus.trajectory.knots.removed`       | Counter `{knot}`                       | `reason` (pre_anchor/backtracking)                                              |
| `trainstatus.trajectory.live_anchor.gap`     | Histogram `m`                          | Individual available anchor gaps                                                |
| `http.server.request.duration`               | Histogram `s`                          | Method, templated route, status, error type; no source dimension                |
| `http.client.request.duration`               | Histogram `s`                          | Method, server address/nondefault port, status, error type                      |
| `db.client.connection.count`                 | ObservableUpDownCounter `{connection}` | Pool `main`, state (idle/used); sampled without acquiring connections           |

No metric includes GTFS IDs, coordinates, timestamps, URLs or error messages. Each replica polls a full source, so use **max per source**, not sum, for membership gauges. Stop times lack scheduled-versus-actual pairs, so telemetry makes no delay or ETA claims.

### Datadog

Local setup is in the [README](../../README.md#optional-datadog-telemetry). For other deployments, run a Datadog Agent (7.48+) on the same host as the backend with OTLP ingestion enabled, or a generic Collector/DDOT for a central endpoint, and point the backend at it:

```sh
# Backend
OTEL_EXPORTER_OTLP_ENDPOINT=http://<agent-address>:4318
OTEL_SERVICE_NAME=trainstatus-backend
OTEL_RESOURCE_ATTRIBUTES=deployment.environment.name=production
OTEL_EXPORTER_OTLP_METRICS_TEMPORALITY_PREFERENCE=Delta

# Agent only, not backend environment
DD_API_KEY=<secret>
DD_SITE=us5.datadoghq.com
DD_OTLP_CONFIG_RECEIVER_PROTOCOLS_HTTP_ENDPOINT=0.0.0.0:4318
DD_LOGS_ENABLED=true
DD_OTLP_CONFIG_LOGS_ENABLED=true
```

Keep port 4318 private. Do not also tail stdout into Datadog, which duplicates OTLP logs. Host, container and Postgres-server metrics belong to Agent integrations. See Datadog's [Agent OTLP ingestion](https://docs.datadoghq.com/opentelemetry/setup/otlp_ingest_in_the_agent/) and [Delta temporality](https://docs.datadoghq.com/opentelemetry/guide/otlp_delta_temporality/) guides.
