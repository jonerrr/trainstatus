<script lang="ts">
	import { page } from '$app/state';

	import type { ExpressionSpecification, FilterSpecification } from 'maplibre-gl';
	import { LineLayer, VectorTileSource } from 'svelte-maplibre-gl';

	import {
		CASING,
		ROUTE_CASING_WIDTH,
		ROUTE_COLOR,
		ROUTE_DIMMED_OPACITY,
		ROUTE_HIGHLIGHT_WIDTH,
		ROUTE_HIT_WIDTH,
		ROUTE_WIDTH,
		SLOT
	} from './mapTheme';

	let {
		filter,
		activeRoute
	}: { filter: FilterSpecification; activeRoute: { id: string; source: string } | null } = $props();
	const match: ExpressionSpecification = $derived([
		'all',
		['==', ['get', 'id'], activeRoute?.id ?? ''],
		['==', ['get', 'source'], activeRoute?.source ?? '']
	]);
	const highlightFilter: FilterSpecification = $derived([
		'all',
		filter,
		match
	] as FilterSpecification);
	function dimmed(base: number | ExpressionSpecification): number | ExpressionSpecification {
		return activeRoute ? ['case', match, base, ['*', base, ROUTE_DIMMED_OPACITY]] : base;
	}
	const opacity: ExpressionSpecification = $derived([
		'interpolate',
		['linear'],
		['zoom'],
		10,
		dimmed(['case', ['==', ['get', 'source'], 'mta_subway'], 0.95, 0.25]),
		14,
		dimmed(['case', ['==', ['get', 'source'], 'mta_subway'], 1, 0.65]),
		17,
		dimmed(1)
	]);
</script>

<VectorTileSource promoteId="id" id="route" url={`${page.url.origin}/martin/active_route_shapes`}>
	<LineLayer
		id="route-casing-layer"
		sourceLayer="active_route_shapes"
		beforeId={SLOT.routes}
		layout={{ 'line-cap': 'round', 'line-join': 'round' }}
		{filter}
		paint={{
			'line-color': CASING,
			'line-width': ROUTE_CASING_WIDTH,
			'line-opacity': dimmed(0.85)
		}}
	/>
	<LineLayer
		id="route-layer"
		sourceLayer="active_route_shapes"
		beforeId={SLOT.routes}
		layout={{ 'line-cap': 'round', 'line-join': 'round' }}
		{filter}
		paint={{
			'line-color': ROUTE_COLOR,
			'line-width': ROUTE_WIDTH,
			'line-opacity': opacity
		}}
	/>
	<LineLayer
		id="route-highlight-layer"
		sourceLayer="active_route_shapes"
		beforeId={SLOT.routes}
		layout={{ 'line-cap': 'round', 'line-join': 'round' }}
		filter={highlightFilter}
		paint={{
			'line-color': ROUTE_COLOR,
			'line-width': ROUTE_HIGHLIGHT_WIDTH,
			'line-opacity': 1
		}}
	/>
	<LineLayer
		id="route-hit-layer"
		sourceLayer="active_route_shapes"
		beforeId={SLOT.routes}
		layout={{ 'line-cap': 'round', 'line-join': 'round' }}
		{filter}
		paint={{
			'line-color': ROUTE_COLOR,
			'line-width': ROUTE_HIT_WIDTH,
			'line-opacity': 0
		}}
	/>
</VectorTileSource>
