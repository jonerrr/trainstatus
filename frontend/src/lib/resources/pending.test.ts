import { expect, it } from 'vitest';

import { awaitingRoute, awaitingRows, unavailableRows } from './pending';

it('shows a skeleton only before the first settled result', () => {
	expect(awaitingRows(undefined, 0)).toBe(true);
	expect(awaitingRows({ available: false, error: null }, 0)).toBe(true);
	expect(awaitingRows({ available: false, error: new Error('down') }, 0)).toBe(false);
	expect(awaitingRows({ available: false, error: null }, 2)).toBe(false);
	expect(awaitingRows({ available: true, error: null }, 0)).toBe(false);
});

it('keeps a monitored route loading until that route is covered', () => {
	const resource = {
		available: true,
		error: null,
		coversRoute: (route: string) => route === 'A'
	};
	expect(awaitingRows(resource, 0, 'B')).toBe(true);
	expect(awaitingRows(resource, 0, 'A')).toBe(false);
	expect(awaitingRows(resource, 3, 'B')).toBe(false);
});

it('marks an empty failed resource unavailable', () => {
	expect(unavailableRows(undefined, 0)).toBe(false);
	expect(unavailableRows({ available: false, error: null }, 0)).toBe(false);
	expect(unavailableRows({ available: false, error: new Error('down') }, 0)).toBe(true);
	expect(unavailableRows({ available: true, error: new Error('down') }, 2)).toBe(false);
});

it('treats a missing monitored route as still loading', () => {
	expect(awaitingRoute(undefined, 'A')).toBe(true);
	expect(awaitingRoute({ error: new Error('down'), coversRoute: () => false }, 'A')).toBe(false);
	expect(awaitingRoute({ error: null, coversRoute: (route) => route === 'A' }, 'B')).toBe(true);
	expect(awaitingRoute({ error: null, coversRoute: () => true }, 'A')).toBe(false);
});
