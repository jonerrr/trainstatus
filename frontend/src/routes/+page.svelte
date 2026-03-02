<script lang="ts">
	import { page } from '$app/state';

	import List from '$lib/List.svelte';
	import { route_pins, stop_pins, trip_pins } from '$lib/pins.svelte';
	import { default_sources } from '$lib/resources/index.svelte';
	import { trip_context } from '$lib/resources/trips.svelte';
	import {
		calculate_route_height,
		calculate_stop_height,
		calculate_trip_height
	} from '$lib/util.svelte';
	import { haversine } from '$lib/util.svelte';

	import { Locate, LocateFixed, LocateOff } from '@lucide/svelte';
	import type { Route, Source, Stop, Trip } from '@trainstatus/client';

	const all_trips = trip_context.get();

	$effect(() => {
		for (const [source, resource] of Object.entries(all_trips)) {
			const map = resource.value;
			if (!map) continue;
			const src = source as Source;
			const current = trip_pins.current[src];
			const valid = current.filter((id) => map.has(id));
			if (valid.length !== current.length) {
				console.log(
					`Removing invalid pins for source ${src}:`,
					current.filter((id) => !map.has(id))
				);
				trip_pins.current[src] = valid;
			}
		}
	});

	const pinned_trips = $derived.by(() => {
		const sources = Object.fromEntries(
			Object.entries(trip_pins.current).map(([source, pins]) => [
				source,
				pins
					.map((pin) => all_trips[source as Source].value?.get(pin))
					.filter((t) => t !== undefined)
			])
		) as Record<Source, Trip[]>;
		return { sources, exists: Object.values(sources).some((pins) => pins.length > 0) };
	});

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

	const nearby_stops = $derived.by(() => {
		const result = {} as Record<Source, StopWithDistance[]>;

		if (!position) return result;
		// reassign to local var to prevent undefined error in map callback.
		const current_coords = position.coords;

		for (const source of default_sources) {
			const data = page.data.stops[source] ?? [];
			result[source] = data
				.map((stop) => {
					const distance = haversine(
						current_coords.latitude,
						current_coords.longitude,
						stop.geom.Point.y,
						stop.geom.Point.x
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

	<div class="flex-1 min-h-0">
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
		/>
		<!-- {:else}
			<p class="p-4 text-sm text-neutral-400">
				Unable to get nearby stops. Please allow location access and refresh the page.
			</p>
		{/if} -->
	</div>
</div>
