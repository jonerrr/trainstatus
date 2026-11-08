import { page } from '$app/state';

import type { Source } from '#lib/client/index.js';
import { COMPASS_DIRECTION_OPTIONS } from '#lib/compassDirections.js';
import { all_sources } from '#lib/sources.js';

import type * as maplibregl from 'maplibre-gl';

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

export type PropertyLayerKey = Exclude<LayerKey, 'trip'>;

export type FilterFieldDef =
	| { type: 'boolean' | 'string'; label: string }
	| { type: 'enum'; label: string; options: readonly string[] };

// Stop filter definitions by source
const stop_filter_defs = {
	mta_subway: {
		north_headsign: {
			type: 'string',
			label: 'North Headsign'
		},
		south_headsign: {
			type: 'string',
			label: 'South Headsign'
		}
	} as const,
	mta_bus: {
		direction: {
			type: 'enum',
			label: 'Direction',
			options: COMPASS_DIRECTION_OPTIONS
		}
	} as const,
	njt_bus: {
		// nothing yet to filter
	} as const
} satisfies Record<Source, Record<string, FilterFieldDef>>;

// Route filter definitions by source
const route_filter_defs = {
	mta_subway: {} as const,
	mta_bus: {
		shuttle: {
			type: 'boolean',
			label: 'Shuttle'
		}
	} as const,
	njt_bus: {} as const
} satisfies Record<Source, Record<string, FilterFieldDef>>;

// All layer filter definitions combined
const layer_filter_defs = {
	route: route_filter_defs,
	stop: stop_filter_defs
} as const;

export function getFilterDefsForLayer(
	layer: PropertyLayerKey
): Record<Source, Record<string, FilterFieldDef>> {
	return layer_filter_defs[layer];
}

// ============================================================================
// FILTER STATE AND EXPRESSIONS
// ============================================================================

export type FilterValue = boolean | string | string[] | undefined;

type PropertyFilterMap = Record<Source, Record<string, FilterValue>>;

function hasFilterValue(value: FilterValue, fieldDef: FilterFieldDef) {
	if (value === undefined) return false;
	if (typeof value === 'string') return value.length > 0;
	if (Array.isArray(value)) {
		if (fieldDef.type === 'enum' && value.length === fieldDef.options.length) return false;
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
	propertyFilters: readonly { layer: PropertyLayerKey; filters: PropertyFilterMap }[];
}) {
	const sameSources =
		initialSources.length === sources.length &&
		initialSources.every((source) => sources.includes(source));
	let count = sameSources ? 0 : 1;
	count += Object.values(layers).filter((enabled) => !enabled).length;

	for (const group of propertyFilters) {
		const groupDefinitions = getFilterDefsForLayer(group.layer);
		for (const [source, sourceFilters] of Object.entries(group.filters)) {
			for (const [property, value] of Object.entries(sourceFilters)) {
				const fieldDef = groupDefinitions[source as Source][property];
				if (!fieldDef) continue;
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
	readonly initialSources = [...page.data.selected_sources];

	// Source selection state
	sources = $state<Source[]>([...this.initialSources]);

	// Property filter state: Record<Source, Record<PropertyName, FilterValue>>
	stop_filters = $state<Record<Source, Record<string, FilterValue>>>(emptyPropertyFilters());

	route_filters = $state<Record<Source, Record<string, FilterValue>>>(emptyPropertyFilters());

	// Layers enabled/disabled
	layers = $state({ route: true, stop: true, trip: true });

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
				if (Array.isArray(value) && value.length > 0) {
					// Multi-select enum: if all options selected, skip filter
					if (value.length === fieldDef.options.length) {
						return null;
					}
					return ['in', ['get', property], ['literal', value]];
				}
				return null;

			case 'string':
				if (typeof value === 'string' && value.length > 0) {
					return ['==', ['get', property], value];
				}
				return null;
		}
	}

	/**
	 * Build a per-source filter for a layer.
	 *
	 * Property filters are ANDed only with the source that defines them, then
	 * those clauses are ORed. A single `all` across every active source would
	 * hide features that do not carry another source's properties — a subway
	 * headsign filter would drop every bus stop.
	 */
	#buildLayerFilter(
		layerKey: PropertyLayerKey,
		propertyFilters: Record<Source, Record<string, FilterValue>>
	): maplibregl.FilterSpecification {
		const filterDefs = getFilterDefsForLayer(layerKey);
		const sourceClauses: maplibregl.ExpressionSpecification[] = [];

		for (const source of this.sources) {
			const sourceFilters = propertyFilters[source];
			const sourceDefs = filterDefs[source];
			const exprs: maplibregl.ExpressionSpecification[] = [['==', ['get', 'source'], source]];

			for (const [property, value] of Object.entries(sourceFilters)) {
				const fieldDef = sourceDefs[property];
				if (!fieldDef) continue;

				const propFilter = this.#buildPropertyFilter(property, value, fieldDef);
				if (propFilter) exprs.push(propFilter);
			}

			sourceClauses.push(
				exprs.length === 1 ? exprs[0] : (['all', ...exprs] as maplibregl.ExpressionSpecification)
			);
		}

		if (sourceClauses.length === 0) return ['literal', false];
		if (sourceClauses.length === 1) return sourceClauses[0];
		return ['any', ...sourceClauses];
	}

	// Final filter expressions for each layer, automatically updated when sources or property filters change
	route = $derived(this.#buildLayerFilter('route', this.route_filters));

	stop = $derived(this.#buildLayerFilter('stop', this.stop_filters));

	activeFilterCount = $derived(
		countActiveFilters({
			initialSources: this.initialSources,
			sources: this.sources,
			layers: this.layers,
			propertyFilters: [
				{ layer: 'stop', filters: this.stop_filters },
				{ layer: 'route', filters: this.route_filters }
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
	}
}
