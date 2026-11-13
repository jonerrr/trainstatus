<script lang="ts">
	import { page } from '$app/state';

	import type { Source } from '#lib/client/index.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import { getCurrentTime, openModal } from '#lib/url-params.svelte.js';

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
	import { BUS_DETAIL_ZOOM, RAIL_DETAIL_ZOOM } from './map-theme';
	import MapFeatureSummary from './MapFeatureSummary.svelte';
	import RouteLayers from './RouteLayers.svelte';
	import StopLayers from './StopLayers.svelte';
	import { createTrajectories } from './trajectories.svelte.js';
	import TripMarkers from './TripMarkers.svelte';

	const currentTime = getCurrentTime();

	type MapOverlay =
		| { kind: 'hover'; target: MapTarget; point: ScreenPoint }
		| { kind: 'choose'; targets: MapTarget[]; point: ScreenPoint }
		| null;

	let map = $state.raw<MapInstance>();
	let center = $state<[number, number]>([-74.006, 40.7128]);
	let zoom = $state(12);
	let bearing = $state(0);
	let settledZoom = $state(12);
	let moving = $state(false);
	let ready = $state(false);
	let width = $state(0);
	let height = $state(0);
	const filters = new MapFilters();
	const interactions = new MapInteractionController();
	const transit = getTransitData();
	const trajectories = createTrajectories(() => ({
		sources: filters.layers.trip ? filters.sources : [],
		at: currentTime.value
	}));
	// Reset any active overlay whenever the visible network or the selected time changes.
	let overlay = $derived.by<MapOverlay>(() => {
		void filters.route;
		void filters.stop;
		void filters.layers.route;
		void filters.layers.stop;
		void filters.layers.trip;
		void currentTime.value;
		return null;
	});
	const hoveredTarget = $derived(overlay?.kind === 'hover' ? overlay.target : null);
	const activeRoute = $derived<MapFeatureKey | null>(
		hoveredTarget?.kind === 'route'
			? { id: hoveredTarget.id, source: hoveredTarget.source }
			: hoveredTarget?.kind === 'trip'
				? { id: hoveredTarget.routeId, source: hoveredTarget.source }
				: null
	);
	const selectedTrip = $derived<MapFeatureKey | null>(
		hoveredTarget?.kind === 'trip' ? { id: hoveredTarget.id, source: hoveredTarget.source } : null
	);
	$effect(() => {
		if (!map) return;
		map.touchZoomRotate.disableRotation();
		map.keyboard.disableRotation();
	});
	$effect(() => {
		if (!map) return;
		const canvas = map.getCanvas();
		canvas.style.cursor = overlay?.kind === 'hover' && !moving ? 'pointer' : '';
		return () => {
			canvas.style.cursor = '';
		};
	});
	function targetsAt(point: ScreenPoint, closest = false): MapTarget[] {
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
						if (kind === 'stop' ? !catalog?.stopsById.has(id) : !catalog?.routesById.has(id))
							return [];
						return [{ kind, id, source }];
					})
			: [];
		return interactions.resolve(point, targets, closest);
	}
	function mousemove(event: MapMouseEvent) {
		if (moving || overlay?.kind === 'choose' || event.originalEvent.buttons) return;
		const target = targetsAt(event.point, true)[0];
		if (target) overlay = { kind: 'hover', target, point: event.point };
		else if (overlay?.kind === 'hover') overlay = null;
	}
	function openTarget(target: MapTarget) {
		overlay = null;
		if (target.kind !== 'trip') {
			openModal({ type: target.kind, source: target.source, id: target.id });
		} else {
			// The trip modal loads the vehicle's route service when it opens.
			openModal({
				type: 'trip',
				source: target.source,
				routeId: target.routeId,
				id: target.id
			});
		}
	}
	function click(event: MapMouseEvent) {
		const targets = targetsAt(event.point);
		if (targets.length === 1) openTarget(targets[0]);
		else overlay = targets.length ? { kind: 'choose', targets, point: event.point } : null;
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
	<Filters {filters} />
	<MapLibre
		bind:map
		bind:center
		bind:zoom
		bind:bearing
		class="size-full"
		autoloadGlobalCss={false}
		attributionControl={false}
		style={`${page.url.origin}/martin/style/dark-matter.json?v=transit-v3`}
		transformRequest={(url, resourceType) => {
			// TODO: Remove once Martin 2.0 is released and deployed (MLT detection fix: maplibre/martin#3486).
			if (resourceType === 'Tile' && new URL(url).pathname.startsWith('/martin/basemap/')) {
				return { url, headers: { ['Accept']: '*/*' } };
			}
			return { url };
		}}
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
			if (overlay?.kind === 'hover') overlay = null;
		}}
		onmovestart={() => {
			moving = true;
			overlay = null;
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
			<TripMarkers
				tables={trajectories.current.tables}
				at={currentTime.value}
				railDetail={settledZoom >= RAIL_DETAIL_ZOOM}
				busDetail={settledZoom >= BUS_DETAIL_ZOOM}
				{selectedTrip}
				onPickerReady={(picker) => interactions.registerVehiclePicker(picker)}
			/>
		{/if}
	</MapLibre>
	{#if overlay?.kind === 'choose'}
		<FeatureChooser
			targets={overlay.targets}
			point={overlay.point}
			viewportWidth={width}
			viewportHeight={height}
			onselect={openTarget}
			ondismiss={() => {
				if (overlay?.kind === 'choose') overlay = null;
			}}
		>
			{#snippet children(target)}<MapFeatureSummary {target} />{/snippet}
		</FeatureChooser>
	{:else if overlay?.kind === 'hover' && !page.state.modal}
		<FeaturePreview
			target={overlay.target}
			point={overlay.point}
			viewportWidth={width}
			viewportHeight={height}
		/>
	{/if}
</div>
