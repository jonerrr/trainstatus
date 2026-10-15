<script lang="ts">
	import { page } from '$app/state';

	import type { Source } from '#lib/client/index.js';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { getCurrentTime, open_modal } from '#lib/url_params.svelte.js';

	import type { Map as MapInstance, MapMouseEvent } from 'maplibre-gl';
	import { AttributionControl, GeolocateControl, MapLibre } from 'svelte-maplibre-gl';

	import 'svelte-maplibre-gl/vite';
	import 'maplibre-gl/dist/maplibre-gl.css';

	import FeatureChooser from './FeatureChooser.svelte';
	import FeaturePreview from './FeaturePreview.svelte';
	import Filters from './Filters.svelte';
	import { MapFilters } from './filters.svelte';
	import {
		MapInteractionController,
		type MapFeatureKey,
		type MapTarget,
		type ScreenPoint
	} from './interactions';
	import MapFeatureSummary from './MapFeatureSummary.svelte';
	import { BUS_DETAIL_ZOOM, RAIL_DETAIL_ZOOM } from './mapTheme';
	import RouteLayers from './RouteLayers.svelte';
	import StopLayers from './StopLayers.svelte';
	import TripMarkersLoader from './TripMarkersLoader.svelte';

	const current_time = getCurrentTime();

	let map = $state.raw<MapInstance>();
	let center = $state<[number, number]>([-74.006, 40.7128]);
	let zoom = $state(12);
	let bearing = $state(0);
	let settledZoom = $state(12);
	let moving = $state(false);
	let ready = $state(false);
	let width = $state(0);
	let height = $state(0);
	let hover = $state<{ target: MapTarget; point: ScreenPoint } | null>(null);
	let chooser = $state<{ targets: MapTarget[]; point: ScreenPoint } | null>(null);
	let selectionError = $state('');
	let filters = $state(new MapFilters());
	const interactions = new MapInteractionController();
	const transit = getTransitData();
	const activeRoute = $derived<MapFeatureKey | null>(
		hover?.target.kind === 'route'
			? { id: hover.target.id, source: hover.target.source }
			: hover?.target.routeId
				? { id: hover.target.routeId, source: hover.target.source }
				: null
	);
	const selectedTrip = $derived<MapFeatureKey | null>(
		hover?.target.kind === 'trip' ? { id: hover.target.id, source: hover.target.source } : null
	);
	// New object whenever the visible network or the selected time changes.
	const interactionScope = $derived({
		route: filters.route,
		stop: filters.stop,
		trip: filters.trip,
		routeLayer: filters.layers.route,
		stopLayer: filters.layers.stop,
		tripLayer: filters.layers.trip,
		at: current_time.value
	});
	$effect(() => {
		void interactionScope;
		hover = null;
		chooser = null;
	});
	$effect(() => {
		if (!map) return;
		map.touchZoomRotate.disableRotation();
		map.keyboard.disableRotation();
	});
	$effect(() => {
		if (!map) return;
		const canvas = map.getCanvas();
		canvas.style.cursor = hover && !moving ? 'pointer' : '';
		return () => {
			canvas.style.cursor = '';
		};
	});
	function targetsAt(point: ScreenPoint): MapTarget[] {
		if (!map) return [];
		const layers = ['stop-subway-hit-layer', 'stop-bus-hit-layer', 'route-hit-layer'].filter((id) =>
			map?.getLayer(id)
		);
		const targets: MapTarget[] = layers.length
			? map
					.queryRenderedFeatures([point.x, point.y], { layers })
					.flatMap((feature): MapTarget[] => {
						const id = String(feature.properties?.id ?? '');
						const source = feature.properties?.source as Source;
						if (!id || !filters.sources.includes(source)) return [];
						const kind = feature.layer.id.startsWith('stop-') ? 'stop' : 'route';
						const catalog = transit.peekCatalog(source);
						if (kind === 'stop' ? !catalog?.stops_by_id.has(id) : !catalog?.routes_by_id.has(id))
							return [];
						return [{ kind, id, source }];
					})
			: [];
		const result = interactions.resolve(point, targets);
		return result.kind === 'choose'
			? result.targets
			: result.kind === 'open'
				? [result.target]
				: [];
	}
	function mousemove(event: MapMouseEvent) {
		if (moving || chooser || event.originalEvent.buttons) return;
		const target = targetsAt(event.point)[0];
		hover = target ? { target, point: event.point } : null;
	}
	function openTarget(target: MapTarget) {
		hover = null;
		chooser = null;
		selectionError = '';
		if (target.kind !== 'trip') {
			open_modal({ type: target.kind, source: target.source, id: target.id });
		} else if (target.routeId) {
			// The trip modal loads the vehicle's route service when it opens.
			open_modal({
				type: 'trip',
				source: target.source,
				route_id: target.routeId,
				id: target.id
			});
		} else {
			selectionError = 'This vehicle has no route. Select another vehicle.';
		}
	}
	function click(event: MapMouseEvent) {
		const targets = targetsAt(event.point);
		selectionError = '';
		hover = null;
		if (targets.length === 1) openTarget(targets[0]);
		else chooser = targets.length ? { targets, point: event.point } : null;
	}
