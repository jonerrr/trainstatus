# Backend architecture

Rust/Axum, authoritative local-SSD files, immutable live snapshots and bounded Moka caches. Sources: `mta_subway` (Helium), `mta_bus` (GTFS-RT/OBA), `njt_bus` (GTFS-RT). Testing policy lives in [backend/TESTING.md](../../backend/TESTING.md).

## Data flow

Feeds → `RealtimeSource` → `CollectedSnapshot` (pinned `NetworkRevision`) → validated accepted changes → durable operations journal → `Arc<SourceSnapshot>` → shared `LiveSnapshots` watch channels → `Operations` / `TrajectoryDeriver` → Axum API.

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
| `storage/`      | Exclusive root lock, bounded typed CBOR/zstd codec, packed independently addressable records, checksummed journals and durable filesystem publication.            |

`network`, `operations`, `alerts` and `trajectory` own their domain, persistence and reads. Typed persisted records/indexes are distinct from HTTP DTOs. Source-specific payload ADTs remain shared where appropriate. No SQL server, embedded database engine, database URL or connection pool participates in the backend.

## API (`/api/v1/`)

- `GET /catalog/{source}`: static catalog, ETag-validated.
- `GET /service/{source}`: subway is whole-source; buses require `?route_ids=`.
- `GET /alerts/{source}`, `GET /trajectories/{source}`.
- Realtime endpoints accept `?at=<unix_timestamp>` for historical reads.

## Network

- Static imports consume owned datasets and publish source-selected typed sections, factored schedules, nearby stops and exact catalog bytes/ETags. One atomic all-source root selects durable files; unchanged sections are reused. Cross-source nearby refreshes do not decode unrelated schedules or geometry.
- If current-root installation fails, every clone fences further imports/restores until reopen. Existing resident revisions and freshness remain unchanged; reopening loads the durable root before another source can publish.
- `RealtimeEngine` calls `NetworkController::ensure_updated(source)` before collection. A typed missing-static-reference error permits one forced import and one renormalization/retry of the same provider batch.
- Empty routes/stops are rejected as incomplete imports; empty schedules are allowed. Retained route/stop descriptions remain readable after removal from current membership. Nearby candidates use EPSG:6538 coordinates with exact directed 150 m boarding eligibility.
- Geometry is content-addressed per source. Ordered accepted bindings cannot be rewritten by reusing a provider name. Live and historical binding views share a byte-weighted decoded cache; retired shapes retain asset references, not coordinate copies. Subway leg IDs preserve station junctions. Runtime imports are the only publication workflow; no separate catalog builder/promotion CLI remains.
- Schedules share immutable `Arc<TripTemplate>` calls across dated occurrences. NJT keeps each occurrence's original expiration across rollover/restart; patterns and child-stop remaps stay pinned during enrichment.
- GTFS times use local noon minus twelve hours plus elapsed seconds, preserving DST and offsets above 24 hours. Helium origin IDs remain wall-clock codes. Missing-date inference and schedule expiry use the provider observation clock rather than polling time.

## Operations and reads

- Deterministic `RunId` is an opaque BLAKE3 digest. Scheduled buses use provider trip ID and explicit/resolved service date; subway uses normalized base train line, origin time and origin code plus resolved date. Mutable vehicle assignment, trip type, destination and current direction do not identify a scheduled occurrence. Missing/ambiguous NJT dates isolate the run. Helium date inference retains a documented finite window.
- Visits use a nonnegative ordered sequence, including repeated stops. Missing bus sequences require one unique scheduled occurrence; conflicting repeated sequences reject a collection.
- Omission removes live membership, not retained calls. Source commits serialize; the root's exclusive writer lock prevents independent backend processes from opening it together.
- Live reads retain the last accepted `Arc` and perform no filesystem reads. Failure preserves it; successful empty collection clears it. Startup returns `503` until a successful collection. Requested routes remain present when empty; unknown bus routes return `404`. Live `updated_at` is the acceptance time, not refetch time.
- `at` selects latest authoritative runs with any arrival/departure in inclusive `[at, at + 4h]`, returning all retained visits. Hot records shadow archive versions before predicates. Historical output omits current vehicle status/freshness. This is latest-record materialization, not prediction replay.
- History admission allows eight running-or-waiting requests, while a separate execution semaphore allows two running requests. Further admission returns `503`; live traffic bypasses both. Cancellation while queued releases admission immediately. Once started, an owned task retains both permits through blocking reads, calculation and response encoding, even if the HTTP caller leaves.
- Coordinates use microsecond timestamps and are first-wins on equal times; equal-time enrichment/linkage can update and strictly older observations are rejected. Anchors select the newest accepted sample in inclusive `[at - 5 min, at]`. Identical polls do not add samples.
- Native `Run<S>` aggregates own metadata, ordered visits and `RunGeometry::{Unresolved, Accepted}`. `SourceSpec` selects concrete source payloads; accepted deltas do not serialize whole runs or API source tags. Collected positions retain `RunRef { id, partition_date }`, including GPS-only archive reopening. The duplicate last-seen map and persisted telemetry counts are gone.
- GPS uses up to 256 samples per immutable page, a bounded tail and a copy-on-write 64-way interval-index tree. Reopen loads descriptors, not all samples. Prepared seals retain the bounded remainder so an uncertain checkpoint cannot lose later tail points during replay.
- Checkpoints reuse unchanged passenger blocks and publish their watermark/archive sequence before WAL reset. Cold runs use one head-tree root per service-date partition, with bounded 64-way nodes; updates copy touched paths only. Passenger-only reads skip description/GPS payloads, and route predicates precede passenger decoding.
- Current and reader-pinned archive/GPS roots protect shared blocks. Reclaim only after checkpoint publication and WAL reset; reconcile uncertain archive roots first. A missing checkpoint after reset or an archive older than its checkpoint fails closed. Historical I/O runs outside the source writer lock. Failed maintenance retries before admission; invalid trajectories never remove passenger calls.
- GPS thinning and age-based history deletion are not part of this cutover.

