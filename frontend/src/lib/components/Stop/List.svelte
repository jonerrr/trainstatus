<script lang="ts">
	import FlexSearch from 'flexsearch';
	import { CircleX } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { derived } from 'svelte/store';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { type Stop } from '$lib/api';
	import { stops as stop_store } from '$lib/stores';
	import List from '$lib/components/List.svelte';
	import StopDialog from '$lib/components/Stop/Dialog.svelte';

	export let title: string = 'Stops';
	export let stop_ids: string[] | null = [];
	// show search bar on bottom
	export let show_search: boolean = false;
	// show ask for location button
	export let show_location: boolean = false;
	// TODO: better loading internally
	// export let loading: boolean = false;

	$: stops = derived(stop_store, ($stop_store) => {
		if (!stop_ids) return $stop_store.slice(0, 15);
		// this preserves the order of stop_ids but its slower
		const st = show_location
			? (stop_ids.map((id) => $stop_store.find((stop) => stop.id === id)).filter(Boolean) as Stop[])
			: $stop_store.filter((st) => stop_ids!.includes(st.id));
		return st;
	});

	let stops_index: FlexSearch.Index;

	// from https://www.okupter.com/blog/svelte-debounce
	const debounce = (callback: Function, wait = 50) => {
		let timeout: ReturnType<typeof setTimeout>;

		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => callback(...args), wait);
		};
	};

	let list_el: HTMLDivElement;
	function searchStops(e: any) {
		// If search is empty, clear search and show all stops
		if (e.target.value === '') {
			stop_ids = null;
			return;
		}

		const results = stops_index.search(e.target.value);
		// console.log(results);
		if (results.length) {
			// Get first 12 results
			stop_ids = results.map((id) => id.toString()).slice(0, 12);
		}

		list_el.scrollIntoView({ behavior: 'smooth' });
	}

	let search_el: HTMLInputElement;

	function clearSearch() {
		stop_ids = null;
		search_el.value = '';
	}

	onMount(() => {
		if (show_search) {
			stops_index = new FlexSearch.Index({ tokenize: 'forward' });

			$stop_store.forEach((stop) => {
				stops_index.add(stop.id, stop.name);
			});
		}
	});

	// Prevent list from getting squished
	$: min_h = $stops.length ? 'min-h-[140px]' : 'min-h-[30px]';
</script>

<!-- Switch from vh because on mobile searchbar blocks bottom -->
<div
	bind:this={list_el}
	class={`relative overflow-auto text-indigo-200 bg-neutral-800/90 border border-neutral-700 p-1 ${min_h} ${show_search ? 'max-h-[calc(100vh-11rem)]' : 'max-h-[calc(100vh-4rem)]'}`}
>
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
	<div class="relative">
		<input
			bind:this={search_el}
			on:input={debounce(searchStops)}
			type="search"
			placeholder="Search stops"
			class="search-stops text-indigo-200 max-w-[calc(100vw-10px)] md:max-w-[60%] fixed bottom-16 pl-10 z-20 w-full h-12 rounded bg-neutral-800 shadow-2xl border-neutral-800/20 ring-1 ring-inset ring-neutral-700 placeholder:text-neutral-400 focus:ring-2 focus:ring-inset focus:ring-indigo-600"
		/>
		<button
			aria-label="Clear search"
			class="z-30 w-6 h-6 text-indigo-600 hover:text-indigo-700 active:text-indigo-700 absolute inset-y-0 right-2 pt-4"
			on:click={clearSearch}
		>
			<CircleX />
		</button>
	</div>
{/if}

<style lang="postcss">
	.search-stops {
		background-image: url('/search.svg');

		background-position: 10px 10px;
		background-repeat: no-repeat;
	}

	/* Remove default styles from search */
	input[type='search']::-webkit-search-decoration,
	input[type='search']::-webkit-search-cancel-button,
	input[type='search']::-webkit-search-results-button,
	input[type='search']::-webkit-search-results-decoration {
		-webkit-appearance: none;
	}
</style>
