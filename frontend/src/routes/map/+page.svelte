<script lang="ts">
	import { page } from '$app/state';

	import type { Source } from '$lib/client';
	import Filters from '$lib/map/Filters.svelte';
	import TripMarkers from '$lib/map/TripMarkers.svelte';
	import { MapFilters } from '$lib/map/filters.svelte';
	import { open_modal } from '$lib/url_params.svelte';

	import maplibregl from 'maplibre-gl';
	import 'maplibre-gl/dist/maplibre-gl.css';
	import {
		CircleLayer,
		GeolocateControl,
		LineLayer,
		MapLibre,
		SymbolLayer,
		VectorTileSource
	} from 'svelte-maplibre-gl';

	let cursor: 'default' | 'pointer' | undefined = $state();

	let map = $state<maplibregl.Map>();

	let filters = $state(new MapFilters());

	// TODO: move filter handing into a svelte.ts file and then share it with the filter ui component
	// const source_filter: maplibregl.FilterSpecification = $derived([
	// 	'in',
	// 	['get', 'source'],
	// 	['literal', page.data.selected_sources]
	// ]);

	$inspect(filters);
</script>

<div class="relative flex w-full h-full">
	<Filters bind:filters />

	<MapLibre
		bind:map
		center={[-74.006, 40.7128]}
		zoom={12}
		{cursor}
		class="size-full"
		autoloadGlobalCss={false}
		style="{page.url.origin}/martin/style/dark-matter.json"
	>
		<!--
		style="https://basemaps.cartocdn.com/gl/dark-matter-gl-style/style.json" -->
		<!-- TODO: adjust bounds -->
		<GeolocateControl
			position="bottom-left"
			trackUserLocation
			showAccuracyCircle
			showUserLocation
			fitBoundsOptions={{ maxZoom: 15 }}
		/>

		<!-- TODO: probably move each source into its own component -->
		<!-- relative urls don't work in tiles param -->
		<!-- TODO: when re-enabling layers, ensure it stays above other layers (e.g. route lines don't block stops) -->
		{#if filters.layers['route']}
			<VectorTileSource promoteId="id" id="route" url={`${page.url.origin}/martin/route`}>
				<LineLayer
					id="route-layer"
					sourceLayer="route"
					layout={{ 'line-cap': 'round', 'line-join': 'round' }}
					filter={filters.route}
					paint={{
						'line-width': [
							'interpolate',
							['linear'],
							['zoom'],
							10,
							[
								'case',
								['boolean', ['feature-state', 'hover'], false],
								2, // Hovered width at zoom 10
								1 // Normal width at zoom 10
							],
							15,
							[
								'case',
								['boolean', ['feature-state', 'hover'], false],
								6, // Hovered width at zoom 15
								3 // Normal width at zoom 15
							],
							18,
							[
								'case',
								['boolean', ['feature-state', 'hover'], false],
								12, // Hovered width at zoom 18
								6 // Normal width at zoom 18
							]
						],
						'line-offset': [
							'step', // Use the 'step' expression
							['zoom'], // Get the current zoom level
							0, // Default value if zoom is less than the first stop (15)
							15, // First stop: zoom level 15
							6 // Value if zoom is 15 or greater
						],
						'line-color': ['get', 'color'],
						'line-opacity': 1.0
					}}
					onmousemove={(e) => {
						cursor = 'pointer';
						// hovered_routes = e.features;
					}}
					onmouseleave={() => {
						cursor = 'default';
						// hovered_routes = undefined;
					}}
					onclick={(e) => {
						console.log(e.features);
						// clicked_routes = e.features;
						// lnglat = e.lngLat;
						// maybe make other features undefined here
						const feat = e.features?.[0].properties;
						if (!feat) return;
						const route = page.data.routes_by_id?.[feat.source as Source]?.[feat.id];
						if (!route) return;
						open_modal({
							type: 'route',
							...route
						});
					}}
				/>

				<!-- {#if hovered_routes}
				{#each hovered_routes as route}
					<FeatureState sourceLayer="route" id={route.id} state={{ hover: true }} />
				{/each}
			{/if} -->
			</VectorTileSource>
		{/if}

		{#if filters.layers['stop']}
			<!-- TODO: add another arrow layer that uses compass direction -->
			<VectorTileSource id="stop" promoteId="id" url={`${page.url.origin}/martin/stop`}>
				<SymbolLayer
					id="stop-label-layer"
					sourceLayer="stop"
					filter={filters.stop}
					minzoom={15}
					layout={{
						'text-field': ['get', 'name'],
						'text-size': ['interpolate', ['linear'], ['zoom'], 15, 9, 18, 12],
						'text-offset': [0, 1.2],
						'text-anchor': 'top',
						'text-font': [
							'Montserrat Regular',
							'Open Sans Regular',
							'Noto Sans Regular',
							'HanWangHeiLight Regular',
							'NanumBarunGothic Regular'
						]
					}}
					paint={{
						'text-color': '#FFFFFF',
						'text-halo-color': 'rgba(0,0,0,0.8)',
						'text-halo-width': 1.5,
						'text-opacity': ['interpolate', ['linear'], ['zoom'], 15, 0, 16, 1]
					}}
				/>

				<CircleLayer
					id="stop-layer"
					sourceLayer="stop"
					filter={filters.stop}
					minzoom={13}
					paint={{
						'circle-radius': ['interpolate', ['linear'], ['zoom'], 10, 1.5, 15, 2.5, 17, 5, 20, 10],
						'circle-color': '#FFFFFF',
						'circle-opacity': ['interpolate', ['linear'], ['zoom'], 10, 0.5, 14, 0.8],
						'circle-stroke-width': ['interpolate', ['linear'], ['zoom'], 13, 0.5, 17, 2],
						'circle-stroke-color': '#000000',
						'circle-stroke-opacity': ['interpolate', ['linear'], ['zoom'], 10, 0.5, 14, 0.8]
					}}
					onmousemove={(e) => {
						cursor = 'pointer';
						// hovered_stop = e.features?.[0];
					}}
					onmouseleave={() => {
						cursor = 'default';
						// hovered_stop = undefined;
					}}
					onclick={(e) => {
						console.log(e.features);
						const feat = e.features?.[0].properties;
						if (!feat) return;
						const stop = page.data.stops_by_id?.[feat.source as Source]?.[feat.id];
						if (!stop) return;
						open_modal({
							type: 'stop',
							...stop
						});
					}}
				/>
				<!-- {#if hovered_stop} -->
				<!-- {#each hovered_routes as route} -->
				<!-- <FeatureState sourceLayer="stop" id={hovered_stop.id} state={{ hover: true }} /> -->
				<!-- {/each} -->
				<!-- {/if} -->
				<!-- Set the click state on the source for the clicked feature -->

				<!-- {#if clicked_feature}
				<FeatureState sourceLayer="station" id={clicked_feature.id} state={{ clicked: true }} />
			{/if} -->
			</VectorTileSource>
		{/if}

		{#if filters.layers['trip']}
			<TripMarkers source="mta_subway" enabled={true} />
		{/if}

		<!-- {#each page.data.selected_sources as source (source)}
			<TripMarkers {map} {source} />
		{/each} -->
	</MapLibre>
</div>
