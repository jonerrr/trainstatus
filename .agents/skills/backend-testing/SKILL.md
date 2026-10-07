---
name: backend-testing
description: Use when adding, changing, reviewing, or running tests for the Train Status Rust backend.
---

# Backend testing

Read [backend/TESTING.md](../../../backend/TESTING.md) for the authoritative placement, assertion and execution policy. Inspect `backend/mise.toml` for current commands before running tests.

Identify the production contract and inspect coverage under its owner. Keep private algorithms in the owner's `src/**/tests/` module; never expose private functions merely to test them externally. Public service-free behavior belongs in `tests/behavior/`; real SQL/cache behavior belongs in `tests/integration/`; opt-in measurements belong in `tests/performance/`. Register nested modules explicitly.

Choose the smallest sufficient test. For timestamp or linkage bugs, exercise the affected persistence boundary with fixed inputs and assert stored and published values, not just successful ingestion. Keep historical selection, live membership and HTTP dispatch under their respective owners.

Use committed captures and explicit service dates. The fast suite must work without SQL, external services, provider credentials or a runtime tile extract. Service-backed tests reuse the existing PostgreSQL server through a maintenance connection; SQLx creates and migrates isolated per-test databases. Use the mise tasks rather than migrating the development `trains` database. Construct fresh stores/services for independent local caches; ordinary tests need no cache containers.

`SQLX_OFFLINE` governs compile-time macro metadata, not runtime SQL. Keep ordinary offline compilation while legacy macros exist; validate metadata with the dedicated task. Follow the existing runtime query convention for new code.

When asked for source parity, compare applicable contracts rather than copying NJT tests. Preserve source-specific OBA enrichment, exact-pattern/remap rules, and subway segment/consist behavior. Output contracts can share matched normalized input setup; adapter tests still need captured parsing assertions.

Before removing coverage, name the obsolete behavior or a surviving assertion that detects its failure. Avoid tests that merely mirror helpers or check nonempty bytes. Decode public outputs and assert relevant values. Report the contract protected and checks actually run, including any environment limitation.
