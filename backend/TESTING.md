# Backend testing

Run commands from `backend/`.

| Task                                        | Purpose                                                                              |
| ------------------------------------------- | ------------------------------------------------------------------------------------ |
| `mise run test:unit`                        | Private unit and public behavior tests; no running services or tile extract required |
| `mise run test`                             | Full suite including integration tests and doctests                                  |
| `mise run test:integration`                 | Filesystem-backed persistence, ingestion, and engine tests                           |
| `mise run test:services integration <name>` | Focused integration filter                                                           |
| `mise run test:performance`                 | Opt-in ingestion and static import/restart benchmarks; use an idle machine           |
| `mise run smoke:storage`                    | End-to-end storage smoke (HTTP, all sources, restart recovery); see root task        |

`smoke:storage` selects the ignored `storage_cutover` integration test in `tests/storage_smoke.rs`. It allocates an isolated temporary root, invokes the same test executable for bootstrap/recovery/HTTP phases, kills the writer after durable acknowledgement, exercises the production TCP router, then cleans the root. Private child-phase environment variables are an internal test protocol, not a CLI. There are no `build-catalog` or `storage-smoke` binaries. Do not rebuild the selected test executable while its supervisor is spawning children.

Bus smoke replay includes both TripUpdates and VehiclePositions; MTA also includes OBA enrichment. Recovery compares independently selected unambiguous captured GPS reports with stored anchors and rejects a report queried immediately before its observation time. It does not substitute polling time for provider evidence.

For a focused service-free test, use `mise run test:unit <name>`. Native Valhalla build dependencies still apply; see `mise.toml`. Normal tests use committed fixtures and make no network requests to transit providers. The test tasks set `MTA_API_URL=https://mta.invalid` because Rust embeds it at compile time; tests need no fnox or provider credentials. The pre-commit Clippy step uses the same placeholder. For direct Cargo commands, supply `MTA_API_URL` explicitly (a placeholder URL suffices for tests). Integration and smoke tests use isolated directories under the filesystem store only.

## Placement and ownership

Private tests live in an owning module's `tests/` directory and are registered with `#[cfg(test)]` in that module. This preserves access to private functions without test bodies in production files. Realtime engine persistence coverage additionally requires `integration-tests`; service-free derivation tests do not.

Cargo has four explicit public test targets:

- `tests/behavior/`: source normalization, static validation, live trajectory derivation, and service-free HTTP behavior.
- `tests/integration/`: ingestion, historical stores, static import lifecycle/persistence, source scenarios and HTTP/geometry output.
- `tests/performance/`: opt-in benchmarks, never part of the ordinary suite.
- `tests/storage_smoke.rs`: the opt-in all-source crash/TCP smoke; private helpers live in `tests/support/storage_smoke.rs`.

For captured NJT schedule import/restart memory, run `mise run test:services performance njt_static_import_and_restart_memory` on an idle machine. This reports expanded trip count, import/restart timing, RSS during overlapping pinned revisions and peak RSS with jemalloc. Expansion uses the explicit captured service clock, not the machine's current date. The full captured GTFS uses a smaller GIS pattern set, so this is not a complete production workload measurement.

The large captured NJT GTFS import remains an ignored library test. Run it explicitly by its name with `--ignored` when checking the full import; the ordinary suite uses smaller committed cases.

Only target entrypoints live at the root of `tests/`. Nested modules must be registered in their owner's `mod.rs`; helpers belong in `tests/support/`. Name files after the behavior, not an ambiguous `trajectory` or `snapshot` bucket. Live trajectory derivation is `behavior/trajectory/live`; historical inputs are `integration/operations/history`; ingestion shape selection is `integration/operations/shape_resolution`.

## Storage layout and isolation

`test:services` selects a suite and supplies its test environment; it does not start or stop Podman pods. Each test constructs fresh store owners in a temporary directory (or uses helpers in `tests/support/`). Set `TRAINSTATUS_DATA_DIR` only when you intentionally share a directory across commands. Normal Rust test parallelism is supported; helpers allocate distinct roots per case where needed.

Each owner shares its intended caches and root pins across readers; construct fresh owners for independent test roots. Never delete a development or production data directory for tests. Test helpers own and remove their temporary directories.

Use fixed historical timestamps and explicit fixture service dates. Live derivation reads the real clock, so its inputs remain relative to that clock. Do not add a public test-only clock API just to relocate tests.

## Choosing assertions

Start with the contract and inspect its owner before adding a test. Prefer observable IDs, linkage, predictions, atomic publication, retained versus live membership, and decoded geometry/Arrow data. Counts and successful calls alone rarely establish the contract. Timestamp precision, unchanged-write tuple checks and rollback tests protect persistence requirements and are intentional.

Source parity means applicable coverage for static normalization, realtime normalization, alerts, persistence, tiles and Arrow. Share setup and contracts, but retain each source's expectations: NJT exact patterns/direction/remaps, MTA bus OBA enrichment and shape membership, subway connected segments/platforms/consists. Equal test counts or copies of the NJT scenario do not provide parity. Output tests use matched normalized inputs; separate adapter tests prove parsing of captured feeds.

Before deleting a test, identify the obsolete requirement or surviving test that catches the same failure. Keep distinct private algorithms, transaction behavior and HTTP dispatch coverage even if their setups resemble each other. Fixture manifests protect capture integrity; production normalization tests protect domain behavior. Ordinary tests must not write frontend fixtures or OpenAPI artifacts as optional environment-driven side effects.

## Fixture commands

Use `mise run fixtures --help` for the complete command help and examples. `capture` fetches provider data; `list`, `verify`, and `update-expected` inspect existing bundles without fetching. Capture needs the source's provider credentials. `update-expected` rewrites supported expected summaries and requires review of the resulting diff; `verify` compares only expected files that already exist and does not replace domain tests.

`--scenario` is an optional bundle label, defaulting to `basic`: `<root>/<source>/<kind>/<scenario>`. It lets you store another captured case alongside the basic capture. It does not change normalization, simulate a detour, or register a new test; a test must load the label explicitly. Reusing the same label overwrites that bundle's captured files. Use `--output /tmp/trainstatus-fixtures` to inspect a capture before updating committed fixtures.
