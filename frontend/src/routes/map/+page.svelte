<script lang="ts">
	import { page } from '$app/state';

	import type { Source } from '$lib/client';
	import Filters from '$lib/map/Filters.svelte';
	import TripMarkersLoader from '$lib/map/TripMarkersLoader.svelte';
	import { MapFilters } from '$lib/map/filters.svelte';
	import type { ActiveVehicle } from '$lib/map/trajectoryArrow';
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

	let hoveredTripId = $state<string | null>(null);
	let hoveredObject = $state<ActiveVehicle | null>(null);
	let hoverX = $state(0);
	let hoverY = $state(0);

	let hoveredLineRouteId = $state<string | null>(null);
	let hoveredLineRouteSource = $state<string | null>(null);

	const activeHoveredRouteId = $derived(hoveredObject?.routeId ?? hoveredLineRouteId);
	const activeHoveredRouteSource = $derived(hoveredObject?.source ?? hoveredLineRouteSource);

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
			<VectorTileSource
				promoteId="id"
				id="route"
				url={`${page.url.origin}/martin/active_route_shapes`}
			>
				<LineLayer
					id="route-layer"
					sourceLayer="active_route_shapes"
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
								[
									'all',
									['==', ['get', 'id'], activeHoveredRouteId ?? ''],
									['==', ['get', 'source'], activeHoveredRouteSource ?? '']
								],
								2.5, // Hovered width at zoom 10
								1 // Normal width at zoom 10
							],
							15,
							[
								'case',
								[
									'all',
									['==', ['get', 'id'], activeHoveredRouteId ?? ''],
									['==', ['get', 'source'], activeHoveredRouteSource ?? '']
								],
								7, // Hovered width at zoom 15
								3 // Normal width at zoom 15
							],
							18,
							[
								'case',
								[
									'all',
									['==', ['get', 'id'], activeHoveredRouteId ?? ''],
									['==', ['get', 'source'], activeHoveredRouteSource ?? '']
								],
								14, // Hovered width at zoom 18
								6 // Normal width at zoom 18
							]
						],
						// 'line-offset': [
						// 	'step', // Use the 'step' expression
						// 	['zoom'], // Get the current zoom level
						// 	0, // Default value if zoom is less than the first stop (15)
						// 	15, // First stop: zoom level 15
						// 	6 // Value if zoom is 15 or greater
						// ],
						'line-color': ['get', 'color'],
						'line-opacity': activeHoveredRouteId
							? [
									'case',
									[
										'all',
										['==', ['get', 'id'], activeHoveredRouteId],
										['==', ['get', 'source'], activeHoveredRouteSource]
									],
									1.0,
									0.35
								]
							: 1.0
					}}
					onmousemove={(e) => {
						// TODO: fix this not working, trip and route are still hovered at the same time
						if (hoveredObject) {
							hoveredLineRouteId = null;
							hoveredLineRouteSource = null;
							return;
						}
						cursor = 'pointer';
						const feat = e.features?.[0]?.properties;
						if (feat) {
							hoveredLineRouteId = feat.id;
							hoveredLineRouteSource = feat.source;
						}
					}}
					onmouseleave={() => {
						if (hoveredObject) return;
						cursor = 'default';
						hoveredLineRouteId = null;
						hoveredLineRouteSource = null;
					}}
					onclick={(e) => {
						if (hoveredObject) return;
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
						if (hoveredObject) return;
						cursor = 'pointer';
						// hovered_stop = e.features?.[0];
					}}
					onmouseleave={() => {
						if (hoveredObject) return;
						cursor = 'default';
						// hovered_stop = undefined;
					}}
					onclick={(e) => {
						if (hoveredObject) return;
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

		{#if filters.layers['trip'] && filters.sources.length > 0}
			<TripMarkersLoader
				sources={filters.sources}
				enabled
				bind:cursor
				bind:hoveredTripId
				bind:hoveredObject
				bind:hoverX
				bind:hoverY
			/>
		{/if}
	</MapLibre>

	{#if hoveredObject}
		{@const route =
			page.data.routes_by_id?.[hoveredObject.source as Source]?.[hoveredObject.routeId]}
		{#if route}
			<div
				class="pointer-events-none absolute z-9999 flex flex-col gap-1.5 rounded-lg border border-neutral-800 bg-neutral-950/90 p-3 text-xs text-white shadow-2xl backdrop-blur-md transition-all duration-75"
				style="left: {hoverX + 15}px; top: {hoverY + 15}px;"
			>
				<div class="flex items-center gap-2">
					<!-- Route Pill/Badge -->
					<div
						class="flex size-6 items-center justify-center rounded-full text-center text-sm font-black text-white"
						style="background-color: {route.color};"
					>
						{route.short_name}
					</div>
					<div class="flex flex-col">
						<span class="font-semibold text-neutral-100">{route.long_name}</span>
						<span class="text-[10px] text-neutral-400 capitalize">
							{hoveredObject.source.replace('_', ' ')}
						</span>
					</div>
				</div>

				<div class="h-px bg-neutral-800 my-0.5"></div>

				<div class="flex flex-col gap-1 text-[11px] text-neutral-300">
					<div>
						<span class="text-neutral-500">Trip ID:</span>
						<code class="rounded bg-neutral-900 px-1 py-0.5 text-neutral-200">
							{hoveredObject.tripId}
						</code>
					</div>
					{#if hoveredObject.passengers !== null && hoveredObject.passengers !== undefined}
						<div>
							<span class="text-neutral-500">Occupancy:</span>
							<span class="text-neutral-200 font-medium">{hoveredObject.passengers} pax</span>
						</div>
					{/if}
				</div>
			</div>
		{/if}
	{/if}
</div>
