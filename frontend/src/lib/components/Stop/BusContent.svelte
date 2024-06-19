<script lang="ts">
	import { derived } from 'svelte/store';
	import { bus_stops, bus_routes, bus_stop_times, monitored_routes } from '$lib/stores';
	import BusTrigger from '$lib/components/Trip/BusTrigger.svelte';
	import BusIcon from '$lib/components/BusIcon.svelte';

	export let stop_id: number;

	// const stop = derived(bus_stops, ($stops) => {
	// 	return $stops.find((s) => s.id === stop_id);
	// });
	$: stop = $bus_stops.find((s) => s.id === stop_id);
	$: stop_route_ids = stop?.routes.map((r) => r.id);
	$: stop_routes = $bus_routes.filter((r) => stop_route_ids?.includes(r.id));

	// make sure route is being monitored (might not be if dialog is opened by query param)
	$: stop_routes.forEach((route) => {
		if (!$monitored_routes.includes(route.id)) {
			$monitored_routes.push(route.id);
		}
	});

	const stop_times = derived(bus_stop_times, ($bus_stop_times) => {
		const st = $bus_stop_times.filter((st) => st.arrival > new Date() && st.stop_id === stop_id);

		return st
			.map((st) => {
				const arrival = st.arrival.getTime();
				const now = new Date().getTime();
				const eta = (arrival - now) / 1000 / 60;

				st.eta = eta;
				return st;
			})
			.sort((a, b) => a.eta! - b.eta!);
	});
	// TODO: get how many stops away the bus is
</script>

{#if stop}
	<div class="p-4">
		<div class="flex gap-1">
			<!-- TODO: fix extra space when wrapping -->
			<div class={`flex flex-wrap gap-1 my-auto`}>
				{#each stop_routes as route}
					<BusIcon {route} />
				{/each}
			</div>
			<div class="flex flex-col gap-1">
				<span class="text-xs text-neutral-300">#{stop.id}</span>

				<h2 class="font-bold text-xl text-indigo-300">{stop.name}</h2>
			</div>
		</div>

		<!-- TODO: get bus transfers and also subway transfers -->
		<!-- {#if $stop.transfers.length}
		<div class="flex gap-2 pb-1 items-center flex-wrap">
			<h2 class="text-lg">Transfers:</h2>

			{#each $stop.transfers as stop_id}
				<Transfer {stop_id} />
			{/each}
		</div>
	{/if} -->

		<div
			class="flex flex-col gap-1 border overflow-auto max-h-96 border-neutral-800 rounded shadow-lg bg-neutral-900/50 text-indigo-400"
		>
			{#if $stop_times.length}
				{#each $stop_times as stop_time}
					<BusTrigger
						{stop_time}
						{stop}
						route={stop_routes.find((r) => r.id === stop_time.route_id)}
					/>
				{/each}
			{:else}
				<h2 class="text-neutral-300 text-center">No upcoming buses</h2>
			{/if}
		</div>
	</div>
{:else}
	<h2>Invalid stop ID</h2>
{/if}
