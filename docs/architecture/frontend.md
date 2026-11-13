# Frontend architecture

SvelteKit 5 app in `frontend/src/`. Testing policy lives in [frontend/TESTING.md](../../frontend/TESTING.md); design rules are in [DESIGN.md](../../DESIGN.md).

## API types

Types are generated from the OpenAPI spec with `@hey-api/openapi-ts` into `src/lib/client`. After a backend contract change, run `cargo run --bin openapi` from `backend/` and then `pnpm openapi-ts --input <exported-json>` from `frontend/`. The export needs no running feeds, frontend or database. Never hand-write API types.

## Naming

- Handwritten variables, functions, parameters, component props and frontend-owned properties use `camelCase`. Types, interfaces, classes and component bindings use `PascalCase`; fixed module-level constants use `UPPER_SNAKE_CASE`.
- Svelte components use `PascalCase.svelte`. Ordinary modules and directories use `kebab-case`, including reactive modules such as `source-preferences.svelte.ts`. Tests keep the subject's basename and append `.test.ts` or `.svelte.test.ts`.
- Preserve SvelteKit's required filenames (`+page.svelte`, `+layout.server.ts`, `hooks.server.ts`, and related framework files). Generated API code in `src/lib/client` is excluded.
- API properties and outgoing query keys retain their contract spelling: read `trip.route_id`, or bind it locally with `const { route_id: routeId } = trip`. Do not recursively recase API payloads or hand-edit generated types. Source identifiers such as `'mta_bus'` are data, not naming violations.
- ESLint enforces identifier and path conventions through `pnpm lint` and the existing hk pre-commit hook. Rename definitions and their consumers together rather than disabling naming checks for ordinary application code.

## Data loading

- The layout loads selected sources and `?at=` without waiting for API data. `+layout.svelte` creates one `TransitData` (`src/lib/data/store.svelte.ts`) and shares it via `setTransitData`; components call `getTransitData()`.
- Create `catalog`, `route`, `trip`, `stopArrivals` or `alerts` bindings during component initialization, passing getters for reactive source/ID/time keys. Read `.current` synchronously inside `$derived`. Single bindings expose a `Read<T>` union discriminated by `state`: `idle` (no key requested), `loading` and `missing` carry no fields, `ready` carries `value`, and `failed` carries `error`; collection bindings expose an array of per-key reads.
- Each binding's explicit effect owns reader interest and idempotent request acquisition. Reads never subscribe, create effects or schedule requests. `peekCatalog`, `loadedTrip` and `loadedStopArrivals` observe snapshots without acquiring requests.
- Subway service loads eagerly outside `/map`. Bus routes load on demand and batch into shared `?route_ids=` requests. Remounts within the 30-second refresh reuse data. Historical (`at`) entries live in a small LRU protected by active bindings. `decode.ts` indexes columnar service by trip and stop.
- The layout's `currentTime.value` (`getCurrentTime()` from `url-params.svelte.ts`) is the `at` argument; `undefined` means live. Narrow source-specific `data` on `data.source`.
- `/map` polls its own trajectories and makes no passenger requests until an itinerary is opened.

## Errors and freshness

Keep error handling minimal. There are exactly two error UIs:

1. The shared **"Can't update data"** indicator with Retry. It covers transport, HTTP and JSON failures while a catalog keeps the app usable. It moves into the dialog while a modal is open.
2. The shared **error screen** (`ErrorView.svelte`, used by the layout boundary and `+error.svelte`). It covers failure to load every required catalog, plus decoding and rendering errors.

Local views have no unavailable-message variants. Failed refreshes retain loaded values. Confirmed-missing entities show a short message without navigating.

Live data older than 90 seconds since its last successful fetch or source commit triggers the shared warning. Passed calls are dropped and future predictions stay visible. Empty detail views say "No predictions to show"; list rows omit prediction labels when none remain. Successful historical data never becomes a live warning.

## Persistence and offline

Persisted preferences use camelCase keys and cached `LocalStorage.current` snapshots: replace the value to mutate and persist, and let the wrapper handle cross-tab updates. Source selection uses `selectedSources` in both localStorage and its server-readable cookie; modal time formatting uses `timeFormat`. List tab keys derive from their camelCase titles, such as `pinnedTripsTab`. Old key spellings are not read or migrated.

The service worker caches the shell per deployment and catalogs in a stable `catalogs` cache. Saved catalogs are served immediately and revalidated with `If-None-Match`; replacements are posted to the open app (`replaceCatalog`). Service, alerts and trajectories are never persisted. Offline navigations fall back to the prerendered `/offline` shell, which boots the real route from saved catalogs.

## Components and routing

Each entity type has `Button.svelte` + `Modal.svelte` in `src/lib/{route,stop,trip}/`. Modal routing uses shallow URL params with the source: `?s=&src=`, `?r=&src=`, `?t=&r=&src=`. View transitions use `document.startViewTransition`.

Pinned trip rows reserve 68px of content height in every state (loading, running and missing). With list padding and borders, their 86px border box matches `calculateTripHeight` and the list skeleton. `List.svelte.test.ts` measures real browser row heights, content bounds and scroll alignment for all three sources at narrow and wide widths; estimator drift fails the component suite without a live API.

Trip-row status descriptions and countdowns share the same target call. An observed stop keeps its own countdown even when its prediction has passed (`Now` while stopped, otherwise `Due`); a missing matching call omits the countdown rather than borrowing another stop's prediction. Unassigned trains use their origin call. Otherwise, without an observed stop, the next upcoming call supplies both.

Opening a trip transfer or stop nearby-route panel shows every available choice
within its existing height-capped scroll area. Trip panels append later
departures below a non-interactive labeled divider. Only the main plus/minus
disclosure remains; there are no secondary expansion controls.

Opening the transfer resource requests service for every candidate route
discovered from the selected sources' same-stop and nearby-stop catalogs, not
just routes already in memory. Rows update as catalog and service responses
arrive, so the list may be partial during loading. Both panels use a consistent
44px leading lane, including loading and empty-state messages and the
later-departure group. The trip transfer rail
aligns with the stop timeline's 22px centerline; nearby-route panels retain their
outer inset. The later-departure label uses sentence case and a subtle trailing
rule instead of a full-width section border.

The Stops page searches the selected source. Exact route IDs filter to that route's stops (in stop-sequence order for subway routes). Otherwise, name matches appear first, followed by an exact stop-ID match if it is not already included; an ID never hides matching names.

On initial load and client-side navigation into Stops, the search field receives focus only when the primary pointer supports hover and fine pointing (`(hover: hover) and (pointer: fine)`). Touch-first devices keep normal page focus so visiting Stops does not open the on-screen keyboard.

Reactive state files use `.svelte.ts`. Current time is scoped per layout through `createCurrentTime`, `setCurrentTime` and `getCurrentTime`; retrieve the context during component initialization.

## Map

Trajectory rendering is described in [trajectories](trajectories.md).
