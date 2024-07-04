<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime.js';
	import { derived } from 'svelte/store';
	import { bus_stop_times as bus_stop_time_store, data_at } from '$lib/stores';

	dayjs.extend(relativeTime);

	export let stop_id: number;
	export let route_id: string;

	const stop_times = derived(bus_stop_time_store, ($bus_stop_time_store) => {
		const st = $bus_stop_time_store.filter(
			(st) => st.stop_id === stop_id && st.route_id === route_id
		);

		return st
			.map((st) => {
				const arrival = st.arrival.getTime();
				const now = $data_at ?? new Date();
				const eta = (arrival - now.getTime()) / 1000 / 60;

				st.eta = eta;
				return st;
			})
			.sort((a, b) => a.eta! - b.eta!);
	});
</script>

<div class="flex gap-2 pr-1">
	{#if $stop_times.length}
		{#each $stop_times.slice(0, 2) as stop_time}
			<div class="text-xs">
				{stop_time.eta?.toFixed(0)}m
			</div>
		{/each}
	{:else}
		<div class="text-xs text-neutral-400">No trips</div>
	{/if}
</div>