## Alerts

A successful collection durably replaces current membership and each alert's translations, periods and affected entities. Immutable versioned records/head pages and an atomic manifest retain history; journal watermarks prevent duplicate replay. Empty polls clear current membership, not history; failures preserve the prior publication. Open-ended declared periods remain open after omission. GTFS alerts without `active_period` instead use persisted observed-presence windows: first observation opens one, successful absence closes it, failed collection does not, and reappearance opens a new window ([specification](https://gtfs.org/documentation/realtime/reference/#message-alert)). The API omits undeclared start/end bounds and sorts affected entities by `sort_order`. MTA clone replacements tombstone originals source-locally. `AlertCollection` structurally ties source and payload; deterministic `AlertId` uses source, original ID and creation time.

Immutable alert content is separate from observation/update clocks. WAL records carry durable content references; unchanged content reuses its packed record while freshness advances. Original-ID hash buckets select time-ordered pages bounded to 128 entries and a 1 MiB encoded budget, with separate open/closed intervals. Freshness updates locate existing heads through current references and rewrite only affected pages, preserving untouched page descriptors. Byte-weighted caches bound retained content/index/page values. Pinned generations protect GC; reopen discovers orphans. An uncertain root replacement preserves WAL and blocks writes until reopen.

An owned save task serializes mutations through durable and in-memory publication, so caller cancellation cannot abandon an installed manifest. Content preparation, journal recovery/append, page/index writes, validation, manifest replacement and journal reset run together off Tokio workers; cache publication finishes before the mutation lock is released.

## Development checks

Rust embeds `MTA_API_URL` at compile time. Checks and capture-replay smoke use `https://mta.invalid` without credentials. Tests use committed captures, explicit dates and isolated filesystem roots. Live collection and fixture capture retain their explicit mise secret grants. From the root run `mise run check`, `mise run test:unit` and `mise run smoke:storage`.

## Local storage

`TRAINSTATUS_DATA_DIR` selects the authoritative root (default `data`). `Storage::open` holds an exclusive writer lock for the shared handle lifetime. After acquiring that lock and before owner recovery, startup removes abandoned publisher temporary files with recognized targets and numeric PID/sequence suffixes from the three owner trees, syncing affected directories. It does not follow symlinks or delete unrelated `.part` files. Catalog chunks, retained identities, shape assets, operations, alerts, checkpoints and journals live under that root. Readiness checks local storage availability, not a database connection.

The native codec/block epoch is 3; journal version is 4. Typed CBOR/zstd records have declared-size bounds and checksums; packed directories permit selected-record positional reads. Atomic publication uses unique temporary names and syncs content and containing directories. Established corruption fails closed; only incomplete unsealed WAL tails are truncated. Old experimental formats are rejected, not silently migrated or wiped.

`LocalApplication::open` constructs the real owners without provider workers. `mise run smoke:storage` selects an ignored integration test that invokes its own executable for crash/reopen phases and exercises actual TCP HTTP with all-source captures. There is no standalone smoke executable, serving CLI or fixture persistence fallback.

For a consistent simple backup, stop the writer and copy the complete root to an independent device/host. Restore into another stopped root and validate before serving. Checksums do not reconstruct lost bytes. Unknown epochs require offline conversion of a copy, not fallback readers. See the [storage plan](../../Train%20Status%20storage%20architecture%20v2.md) for the format/recovery contract and research limitations.

## Optional telemetry

`telemetry::init` installs stdout logging (`RUST_LOG`) before application construction. OTLP `http/protobuf` export for traces, metrics and logs is runtime opt-in: it starts only when `OTEL_EXPORTER_OTLP_ENDPOINT` or a signal-specific endpoint is set. Disable signals with `OTEL_{TRACES,METRICS,LOGS}_EXPORTER=none` or everything with `OTEL_SDK_DISABLED=true`. Invalid configuration disables only the affected signal, and exporter availability never gates readiness. Generic endpoints get `/v1/{traces,metrics,logs}` appended; signal-specific endpoints are full URLs. Standard OTEL resource, sampler, header, timeout and interval settings apply; defaults are 60-second cumulative metrics and parent-based always-on sampling. `service.version` defaults to the crate version, `service.instance.id` is a process UUID, and the deployment environment is never inferred.

