# Frontend testing

Run commands from `frontend/`, prefixed with `mise exec --`. These tasks do not start or stop the API or Martin.

| Task                        | Purpose                                                                  |
| --------------------------- | ------------------------------------------------------------------------ |
| `mise run test`             | Unit and component tests. Deterministic. The API can be stopped.         |
| `mise run test:unit`        | Colocated Vitest node tests (`src/**/*.test.ts`)                         |
| `mise run test:component`   | Vitest browser tests (`src/**/*.svelte.test.ts`)                         |
| `mise run test:integration` | Live chart itinerary audit. Requires the API at `http://127.0.0.1:3055`. |
| `mise run test:e2e`         | Playwright against the preview app. Requires the same API.               |

`test` selects the Vitest server and client projects, so it does not open the network. `test:integration` and `test:e2e` fail closed when `:3055` refuses the connection. Martin is assumed up at `http://127.0.0.1:3000`; preview proxies `/martin` there. Map E2E opens `/map` and requires Martin to serve `/martin/style/dark-matter.json`. It runs in desktop Chromium, desktop Firefox, and Chromium mobile touch. Install the browsers with `mise exec -- pnpm test:install`.

## Layers

- **Unit.** Pure functions and request-shaping hooks. No browser, no network. Synthetic inputs stay next to the behavior. `chartFixture` remains the deterministic chart cases, including loops that live service may never produce. `hooks.server.test.ts` stays here: it checks how the hook rewrites a request, using an echo `fetch`, and does not stand in for the API.
- **Component.** Browser tests for modal history, focus, chart SVG rendering, map panels and route badges, and real MapLibre/deck.gl rendering with synthetic data. They share one `$app` stand-in. Chart-only dependencies stay in the chart test. They do not intercept HTTP.
- **Integration.** The live chart itinerary audit only. It runs `buildChartData` on real payloads from `:3055` and fails immediately if that server is down. Connection refused, non-OK HTTP, a payload that breaks while building, and an itinerary that reverses or drops stops fail the run. Expected trip membership and visits are derived from the inputs before building, so dropping an entire active trip also fails. Zero trips, or zero groups with future predictions, passes. Empty stops fail when any input trip has future visits that cannot be resolved.
- **E2E.** User flows against the preview app and the same API. Assert navigation, persistence, and visible structure. Discover stop, route, and trip ids from the API through the preview proxy. An empty trip snapshot passes the trip cold-load. Map coverage asserts camera movement through filters, layer toggles, modal dismissal, navigation, and touch cancellation. It checks style availability rather than pixel-matching changing live tiles. Alert rows and chart paths are not asserted.

Trajectory polling has deterministic tests using an injected fetcher and real synthetic Arrow payloads, covering cancellation, late responses, retained data on failure, empty replacements, and historical requests. The query owner has deterministic unit coverage for generations, route coverage, waiter outcomes, retries, and destruction. `resources.svelte.test.ts` uses injected fetchers through `ResourceHarness.svelte` to cover the Svelte lifecycle, route reference counting, retained-data UI, indicator geometry, and direct retry. It also scrolls the actual virtualized bus list to verify that replacing rows on the same routes does not restart completed, pending, or failed requests, and that unmounting releases subscriptions. Historical requests retry failures but stop polling after a successful update. Non-OK and network failures count as failed live updates and retain existing data. Static routes and stops load normally and are not tracked or retried by the indicator. There are no HTTP intercepts in integration or e2e.

## Fixtures

Component tests share [src/lib/test/page.svelte.ts](src/lib/test/page.svelte.ts). It owns `page` (`url`, `state`, `data`), `goto`, and `reset()`. The Vitest client setup mocks only `$app/state` and `$app/navigation` from that module and calls `reset()` before each test. Node projects do not load that setup. Node tests, including `src/lib/map/filters.test.ts`, keep their own `$app/state` mocks.

`setFixture` stays next to the chart tests. It writes the shared page plus the chart resource maps. The chart test keeps its mocks for `url_params` and the trip, stop-time, and alert contexts.

Map rendering uses `src/lib/test/MapHarness.svelte`: a local background-only style, real WebGL layers, and the installed MapLibre worker modules loaded as blobs to avoid Vitest's window-only module instrumentation. No backend requests or HTTP intercepts are involved. `map-commands.ts` drives genuine mouse gestures through Playwright. Arrow fixtures live beside trajectory tests. Badge tests assert measured text bounds instead of Tailwind classes.

Integration uses [tests/support/api.ts](tests/support/api.ts). The origin is `http://127.0.0.1:3055`. `readJson` throws on non-OK HTTP. Network failures include the request URL and preserve the original error as their cause.

Playwright tests import `test` and `expect` from [e2e/fixtures.ts](e2e/fixtures.ts). The fixture clears storage and cookies, blocks the service worker (already set in the Playwright config), and waits until the navbar is visible. It does not wait for network idle. The cold-load test waits for the dialog itself. Settings, pins, and `?at=` use this isolated context.

## Assertions

Navbar links set `aria-current="page"` when the link path equals `page.url.pathname`, and omit the attribute otherwise. That check ignores `?at=`. End-to-end navigation checks the navbar attribute and the URL. It does not check Tailwind classes.

Unit and component tests keep synthetic fixtures. The integration audit checks itinerary invariants (trip membership, order, and surviving visits), not fixed live ids. Stops E2E checks mounted route results and scrolls to the final stop instead of equating virtualized DOM row counts with total matches.

The stops filter in `src/routes/stops/+page.svelte` is not extracted just so it can be unit-tested. Name search is unit-tested through `StopSearch`. Stop-id and route-id filtering are end-to-end. The no-match fallback stays untested because that behavior is still unfinished.

## Follow-up

This suite does not extract page logic just to unit-test it. When a behavior is still unfinished, leave it untested and point a TODO at this file rather than freezing the current fallback.
