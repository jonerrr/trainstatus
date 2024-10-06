<script lang="ts">
	import { Locate, LocateOff, LocateFixed } from 'lucide-svelte';
	import { page } from '$app/stores';
	import { type Stop, is_bus, is_train } from '$lib/static';
	import {
		persisted_rune,
		haversine,
		stop_pins_rune,
		trip_pins_rune,
		route_pins_rune
	} from '$lib/util.svelte';
	import List from '$lib/List.svelte';
	import StopButton from '$lib/Stop/Button.svelte';

	let nearby_train_stops = $state<Stop<'train'>[]>([]);
	let nearby_bus_stops = $state<Stop<'bus'>[]>([]);

	const location_status = persisted_rune<'unknown' | 'loading' | 'granted' | 'denied'>(
		'location_status',
		'unknown'
	);
	// console.log($page.data.routes);
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

	// $inspect($page.data);

	// $inspect(bus_stops, train_stops, $page.data.stops);

	if (location_status.value === 'granted' || location_status.value === 'loading') {
		get_nearby_stops();
	}

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
</script>

<!-- TODO: better initial loading animation -->

{#snippet locate_button()}
	<button
		onclick={get_nearby_stops}
		class:bg-neutral-800={location_status.value === 'granted'}
		aria-label="Nearby stops"
		class=" text-white rounded p-1 active:bg-neutral-600 hover:bg-neutral-600"
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

{#snippet stop_button(stop: Stop<'bus' | 'train'>, large: boolean)}
	<StopButton {stop} pin_rune={stop_pins_rune} {large} />
{/snippet}

{#if stop_pins_rune.value.length}
	<List
		title="Pinned stops"
		button={stop_button}
		bus_data={pinned_bus_stops}
		train_data={pinned_train_stops}
		min_items={2}
	/>
{/if}

<List
	title="Nearby Stops"
	button={stop_button}
	bus_data={nearby_bus_stops}
	train_data={nearby_train_stops}
	{locate_button}
	class="mb-16"
/>
