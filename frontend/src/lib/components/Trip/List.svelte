<script lang="ts">
	import { BusFront, TrainFront } from 'lucide-svelte';
	import { createTabs, melt } from '@melt-ui/svelte';
	import { onMount } from 'svelte';
	import { derived, writable, type Writable } from 'svelte/store';
	import { type Stop } from '$lib/api';
	import type { BusStop } from '$lib/bus_api';
	import {
		stops as stop_store,
		bus_stops as bus_stop_store,
		monitored_routes,
		trips,
		bus_trips,
		pinned_trips
	} from '$lib/stores';
	import List from '$lib/components/List.svelte';
	import Trigger from '$lib/components/Trip/ListTrigger.svelte';

	const {
		elements: { root, list, content, trigger },
		states: { value }
	} = createTabs({
		defaultValue: 'Train'
	});

	// const triggers = ['Train', 'Bus'];

	export let title: string = 'Trips';
	export let trip_ids: Writable<string[]> = writable([]);
	export let bus_trip_ids: Writable<string[]> = writable([]);
	// show search bar on bottom

	const wanted_trips = derived([trip_ids, trips], ([$trip_ids, $trip_store]) => {
		// this preserves the order of stop_ids but its slower
		return $trip_store.filter((st) => $trip_ids.includes(st.id));
	});

	// remove from pinned trips if it no longer exists
	$: $pinned_trips = $pinned_trips.filter((p) => $wanted_trips.find((t) => t.id === p));

	// calculate height of list
	// const item_heights: number[] = [];
	// $: min_h = item_heights.slice(0, 2).reduce((acc, cur) => acc + cur, 0);
	$: min_h = 50;
</script>

<List bind:min_h>
	<div use:melt={$root} class="flex border border-neutral-800 flex-col rounded-xl shadow-lg">
		<div class="flex pb-1 justify-between">
			<div class="flex gap-2">
				<div class="font-semibold text-lg text-indigo-300">{title}</div>
			</div>

			<div
				use:melt={$list}
				class="grid grid-cols-2 bg-neutral-900 rounded shrink-0 overflow-x-auto text-indigo-100 border border-neutral-500"
				aria-label="List"
			>
				<button
					use:melt={$trigger('Train')}
					class="trigger px-2 rounded-l relative border-neutral-400 border-r data-[state=active]:bg-indigo-800"
				>
					<TrainFront />
				</button>
				<button
					use:melt={$trigger('Bus')}
					class="px-2 trigger rounded-r relative border-neutral-400 border-l data-[state=active]:bg-indigo-800"
				>
					<BusFront />
				</button>
			</div>
		</div>
		<!-- TODO: use melt $content instead of if statement -->
		<div class={`flex flex-col gap-1 max-h-[calc(100dvh-4rem)]`}>
			{#if $value === 'Train'}
				{#if $wanted_trips}
					{#each $wanted_trips as trip (trip.id)}
						<Trigger {trip} />
					{/each}
				{/if}
			{:else if $value === 'Bus'}
				Coming soon (sorry)
			{/if}
		</div>
	</div>
</List>
