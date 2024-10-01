<script lang="ts">
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime.js';
	import { derived } from 'svelte/store';
	import { type Direction, type Stop } from '$lib/api';
	import { stop_times as stop_time_store, data_at } from '$lib/stores';
	import Icon from '$lib/components/Icon.svelte';

	dayjs.extend(relativeTime);

	export let stop: Stop;
	export let direction: Direction;
	export let route_id: string;
	// if the route is usually at the stop
	export let base_route: boolean = true;

	// if user provided date, we use that as now

	const stop_times = derived(stop_time_store, ($stop_time_store) => {
		const now = $data_at ?? new Date();

		const st = $stop_time_store.filter(
			(st) =>
				st.stop_id === stop.id &&
				st.direction === direction &&
				st.route_id === route_id &&
				st.arrival > now
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

<!-- only show no trips if the route usually stops there -->
<div class="flex gap-1">
	{#if base_route || $stop_times.length}
		<div class="flex gap-1">
			<Icon name={route_id} />
		</div>
		<div class="flex gap-2">
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
	{/if}
</div>