</script>

<div
	class="relative h-full w-full overflow-hidden"
	bind:clientWidth={width}
	bind:clientHeight={height}
	data-map-ready={ready}
	data-map-center={center.join(',')}
	data-map-zoom={zoom}
	data-map-bearing={bearing}
>
	<Filters bind:filters />
	<MapLibre
		bind:map
		bind:center
		bind:zoom
		bind:bearing
		class="size-full"
		autoloadGlobalCss={false}
		attributionControl={false}
		style={`${page.url.origin}/martin/style/dark-matter.json?v=transit-v3`}
		minZoom={7}
		maxZoom={20}
		maxPitch={0}
		pitch={0}
		dragRotate={false}
		touchPitch={false}
		renderWorldCopies={false}
		onload={() => {
			ready = true;
		}}
		onclick={click}
		onmousemove={mousemove}
		onmouseout={() => {
			hover = null;
		}}
		onmovestart={() => {
			moving = true;
			hover = null;
			chooser = null;
		}}
		onmoveend={() => {
			moving = false;
			settledZoom = map?.getZoom() ?? zoom;
		}}
	>
		<AttributionControl position="top-right" compact />
		<GeolocateControl
			position="bottom-right"
			trackUserLocation
			showAccuracyCircle
			showUserLocation
			fitBoundsOptions={{ maxZoom: 15 }}
		/>
		{#if filters.layers.route}<RouteLayers filter={filters.route} {activeRoute} />{/if}
		{#if filters.layers.stop}<StopLayers filter={filters.stop} />{/if}
		{#if filters.layers.trip && filters.sources.length}
			<TripMarkersLoader
				sources={filters.sources}
				railDetail={settledZoom >= RAIL_DETAIL_ZOOM}
				busDetail={settledZoom >= BUS_DETAIL_ZOOM}
				{selectedTrip}
				onPickerReady={(picker) => interactions.registerVehiclePicker(picker)}
			/>
		{/if}
	</MapLibre>
	{#if chooser}
		<FeatureChooser
			targets={chooser.targets}
			point={chooser.point}
			viewportWidth={width}
			viewportHeight={height}
			onselect={openTarget}
			ondismiss={() => {
				chooser = null;
			}}
		>
			{#snippet children(target)}<MapFeatureSummary {target} />{/snippet}
		</FeatureChooser>
	{:else if hover && !page.state.modal}
		<FeaturePreview
			target={hover.target}
			point={hover.point}
			viewportWidth={width}
			viewportHeight={height}
		/>
	{/if}
	{#if selectionError}<div
			role="status"
			class="absolute right-12 bottom-3 left-3 rounded-lg bg-neutral-950 p-3 text-sm text-amber-200"
		>
			{selectionError}
		</div>{/if}
</div>
