<script lang="ts">
	import { Locate, LocateOff, LocateFixed } from 'lucide-svelte';
	import { page } from '$app/stores';
	import { type Route, type Stop, is_bus, is_train } from '$lib/static';
	import {
		persisted_rune,
		haversine,
		stop_pins_rune,
		trip_pins_rune,
		route_pins_rune
	} from '$lib/util.svelte';
	import List from '$lib/List.svelte';
	import StopButton from '$lib/Stop/Button.svelte';
	import RouteButton from '$lib/Route/Button.svelte';
	import TripButton from '$lib/Trip/Button.svelte';
	import {
		type BusTripData,
		is_bus_route,
		is_train_route,
		type TrainTripData,
		type Trip,
		trips
	} from '$lib/trips.svelte';
	import { untrack } from 'svelte';
	// import { monitored_routes } from '$lib/stop_times.svelte';

	const { pinned_bus_stops, pinned_train_stops } = $derived(
		stop_pins_rune.value
			.map((id) => $page.data.stops[id])
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
			.map((id) => $page.data.routes[id])
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

	$effect.pre(() => {
		console.log('removing old trip pins');
		untrack(
			() => (trip_pins_rune.value = trip_pins_rune.value.filter((id) => trips.trips.has(id)))
		);
	});

	// need to define interface here bc my IDE was acting up
	interface AccumulatedTrips {
		pinned_bus_trips: Trip<BusTripData, Route>[];
		pinned_train_trips: Trip<TrainTripData, Route>[];
	}

	const { pinned_bus_trips, pinned_train_trips } = $derived(
		trip_pins_rune.value
			.map((id) => trips.trips.get(id)!)
			.reduce(
				(acc: AccumulatedTrips, trip) => {
					const route = $page.data.routes[trip.route_id];

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

	// $effect(() => {
	// 	if (monitored_trip_routes.length) {
	// 		console.log('monitoring pinned trip bus routes');
	// 		monitored_routes.set('pinned_trip', monitored_trip_routes);
	// 	}
	// });

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
				nearby_train_stops = $page.data.train_stops
					.map((stop: Stop<'train'>) => {
						const distance = haversine(
							position.coords.latitude,
							position.coords.longitude,
							stop.lat,
							stop.lon
						);
						return { ...stop, distance };
					})
					.sort((a, b) => a.distance - b.distance)
					.slice(0, 13);

				nearby_bus_stops = $page.data.bus_stops
					.map((stop: Stop<'bus'>) => {
						const distance = haversine(
							position.coords.latitude,
							position.coords.longitude,
							stop.lat,
							stop.lon
						);
						return { ...stop, distance };
					})
					.sort((a, b) => a.distance - b.distance)
					.slice(0, 13);

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

	// let pin_list_heights: number = $state(0);
</script>

<!-- TODO: better initial loading animation -->

{#snippet trip_button(trip: Trip<TrainTripData | BusTripData, Route>)}
	<TripButton {trip} pin_rune={trip_pins_rune} />
{/snippet}

{#snippet locate_button()}
	<button
		onclick={get_nearby_stops}
		class:bg-neutral-800={location_status.value === 'granted'}
		aria-label="Nearby stops"
		class="text-white rounded p-1 bg-neutral-700 active:bg-neutral-600 hover:bg-neutral-600"
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

{#snippet stop_button(stop: Stop<'bus' | 'train'>)}
	<StopButton {stop} pin_rune={stop_pins_rune} />
{/snippet}

{#snippet route_button(route: Route)}
	<RouteButton {route} pin_rune={route_pins_rune} />
{/snippet}

<!-- <div bind:offsetHeight={pin_list_heights}> -->
{#if trip_pins_rune.value.length}
	<List
		title="Pinned Trips"
		bus_data={pinned_bus_trips}
		train_data={pinned_train_trips}
		button={trip_button}
		min_items={2}
	/>
{/if}

{#if route_pins_rune.value.length}
	<List
		title="Pinned Routes"
		bus_data={pinned_bus_routes}
		train_data={pinned_train_routes}
		button={route_button}
		min_items={2}
	/>
{/if}

{#if stop_pins_rune.value.length}
	<List
		title="Pinned stops"
		button={stop_button}
		bus_data={pinned_bus_stops}
		train_data={pinned_train_stops}
		min_items={2}
		monitor_routes
	/>
{/if}
<!-- </div> -->

<!-- by only showing nearby stops only when the nearby stops array is not empty, the list won't automatically switch tabs bc theres no data yet -->
<!-- {#if nearby_bus_stops.length || nearby_train_stops.length} -->
<List
	title="Nearby Stops"
	button={stop_button}
	bus_data={nearby_bus_stops}
	train_data={nearby_train_stops}
	{locate_button}
	class="mb-16"
	monitor_routes
/>
<!-- {/if} -->