Application code is vendor-neutral and reads no `DD_*` settings. SDK and lifecycle diagnostics go to stdout only, avoiding recursive export failures. Providers use dedicated threads with blocking HTTP export and are drained off Tokio workers on shutdown (best effort; panics and SIGKILL excluded).

**Logs.** OTLP logs carry backend events only and inherit only `trainstatus.source` and `trainstatus.operation` from enclosing spans via `TracingSpanAttributes::allowlist`, even when trace export is disabled.

**Spans.** Finite operation spans and duration histograms cover realtime poll/collect/ingest, static imports, alert attempts and live trajectory refreshes. `app::http_service` owns the production router; observability wraps compression, error handling, buffering and rate limiting, and path normalization wraps routing. Server span names and `http.route` use matched templates (unmatched routes omit the attribute), unknown methods use `_OTHER`, and only 5xx sets server error status. `SourcePath` in `api/extract.rs` records the validated source on the server span. Incoming traces extract W3C trace context only; none is injected into provider requests. `integrations::http::fetch` measures send plus body decoding and strips URLs from errors, so paths, queries and credentials never become attributes.

**Metrics.** Initialized once, after the metric exporter succeeds. Live/static gauges read published owners without SQL; alert and trajectory gauges keep tiny post-publication summaries. Ages are computed at collection time, so failed polls cannot renew them. Successful empty publications report zeros; never-published state stays absent (except `trainstatus.live.ready=0`). Domain instruments use only `trainstatus.source` plus the bounded dimensions below; durations and ages are seconds.

| Instrument                                   | Type/unit                  | Dimensions / semantics                                                          |
| -------------------------------------------- | -------------------------- | ------------------------------------------------------------------------------- |
| `trainstatus.operation.duration`             | Histogram `s`              | `operation`, `outcome` (success/error); one finished attempt                    |
| `trainstatus.realtime.changed`               | Counter `{entity}`         | `entity` (trip/stop_time/vehicle_observation/resolved_shape); committed changes |
| `trainstatus.realtime.static_recovery`       | Counter `{recovery}`       | Forced import/retry starts                                                      |
| `trainstatus.live.ready`                     | ObservableGauge `1`        | First committed snapshot, including empty                                       |
| `trainstatus.live.entities`                  | ObservableGauge `{entity}` | `entity` (trip/stop_time/vehicle_observation/located_vehicle/linked_vehicle)    |
| `trainstatus.live.snapshot.age`              | ObservableGauge `s`        | Since live commit                                                               |
| `trainstatus.live.vehicle_observation.age`   | Histogram `s`              | Accepted observation age at each commit, clamped to zero                        |
| `trainstatus.static.entities`                | ObservableGauge `{entity}` | `entity` (route/stop/shape); published revision                                 |
| `trainstatus.alerts.entities`                | ObservableGauge `{alert}`  | Saved membership, not active-now candidates                                     |
| `trainstatus.alerts.snapshot.age`            | ObservableGauge `s`        | Since successful alert save                                                     |
| `trainstatus.trajectory.entities`            | ObservableGauge `{entity}` | `entity` (plan/render_unit); live hot snapshot                                  |
| `trainstatus.trajectory.snapshot.age`        | ObservableGauge `s`        | Since input commit                                                              |
| `trainstatus.trajectory.inputs`              | Counter `{trip}`           | `outcome`; per-trip derivation results                                          |
| `trainstatus.trajectory.continuity.discards` | Counter `{discard}`        | `reason` (stale_prev_state/backward_gap/forward_jump)                           |
| `trainstatus.trajectory.knots.removed`       | Counter `{knot}`           | `reason` (pre_anchor/backtracking)                                              |
| `trainstatus.trajectory.live_anchor.gap`     | Histogram `m`              | Individual available anchor gaps                                                |
| `http.server.request.duration`               | Histogram `s`              | Method, templated route, status, error type; no source dimension                |
| `http.client.request.duration`               | Histogram `s`              | Method, server address/nondefault port, status, error type                      |

No metric includes GTFS IDs, coordinates, timestamps, URLs or error messages. Each replica polls a full source, so use **max per source**, not sum, for membership gauges. Stop times lack scheduled-versus-actual pairs, so telemetry makes no delay or ETA claims.

### Datadog

Local setup is in the [README](../../README.md#optional-telemetry). For other deployments, run a Datadog Agent (7.48+) on the same host as the backend with OTLP ingestion enabled, or a generic Collector/DDOT for a central endpoint, and point the backend at it:

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

Keep port 4318 private. Do not also tail stdout into Datadog, which duplicates OTLP logs. Host/container metrics belong to Agent integrations. See [Agent OTLP ingestion](https://docs.datadoghq.com/opentelemetry/setup/otlp_ingest_in_the_agent/) and [Delta temporality](https://docs.datadoghq.com/opentelemetry/guide/otlp_delta_temporality/).
