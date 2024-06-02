<script lang="ts">
	import { melt } from '@melt-ui/svelte';
	import { derived } from 'svelte/store';
	import { type Direction, type Trip, stop_time_store, trip_store, stop_store } from '$lib/api_new';
	import { Dialog } from '$lib/components/Dialog';
	import List from '$lib/components/List.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import TripPreview from '$lib/components/trip/Preview.svelte';

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
</script>

<!-- TODO: fix max-h -->
<List loading={false} class="h-96">
	{#if $stop_times}
		{#each $stop_times as stop_time}
			<Dialog.Trigger name={stop_time.stop_id}>
				<div
					class="w-full border-neutral-700 bg-neutral-800 rounded border shadow-2xl hover:bg-neutral-900 px-1 text-neutral-300"
				>
					<!-- TODO: show current stop / how many stops away -->
					<div class="flex gap-2 items-center justify-between mx-1">
						<div class="flex gap-2 items-center">
							<Icon name={stop_time.route_id} />
							<div>
								{stop_time.eta?.toFixed(0)}m
							</div>
						</div>
						<!-- <div class="text-right">
							{$stop_store.find((s) => s.id === trip.stop_times[trip.stop_times.length - 1].stop_id)
								?.name}
						</div> -->
					</div>
				</div>
			</Dialog.Trigger>

			<Dialog.Content name={stop_time.stop_id} let:title let:description let:close>
				<div class="flex items-center gap-2 py-1" use:melt={title}>
					<Icon name={stop_time.route_id} /> title
				</div>

				<div use:melt={description}>description</div>
				<button
					class="z-40 text-indigo-400 font-bold absolute bottom-0 right-0 rounded p-2 m-6 shadow-xl bg-neutral-900/75 active:bg-neutral-800 hover:bg-neutral-800"
					use:melt={close}>Close</button
				>
			</Dialog.Content>
		{/each}
	{:else}
		<div class="text-indigo-300 text-center">No trips found</div>
	{/if}
</List>
