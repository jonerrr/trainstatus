<script lang="ts">
	import { derived } from 'svelte/store';
	import { type Direction, type Stop } from '$lib/api';
	import { stop_times as stop_time_store, data_at } from '$lib/stores';
	import Trigger from '$lib/components/Trip/Trigger.svelte';
	import List from '$lib/components/ContentList.svelte';

	export let stop: Stop;
	export let direction: Direction;

	const stop_times = derived(stop_time_store, ($stop_time_store) => {
		const now = $data_at ?? new Date();

		const st = $stop_time_store.filter(
			(st) => st.stop_id === stop.id && st.direction === direction && st.arrival > now
		);

		return st.map((st) => {
			const arrival = st.arrival.getTime();
			const eta = (arrival - now.getTime()) / 1000 / 60;

			st.eta = eta;
			return st;
		});
		// .sort((a, b) => a.eta! - b.eta!);
	});
</script>

<!-- <div class="flex gap-1 flex-col overflow-auto max-h-96"> -->
<List>
	{#if $stop_times.length}
		{#each $stop_times as stop_time (stop_time.trip_id)}
			<Trigger {stop_time} />
		{/each}
	{:else}
		<div class="text-neutral-300 text-center">No trips found</div>
	{/if}
</List>
