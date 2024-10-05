<script lang="ts">
	import { Locate, LocateOff, LocateFixed } from 'lucide-svelte';
	import { page } from '$app/stores';
	import type { Stop } from '$lib/static';
	import { persisted_rune, haversine, stop_pins_rune } from '$lib/util.svelte';
	// import { stop_times, monitored_routes } from '$lib/stop_times.svelte';
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

				nearby_train_stops = all_train_stops
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

				nearby_bus_stops = all_bus_stops
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

	// $inspect(bus_stops, train_stops, $page.data.stops);

	if (location_status.value === 'granted' || location_status.value === 'loading') {
		get_nearby_stops();
	}

	// $effect(() => {
	// else if (location_status.value === 'loading') {
	// 	get_nearby_stops();
	// 	// TODO: reset to neverasked instead maybe
	// }
	// });

	// const stop_pin_rune = persisted_rune<number[]>('stop_pins', []);

	// const pinned_stops = $page.data.stops.filter(stop => stop_pin_rune.value.includes(stop.id));
	const pinned_stops = $derived(
		$page.data.stops.filter((stop) => stop_pins_rune.value.includes(stop.id))
	);
	// use reduced to get pinned bus stops and train stops
	const { pinned_bus_stops, pinned_train_stops } = $derived.by(() => {
		return pinned_stops.reduce(
			(acc, stop) => {
				if (stop.type === 'bus') {
					acc.pinned_bus_stops.push(stop);
				} else if (stop.type === 'train') {
					acc.pinned_train_stops.push(stop);
				}
				return acc;
			},
			{ pinned_bus_stops: [], pinned_train_stops: [] }
		);
	});
</script>

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

<!-- {@const pin_rune = persisted_rune<number[]>('stop_pins', [])} -->

{#snippet stop_button(stop: Stop<'bus' | 'train'>, large: boolean)}
	<StopButton {stop} pin_rune={stop_pins_rune} {large} />
{/snippet}

{#if pinned_stops.length}
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
