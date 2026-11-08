# Train Status - Agent Instructions

Real-time tracker for MTA subway, MTA buses and NJT buses. Rust/Axum backend, SvelteKit frontend, PostgreSQL + PostGIS. Sources: `mta_subway`, `mta_bus`, `njt_bus`.

## Commands

Run from the directory shown. `mise.toml` in each directory is authoritative.

| Where       | Command                                                                                                       |
| ----------- | ------------------------------------------------------------------------------------------------------------- |
| root        | `mise run check` / `mise run fix` (hk on changed files)                                                       |
| root        | `mise run test:unit` (service-free backend + frontend suites)                                                 |
| `backend/`  | `mise run test` (needs dev Postgres), `mise run sqlx:prepare`                                                 |
| `frontend/` | `mise run test`, `pnpm check`, `pnpm lint`                                                                    |
| `backend/`  | `cargo run --bin openapi`, then `pnpm openapi-ts --input <json>` in `frontend/` after any API contract change |

See [README.md](README.md) for setup and environment variables.

## Read before working

| Touching                    | Read first                                                                                               |
| --------------------------- | -------------------------------------------------------------------------------------------------------- |
| Anything backend            | [docs/architecture/backend.md](docs/architecture/backend.md)                                             |
| Anything under `frontend/`  | [docs/architecture/frontend.md](docs/architecture/frontend.md), [frontend/AGENTS.md](frontend/AGENTS.md) |
| Trajectories or the map     | [docs/architecture/trajectories.md](docs/architecture/trajectories.md)                                   |
| Backend tests or SQL/schema | [.agents/skills/backend-testing/SKILL.md](.agents/skills/backend-testing/SKILL.md)                       |
| Frontend tests              | [.agents/skills/frontend-testing/SKILL.md](.agents/skills/frontend-testing/SKILL.md)                     |
| UI, styling                 | [DESIGN.md](DESIGN.md)                                                                                   |

## Core invariants

- Live reads use the last committed source snapshot and never touch SQL. Failed collection preserves it; a successful empty collection clears it.
- Each owner module (`network`, `operations`, `alerts`, `trajectory`) owns its SQL. SQL rows never leave their owner.
- Add a source by implementing `RealtimeSource` + `AlertsAdapter` + `StaticAdapter` in `backend/src/sources/<name>/`, then register it in `app.rs`, `Source::ALL` and the per-source `LiveSnapshots` registry.
- Backend errors: `AppError(anyhow::Error)` maps to 500; use `?` freely.
- Use SQLx compile-time macros for describable SQL. After changing checked SQL or schema, follow the metadata workflow in [backend/TESTING.md](backend/TESTING.md).
- Frontend API types come only from the generated client in `frontend/src/lib/client`.
- Components read passenger API data only through `TransitData`; they hold no request state. `TransitMap.svelte` fetches trajectories separately through `createTrajectories`, which registers their update status with `TransitData`.
- Frontend error handling stays minimal: the shared update indicator and the shared error screen only. Do not add per-view error states or UIs.

## Rules

- Prefer the simplest implementation that meets the requirement. Search for existing helpers and conventions before adding new ones.
- **Dependencies:** the bar is high. Prefer the standard library, the existing stack, or a few lines of code. Ask first, with the reason, alternatives and cost.
- Do not add functionality or architecture beyond what was requested without discussing it first.
- **Ask** before deciding anything meaningful: ambiguity, trade-offs, unexpected problems. Present options instead of silently choosing or working around.
- Make routine, low-risk decisions yourself.
- **No backward compatibility** unless asked: no shims, fallbacks or deprecated paths. Remove obsolete code and references when replacing something.
- Development databases are disposable; prefer schema updates and a reset over migration compatibility. Ask before destructive operations.
- No speculative future-proofing or needless defensive code.
- Favor maintainability over a small diff. Surface simplification opportunities, but ask before expanding scope.
- Comments explain why, not what.

## Documentation

- `docs/architecture/` describes how the system works **now**. Update the relevant doc in the same change as the code.
- Plans and scratch notes go in the gitignored `plans/` directory. When work ships, fold lasting facts into the architecture docs and delete the plan.
- Never link committed files to uncommitted ones.

## Commits and PRs

Conventional Commit subject for every commit and PR title: `<type>(<scope>): <imperative description>`, lowercase after the colon. Scopes: `backend`, `frontend`, `database`, `api`, `deps`; omit when none fits. Types: `feat` (user-visible), `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`, `chore`. Mark breaking changes with `!` and explain in the body or a `BREAKING CHANGE:` footer. Squash merge; the PR title becomes the commit, and Release Please reads `feat` and `fix`. Add a commit body only when context is needed.
