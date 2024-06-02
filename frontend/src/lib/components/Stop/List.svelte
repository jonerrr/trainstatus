<script lang="ts">
	import FlexSearch from 'flexsearch';
	import { derived } from 'svelte/store';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { type Stop } from '$lib/api_new';
	import { stops as stop_store } from '$lib/stores';
	import StopDialog from '$lib/components/Stop/Dialog.svelte';
	import { onMount } from 'svelte';

	export let title: string = 'Stops';
	export let stop_ids: string[] | null = [];
	// show search bar on bottom
	export let show_search: boolean = false;
	// show ask for location button
	export let show_location: boolean = false;
	// TODO: better loading internally
	// export let loading: boolean = false;

	$: stops = derived(stop_store, ($stop_store) => {
		if (!stop_ids) return $stop_store.slice(0, 20);
		// const st = $stop_store.filter((st) => stop_ids.includes(st.id));
		// return st;
		// this preserves the order of stop_ids but its slower
		const st = show_location
			? (stop_ids.map((id) => $stop_store.find((stop) => stop.id === id)).filter(Boolean) as Stop[])
			: $stop_store.filter((st) => stop_ids.includes(st.id));
		return st;
	});

	let stops_index: FlexSearch.Index;

	function searchStops(e) {
		const results = stops_index.search(e.target.value);
		console.log(results);
		if (results.length && results.length < 15) {
			stop_ids = results.map((id) => id.toString());
		}
	}

	onMount(() => {
		if (show_search) {
			stops_index = new FlexSearch.Index({ tokenize: 'forward' });

			$stop_store.forEach((stop) => {
				stops_index.add(stop.id, stop.name);
			});
		}
	});
</script>

<div class={`overflow-auto text-indigo-200 bg-neutral-800/90 border border-neutral-700 p-1`}>
	<div class="flex gap-2">
		<div class="font-semibold text-lg text-indigo-300">{title}</div>

		{#if show_location}
			<slot name="location" />
		{/if}
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

{#if show_search}
	<input
		on:input={searchStops}
		type="search"
		placeholder="Search stops"
		class="search-stops fixed pl-10 p-3 border border-neutral-700 z-40 bottom-16 w-full h-12 rounded bg-neutral-600"
	/>
{/if}

<style lang="postcss">
	.search-stops {
		background-image: url('/search.svg');

		background-position: 10px 10px;
		background-repeat: no-repeat;
	}
</style>
