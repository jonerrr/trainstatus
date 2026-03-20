<script lang="ts">
	import { page } from '$app/state';

	import { open_modal } from '$lib/url_params.svelte';

	import type { Source } from '@trainstatus/client';
	import maplibregl from 'maplibre-gl';
	import {
		AttributionControl,
		CircleLayer,
		FeatureState,
		GeoJSONSource,
		GeolocateControl,
		LineLayer,
		MapLibre,
		SymbolLayer,
		VectorTileSource
	} from 'svelte-maplibre-gl';

	let cursor: 'default' | 'pointer' | undefined = $state();

	let map = $state<maplibregl.Map>();

	const source_filter: maplibregl.FilterSpecification = $derived([
		'in',
		['get', 'source'],
		['literal', page.data.selected_sources]
	]);
</script>

<!-- diffStyleUpdates -->
<div class="relative flex w-full h-full">
	<MapLibre
		bind:map
		onload={async (map) => {
			// list layers
			// const layers = map.getStyle().layers;
			// console.log(layers.map((l) => l.id));
			// layers.forEach((layer) => {
			// 	// if (layer.id === 'background') {
			// 	// 	map.moveLayer(layer.id, 'waterway-label');
			// 	// }
			// 	map.setLayoutProperty(layer.id, 'visibility', 'none');
			// });
			// disable background
			// map.setLayoutProperty('background', 'visibility', 'none');
			// map.setLayoutProperty('background', 'visibility', 'none');
			// map.setLayoutProperty('waterway', 'visibility', 'none');
			// map.setLayoutProperty('water_shadow', 'visibility', 'none');
			// map.setLayoutProperty('background', 'visibility', 'none');
			// map.setLayoutProperty('background', 'visibility', 'none');
			// const bus_up = await map.loadImage($mode !== 'light' ? '/bus-up-white.png' : '/bus-up.png');
			// map.addImage('bus_up', bus_up.data);
			// const images = [
			// 	`bus_${$mode !== 'light' ? 'white' : 'black'}_left`,
			// 	// `but_white_right`,
			// 	'bus_full',
			// 	'bus_half_full'
			// ];
			// Promise.all([
			// 	...images.map(async (img) => {
			// 		const image = await map.loadImage(`/${img}.png`);
			// 		map.addImage(img, image.data);
			// 	})
			// ]);
			// generated using https://jobtalle.com/SDFMaker/
			// const img_sdf = await map.loadImage('/bus_sdf.png');
			// map.addImage('bus_sdf', img_sdf.data, { sdf: true });
		}}
		center={[-74.006, 40.7128]}
		zoom={12}
		maxZoom={17}
		{cursor}
		class="size-full"
		style="https://basemaps.cartocdn.com/gl/dark-matter-gl-style/style.json"
	>
		<!-- TODO: custom basemap -->
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
		<VectorTileSource
			promoteId="id"
			id="route"
			tiles={[`${page.url.origin}/martin/route/{z}/{x}/{y}`]}
		>
			<LineLayer
				id="route-layer"
				sourceLayer="route"
				layout={{ 'line-cap': 'round', 'line-join': 'round' }}
				filter={source_filter}
				paint={{
					'line-width': [
						'case',
						['boolean', ['feature-state', 'hover'], false],
						6, // Hovered width
						3 // Normal width
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
				}}
			/>

			<!-- {#if hovered_routes}
				{#each hovered_routes as route}
					<FeatureState sourceLayer="route" id={route.id} state={{ hover: true }} />
				{/each}
			{/if} -->
		</VectorTileSource>

		<!-- TODO: add another arrow layer that uses compass direction -->
		<VectorTileSource
			id="stop"
			promoteId="id"
			tiles={[`${page.url.origin}/martin/stop/{z}/{x}/{y}`]}
		>
			<CircleLayer
				id="stop-layer"
				sourceLayer="stop"
				filter={source_filter}
				paint={{
					'circle-radius': ['interpolate', ['linear'], ['zoom'], 15, 3, 17, 15],
					'circle-color': '#0074D9',
					'circle-opacity': 1,
					'circle-stroke-width': 2,
					'circle-stroke-color': '#FFFFFF'
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

		<!-- {#if show_routes}
		<Routes geojson={route_geojson} />
	{/if}
	{#if show_stops}
		<Stops geojson={stop_geojson} />
	{/if} -->
		<!-- {#if show_trips}
		<Trips geojson={page.data.trips} map={map!} {show_overlapping} filter={trips_filter} />
	{/if} -->
	</MapLibre>
</div>
