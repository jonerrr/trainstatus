<script lang="ts">
	import type { FilterSpecification } from 'maplibre-gl';
	import { CircleLayer, GeoJSONSource, SymbolLayer } from 'svelte-maplibre-gl';

	import {
		BUS_SOURCE_FILTER,
		BUS_STOP_FILL,
		BUS_STOP_HIT_RADIUS,
		BUS_STOP_RADIUS,
		CASING,
		LABEL_FONT,
		SLOT,
		STOP_BEAD_RADIUS,
		STOP_BEAD_STROKE,
		STOP_FILL,
		STOP_HIT_RADIUS,
		SUBWAY_SOURCE_FILTER
	} from './map-theme';
	import type { StopsGeoJSON } from './stops-geojson.js';

	let { data, filter }: { data: StopsGeoJSON; filter: FilterSpecification } = $props();
	const subwayStopFilter: FilterSpecification = $derived([
		'all',
		filter,
		SUBWAY_SOURCE_FILTER
	] as FilterSpecification);
	const busStopFilter: FilterSpecification = $derived([
		'all',
		filter,
		BUS_SOURCE_FILTER
	] as FilterSpecification);
</script>

<GeoJSONSource id="stop" promoteId="id" {data}>
	<CircleLayer
		id="stop-subway-layer"
		beforeId={SLOT.stops}
		filter={subwayStopFilter}
		minzoom={12}
		paint={{
			'circle-radius': STOP_BEAD_RADIUS,
			'circle-color': STOP_FILL,
			'circle-stroke-width': STOP_BEAD_STROKE,
			'circle-stroke-color': CASING,
			'circle-opacity': ['interpolate', ['linear'], ['zoom'], 12, 0.75, 14, 1],
			'circle-pitch-alignment': 'map'
		}}
	/>
	<CircleLayer
		id="stop-bus-layer"
		beforeId={SLOT.stops}
		filter={busStopFilter}
		minzoom={15}
		paint={{
			'circle-radius': BUS_STOP_RADIUS,
			'circle-color': BUS_STOP_FILL,
			'circle-opacity': ['interpolate', ['linear'], ['zoom'], 15, 0.35, 15.5, 0.6, 18, 0.9],
			'circle-stroke-width': ['interpolate', ['linear'], ['zoom'], 16.5, 0, 18, 1],
			'circle-stroke-color': CASING,
			'circle-pitch-alignment': 'map'
		}}
	/>
	<CircleLayer
		id="stop-subway-hit-layer"
		beforeId={SLOT.stops}
		filter={subwayStopFilter}
		minzoom={12}
		paint={{ 'circle-radius': STOP_HIT_RADIUS, 'circle-opacity': 0 }}
	/>

	<CircleLayer
		id="stop-bus-hit-layer"
		beforeId={SLOT.stops}
		filter={busStopFilter}
		minzoom={15}
		paint={{ 'circle-radius': BUS_STOP_HIT_RADIUS, 'circle-opacity': 0 }}
	/>

	<SymbolLayer
		id="stop-subway-label-layer"
		beforeId={SLOT.stopLabels}
		filter={subwayStopFilter}
		minzoom={14}
		layout={{
			'text-field': ['get', 'name'],
			'text-size': ['interpolate', ['linear'], ['zoom'], 14, 10, 18, 13],
			'text-offset': [0, 1.1],
			'text-anchor': 'top',
			'text-font': LABEL_FONT
		}}
		paint={{
			'text-color': '#FFFFFF',
			'text-halo-color': CASING,
			'text-halo-width': 1.5,
			'text-opacity': ['interpolate', ['linear'], ['zoom'], 14, 0, 15, 1]
		}}
	/>

	<SymbolLayer
		id="stop-bus-label-layer"
		beforeId={SLOT.stopLabels}
		filter={busStopFilter}
		minzoom={17}
		layout={{
			'text-field': ['get', 'name'],
			'text-size': ['interpolate', ['linear'], ['zoom'], 17, 9, 20, 12],
			'text-offset': [0, 1],
			'text-anchor': 'top',
			'text-font': LABEL_FONT
		}}
		paint={{
			'text-color': '#C9D2DC',
			'text-halo-color': CASING,
			'text-halo-width': 1.2,
			'text-opacity': ['interpolate', ['linear'], ['zoom'], 17, 0, 17.6, 0.9]
		}}
	/>
</GeoJSONSource>
