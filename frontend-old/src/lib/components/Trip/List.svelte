<script lang="ts">
	import { derived, writable } from 'svelte/store';
	import { monitored_routes, trips, bus_trips, pinned_trips, pinned_bus_trips } from '$lib/stores';
	import List from '$lib/components/List.svelte';
	import Trigger from '$lib/components/Trip/ListTrigger.svelte';
	import BusTrigger from '$lib/components/Trip/ListBusTrigger.svelte';

	export let title: string = 'Trips';
	export let trip_ids = writable<string[]>([]);
	export let manage_height = true;
	export let bus_trip_ids = writable<string[]>([]);
	// show search bar on bottom

	const wanted_trips = derived([trip_ids, trips], ([$trip_ids, $trip_store]) => {
		// this preserves the order of stop_ids but its slower
		return $trip_store.filter((st) => $trip_ids.includes(st.id));
	});

	const wanted_bus_trips = derived(
		[bus_trip_ids, bus_trips],
		([$bus_trip_ids, $bus_trip_store]) => {
			const trip_info = $bus_trip_ids.map((t) => t.split('_'));
			const trip_ids = trip_info.map((t) => t[0]);
			const trip_routes = trip_info.map((t) => t[1]);
			$monitored_routes = [...new Set([...trip_routes, ...$monitored_routes])].slice(0, 15);

			// this preserves the order of stop_ids but its slower
			return $bus_trip_store.filter((st) => trip_ids.includes(st.id));
		}
	);

	console.log($wanted_bus_trips);

	// remove from pinned trips if it no longer exists
	// $: $pinned_trips = $pinned_trips.filter((p) => $wanted_trips.find((t) => t.id === p));
	// TODO: figure out a better way to remove old bus trips
	// $: $pinned_bus_trips = $pinned_bus_trips.filter((p) => $wanted_bus_trips.find((t) => t.id === p));
</script>

<List bind:manage_height bind:title>
	<div slot="train" class="divide-y divide-neutral-800">
		{#if $wanted_trips.length}
			{#each $wanted_trips as trip (trip.id)}
				<Trigger {trip} />
			{/each}
		{/if}
	</div>

	<div slot="bus" class="divide-y divide-neutral-800">
		{#if $wanted_bus_trips.length}
			{#each $wanted_bus_trips as trip (trip.id)}
				<BusTrigger {trip} />
			{/each}
		{/if}
	</div>
</List>
