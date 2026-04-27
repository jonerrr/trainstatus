import { page } from '$app/state';

import type { Source } from '$lib/client';

import maplibregl from 'maplibre-gl';

// Layer metadata
export const layer_data = {
	route: {
		name: 'Routes'
	},
	stop: {
		name: 'Stops'
	},
	trip: {
		name: 'Trips'
	}
} as const;

type LayerKey = keyof typeof layer_data;

export type FilterFieldType = 'boolean' | 'enum' | 'string' | 'number';

export interface FilterFieldDef {
	type: FilterFieldType;
	label: string;
	options?: readonly string[];
	min?: number;
	max?: number;
}

// Stop filter definitions by source
const stop_filter_defs = {
	mta_subway: {
		ada: {
			type: 'boolean',
			label: 'ADA Accessible'
		} as FilterFieldDef,
		borough: {
			type: 'enum',
			label: 'Borough',
			options: ['brooklyn', 'queens', 'bronx', 'staten_island', 'manhattan'] as const
		} as FilterFieldDef,
		north_headsign: {
			type: 'string',
			label: 'North Headsign'
		} as FilterFieldDef,
		south_headsign: {
			type: 'string',
			label: 'South Headsign'
		} as FilterFieldDef
	} as const,
	mta_bus: {
		direction: {
			type: 'enum',
			label: 'Direction',
			options: ['s_w', 's', 's_e', 'e', 'w', 'n_e', 'n_w', 'n', 'unknown'] as const
		} as FilterFieldDef
	} as const,
	njt_bus: {
		// nothing yet to filter
	} as const
} satisfies Record<Source, Record<string, FilterFieldDef>>;

type StopFilterDefs = typeof stop_filter_defs;

// Route filter definitions by source (minimal for MVP)
const route_filter_defs = {
	mta_subway: {} as const,
	mta_bus: {
		shuttle: {
			type: 'boolean',
			label: 'Shuttle'
		} as FilterFieldDef
	} as const,
	njt_bus: {} as const
} satisfies Record<Source, Record<string, FilterFieldDef>>;

type RouteFilterDefs = typeof route_filter_defs;

// Trip filter definitions (none for MVP - trips have minimal source-specific data)
const trip_filter_defs = {
	mta_subway: {} as const,
	mta_bus: {} as const,
	njt_bus: {} as const
} satisfies Record<Source, Record<string, FilterFieldDef>>;

type TripFilterDefs = typeof trip_filter_defs;

// All layer filter definitions combined
const layer_filter_defs = {
	route: route_filter_defs,
	stop: stop_filter_defs,
	trip: trip_filter_defs
} as const;

export function getFilterDefsForLayer(
	layer: LayerKey
): Record<Source, Record<string, FilterFieldDef>> {
	return layer_filter_defs[layer];
}

// ============================================================================
// FILTER STATE AND EXPRESSIONS
// ============================================================================

export type FilterValue = boolean | string | string[] | [number, number] | undefined;

export class MapFilters {
	// Source selection state
	sources = $state<Source[]>(page.data.selected_sources ?? []);

	// Property filter state: Record<Source, Record<PropertyName, FilterValue>>
	stop_filters = $state<Record<Source, Record<string, FilterValue>>>({
		mta_subway: {},
		mta_bus: {},
		njt_bus: {}
	});

	route_filters = $state<Record<Source, Record<string, FilterValue>>>({
		mta_subway: {},
		mta_bus: {},
		njt_bus: {}
	});

	trip_filters = $state<Record<Source, Record<string, FilterValue>>>({
		mta_subway: {},
		mta_bus: {},
		njt_bus: {}
	});

	// Layers enabled/disabled
	layers = $state(
		Object.fromEntries(Object.keys(layer_data).map((layer) => [layer, true])) as Record<
			LayerKey,
			boolean
		>
	);

	// Base source filter expression
	#source_filter: maplibregl.ExpressionSpecification = $derived([
		'in',
		['get', 'source'],
		['literal', this.sources]
	]);

	/**
	 * Build a MapLibre filter expression for a single property filter.
	 * Returns null if the filter should be skipped (e.g., undefined value or all options selected).
	 */
	#buildPropertyFilter(
		property: string,
		value: FilterValue,
		fieldDef: FilterFieldDef
	): maplibregl.ExpressionSpecification | null {
		if (value === undefined) return null;

		switch (fieldDef.type) {
			case 'boolean':
				if (typeof value === 'boolean') {
					return ['==', ['get', property], value];
				}
				return null;

			case 'enum':
				if (Array.isArray(value) && value.length > 0 && typeof value[0] === 'string') {
					// Multi-select enum: if all options selected, skip filter
					if (fieldDef.options && value.length === fieldDef.options.length) {
						return null;
					}
					return ['in', ['get', property], ['literal', value]];
				}
				// Single value enum
				if (typeof value === 'string') {
					return ['==', ['get', property], value];
				}
				return null;

			case 'number':
				if (
					Array.isArray(value) &&
					value.length === 2 &&
					typeof value[0] === 'number' &&
					typeof value[1] === 'number'
				) {
					const [min, max] = value;
					return ['all', ['>=', ['get', property], min], ['<=', ['get', property], max]];
				}
				return null;

			case 'string':
				// For MVP, treat string as exact match (not regex)
				if (typeof value === 'string' && value.length > 0) {
					return ['==', ['get', property], value];
				}
				return null;

			default:
				return null;
		}
	}

	/**
	 * Build combined filter expression for a layer, combining source filter with all property filters.
	 */
	#buildLayerFilter(
		layerKey: LayerKey,
		propertyFilters: Record<Source, Record<string, FilterValue>>
	): maplibregl.FilterSpecification {
		const filterExpressions: maplibregl.ExpressionSpecification[] = [this.#source_filter];

		const filterDefs = layer_filter_defs[layerKey];

		// For each active source, add its property filters
		for (const source of this.sources) {
			const sourceFilters = propertyFilters[source];
			const sourceDefs = filterDefs[source];

			if (!sourceDefs) continue;

			for (const [property, value] of Object.entries(sourceFilters)) {
				const fieldDef = sourceDefs[property as keyof typeof sourceDefs] as
					| FilterFieldDef
					| undefined;
				if (!fieldDef) continue;

				const propFilter = this.#buildPropertyFilter(property, value, fieldDef);
				if (propFilter) {
					filterExpressions.push(propFilter);
				}
			}
		}

		if (filterExpressions.length === 1) {
			return this.#source_filter;
		}

		return ['all', ...filterExpressions];
	}

	// Final filter expressions for each layer, automatically updated when sources or property filters change
	route: maplibregl.FilterSpecification = $derived.by(() => {
		return this.#buildLayerFilter('route', this.route_filters);
	});

	stop: maplibregl.FilterSpecification = $derived.by(() => {
		return this.#buildLayerFilter('stop', this.stop_filters);
	});

	trip: maplibregl.FilterSpecification = $derived.by(() => {
		return this.#buildLayerFilter('trip', this.trip_filters);
	});
}
