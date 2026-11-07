# Frontend testing

Run from `frontend/`. These tasks do not start or stop the API or Martin.

| Task                        | Purpose                                                                  |
| --------------------------- | ------------------------------------------------------------------------ |
| `mise run test`             | Unit and component tests. Deterministic; the API can be stopped.         |
| `mise run test:unit`        | Vitest node tests (`src/**/*.test.ts`)                                   |
| `mise run test:component`   | Vitest browser tests (`src/**/*.svelte.test.ts`)                         |
| `mise run test:integration` | Live chart itinerary audit. Requires the API at `http://127.0.0.1:3055`. |
| `mise run test:e2e`         | Playwright against the preview app. Requires the same API.               |

`test:integration` and `test:e2e` fail closed when `:3055` refuses the connection. Martin is assumed at `http://127.0.0.1:3000`; preview proxies `/martin` there, and map E2E needs `/martin/style/dark-matter.json`. E2E runs desktop Chromium, desktop Firefox and Chromium mobile touch. Install browsers with `pnpm test:install`.

For map E2E on a machine with a GPU, run `pnpm test:e2e --headed --workers=1`. Headless Chromium uses SwiftShader, which can make the animated map exceed the 90-second limit.

## Principles

- Test observable behavior, not implementation: no Tailwind class assertions, no accessor internals. Badge tests assert measured text bounds; navigation asserts `aria-current` and the URL.
- Use the real app with synthetic fixtures. Do not intercept HTTP; components get data through `TransitData` with an injected fetcher.
- Do not extract page logic just to unit-test it. Leave unfinished behavior untested rather than freezing a placeholder.
- Add a regression test for every bug fix at the lowest layer that catches it.
- Control time with Vitest (`Date.now`) or by mocking `getCurrentTime`, never real timers or the 15-second native timeouts.

## Layers

- **Unit:** pure functions and request shaping. No browser or network. `chartFixture` holds deterministic chart cases, including loops live data may never produce.
- **Component:** browser tests for modals, focus, chart SVG, map panels, badges and real MapLibre/deck.gl rendering with synthetic data. Route-level tests mount the real layout and pages (`routes/*.svelte.test.ts`) to cover cold loads, shared links, pins, errors and stops.
- **Integration:** the live chart itinerary audit only. It runs `buildChartData` on real payloads and checks itinerary invariants (trip membership, order, surviving visits), never fixed live ids.
- **E2E:** user flows against the preview app. Discover ids from the API through the preview proxy. Assert navigation, persistence and visible structure; check map style availability rather than pixels.

## Shared test support

- [src/lib/test/page.svelte.ts](src/lib/test/page.svelte.ts) owns `page`, `goto` and `reset()`. The client setup mocks only `$app/state` and `$app/navigation` from it. Node tests keep their own `$app/state` mocks.
- Components reading API data render through [TransitProvider.svelte](src/lib/test/TransitProvider.svelte) with `fixtureTransitData` from [transit-fixture.ts](src/lib/test/transit-fixture.ts).
- Map rendering uses [MapHarness.svelte](src/lib/test/MapHarness.svelte) (local style, real WebGL, MapLibre workers loaded as blobs). `map-commands.ts` drives real gestures through Playwright.
- Integration uses [tests/support/api.ts](tests/support/api.ts); `readJson` throws on non-OK HTTP and network errors include the URL.
- Playwright imports `test` and `expect` from [e2e/fixtures.ts](e2e/fixtures.ts). It clears storage, blocks the service worker and waits for the navbar, not network idle. Only `offline.e2e.ts` enables the service worker.

## Contracts worth knowing

- Failures follow the two-UI error policy in [architecture/frontend.md](../docs/architecture/frontend.md): tests cover the shared indicator with Retry, retained values on refresh failure, and the shared error screen, not per-view variants.
- When shared links cold-load, assert modal content and the home readers together so Vitest's unhandled-error reporting catches `effect_update_depth_exceeded`.
- Trajectory motion policy is tested in backend fixtures; the frontend tests decoded geometry and the real marker renderer. See [trajectories](../docs/architecture/trajectories.md).
