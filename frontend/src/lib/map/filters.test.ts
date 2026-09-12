import { describe, expect, it, vi } from 'vitest';

import { MapFilters, countActiveFilters } from './filters.svelte';

vi.mock('$app/state', () => ({
	page: { data: { selected_sources: ['mta_subway', 'mta_bus'] } }
}));

describe('countActiveFilters', () => {
	it('counts source, layer, and property changes', () => {
		expect(
			countActiveFilters({
				initialSources: ['mta_subway', 'mta_bus'],
				sources: ['mta_subway'],
				layers: { route: true, stop: false, trip: true },
				propertyFilters: [
					{
						layer: 'stop',
						filters: { mta_subway: { ada: true }, mta_bus: {}, njt_bus: {} }
					},
					{ layer: 'route', filters: { mta_subway: {}, mta_bus: {}, njt_bus: {} } },
					{ layer: 'trip', filters: { mta_subway: {}, mta_bus: {}, njt_bus: {} } }
				]
			})
		).toBe(3);
	});

	it('ignores empty property values and equivalent source ordering', () => {
		expect(
			countActiveFilters({
				initialSources: ['mta_bus', 'mta_subway'],
				sources: ['mta_subway', 'mta_bus'],
				layers: { route: true, stop: true, trip: true },
				propertyFilters: [
					{
						layer: 'stop',
						filters: {
							mta_subway: {
								borough: ['brooklyn', 'queens', 'bronx', 'staten_island', 'manhattan'],
								north_headsign: ''
							},
							mta_bus: {},
							njt_bus: {}
						}
					}
				]
			})
		).toBe(0);
	});

	it('resets sources, layers, and property filters together', () => {
		const filters = new MapFilters();
		filters.sources = ['mta_bus'];
		filters.layers.stop = false;
		filters.stop_filters.mta_bus.direction = 'N';

		filters.reset();

		expect(filters.sources).toEqual(['mta_subway', 'mta_bus']);
		expect(filters.layers).toEqual({ route: true, stop: true, trip: true });
		expect(filters.stop_filters).toEqual({ mta_subway: {}, mta_bus: {}, njt_bus: {} });
		expect(filters.activeFilterCount).toBe(0);
	});

	it('toggles sources on and off', () => {
		const filters = new MapFilters();
		expect(filters.isSourceEnabled('mta_subway')).toBe(true);

		filters.toggleSource('mta_subway');
		expect(filters.isSourceEnabled('mta_subway')).toBe(false);
		expect(filters.sources).toEqual(['mta_bus']);

		filters.toggleSource('mta_subway');
		expect(filters.isSourceEnabled('mta_subway')).toBe(true);
		expect(filters.sources).toEqual(['mta_bus', 'mta_subway']);
	});
});
