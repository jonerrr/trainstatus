import { page } from '$app/state';

import type { Source } from '$lib/client';
import { COMPASS_DIRECTION_OPTIONS } from '$lib/compassDirections';
import { all_sources } from '$lib/resources/index.svelte';

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

export type LayerKey = keyof typeof layer_data;

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
			options: COMPASS_DIRECTION_OPTIONS
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

type PropertyFilterMap = Partial<Record<Source, Record<string, FilterValue>>>;

function hasFilterValue(value: FilterValue, fieldDef?: FilterFieldDef) {
	if (value === undefined) return false;
	if (typeof value === 'string') return value.length > 0;
	if (Array.isArray(value)) {
		if (fieldDef?.type === 'enum' && fieldDef.options && value.length === fieldDef.options.length)
			return false;
		return value.length > 0;
	}
	return true;
}

export function countActiveFilters({
	initialSources,
	sources,
	layers,
	propertyFilters
}: {
	initialSources: readonly Source[];
	sources: readonly Source[];
	layers: Record<LayerKey, boolean>;
	propertyFilters: readonly { layer: LayerKey; filters: PropertyFilterMap }[];
}) {
	const sameSources =
		initialSources.length === sources.length &&
		initialSources.every((source) => sources.includes(source));
	let count = sameSources ? 0 : 1;
	count += Object.values(layers).filter((enabled) => !enabled).length;

	for (const group of propertyFilters) {
		const groupDefinitions = layer_filter_defs[group.layer] as Record<
			Source,
			Record<string, FilterFieldDef>
		>;
		for (const [source, sourceFilters] of Object.entries(group.filters)) {
			for (const [property, value] of Object.entries(sourceFilters ?? {})) {
				const fieldDef = groupDefinitions[source as Source]?.[property];
				if (hasFilterValue(value, fieldDef)) count += 1;
			}
		}
	}

	return count;
}

function emptyPropertyFilters(): Record<Source, Record<string, FilterValue>> {
	return Object.fromEntries(all_sources.map((source) => [source, {}])) as Record<
		Source,
		Record<string, FilterValue>
	>;
}

export class MapFilters {
	readonly initialSources = [...(page.data.selected_sources ?? [])];

	// Source selection state
	sources = $state<Source[]>([...this.initialSources]);

	// Property filter state: Record<Source, Record<PropertyName, FilterValue>>
	stop_filters = $state<Record<Source, Record<string, FilterValue>>>(emptyPropertyFilters());

	route_filters = $state<Record<Source, Record<string, FilterValue>>>(emptyPropertyFilters());

	trip_filters = $state<Record<Source, Record<string, FilterValue>>>(emptyPropertyFilters());

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

	activeFilterCount = $derived(
		countActiveFilters({
			initialSources: this.initialSources,
			sources: this.sources,
			layers: this.layers,
			propertyFilters: [
				{ layer: 'stop', filters: this.stop_filters },
				{ layer: 'route', filters: this.route_filters },
				{ layer: 'trip', filters: this.trip_filters }
			]
		})
	);

	isSourceEnabled(source: Source): boolean {
		return this.sources.includes(source);
	}

	toggleSource(source: Source) {
		this.sources = this.isSourceEnabled(source)
			? this.sources.filter((s) => s !== source)
			: [...this.sources, source];
	}

	reset() {
		this.sources = [...this.initialSources];
		this.layers = { route: true, stop: true, trip: true };
		this.stop_filters = emptyPropertyFilters();
		this.route_filters = emptyPropertyFilters();
		this.trip_filters = emptyPropertyFilters();
	}
}
