<script lang="ts">
	import { liveQuery } from 'dexie';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime.js';
	import { derived } from 'svelte/store';
	import Icon from '$lib/components/Icon.svelte';
	import { type Direction, type StopTime, db } from '$lib/db';
	import { stop_time_store } from '$lib/api_db';
	import type { Route } from '$lib/api';

	dayjs.extend(relativeTime);

	// export let stop_times: StopTime[];
	export let stop_id: string;
	export let direction: Direction;
	export let route: Route;

	// $: stop_times = liveQuery(async () => {
	// 	const stop_times = await db.stop_time
	// 		.where('stop_id')
	// 		.equals(stop_id)
	// 		.and((st) => st.direction === direction)
	// 		.toArray();

	// 	// .and(
	// 	// 	(st) => st.arrival >= new Date() && st.direction === direction && st.route_id === route.id
	// 	// )

	// 	// calculate etas
	// 	const stop_times_w_eta = stop_times
	// 		.filter((st) => st.direction === direction && st.route_id === route.id)
	// 		.map((st) => {
	// 			const arrival = st.arrival.getTime();
	// 			const now = new Date().getTime();
	// 			const eta = (arrival - now) / 1000 / 60;

	// 			st.eta = eta;
	// 			return st;
	// 		})
	// 		.sort((a, b) => a.eta! - b.eta!);
	// 	// console.log(stop_times_w_eta);
	// 	return stop_times_w_eta;
	// });
	const stop_times = derived(stop_time_store, ($stop_time_store) => {
		const st = $stop_time_store.filter(
			(st) =>
				st.arrival > new Date() &&
				st.stop_id === stop_id &&
				st.direction === direction &&
				st.route_id === route.id
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
</script>

<div class="flex gap-1">
	<div class="flex gap-1">
		<Icon name={route.id} />
	</div>
	<div class="flex gap-2">
		{#if $stop_times}
			{#each $stop_times.slice(0, 2) as stop_time}
				<div class="text-xs">
					{stop_time.eta.toFixed(0)}m
				</div>
			{/each}
		{/if}
	</div>
</div>
