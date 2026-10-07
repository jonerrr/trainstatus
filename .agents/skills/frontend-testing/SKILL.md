---
name: frontend-testing
description: Use when adding, changing, reviewing, or running Train Status frontend tests.
---

# Frontend testing

Read [frontend/TESTING.md](../../../frontend/TESTING.md) for the authoritative placement, command, fixture, and assertion policy. Inspect `frontend/mise.toml` for current commands before running tests.

Pick the smallest layer whose owner holds the contract. Unit tests cover pure functions and request shaping with synthetic inputs. Component tests cover isolated UI through the shared `$app` page. Integration is the live chart itinerary audit against the API on `:3055`. End-to-end covers user flows through preview against that same API. Do not add an HTTP intercept, and do not start the API or Martin from a test task.

Use [frontend/src/lib/test/page.svelte.ts](../../../frontend/src/lib/test/page.svelte.ts) for component `$app` state, and [frontend/tests/support/api.ts](../../../frontend/tests/support/api.ts) for integration reads. Do not extract page logic only to unit-test it. Name the contract protected and the commands actually run.
