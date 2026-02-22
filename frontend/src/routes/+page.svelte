<script lang="ts">
	import { page } from '$app/state';

	import List from '$lib/List.svelte';
	import { route_pins, stop_pins, trip_pins } from '$lib/pins.svelte';
	import { trip_context } from '$lib/resources/trips.svelte';
	import { calculate_route_height, calculate_stop_height } from '$lib/static';
	import { haversine } from '$lib/util.svelte';

	import { Locate, LocateFixed, LocateOff } from '@lucide/svelte';
	import type { Route, Source, Stop } from '@trainstatus/client';

	// TODO: add check that trip still exists
	// const pinned_trips = $derived(
	// 	Object.fromEntries(
	// 		Object.entries(trip_pins.current).map(([source, pins]) => [
	// 			source,
	// 			pins.map((pin) => page.data.trips_by_id[source as Source][pin])
	// 		])
	// 	) as Record<Source, Route[]>
	// );

	const pinned_stops = $derived.by(() => {
		const sources = Object.fromEntries(
			Object.entries(stop_pins.current).map(([source, pins]) => [
				source,
				pins.map((pin) => page.data.stops_by_id[source as Source][pin])
			])
		) as Record<Source, Stop[]>;
		return { sources, exists: Object.values(sources).some((pins) => pins.length > 0) };
	});

	const pinned_routes = $derived.by(() => {
		const sources = Object.fromEntries(
			Object.entries(route_pins.current).map(([source, pins]) => [
				source,
				pins.map((pin) => page.data.routes_by_id[source as Source][pin])
			])
		) as Record<Source, Route[]>;
		return { sources, exists: Object.values(sources).some((pins) => pins.length > 0) };
	});

	// TODO: maybe don't bother with saving status myself. the user can just refresh or permanently block location access in their browser settings.
	// Geolocation state
	// const location_status = new LocalStorage<'unknown' | 'granted' | 'denied'>(
	// 	'location_status',
	// 	'unknown'
	// );

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
		$inspect.trace('Setting up geolocation watch');
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
	interface StopWithDistance extends Stop {
		distance: number;
	}

	const source_order: Source[] = ['mta_subway', 'mta_bus'];

	// Compute nearby stops sorted by distance for each source
	// : Record<Source, StopWithDistance[]>
	// async function get_nearby_stops() {
	// 	console.log('Getting nearby stops...');
	// 	const result = {} as Record<Source, StopWithDistance[]>;

	// 	try {
	// 		// TODO: maybe watch events instead of getting position on demand
	// 		const position = await get_position();
	// 		location_status.current = 'granted';

	// 		for (const source of source_order) {
	// 			const data = page.data.stops[source] ?? [];

	// 			result[source] = data
	// 				.map((stop: Stop) => {
	// 					const distance = haversine(
	// 						position.coords.latitude,
	// 						position.coords.longitude,
	// 						stop.geom.Point.y,
	// 						stop.geom.Point.x
	// 					);
	// 					return { ...stop, distance };
	// 				})
	// 				.sort((a, b) => a.distance - b.distance);
	// 		}
	// 	} catch (error) {
	// 		console.error('Error getting nearby stops:', error);
	// 		location_status.current = 'denied';
	// 	}
	// 	return result;
	// }

	const nearby_stops = $derived.by(() => {
		const result = {} as Record<Source, StopWithDistance[]>;

		if (!position) return result;

		for (const source of source_order) {
			const data = page.data.stops[source] ?? [];
			result[source] = data
				.map((stop) => {
					// TODO: why can position be undefined here?
					const distance = haversine(
						position.coords.latitude,
						position.coords.longitude,
						stop.geom.Point.y,
						stop.geom.Point.x
					);
					return { ...stop, distance };
				})
				.sort((a, b) => a.distance - b.distance);
		}
		return result;
	});

	// Height calculation TODO: Fix
	let list_height = $state(0);
	let pin_list_height = $state(0);
	// const nearby_list_height = $derived(list_height - pin_list_height);
	const nearby_list_height = $state(800);
</script>

{#snippet locate_button()}
	<!-- TODO: fix onclick -->
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

<!--  overflow-hidden -->
<div class="flex max-h-[calc(100dvh-10.5rem)] flex-col" bind:offsetHeight={list_height}>
	<!-- Pinned items section - no scroll -->
	<div class="flex-none overflow-hidden" bind:offsetHeight={pin_list_height}>
		<!-- TODO: add back trip list -->
		<!-- {#if trip_pins.current.length}
			<List
				title="Pinned Trips"
				bus_data={pinned_bus_trips}
				train_data={pinned_train_trips}
				type="trip"
				pin_rune={trip_pins_rune}
				height_calc={calculate_trip_height}
				items_before_scroll={2}
				class="max-h-[25dvh]"
			/>
		{/if} -->

		{#if pinned_routes.exists}
			<List
				title="Pinned Routes"
				pins={route_pins}
				sources={pinned_routes.sources}
				type="route"
				height_calc={calculate_route_height}
				items_before_scroll={2}
				class="max-h-[25dvh]"
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
				class="max-h-[25dvh]"
			/>
		{/if}
	</div>

	<div>
		<!-- {#if nearby_stops} -->
		<List
			title="Nearby Stops"
			type="stop"
			style="max-height: {nearby_list_height}px"
			sources={nearby_stops}
			pins={stop_pins}
			header_slot={locate_button}
			height_calc={calculate_stop_height}
		/>
		<!-- {:else}
			<p class="p-4 text-sm text-neutral-400">
				Unable to get nearby stops. Please allow location access and refresh the page.
			</p>
		{/if} -->
	</div>
</div>
