<script lang="ts">
	import { derived } from 'svelte/store';
	import { bus_stops, bus_routes } from '$lib/stores';
	// import Trigger from '$lib/components/Trip/Trigger.svelte';
	import BusIcon from '$lib/components/BusIcon.svelte';

	export let stop_id: number;

	// const stop = derived(bus_stops, ($stops) => {
	// 	return $stops.find((s) => s.id === stop_id);
	// });
	$: stop = $bus_stops.find((s) => s.id === stop_id);
	$: stop_route_ids = stop?.routes.map((r) => r.id);
	$: stop_routes = $bus_routes.filter((r) => stop_route_ids?.includes(r.id));
</script>

{#if stop}
	<div class="flex items-center gap-2 py-1">
		<div class="flex flex-wrap gap-1">
			{#each stop_routes as route}
				<BusIcon {route} />
			{/each}
		</div>

		<h2 class="font-bold text-xl w-[80%] text-indigo-300">{stop.name}</h2>
	</div>

	<!-- {#if $stop.transfers.length}
		<div class="flex gap-2 pb-1 items-center flex-wrap">
			<h2 class="text-lg">Transfers:</h2>

			{#each $stop.transfers as stop_id}
				<Transfer {stop_id} />
			{/each}
		</div>
	{/if} -->

	<div>
		<div
			class="flex border border-neutral-800 flex-col rounded-xl shadow-lg data-[orientation=vertical]:flex-row bg-neutral-900/50 text-indigo-400"
		>
			Bus stuff
		</div>
	</div>
{:else}
	<h2>Invalid stop ID</h2>
{/if}
