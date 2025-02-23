<script lang="ts">
	import { Locate, LocateOff, LocateFixed } from 'lucide-svelte';
	import { untrack } from 'svelte';
	import { page } from '$app/state';
	import {
		type Route,
		type Stop,
		calculate_route_height,
		calculate_stop_height,
		is_bus,
		is_train
	} from '$lib/static';
	import {
		persisted_rune,
		haversine,
		stop_pins_rune,
		trip_pins_rune,
		route_pins_rune
	} from '$lib/util.svelte';
	import List from '$lib/List.svelte';
	import {
		type BusTripData,
		calculate_trip_height,
		is_bus_route,
		is_train_route,
		type TrainTripData,
		type Trip,
		trips
	} from '$lib/trips.svelte';

	const { pinned_bus_stops, pinned_train_stops } = $derived(
		stop_pins_rune.value
			.map((id) => page.data.stops[id])
			.reduce(
				(acc: { pinned_bus_stops: Stop<'bus'>[]; pinned_train_stops: Stop<'train'>[] }, stop) => {
					if (is_bus(stop)) {
						acc.pinned_bus_stops.push(stop);
					} else if (is_train(stop)) {
						acc.pinned_train_stops.push(stop);
					}
					return acc;
				},
				{ pinned_bus_stops: [], pinned_train_stops: [] }
			)
	);

	const { pinned_bus_routes, pinned_train_routes } = $derived(
		route_pins_rune.value
			.map((id) => page.data.routes[id])
			.reduce(
				(acc: { pinned_bus_routes: Route[]; pinned_train_routes: Route[] }, route) => {
					if (route.route_type === 'bus') {
						acc.pinned_bus_routes.push(route);
					} else {
						acc.pinned_train_routes.push(route);
					}
					return acc;
				},
				{ pinned_bus_routes: [], pinned_train_routes: [] }
			)
	);

	// TODO: maybe shouldn't remove trips if current_time was specified
	$effect.pre(() => {
		trips.trips;
		const valid_trips = untrack(() => trip_pins_rune.value.filter((id) => trips.trips.has(id)));

		// console.log('removing old trip pins');
		trip_pins_rune.value = valid_trips;
	});

	// need to define interface here bc my IDE was acting up
	interface AccumulatedTrips {
		pinned_bus_trips: Trip<BusTripData, Route>[];
		pinned_train_trips: Trip<TrainTripData, Route>[];
	}

	// TODO: prevent pinned trips from updating twice
	const { pinned_bus_trips, pinned_train_trips } = $derived(
		trip_pins_rune.value
			.map((id) => trips.trips.get(id)!)
			.reduce(
				(acc: AccumulatedTrips, trip) => {
					if (!trip) {
						console.log('trip not found');
					}
					const route = page.data.routes[trip.route_id];

					if (is_bus_route(route, trip)) {
						acc.pinned_bus_trips.push({ ...trip, route });
						// monitored_trip_routes.push(route.id);
					} else if (is_train_route(route, trip)) {
						acc.pinned_train_trips.push({ ...trip, route });
					}
					return acc;
				},
				{ pinned_bus_trips: [], pinned_train_trips: [] }
			)
	);

	let nearby_train_stops = $state<Stop<'train'>[]>([]);
	let nearby_bus_stops = $state<Stop<'bus'>[]>([]);

	const location_status = persisted_rune<'unknown' | 'loading' | 'granted' | 'denied'>(
		'location_status',
		'unknown'
	);

	function get_nearby_stops() {
		location_status.value = 'loading';
		navigator.geolocation.getCurrentPosition(
			(position) => {
				nearby_train_stops = page.data.train_stops
					.map((stop: Stop<'train'>) => {
						const distance = haversine(
							position.coords.latitude,
							position.coords.longitude,
							stop.lat,
							stop.lon
						);
						return { ...stop, distance };
					})
					.sort((a, b) => a.distance - b.distance);

				nearby_bus_stops = page.data.bus_stops
					.map((stop: Stop<'bus'>) => {
						const distance = haversine(
							position.coords.latitude,
							position.coords.longitude,
							stop.lat,
							stop.lon
						);
						return { ...stop, distance };
					})
					.sort((a, b) => a.distance - b.distance);
				// .slice(0, 70);

				location_status.value = 'granted';
			},
			(e) => {
				console.error('Error getting location', e);
				location_status.value = 'denied';
			}
		);
	}

	if (location_status.value === 'granted' || location_status.value === 'loading') {
		get_nearby_stops();
	}

	// use this to calculate the height for the nearby stops list
	let list_height = $state(0);
	let pin_list_height = $state(0);
	const nearby_list_height = $derived(list_height - pin_list_height);

	// $inspect({ nearby_list_height });
</script>

{#snippet locate_button()}
	<button
		onclick={get_nearby_stops}
		class="locate-button {location_status.value === 'granted'
			? 'locate-active'
			: location_status.value === 'denied'
				? 'locate-denied'
				: 'locate-inactive'}"
		aria-label="Nearby stops"
		title="Nearby stops"
	>
		{#if location_status.value === 'denied'}
			<LocateOff />
		{:else if location_status.value === 'granted'}
			<LocateFixed />
		{:else if location_status.value === 'loading'}
			<Locate class="animate-spin" />
		{:else}
			<Locate />
		{/if}
	</button>
{/snippet}

<!--  overflow-hidden -->
<div class="flex flex-col max-h-[calc(100dvh-10.5rem)]" bind:offsetHeight={list_height}>
	<!-- Pinned items section - no scroll -->
	<div class="flex-none overflow-hidden" bind:offsetHeight={pin_list_height}>
		{#if trip_pins_rune.value.length}
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
		{/if}

		{#if route_pins_rune.value.length}
			<List
				title="Pinned Routes"
				bus_data={pinned_bus_routes}
				train_data={pinned_train_routes}
				pin_rune={route_pins_rune}
				type="route"
				height_calc={calculate_route_height}
				items_before_scroll={2}
				class="max-h-[25dvh]"
			/>
		{/if}

		<!-- TODO: loading pinned stops in SSR always puts the 1 as the first -->
		{#if stop_pins_rune.value.length}
			<List
				title="Pinned stops"
				bus_data={pinned_bus_stops}
				train_data={pinned_train_stops}
				pin_rune={stop_pins_rune}
				type="stop"
				height_calc={calculate_stop_height}
				items_before_scroll={2}
				ssr_min={0}
				class="max-h-[25dvh]"
			/>
		{/if}
	</div>

	<div>
		<List
			title="Nearby Stops"
			type="stop"
			style="max-height: {nearby_list_height}px"
			bus_data={nearby_bus_stops}
			train_data={nearby_train_stops}
			pin_rune={stop_pins_rune}
			{locate_button}
			height_calc={calculate_stop_height}
		/>
	</div>
</div>

<style>
	@reference "../app.css";

	.locate-button {
		@apply relative flex items-center justify-center
			 p-1 rounded-lg
			 transition-all duration-200
			 shadow-lg;
	}

	.locate-active {
		@apply bg-blue-600 text-white;
	}

	.locate-denied {
		@apply bg-red-600 hover:bg-red-500 active:bg-red-700 text-white;
	}

	.locate-inactive {
		@apply bg-neutral-700 hover:bg-neutral-600 active:bg-neutral-800 text-white;
	}
</style>
