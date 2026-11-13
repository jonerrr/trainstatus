<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogRoute, CatalogStop, Source } from '#lib/client/index.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import { geolocation } from '#lib/geolocation.svelte.js';
	import List, { type TripItem } from '#lib/List.svelte';
	import { parseTripPin, routePins, stopPins, tripPins } from '#lib/pins.svelte.js';
	import { getCurrentTime } from '#lib/url-params.svelte.js';
	import {
		calculateRouteHeight,
		calculateStopHeight,
		calculateTripHeight,
		haversine
	} from '#lib/util.svelte.js';

	import { Locate, LocateFixed, LocateOff } from '@lucide/svelte';

	const transit = getTransitData();
	const currentTime = getCurrentTime();

	const catalogResources = transit.catalogs(() => page.data.selectedSources);
	const catalogs = $derived(catalogResources.current.filter((r) => r.state === 'ready'));
	const catalogsLoading = $derived(catalogResources.current.some((c) => c.state === 'loading'));

	function bySource<T>(pick: (source: Source) => T[]) {
		const sources = Object.fromEntries(
			page.data.selectedSources.map((source) => [source, pick(source)])
		) as Record<Source, T[]>;
		return { sources, exists: Object.values(sources).some((items) => items.length > 0) };
	}

	const pinnedTrips = $derived(
		bySource((source) =>
			(tripPins.current[source] ?? []).flatMap((id): TripItem[] => {
				const pin = parseTripPin(id);
				return pin ? [{ id, source, ...pin }] : [];
			})
		)
	);
	const pinnedService = transit.trips(() =>
		currentTime.value === undefined ? Object.values(pinnedTrips.sources).flat() : []
	);
	$effect(() => {
		const pins = tripPins.current;
		let next = pins;
		for (const read of pinnedService.current) {
			if (read.state !== 'missing') continue;
			const kept = next[read.source].filter((key) => {
				const pin = parseTripPin(key);
				return pin?.routeId !== read.routeId || pin.tripId !== read.tripId;
			});
			if (kept.length !== next[read.source].length) next = { ...next, [read.source]: kept };
		}
		if (next !== pins) tripPins.current = next;
	});

	const catalogOf = (source: Source) => catalogs.find((c) => c.source === source)?.value;

	const pinnedStops = $derived(
		bySource((source) =>
			(stopPins.current[source] ?? [])
				.map((pin) => catalogOf(source)?.stopsById.get(pin))
				.filter((s): s is CatalogStop => s !== undefined)
		)
	);

	const pinnedRoutes = $derived(
		bySource((source) =>
			(routePins.current[source] ?? [])
				.map((pin) => catalogOf(source)?.routesById.get(pin))
				.filter((r): r is CatalogRoute => r !== undefined)
		)
	);

	// Interface for stops with distance
	interface StopWithDistance extends CatalogStop {
		distance: number;
	}

	const nearbyStops = $derived.by(() => {
		const result = {} as Record<Source, StopWithDistance[]>;

		if (!geolocation.position) return result;
		const currentCoords = geolocation.position.coords;

		for (const { source, value: catalog } of catalogs) {
			result[source] = (catalog?.stops ?? [])
				.map((stop) => {
					const distance = haversine(
						currentCoords.latitude,
						currentCoords.longitude,
						stop.lat,
						stop.lon
					);
					return { ...stop, distance };
				})
				.sort((a, b) => a.distance - b.distance);
		}
		return result;
	});
</script>

{#snippet locate_button()}
	<button
		onclick={() => geolocation.resume()}
		class={[
			'relative flex items-center justify-center rounded-none p-1 shadow-lg transition-colors duration-200',
			{
				'bg-border-default text-primary hover:bg-border-strong active:bg-surface-2':
					geolocation.status === 'loading',
				'bg-accent text-accent-fg hover:bg-accent-hover active:bg-accent':
					geolocation.status === 'active',
				'bg-red-600 text-white hover:bg-red-500 active:bg-red-700': geolocation.status === 'denied'
			}
		]}
		aria-label="Nearby stops"
		title="Nearby stops"
	>
		{#if geolocation.status === 'loading'}
			<Locate class="animate-spin" />
		{:else if geolocation.status === 'denied'}
			<LocateOff />
		{:else}
			<LocateFixed />
		{/if}
	</button>
{/snippet}

<div class="flex h-full flex-col overflow-hidden">
	<!-- Pinned items section - no scroll -->
	<div class="flex-none overflow-hidden">
		{#if pinnedTrips.exists}
			<List
				title="Pinned Trips"
				sources={pinnedTrips.sources}
				type="trip"
				pins={tripPins}
				heightCalc={calculateTripHeight}
				itemsBeforeScroll={2}
				listClass="max-h-[25dvh]"
			/>
		{/if}

		{#if pinnedRoutes.exists}
			<List
				title="Pinned Routes"
				pins={routePins}
				sources={pinnedRoutes.sources}
				type="route"
				heightCalc={calculateRouteHeight}
				itemsBeforeScroll={2}
				listClass="max-h-[25dvh]"
			/>
		{/if}

		<!-- TODO: loading pinned stops in SSR always puts the 1 as the first -->
		{#if pinnedStops.exists}
			<List
				title="Pinned stops"
				pins={stopPins}
				sources={pinnedStops.sources}
				type="stop"
				heightCalc={calculateStopHeight}
				itemsBeforeScroll={2}
				ssrMin={0}
				listClass="max-h-[25dvh]"
			/>
		{/if}
	</div>

	<div class="min-h-0 flex-1">
		<!-- TODO: either hide or show error message when nearby_stops is empty and location perms were denied -->
		<!-- maybe put it inside of List, since we should also start showing an error message when the stop search returns empty -->
		<!-- {#if nearby_stops} -->
		<List
			title="Nearby Stops"
			type="stop"
			containerClass="h-full"
			sources={nearbyStops}
			pins={stopPins}
			headerSlot={locate_button}
			heightCalc={calculateStopHeight}
			loading={catalogsLoading || (geolocation.status === 'loading' && !geolocation.position)}
		/>
		<!-- {:else}
			<p class="p-4 text-sm text-muted">
				Unable to get nearby stops. Please allow location access and refresh the page.
			</p>
		{/if} -->
	</div>
</div>
