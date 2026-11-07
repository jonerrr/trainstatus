# Trajectories

Motion validity belongs to the backend, which builds and reconciles trajectory plans. The browser evaluates those plans and places render units on the supplied geometry.

## Wire format and evaluation

- Arrow carries one WGS84 polyline plus EPSG:6538 cumulative meter distances per distinct shape, one float64 cubic motion plan per trip, and unit metadata with distance offsets. Shared definitions appear on their first unit row. The browser resolves references before rendering and releases the raw table.
- Both evaluators use normalized `[t0,t1,a,b,c,d]` intervals. Position and velocity are continuous at ordinary joins; holds and outer endpoints have zero speed. Shared fixed vectors check agreement within `1e-7`.
- Markers follow the source polyline every frame. Cars derive positions independently from one head distance. Orientation comes from nearby shape points.

## Backend derivation

- Prediction-only updates evaluate the previous compatible plan at the new derivation time. Feasible replacements preserve distance and velocity; delayed targets brake to a hold. Impossible map arrivals are retimed later without changing passenger ETAs. Different shape identities reset continuity. If rolling ETAs put an estimate more than the backward-gap allowance behind the prior plan, continuity is discarded with a `BackwardGap` reason.
- Bus sources use the newest accepted observation, age and cross-track checks, directed branch candidates and recent distinct fixes. A fix contradicting a previous synthetic target is accepted immediately.
- Stop geometry follows `stop_sequence`, including repeats and colocated stops. Ambiguous projections are omitted.
- GPS estimates compensate observation latency before bounded speed decay. A sole fix remains a valid stationary plan.

## Frontend display

The browser retains displayed distance and speed per source/trip. Replacements start from the displayed state advanced to receipt time, accelerating, braking and holding without reversing. GPS targets carry an absolute correction deadline from their first source commit; infeasible or expired corrections resynchronize. Hidden gaps over 15 s, changed geometry/run identity and historical-time changes reset display state. Historical rendering bypasses the live controller and uses recorded observation times.

## Limits

Conservative defaults validated on synthetic cases only. They have **not** been tuned from production captures or a ride-along.

| Policy                                         | Default                                                          |
| ---------------------------------------------- | ---------------------------------------------------------------- |
| Accepted live GPS age / cross-track offset     | 120 s / 100 m                                                    |
| Projection alternatives / corner consolidation | 5 m / 5 m                                                        |
| Bus speed estimate cap                         | 30 m/s                                                           |
| GPS extrapolation                              | Pair: at most 120 s / 1,000 m. Single-fix fallback: 30 s / 300 m |
| Retained live report history                   | At most 8 observations per run within 240 s                      |
| GPS correction deadline                        | First source commit + 10 s                                       |
| Display bus speed / accel / braking            | 30 m/s / 1 m/s² / 1.5 m/s²                                       |
| Display subway speed / accel / braking         | 25 m/s / 0.8 m/s² / 1 m/s²                                       |
| Correction horizon / hidden-gap reset          | 8 s / 15 s                                                       |

The subway ceiling rounds the 55 mph not-to-exceed speed from the [MTA speed and capacity review](https://www.mta.info/document/13046). Before treating limits as tuned, replay consecutive provider captures, profile full-network geometry and mobile frames, and compare GPS targets with later observations and a bus ride-along. Full-network heap/RSS and mobile frame cost remain unmeasured.

## Where it is tested

Backend, from `backend/`: `SQLX_OFFLINE=true cargo test --locked --features fixture-capture --lib trajectory::`

- [Long replays](../../backend/src/trajectory/tests/long_replay.rs) (inputs in [long-replays.json](../../backend/tests/fixtures/trajectory/long-replays.json)): handoff continuity, forward motion, speed bounds, prediction-only NJT progress.
- [GPS replays](../../backend/src/trajectory/tests/gps.rs) and [continuity tests](../../backend/src/trajectory/tests/continuity.rs): delayed and sparse fixes, holds, delayed arrivals.

Frontend, from `frontend/`: `pnpm test:unit src/lib/map` and `pnpm test:component src/lib/map`

- [Geometry tests](../../frontend/src/lib/map/trajectory.test.ts): decoded coordinates, bearings, car offsets, backend polynomial vectors.
- [Marker tests](../../frontend/src/lib/map/TripMarkers.svelte.test.ts): production MapLibre/deck.gl renderer, same-ID refreshes, transparent padding, historical time.
- [Lifecycle tests](../../frontend/src/lib/map/rendering.svelte.test.ts) and [polling tests](../../frontend/src/lib/map/trajectories.test.ts).

These suites establish reproducible behavior, not live fleet accuracy or mobile frame performance.
