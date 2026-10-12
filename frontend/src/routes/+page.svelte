<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogRoute, CatalogStop, Source } from '#lib/client/index.js';
	import List, { type TripItem } from '#lib/List.svelte';
	import { parse_trip_pin, route_pins, stop_pins, trip_pins } from '#lib/pins.svelte.js';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import {
		calculate_route_height,
		calculate_stop_height,
		calculate_trip_height,
		haversine
	} from '#lib/util.svelte.js';

	import { Locate, LocateFixed, LocateOff } from '@lucide/svelte';

	const transit = getTransitData();

	const catalogs = $derived(
		page.data.selected_sources.map((source) => ({ source, ...transit.catalogState(source) }))
	);
	const catalogs_loading = $derived(catalogs.some((c) => !c.catalog && !c.error));

	function by_source<T>(pick: (source: Source) => T[]) {
		const sources = Object.fromEntries(
			page.data.selected_sources.map((source) => [source, pick(source)])
		) as Record<Source, T[]>;
		return { sources, exists: Object.values(sources).some((items) => items.length > 0) };
	}

	// Pins persist until unpinned; a trip that is not running shows that in its row.
	const pinned_trips = $derived(
		by_source((source) =>
			(trip_pins.current[source] ?? []).flatMap((id): TripItem[] => {
				const pin = parse_trip_pin(id);
				return pin ? [{ id, source, ...pin }] : [];
			})
		)
	);

	const catalog_of = (source: Source) => catalogs.find((c) => c.source === source)?.catalog;

	const pinned_stops = $derived(
		by_source((source) =>
			(stop_pins.current[source] ?? [])
				.map((pin) => catalog_of(source)?.stops_by_id.get(pin))
				.filter((s): s is CatalogStop => s !== undefined)
		)
	);

	const pinned_routes = $derived(
		by_source((source) =>
			(route_pins.current[source] ?? [])
				.map((pin) => catalog_of(source)?.routes_by_id.get(pin))
				.filter((r): r is CatalogRoute => r !== undefined)
		)
	);

	let watch_id: number;
	// TODO: types
	let position = $state<GeolocationPosition>();
	let position_denied = $state(false);

	function update_position(_position: GeolocationPosition) {
		position = _position;
		// location_status.current = 'granted';
	}

	// || location_status.current === 'denied'
	function resume() {
		// TODO: maybe add a check to delete the existing watch before starting a new one
		if (!navigator) return;
		watch_id = navigator.geolocation.watchPosition(
			update_position,
			(err) => {
				console.error('Error watching position:', err);
				// location_status.current = 'denied';
				position_denied = true;
			},
			{
				// enableHighAccuracy: true,
				maximumAge: 30000 // 30 seconds
				// timeout: 27000 // 27 seconds to allow for some delay in getting a position
			}
		);
	}

	$effect(() => {
		// if (location_status.current !== 'denied') {
		resume();
		// }

		// Stop watching position on unmount
		return () => {
			if (watch_id !== undefined) {
				navigator.geolocation.clearWatch(watch_id);
			}
		};
	});

	const position_loading = $derived(position === undefined && !position_denied);

	// Interface for stops with distance
	interface StopWithDistance extends CatalogStop {
		distance: number;
	}

	const nearby_stops = $derived.by(() => {
		const result = {} as Record<Source, StopWithDistance[]>;

		if (!position) return result;
		// reassign to local var to prevent undefined error in map callback.
		const current_coords = position.coords;

		for (const { source, catalog } of catalogs) {
			result[source] = (catalog?.stops ?? [])
				.map((stop) => {
					const distance = haversine(
						current_coords.latitude,
						current_coords.longitude,
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
	<!-- TODO: double check the location states are all working and onclick is working -->
	<button
		onclick={() => resume()}
		class={[
			'relative flex items-center justify-center rounded-lg p-1 shadow-lg transition-all duration-200 hover:bg-neutral-600 active:bg-neutral-600',
			{
				'bg-blue-600 text-white': position,
				'bg-red-600 text-white hover:bg-red-500 active:bg-red-700': position_denied
				// 'bg-neutral-700 text-white hover:bg-neutral-600 active:bg-neutral-800':
				// 	location_status.current === 'unknown'
			}
		]}
		aria-label="Nearby stops"
		title="Nearby stops"
	>
		{#if position_loading}
			<Locate class="animate-spin" />
		{:else if position_denied}
			<LocateOff />
		{:else}
			<LocateFixed />
		{/if}
	</button>
{/snippet}

<div class="flex h-full flex-col overflow-hidden">
	<!-- Pinned items section - no scroll -->
	<div class="flex-none overflow-hidden">
		{#if pinned_trips.exists}
			<List
				title="Pinned Trips"
				sources={pinned_trips.sources}
				type="trip"
				pins={trip_pins}
				height_calc={calculate_trip_height}
				items_before_scroll={2}
				list_class="max-h-[25dvh]"
			/>
		{/if}

		{#if pinned_routes.exists}
			<List
				title="Pinned Routes"
				pins={route_pins}
				sources={pinned_routes.sources}
				type="route"
				height_calc={calculate_route_height}
				items_before_scroll={2}
				list_class="max-h-[25dvh]"
			/>
		{/if}

		<!-- TODO: loading pinned stops in SSR always puts the 1 as the first -->
		{#if pinned_stops.exists}
			<List
				title="Pinned stops"
				pins={stop_pins}
				sources={pinned_stops.sources}
				type="stop"
				height_calc={calculate_stop_height}
				items_before_scroll={2}
				ssr_min={0}
				list_class="max-h-[25dvh]"
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
			container_class="h-full"
			sources={nearby_stops}
			pins={stop_pins}
			header_slot={locate_button}
			height_calc={calculate_stop_height}
			loading={catalogs_loading || (position === undefined && !position_denied)}
		/>
		<!-- {:else}
			<p class="p-4 text-sm text-neutral-400">
				Unable to get nearby stops. Please allow location access and refresh the page.
			</p>
		{/if} -->
	</div>
</div>
