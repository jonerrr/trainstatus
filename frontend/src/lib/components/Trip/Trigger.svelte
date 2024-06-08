<script lang="ts">
	import { derived } from 'svelte/store';
	import { pushState } from '$app/navigation';
	import { type Direction, type Stop } from '$lib/api';
	import { stops, stop_times as stop_time_store } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';

	export let stop_id: string;
	export let direction: Direction;

	const stop_times = derived(stop_time_store, ($stop_time_store) => {
		const st = $stop_time_store.filter(
			(st) => st.arrival > new Date() && st.stop_id === stop_id && st.direction === direction
		);

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

	function last_stop(trip_id: string): Stop {
		const stop_times = $stop_time_store.filter((st) => st.trip_id === trip_id);
		const last_stop_time = stop_times[stop_times.length - 1];
		return $stops.find((s) => s.id === last_stop_time.stop_id)!;
	}
</script>

<div class="flex flex-col overflow-auto max-h-96">
	{#if $stop_times.length}
		{#each $stop_times as stop_time}
			<button
				class="w-full flex justify-between items-center py-1"
				on:click={() => {
					pushState('', {
						dialog_open: true,
						dialog_id: stop_time.trip_id,
						dialog_type: 'trip'
					});
				}}
			>
				<div
					class="w-full border-neutral-700 bg-neutral-800 rounded border shadow-2xl hover:bg-neutral-900 px-1 text-neutral-300"
				>
					<div class="flex gap-12 items-center justify-between mx-1">
						<div class="flex gap-2 items-center">
							<Icon name={stop_time.route_id} />
							<div>
								{stop_time.eta?.toFixed(0)}m
							</div>
						</div>
						<div class="text-right text-nowrap">
							{last_stop(stop_time.trip_id).name}
						</div>
					</div>
				</div>
			</button>
		{/each}
	{:else}
		<div class="text-neutral-300 text-center">No trips found</div>
	{/if}
</div>
