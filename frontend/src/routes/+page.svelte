<script lang="ts">
	import { Locate, LocateOff, LocateFixed } from 'lucide-svelte';
	import { page } from '$app/stores';
	import type { Stop } from '$lib/static';
	import { persisted_rune, haversine } from '$lib/util.svelte';
	import { stop_times, monitored_routes } from '$lib/stop_times.svelte';
	import List from '$lib/List.svelte';
	import StopButton from '$lib/Stop/Button.svelte';
	import Icon from '$lib/Icon.svelte';

	let train_stops = $state<Stop<'train'>[]>([]);
	let bus_stops = $state<Stop<'bus'>[]>([]);

	const location_status = persisted_rune<'unknown' | 'loading' | 'granted' | 'denied'>(
		'location_status',
		'unknown'
	);

	function get_nearby_stops() {
		location_status.value = 'loading';
		navigator.geolocation.getCurrentPosition(
			(position) => {
				const {
					all_bus_stops,
					all_train_stops
				}: { all_bus_stops: Stop<'bus'>[]; all_train_stops: Stop<'train'>[] } =
					$page.data.stops.reduce(
						(acc, stop) => {
							if (stop.type === 'bus') {
								acc.all_bus_stops.push(stop);
							} else if (stop.type === 'train') {
								acc.all_train_stops.push(stop);
							}
							return acc;
						},
						{ all_bus_stops: [], all_train_stops: [] }
					);

				// console.log(all_bus_stops, all_train_stops, $page.data.stops);

				train_stops = all_train_stops
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
					.slice(0, 20);

				bus_stops = all_bus_stops
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
					.slice(0, 20);

				location_status.value = 'granted';
			},
			(e) => {
				console.error('Error getting location', e);
				location_status.value = 'denied';
			}
		);
	}

	// $inspect(bus_stops, train_stops, $page.data.stops);

	if (location_status.value === 'granted' || location_status.value === 'loading') {
		get_nearby_stops();
	}

	$effect(() => {
		// else if (location_status.value === 'loading') {
		// 	get_nearby_stops();
		// 	// TODO: reset to neverasked instead maybe
		// }
	});

	const stop_pin_rune = persisted_rune<number[]>('stop_pins', []);
</script>

<!-- <button
	class="text-indigo-100 bg-indigo-500 rounded"
	onclick={() => {
		const route = $page.data.routes[Math.floor(Math.random() * $page.data.routes.length)];
		console.log(route);
		monitored_routes.push(route.id);

		// stop_times.monitor_route(route.route_id);
	}}>add random bus route</button
> -->

{#snippet locate_button()}
	<button
		onclick={get_nearby_stops}
		aria-label="Nearby stops"
		class="bg-neutral-800 text-white rounded p-1 active:bg-neutral-700 hover:bg-neutral-700"
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

<!-- {@const pin_rune = persisted_rune<number[]>('stop_pins', [])} -->

{#snippet bus_tab(stops: Stop<'bus'>[])}
	{#each stops as stop}
		<StopButton {stop} pin_rune={stop_pin_rune} />
	{/each}
{/snippet}

{#snippet train_tab()}
	<!-- {@const pin_rune = persisted_rune<number[]>('stop_pins', [])} -->
	{#each train_stops as stop}
		<StopButton {stop} pin_rune={stop_pin_rune} />
	{/each}
{/snippet}

<List title="Pinned Stops" bus_tab={bus_tab(bus_stops)} {train_tab} min_items={2} />

<List title="Nearby Stops" {bus_tab} {train_tab} {locate_button} min_items={2} />
