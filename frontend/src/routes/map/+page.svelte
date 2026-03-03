<script lang="ts">
	import { page } from '$app/state';

	import maplibregl from 'maplibre-gl';
	import {
		AttributionControl,
		GeoJSONSource,
		GeolocateControl,
		MapLibre,
		VectorTileSource
	} from 'svelte-maplibre-gl';

	let map = $state<maplibregl.Map>();
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
		maxBounds={[
			[-74.45909, 40.277399],
			[-73.000272, 41.017577]
		]}
		zoom={12}
		maxZoom={17}
		class="size-full"
		attributionControl={{ compact: true }}
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
