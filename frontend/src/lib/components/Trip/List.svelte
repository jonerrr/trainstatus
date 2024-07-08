<script lang="ts">
	import { BusFront, TrainFront } from 'lucide-svelte';
	import { createTabs, melt } from '@melt-ui/svelte';
	import { derived, writable } from 'svelte/store';
	import { monitored_routes, trips, bus_trips, pinned_trips, pinned_bus_trips } from '$lib/stores';
	import List from '$lib/components/List.svelte';
	import Trigger from '$lib/components/Trip/ListTrigger.svelte';
	import BusTrigger from '$lib/components/Trip/ListBusTrigger.svelte';

	const {
		elements: { root, list, content, trigger },
		states: { value }
	} = createTabs({
		defaultValue: 'Train'
	});

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
	$: $pinned_trips = $pinned_trips.filter((p) => $wanted_trips.find((t) => t.id === p));
	// TODO: figure out a better way to remove old bus trips
	// $: $pinned_bus_trips = $pinned_bus_trips.filter((p) => $wanted_bus_trips.find((t) => t.id === p));
</script>

<List bind:manage_height>
	<div use:melt={$root} class="flex border border-neutral-800 flex-col rounded-xl shadow-lg">
		<div id="list-item" class="flex pb-1 justify-between">
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
				{#if $wanted_trips.length}
					{#each $wanted_trips as trip (trip.id)}
						<Trigger {trip} />
					{/each}
				{/if}
			{:else if $value === 'Bus'}
				{#if $wanted_bus_trips.length}
					{#each $wanted_bus_trips as trip (trip.id)}
						<BusTrigger {trip} />
					{/each}
				{/if}
			{/if}
		</div>
	</div>
</List>
