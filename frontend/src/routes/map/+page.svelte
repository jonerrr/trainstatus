<script lang="ts">
	import { onMount } from 'svelte';

	import { page } from '$app/state';

	import type { Source } from '$lib/client';
	import FeatureChooser from '$lib/map/FeatureChooser.svelte';
	import Filters from '$lib/map/Filters.svelte';
	import TripMarkersLoader from '$lib/map/TripMarkersLoader.svelte';
	import { MapFilters } from '$lib/map/filters.svelte';
	import { MapHover } from '$lib/map/hover.svelte';
	import {
		MapInteractionController,
		type MapTarget,
		type ScreenPoint,
		type VehiclePicker
	} from '$lib/map/interactions';
	import {
		BUS_SOURCE_FILTER,
		BUS_STOP_FILL,
		BUS_STOP_HIT_RADIUS,
		BUS_STOP_RADIUS,
		CASING,
		LABEL_FONT,
		ROUTE_CASING_WIDTH,
		ROUTE_COLOR,
		ROUTE_DIMMED_OPACITY,
		ROUTE_HIGHLIGHT_WIDTH,
		ROUTE_HIT_WIDTH,
		ROUTE_WIDTH,
		SLOT,
		STOP_BEAD_RADIUS,
		STOP_BEAD_STROKE,
		STOP_FILL,
		STOP_HIT_RADIUS,
		SUBWAY_SOURCE_FILTER,
		normalizeRouteColor
	} from '$lib/map/mapTheme';
	import { source_info } from '$lib/resources/index.svelte';
	import { trip_context } from '$lib/resources/trips.svelte';
	import { open_modal } from '$lib/url_params.svelte';

	import maplibregl from 'maplibre-gl';
	import 'maplibre-gl/dist/maplibre-gl.css';
	import {
		CircleLayer,
		GeolocateControl,
		LineLayer,
		MapLibre,
		NavigationControl,
		SymbolLayer,
		VectorTileSource
	} from 'svelte-maplibre-gl';

	let map = $state<maplibregl.Map>();
	let zoom = $state(12);
	let bearing = $state(0);
	let settledZoom = $state(12);
	let pixelRatio = $state(1);
	let cameraMoving = $state(false);

	const HOME_CENTER: [number, number] = [-74.006, 40.7128];
	// TODO: is this even needed? martin returns ETags for caching
	const BASEMAP_STYLE_REVISION = 'transit-v2';
	const RAIL_DETAIL_ZOOM = 14.1;
	const BUS_DETAIL_ZOOM = 16.7;
	const ROUTE_HIT_LAYERS = ['route-hit-layer'];
	const STOP_HIT_LAYERS = ['stop-subway-hit-layer', 'stop-bus-hit-layer'];

	const interactionController = new MapInteractionController();
	const tripResources = trip_context.get();
	const railDetail = $derived(settledZoom >= RAIL_DETAIL_ZOOM);
	const busDetail = $derived(settledZoom >= BUS_DETAIL_ZOOM);

	let chooser = $state<{ targets: MapTarget[]; point: ScreenPoint } | null>(null);

	/**
	 * Pointer affordance. MapLibre's own stylesheet puts `cursor: grab` on
	 * `.maplibregl-canvas-container.maplibregl-interactive`, which is the canvas's
	 * parent, so setting a cursor on the canvas itself does not reliably win.
	 * `maplibregl-track-pointer` is MapLibre's supported class for exactly this,
	 * and it composes correctly with the `:active` grabbing rule while dragging.
	 */
	$effect(() => {
		const container = map?.getCanvasContainer();
		if (!container) return;
		container.classList.toggle('maplibregl-track-pointer', hover.cursor === 'pointer');
	});

	// TODO: why do we need a custom pixel ratio handler thing? maplibre already calculates pixel ratio internally
	onMount(() => {
		pixelRatio = Math.min(window.devicePixelRatio || 1, 2);
	});

	$effect(() => {
		const currentMap = map;
		if (!currentMap) return;
		currentMap.touchZoomRotate.disableRotation();
		currentMap.keyboard.disableRotation();
	});

	let filters = $state(new MapFilters());
	const hover = new MapHover();

	let viewportWidth = $state(0);
	let viewportHeight = $state(0);

	$effect(() => {
		// Closing transient interaction state from one place prevents a hover or
		// chooser from referring to layers that a filter just removed.
		filters.sources;
		filters.layers.route;
		filters.layers.stop;
		filters.layers.trip;
		filters.route;
		filters.stop;
		filters.trip;
		hover.clear();
		chooser = null;
	});

	/**
	 * Matches the route currently being emphasised. When nothing is hovered this
	 * compares against '', which no real feature id equals — so the same
	 * expression doubles as "match nothing" and the highlight layer can stay
	 * mounted instead of being added and removed on every hover.
	 */
	const isActiveRoute: maplibregl.ExpressionSpecification = $derived([
		'all',
		['==', ['get', 'id'], hover.routeId ?? ''],
		['==', ['get', 'source'], hover.routeSource ?? '']
	]);

	/** Fades everything that is not the active route once something is hovered. */
	function dimmed(base: number): number | maplibregl.ExpressionSpecification {
		if (!hover.routeId) return base;
		return ['case', isActiveRoute, base, base * ROUTE_DIMMED_OPACITY];
	}

	const subwayStopFilter = $derived([
		'all',
		filters.stop,
		SUBWAY_SOURCE_FILTER
	] as maplibregl.FilterSpecification);

	const busStopFilter = $derived([
		'all',
		filters.stop,
		BUS_SOURCE_FILTER
	] as maplibregl.FilterSpecification);

	const highlightFilter = $derived([
		'all',
		filters.route,
		isActiveRoute
	] as maplibregl.FilterSpecification);

	// Shared by both stop hit layers. Stops outrank routes in MapHover, so
	// hovering a bead near a line clears the line's hover rather than fighting it.
	function handleStopHover(e: maplibregl.MapLayerMouseEvent) {
		if (cameraMoving) return;
		const feat = e.features?.[0]?.properties;
		if (!feat) return;
		const source = feat.source as Source;
		const stopId = String(feat.id);
		const currentStop = hover.stop;
		if (
			currentStop &&
			currentStop.stopId === stopId &&
			currentStop.source === source &&
			currentStop.x === e.point.x &&
			currentStop.y === e.point.y
		)
			return;
		hover.setStop({
			kind: 'stop',
			stopId,
			source,
			x: e.point.x,
			y: e.point.y
		});
	}

	function handleRouteHover(e: maplibregl.MapLayerMouseEvent) {
		if (cameraMoving) return;
		const feat = e.features?.[0]?.properties;
		if (!feat) return;
		const source = feat.source as Source;
		const routeId = String(feat.id);
		const currentRoute = hover.route;
		if (
			currentRoute &&
			currentRoute.routeId === routeId &&
			currentRoute.source === source &&
			currentRoute.x === e.point.x &&
			currentRoute.y === e.point.y
		)
			return;
		hover.setRoute({
			kind: 'route',
			routeId,
			source,
			x: e.point.x,
			y: e.point.y
		});
	}

	const hoveredRoute = $derived(
		hover.active?.kind === 'vehicle'
			? page.data.routes_by_id?.[hover.active.source]?.[hover.active.routeId]
			: hover.active?.kind === 'route'
				? page.data.routes_by_id?.[hover.active.source]?.[hover.active.routeId]
				: undefined
	);

	const hoveredStop = $derived(
		hover.active?.kind === 'stop'
			? page.data.stops_by_id?.[hover.active.source]?.[hover.active.stopId]
			: undefined
	);

	// Keep the tooltip on screen by flipping it to the other side of the cursor
	// near the right/bottom edges rather than letting it overflow.
	const TOOLTIP_WIDTH = 260;
	const TOOLTIP_HEIGHT = 150;
	const TOOLTIP_GAP = 15;

	const tooltipLeft = $derived.by(() => {
		const x = hover.active?.x ?? 0;
		return viewportWidth && x + TOOLTIP_GAP + TOOLTIP_WIDTH > viewportWidth
			? Math.max(0, x - TOOLTIP_GAP - TOOLTIP_WIDTH)
			: x + TOOLTIP_GAP;
	});

	const tooltipTop = $derived.by(() => {
		const y = hover.active?.y ?? 0;
		return viewportHeight && y + TOOLTIP_GAP + TOOLTIP_HEIGHT > viewportHeight
			? Math.max(0, y - TOOLTIP_GAP - TOOLTIP_HEIGHT)
			: y + TOOLTIP_GAP;
	});

	function targetsFromMap(point: ScreenPoint): MapTarget[] {
		if (!map) return [];
		const layerIds = [...STOP_HIT_LAYERS, ...ROUTE_HIT_LAYERS].filter((id) => map?.getLayer(id));
		if (layerIds.length === 0) return [];

		return map
			.queryRenderedFeatures([point.x, point.y], { layers: layerIds })
			.flatMap((feature): MapTarget[] => {
				const id = String(feature.properties?.id ?? '');
				const source = feature.properties?.source as Source;
				if (!id) return [];

				if (feature.layer.id.startsWith('stop-')) {
					const stop = page.data.stops_by_id[source]?.[id];
					if (!stop) return [];
					return [
						{
							kind: 'stop',
							id,
							source,
							label: stop.name,
							subtitle: `${source_info[source].name} stop`
						}
					];
				}

				const route = page.data.routes_by_id[source]?.[id];
				if (!route) return [];
				return [
					{
						kind: 'route',
						id,
						source,
						label: route.short_name || route.long_name,
						subtitle: route.long_name
					}
				];
			});
	}

	async function openTarget(target: MapTarget) {
		chooser = null;

		if (target.kind === 'stop') {
			const stop = page.data.stops_by_id[target.source]?.[target.id];
			if (stop) open_modal({ type: 'stop', ...stop });
			return;
		}

		if (target.kind === 'route') {
			const route = page.data.routes_by_id[target.source]?.[target.id];
			if (route) open_modal({ type: 'route', ...route });
			return;
		}

		const resource = tripResources[target.source];
		if (!resource) return;
		try {
			const trip = resource.current?.get(target.id) ?? (await resource.whenReady()).get(target.id);
			if (trip) open_modal({ type: 'trip', ...trip });
		} catch (error) {
			console.error('Unable to resolve trip for map selection:', error);
		}
	}

	function handleMapClick(event: maplibregl.MapMouseEvent) {
		const point = { x: event.point.x, y: event.point.y };
		const resolution = interactionController.resolve(point, targetsFromMap(point));
		if (resolution.kind === 'none') {
			chooser = null;
			return;
		}
		if (resolution.kind === 'open') {
			void openTarget(resolution.target);
			return;
		}
		chooser = { targets: resolution.targets, point };
	}
	// TODO: why so many custom handlers?
	function handleMoveStart() {
		cameraMoving = true;
		hover.clear();
		chooser = null;
	}

	function handleMoveEnd() {
		settledZoom = map?.getZoom() ?? zoom;
		cameraMoving = false;
	}
	// TODO: is this even being used?
	function handleMapIdle() {
		window.dispatchEvent(new Event('trainstatus:map-idle'));
	}
	// TODO: why is this being passed to tripmarkersloader?
	function registerVehiclePicker(picker: VehiclePicker | null) {
		interactionController.registerVehiclePicker(picker);
	}
