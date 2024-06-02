<script lang="ts">
	import { derived } from 'svelte/store';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { type Stop } from '$lib/api_new';
	import { stops as stop_store } from '$lib/stores';
	import StopDialog from '$lib/components/Stop/Dialog.svelte';

	export let title: string = 'Stops';
	export let stop_ids: string[] = [];
	// show search bar on bottom
	export let show_search: boolean = false;
	// show ask for location button
	export let show_location: boolean = false;

	// TODO: better loading internally
	export let loading: boolean = false;

	let loading_location = false;

	$: stops = derived(stop_store, ($stop_store) => {
		// const st = $stop_store.filter((st) => stop_ids.includes(st.id));
		// return st;
		// this preserves the order of stop_ids but its slower
		const st = show_location
			? (stop_ids.map((id) => $stop_store.find((stop) => stop.id === id)).filter(Boolean) as Stop[])
			: $stop_store.filter((st) => stop_ids.includes(st.id));
		return st;
	});
</script>

<div
	class={`relative max-h-[40%] overflow-auto text-indigo-200 bg-neutral-800/90 border border-neutral-700 p-1`}
>
	<div class="">
		<div class="flex gap-2">
			<div class="font-semibold text-lg text-indigo-300">{title}</div>

			{#if show_location}
				<slot name="location" />
			{/if}
		</div>
	</div>
	{#if $stops}
		{#each $stops as stop (stop?.id)}
			<div
				class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl my-1 hover:bg-neutral-900 px-1"
				transition:slide={{ easing: quintOut, axis: 'y' }}
			>
				<StopDialog {stop} />
			</div>
		{/each}
	{/if}
</div>
