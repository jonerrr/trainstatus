# Frontend architecture

SvelteKit 5 app in `frontend/src/`. Testing policy lives in [frontend/TESTING.md](../../frontend/TESTING.md); design rules are in [DESIGN.md](../../DESIGN.md).

## API types

Types are generated from the OpenAPI spec with `@hey-api/openapi-ts` into `src/lib/client`. After a backend contract change, run `cargo run --bin openapi` from `backend/` and then `pnpm openapi-ts --input <exported-json>` from `frontend/`. The export needs no running feeds, frontend or database. Never hand-write API types.

## Data loading

- The layout loads selected sources and `?at=` without waiting for API data. `+layout.svelte` creates one `TransitData` (`src/lib/transit/store.svelte.ts`) and shares it via `setTransitData`; components call `getTransitData()`.
- Create `catalog`, `route`, `trip`, `stopArrivals` or `alerts` bindings during component initialization, passing getters for reactive source/ID/time keys. Read `.current` synchronously inside `$derived`. Single bindings expose `{ value, loading, error }`; collection bindings expose an array of per-key reads.
- Each binding's explicit effect owns reader interest and idempotent request acquisition. Reads never subscribe, create effects or schedule requests. `peekCatalog`, `loadedTrip` and `loadedStopArrivals` observe snapshots without acquiring requests.
- Subway service loads eagerly outside `/map`. Bus routes load on demand and batch into shared `?route_ids=` requests. Remounts within the 30-second refresh reuse data. Historical (`at`) entries live in a small LRU protected by active bindings. `decode.ts` indexes columnar service by trip and stop.
- The layout's `current_time.value` (`getCurrentTime()` from `url_params.svelte.ts`) is the `at` argument; `undefined` means live. Narrow source-specific `data` on `data.source`.
- `/map` polls its own trajectories and makes no passenger requests until an itinerary is opened.

## Errors and freshness

Keep error handling minimal. There are exactly two error UIs:

1. The shared **"Can't update data"** indicator with Retry. It covers transport, HTTP and JSON failures while a catalog keeps the app usable. It moves into the dialog while a modal is open.
2. The shared **error screen** (`ErrorView.svelte`, used by the layout boundary and `+error.svelte`). It covers failure to load every required catalog, plus decoding and rendering errors.

Local views have no unavailable-message variants. Failed refreshes retain loaded values. Confirmed-missing entities show a short message without navigating.

Live data older than 90 seconds since its last successful fetch or source commit triggers the shared warning. Passed calls are dropped and future predictions stay visible. Empty detail views say "No predictions to show"; list rows omit prediction labels when none remain. Successful historical data never becomes a live warning.

## Persistence and offline

Persisted preferences use cached `LocalStorage.current` snapshots: replace the value to mutate and persist, and let the wrapper handle cross-tab updates.

The service worker caches the shell per deployment and catalogs in a stable `catalogs` cache. Saved catalogs are served immediately and revalidated with `If-None-Match`; replacements are posted to the open app (`replaceCatalog`). Service, alerts and trajectories are never persisted. Offline navigations fall back to the prerendered `/offline` shell, which boots the real route from saved catalogs.

## Components and routing

Each entity type has `Button.svelte` + `Modal.svelte` in `src/lib/{Route,Stop,Trip}/`. Modal routing uses shallow URL params with the source: `?s=&src=`, `?r=&src=`, `?t=&r=&src=`. View transitions use `document.startViewTransition`.

Reactive state files use `.svelte.ts`. Current time is scoped per layout through `createCurrentTime`, `setCurrentTime` and `getCurrentTime`; retrieve the context during component initialization.

## Map

Trajectory rendering is described in [trajectories](trajectories.md).