</script>

<div
	class="relative flex w-full h-full"
	data-map-zoom={zoom}
	data-map-bearing={bearing}
	bind:clientWidth={viewportWidth}
	bind:clientHeight={viewportHeight}
>
	<Filters bind:filters />

	<MapLibre
		bind:map
		bind:zoom
		bind:bearing
		center={HOME_CENTER}
		class="size-full"
		autoloadGlobalCss={false}
		style={`${page.url.origin}/martin/style/dark-matter.json?v=${BASEMAP_STYLE_REVISION}`}
		minZoom={7}
		maxZoom={20}
		maxPitch={0}
		pitch={0}
		dragRotate={false}
		touchPitch={false}
		renderWorldCopies={false}
		{pixelRatio}
		onclick={handleMapClick}
		onidle={handleMapIdle}
		onmovestart={handleMoveStart}
		onmoveend={handleMoveEnd}
	>
		<NavigationControl position="top-right" showCompass={false} visualizePitch={false} />
		<GeolocateControl
			position="top-right"
			trackUserLocation
			showAccuracyCircle
			showUserLocation
			fitBoundsOptions={{ maxZoom: 15 }}
		/>

		<!-- TODO: probably move each source into its own component -->
		<!-- relative urls don't work in tiles param -->
		<!--
			Every layer below is anchored with `beforeId` (see SLOT in mapTheme), so
			stacking no longer depends on the order components happen to mount in.
			Toggling a group off and on used to re-add it on top of everything.
		-->
		{#if filters.layers['route']}
			<VectorTileSource
				promoteId="id"
				id="route"
				url={`${page.url.origin}/martin/active_route_shapes`}
			>
				<!-- Dark stroke under the route, so lines read against the basemap and
				     the station beads have something to sit in. -->
				<LineLayer
					id="route-casing-layer"
					sourceLayer="active_route_shapes"
					beforeId={SLOT.routes}
					layout={{ 'line-cap': 'round', 'line-join': 'round' }}
					filter={filters.route}
					paint={{
						'line-color': CASING,
						'line-width': ROUTE_CASING_WIDTH,
						'line-opacity': dimmed(0.85)
					}}
				/>

				<!--
					`line-width` here is deliberately independent of hover. Widening the
					line the cursor is over moved it under the cursor, which changed
					MapLibre's hit test on the next mousemove, which toggled the highlight
					and resized the line again — the flicker the map used to have. All
					hover emphasis lives in the non-interactive highlight layer below;
					opacity is safe to vary because it does not affect hit testing.
					Interaction lives in route-hit-layer, so this layer is purely visual.
				-->
				<LineLayer
					id="route-layer"
					sourceLayer="active_route_shapes"
					beforeId={SLOT.routes}
					layout={{ 'line-cap': 'round', 'line-join': 'round' }}
					filter={filters.route}
					paint={{
						'line-color': ROUTE_COLOR,
						'line-width': ROUTE_WIDTH,
						'line-opacity': dimmed(1)
					}}
				/>

				<!-- Non-interactive: nothing hit-tests this, so it is free to change width. -->
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

				<!--
					Invisible grab area. A 3px line is a poor mouse target, and MapLibre
					buffers line hit-tests by `line-width`, so painting a much wider copy
					at zero opacity widens the target without drawing anything. Features
					from a zero-opacity layer are still returned by queryRenderedFeatures
					(only `visibility: none` and the zoom range remove a layer from
					queries), which is what makes this work.
				-->
				<LineLayer
					id="route-hit-layer"
					sourceLayer="active_route_shapes"
					beforeId={SLOT.routes}
					layout={{ 'line-cap': 'round', 'line-join': 'round' }}
					filter={filters.route}
					paint={{
						'line-color': ROUTE_COLOR,
						'line-width': ROUTE_HIT_WIDTH,
						'line-opacity': 0
					}}
					onmousemove={handleRouteHover}
					onmouseleave={() => hover.clearRoute()}
				/>
			</VectorTileSource>
		{/if}

		{#if filters.layers['stop']}
			<!-- TODO: add another arrow layer that uses compass direction -->
			<VectorTileSource id="stop" promoteId="id" url={`${page.url.origin}/martin/stop`}>
				<!--
					Subway stations render as beads threaded onto the route: drawn above
					the line, with a radius tied to the same zoom ramp as `line-width` so
					the bead always stays a little wider than the line it sits on.
				-->
				<CircleLayer
					id="stop-subway-layer"
					sourceLayer="stop"
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

				<!--
					Bus stops are an order of magnitude denser than stations and sit on the
					curb rather than on the line, so they get their own quieter treatment
					and stay hidden until z15 instead of peppering the mid-zoom map.
				-->
				<CircleLayer
					id="stop-bus-layer"
					sourceLayer="stop"
					beforeId={SLOT.stops}
					filter={busStopFilter}
					minzoom={15}
					paint={{
						'circle-radius': BUS_STOP_RADIUS,
						'circle-color': BUS_STOP_FILL,
						'circle-opacity': ['interpolate', ['linear'], ['zoom'], 15, 0, 15.5, 0.55, 18, 0.8],
						'circle-stroke-width': ['interpolate', ['linear'], ['zoom'], 16.5, 0, 18, 1],
						'circle-stroke-color': CASING,
						'circle-pitch-alignment': 'map'
					}}
				/>

				<!--
					Grab areas for the stop circles, which are far smaller than the route
					line (a station bead is 1.6px across at z12). Each mirrors its visible
					layer's filter and minzoom so there is never a hit target over a stop
					that has not been drawn.
				-->
				<CircleLayer
					id="stop-subway-hit-layer"
					sourceLayer="stop"
					beforeId={SLOT.stops}
					filter={subwayStopFilter}
					minzoom={12}
					paint={{ 'circle-radius': STOP_HIT_RADIUS, 'circle-opacity': 0 }}
					onmousemove={handleStopHover}
					onmouseleave={() => hover.setStop(null)}
				/>

				<CircleLayer
					id="stop-bus-hit-layer"
					sourceLayer="stop"
					beforeId={SLOT.stops}
					filter={busStopFilter}
					minzoom={15}
					paint={{ 'circle-radius': BUS_STOP_HIT_RADIUS, 'circle-opacity': 0 }}
					onmousemove={handleStopHover}
					onmouseleave={() => hover.setStop(null)}
				/>

				<SymbolLayer
					id="stop-subway-label-layer"
					sourceLayer="stop"
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
					sourceLayer="stop"
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
			</VectorTileSource>
		{/if}

		{#if filters.layers['trip'] && filters.sources.length > 0}
			<!-- TODO: why are we pausing on camera move? -->
			<TripMarkersLoader
				sources={filters.sources}
				{hover}
				paused={cameraMoving}
				{railDetail}
				{busDetail}
				{pixelRatio}
				onPickerReady={registerVehiclePicker}
				enabled
			/>
		{/if}
	</MapLibre>

	{#if chooser}
		<FeatureChooser
			targets={chooser.targets}
			point={chooser.point}
			{viewportWidth}
			{viewportHeight}
			onselect={(target) => void openTarget(target)}
			ondismiss={() => (chooser = null)}
		/>
	{/if}

	<!-- TODO: improve tooltip ui/ux -->
	{#if hover.active && (hoveredRoute || hoveredStop)}
		<div
			class="pointer-events-none absolute z-9999 flex flex-col gap-1.5 rounded-lg border border-neutral-800 bg-neutral-950/90 p-3 text-xs text-white shadow-2xl backdrop-blur-md"
			data-map-tooltip-kind={hover.active.kind}
			style="left: {tooltipLeft}px; top: {tooltipTop}px; width: {TOOLTIP_WIDTH}px;"
		>
			{#if hoveredStop && hover.active.kind === 'stop'}
				<div class="flex min-w-0 flex-col gap-0.5">
					<span class="truncate font-semibold text-neutral-100">{hoveredStop.name}</span>
					<span class="text-[10px] text-neutral-400">
						{source_info[hover.active.source].name} stop · {hoveredStop.id}
					</span>
				</div>
			{:else if hoveredRoute}
				<div class="flex items-center gap-2">
					<div
						class="flex size-6 shrink-0 items-center justify-center rounded-full text-center text-sm font-black text-white"
						style="background-color: {normalizeRouteColor(hoveredRoute.color)};"
					>
						{hoveredRoute.short_name}
					</div>
					<div class="flex min-w-0 flex-col">
						<span class="truncate font-semibold text-neutral-100">{hoveredRoute.long_name}</span>
						<span class="text-[10px] text-neutral-400">
							{source_info[hover.active.source].name}
							{hover.active.kind === 'vehicle' ? 'vehicle' : 'route'}
						</span>
					</div>
				</div>

				{#if hover.active.kind === 'vehicle'}
					<div class="my-0.5 h-px bg-neutral-800"></div>
					<div class="flex flex-col gap-1 text-[11px] text-neutral-300">
						<div class="truncate">
							<span class="text-neutral-500">Trip ID:</span>
							<code class="rounded bg-neutral-900 px-1 py-0.5 text-neutral-200">
								{hover.active.tripId}
							</code>
						</div>
						{#if hover.active.vehicle.passengers !== null && hover.active.vehicle.passengers !== undefined}
							<div>
								<span class="text-neutral-500">Occupancy:</span>
								<span class="font-medium text-neutral-200">
									{hover.active.vehicle.passengers} pax
								</span>
							</div>
						{/if}
					</div>
				{/if}
			{/if}
		</div>
	{/if}
</div>

<style>
	:global(.maplibregl-ctrl-top-right) {
		top: 0.375rem;
		right: 0.375rem;
	}

	:global(.maplibregl-ctrl-top-right .maplibregl-ctrl) {
		margin: 0 0 0.25rem;
	}

	:global(.maplibregl-ctrl-group) {
		overflow: hidden;
		border: 1px solid rgb(82 82 82 / 0.5);
		border-radius: 0.4rem;
		box-shadow: 0 3px 12px rgb(0 0 0 / 0.2);
	}
</style>
