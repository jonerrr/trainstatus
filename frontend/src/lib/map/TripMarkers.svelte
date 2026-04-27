<script lang="ts">
	import { onDestroy, onMount } from 'svelte';

	import { page } from '$app/state';

	import type { Source } from '$lib/client';
	import { trip_context } from '$lib/resources/trips.svelte';
	import { open_modal } from '$lib/url_params.svelte';

	import type { MapLayerMouseEvent, GeoJSONSource as MapLibreGeoJSONSource } from 'maplibre-gl';
	import { CircleLayer, GeoJSONSource, getMapContext } from 'svelte-maplibre-gl';

	// TODO: use generated types
	interface TrajectoryData {
		trip_id: string;
		route_id: string;
		color: [number, number, number];
		path: [number, number][];
		timestamps: number[];
	}

	interface IndexedTrajectory {
		tripId: string;
		routeId: string;
		color: string;
		path: [number, number][];
		timestamps: number[];
		startTime: number;
		endTime: number;
	}

	interface MarkerFeature {
		type: 'Feature';
		properties: {
			trip_id: string;
			route_id: string;
			color: string;
		};
		geometry: {
			type: 'Point';
			coordinates: [number, number];
		};
	}

	interface MarkerFeatureCollection {
		type: 'FeatureCollection';
		features: MarkerFeature[];
	}

	let {
		source = 'mta_subway',
		routeIds = [],
		refreshInterval = 30_000,
		enabled = true,
		// will be passed down to the CircleLayer
		...layerProps
	}: {
		source?: Source;
		routeIds?: string[];
		refreshInterval?: number;
		enabled?: boolean;
		[key: string]: any;
	} = $props();

	const tripResource = $derived(trip_context.getSource(source));
	const mapCtx = getMapContext();

	const markerSourceId = $derived(`trip-markers-source-${source}`);

	const emptyFeatureCollection: MarkerFeatureCollection = {
		type: 'FeatureCollection',
		features: []
	};

	const filteredRouteIds = $derived(routeIds.filter((routeId) => routeId.length > 0));
	const fixedAt = $derived.by(() => {
		const atParam = page.url.searchParams.get('at');
		if (!atParam) return null;
		const parsed = Number(atParam);
		return Number.isFinite(parsed) ? parsed : null;
	});
	let indexedTrajectories = $state<IndexedTrajectory[]>([]);
	let animationId: number | undefined;
	let fetchTimer: ReturnType<typeof setInterval> | undefined;
	let renderInvalidated = $state(true);
	let lastRenderedTime: number | null = $state(null);

	function colorToCss(color: [number, number, number]): string {
		return `rgb(${color[0]}, ${color[1]}, ${color[2]})`;
	}

	function preprocessTrajectory(trajectory: TrajectoryData): IndexedTrajectory | null {
		if (trajectory.path.length < 2 || trajectory.timestamps.length < 2) return null;

		const pointCount = Math.min(trajectory.path.length, trajectory.timestamps.length);
		const path = trajectory.path.slice(0, pointCount);
		const timestamps = trajectory.timestamps.slice(0, pointCount);
		const startTime = timestamps[0];
		const endTime = timestamps[timestamps.length - 1];

		if (!Number.isFinite(startTime) || !Number.isFinite(endTime) || endTime <= startTime) {
			return null;
		}

		return {
			tripId: trajectory.trip_id,
			routeId: trajectory.route_id,
			color: colorToCss(trajectory.color),
			path,
			timestamps,
			startTime,
			endTime
		};
	}

	function interpolatePoint(
		path: [number, number][],
		timestamps: number[],
		t: number
	): [number, number] | null {
		if (timestamps.length < 2 || t < timestamps[0] || t > timestamps[timestamps.length - 1])
			return null;

		let left = 0;
		let right = timestamps.length - 1;

		while (left <= right) {
			const mid = Math.floor((left + right) / 2);
			const value = timestamps[mid];

			if (value === t) return path[mid];

			if (value < t) left = mid + 1;
			else right = mid - 1;
		}

		const upper = left;
		const lower = upper - 1;
		if (lower < 0 || upper >= timestamps.length) return null;

		const t0 = timestamps[lower];
		const t1 = timestamps[upper];
		const p0 = path[lower];
		const p1 = path[upper];

		if (Math.abs(t1 - t0) < 1e-9) return p0;

		const ratio = (t - t0) / (t1 - t0);
		return [p0[0] + ratio * (p1[0] - p0[0]), p0[1] + ratio * (p1[1] - p0[1])];
	}

	function buildFeatureCollection(t: number): MarkerFeatureCollection {
		const features: MarkerFeature[] = [];

		for (const trajectory of indexedTrajectories) {
			if (t < trajectory.startTime || t > trajectory.endTime) continue;

			const coordinates = interpolatePoint(trajectory.path, trajectory.timestamps, t);
			if (!coordinates) continue;

			features.push({
				type: 'Feature',
				properties: {
					trip_id: trajectory.tripId,
					route_id: trajectory.routeId,
					color: trajectory.color
				},
				geometry: {
					type: 'Point',
					coordinates
				}
			});
		}

		return { type: 'FeatureCollection', features };
	}

	function clearMarkers() {
		const map = mapCtx.map;
		if (!map) return;

		const sourceRef = map.getSource(markerSourceId) as MapLibreGeoJSONSource | undefined;
		sourceRef?.setData(emptyFeatureCollection);
		lastRenderedTime = null;
	}

	function renderMarkers(t: number) {
		const map = mapCtx.map;
		if (!map) return;

		const sourceRef = map.getSource(markerSourceId) as MapLibreGeoJSONSource | undefined;
		if (!sourceRef) return;

		// Feed the data directly into MapLibre's WebGL engine
		sourceRef.setData(buildFeatureCollection(t));

		lastRenderedTime = t;
		renderInvalidated = false;
	}

	async function openTripModal(tripId: string, routeId: string, color: string) {
		const trip = tripResource?.current?.get(tripId);
		if (trip) {
			open_modal({ type: 'trip', ...trip });
			return;
		}

		console.log('Trip marker clicked', { source, trip_id: tripId, route_id: routeId, color });

		if (!tripResource) return;

		try {
			const trips = await tripResource.whenReady();
			const readyTrip = trips.get(tripId);
			if (readyTrip) {
				open_modal({ type: 'trip', ...readyTrip });
			}
		} catch (err) {
			console.error('Unable to resolve trip for marker click:', err);
		}
	}

	async function fetchTrajectories() {
		if (!enabled) {
			indexedTrajectories = [];
			renderInvalidated = true;
			clearMarkers();
			return;
		}

		try {
			const params = new URLSearchParams();
			if (filteredRouteIds.length > 0) params.set('route_ids', filteredRouteIds.join(','));
			if (fixedAt !== null) params.set('at', String(fixedAt));

			const queryString = params.toString();
			const url = `/api/v1/trajectories/${source}${queryString ? `?${queryString}` : ''}`;

			const res = await fetch(url);
			if (!res.ok) {
				console.error(`Failed to fetch trajectories: ${res.status}`);
				return;
			}

			const data = await res.json();
			const rawTrajectories: TrajectoryData[] = Array.isArray(data.trajectories)
				? data.trajectories
				: [];

			indexedTrajectories = rawTrajectories
				.map((trajectory) => preprocessTrajectory(trajectory))
				.filter((trajectory): trajectory is IndexedTrajectory => trajectory !== null);

			renderInvalidated = true;
		} catch (err) {
			console.error('Error fetching trajectories:', err);
		}
	}

	function animate() {
		if (!enabled) {
			animationId = requestAnimationFrame(animate);
			return;
		}

		const t = fixedAt ?? Date.now() / 1000;
		const shouldRender =
			fixedAt === null ||
			renderInvalidated ||
			lastRenderedTime === null ||
			Math.abs(lastRenderedTime - t) > 1e-6;

		if (shouldRender) {
			renderMarkers(t);
		}

		animationId = requestAnimationFrame(animate);
	}

	$effect(() => {
		fixedAt;
		renderInvalidated = true;
	});

	$effect(() => {
		enabled;
		source;
		filteredRouteIds.join(',');
		void fetchTrajectories();
	});

	$effect(() => {
		if (fetchTimer !== undefined) {
			clearInterval(fetchTimer);
			fetchTimer = undefined;
		}

		if (!enabled) return;

		fetchTimer = setInterval(() => {
			void fetchTrajectories();
		}, refreshInterval);

		return () => {
			if (fetchTimer !== undefined) {
				clearInterval(fetchTimer);
				fetchTimer = undefined;
			}
		};
	});

	onMount(() => {
		animationId = requestAnimationFrame(animate);
	});

	onDestroy(() => {
		if (animationId !== undefined) cancelAnimationFrame(animationId);
		if (fetchTimer !== undefined) clearInterval(fetchTimer);
	});
</script>

{#if enabled}
	<GeoJSONSource id={markerSourceId} data={emptyFeatureCollection}>
		<CircleLayer
			paint={{
				'circle-color': ['get', 'color'],
				'circle-stroke-color': '#ffffff',
				'circle-stroke-width': 1.5,
				'circle-opacity': 0.95,
				'circle-radius': ['interpolate', ['linear'], ['zoom'], 11, 12.5, 14, 4, 17, 7]
			}}
			{...layerProps}
			onclick={(e: MapLayerMouseEvent) => {
				const properties = e.features?.[0]?.properties;
				const tripId = properties?.trip_id;

				if (tripId) {
					openTripModal(tripId, properties.route_id ?? '', properties.color ?? '');
				}

				// Fire custom onclick if passed down from the parent
				if (layerProps.onclick) {
					layerProps.onclick(e);
				}
			}}
		/>
	</GeoJSONSource>
{/if}
