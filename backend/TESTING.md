# Backend testing

Run commands from `backend/`, prefixed with `mise exec --`.

| Task                                        | Purpose                                                                              |
| ------------------------------------------- | ------------------------------------------------------------------------------------ |
| `mise run test:unit`                        | Private unit and public behavior tests; no running services or tile extract required |
| `mise run test`                             | Ordinary suite on existing PostgreSQL, including integration tests and doctests      |
| `mise run test:integration`                 | SQL, HTTP persistence and private engine tests                                |
| `mise run test:services integration <name>` | Focused integration filter                                                           |
| `mise run test:performance`                 | Opt-in ingestion WAL and static import/restart benchmarks; use an idle machine       |
| `mise run sqlx:check`                       | Verify committed SQLx metadata in the separate trainstatus_sqlx_check database       |

For a focused service-free test, use `SQLX_OFFLINE=true cargo test --features fixture-capture --lib --test behavior <name>` through `mise exec --`. Native Valhalla build dependencies still apply; see `mise.toml`. Normal tests use committed fixtures and require no provider credentials or network requests to transit providers. Service-backed tests need the development PostgreSQL/PostGIS server running. Tests use local Rust caches; no cache containers are needed.

## Placement and ownership

Private tests live in an owning module's `tests/` directory and are registered with `#[cfg(test)]` in that module. This preserves access to private functions without test bodies in production files. The five private realtime engine tests additionally require `integration-tests` because they use services. The old, inactive `integrations/gtfs_static` module remains inactive; relocating its tests does not enable legacy production code.

Cargo has three explicit public test targets:

- `tests/behavior/`: source normalization, static validation, live trajectory derivation, and service-free HTTP behavior.
- `tests/integration/`: ingestion transactions, historical stores, static import lifecycle and persistence, service-backed source scenarios, and HTTP/geometry output.
- `tests/performance/`: opt-in benchmarks, never part of the ordinary suite.

For captured NJT schedule import/restart memory, run `mise exec -- mise run test:services performance njt_static_import_and_restart_memory` on an idle PostgreSQL server. This reports expanded trip count, import/restart timing, RSS during overlapping pinned revisions, and peak RSS with the executable's jemalloc allocator. The capture's calendar must include today; the test fails explicitly when it has expired. It uses the full captured GTFS but a small captured GIS pattern set, so it does not measure the complete production workload.

The large captured NJT GTFS import remains an ignored library test. Run it explicitly by its name with `--ignored` when checking the full import; the ordinary suite uses smaller committed cases.

Only target entrypoints live at the root of `tests/`. Nested modules must be registered in their owner's `mod.rs`; helpers belong in `tests/support/`. Name files after the behavior, not an ambiguous `trajectory` or `snapshot` bucket. Live trajectory derivation is `behavior/trajectory/live`; historical inputs are `integration/stores/trajectory`; ingestion shape selection is `integration/realtime/shape_resolution`.

## Services and clocks

`test:services` selects a suite and supplies its test environment; it does not start or stop PostgreSQL. By default it connects to `postgres://trains:trains@localhost:5432/postgres` on the existing development server. Override this connection with `TEST_DATABASE_URL` when using another server. The `postgres` maintenance database holds SQLx's `_sqlx_test` bookkeeping; test migrations and data go into SQLx-created databases, never the development `trains` database.

Tests accepting `PgPool` through `#[sqlx::test]` get a fresh database with migrations applied automatically. Successful test databases are deleted; failed ones remain for debugging and SQLx cleans old test databases on a later run. The connection role needs permission to create databases and install the migration extensions (the local `trains` image role is a superuser). SQLx manages databases, not the PostgreSQL server. Normal Rust test parallelism is supported. Avoid overlapping independent suite invocations on this same server: SQLx's cleanup/database names can collide across runs; use separate servers if that is needed.

`sqlx:check` is separate because it is a CLI schema check, not a `#[sqlx::test]`. It creates/migrates the dedicated `trainstatus_sqlx_check` database on the same server. Override its URL with `SQLX_CHECK_DATABASE_URL`; this must point to a metadata-check database, not `trains`. The check database is retained for reuse.

Each store/service owns its in-memory caches. Construct fresh stores for test isolation; no external cache or container cleanup is required. Never reset the development pod for tests.

Use fixed historical timestamps and explicit fixture service dates. Live derivation reads the real clock, so its inputs remain relative to that clock; tile activity uses PostgreSQL's five-minute `NOW()` window. Do not add a public test-only clock API just to relocate tests.

## Choosing assertions

Start with the contract and inspect its owner before adding a test. Prefer observable IDs, linkage, predictions, atomic publication, retained versus live membership, and decoded geometry/Arrow data. Counts and successful calls alone rarely establish the contract. Timestamp precision, unchanged-write tuple checks and rollback tests protect persistence requirements and are intentional.

Source parity means applicable coverage for static normalization, realtime normalization, alerts, persistence, tiles and Arrow. Share setup and contracts, but retain each source's expectations: NJT exact patterns/direction/remaps, MTA bus OBA enrichment and shape membership, subway connected segments/platforms/consists. Equal test counts or copies of the NJT scenario do not provide parity. Output tests use matched normalized inputs; separate adapter tests prove parsing of captured feeds.

Before deleting a test, identify the obsolete requirement or surviving test that catches the same failure. Keep distinct private algorithms, transaction behavior and HTTP dispatch coverage even if their setups resemble each other. Fixture manifests protect capture integrity; production normalization tests protect domain behavior. Ordinary tests must not write frontend fixtures or OpenAPI artifacts as optional environment-driven side effects.

## Fixture commands

Use `mise exec -- mise run fixtures --help` for the complete command help and examples. `capture` fetches provider data; `list`, `verify`, and `update-expected` inspect existing bundles without fetching. Capture needs the source's provider credentials. `update-expected` rewrites supported expected summaries and requires review of the resulting diff; `verify` compares only expected files that already exist and does not replace domain tests.

`--scenario` is an optional bundle label, defaulting to `basic`: `<root>/<source>/<kind>/<scenario>`. It lets you store another captured case alongside the basic capture. It does not change normalization, simulate a detour, or register a new test; a test must load the label explicitly. Reusing the same label overwrites that bundle's captured files. Use `--output /tmp/trainstatus-fixtures` to inspect a capture before updating committed fixtures.
