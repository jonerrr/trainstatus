<script lang="ts">
	import { BusFront, CircleX, TrainFront } from 'lucide-svelte';
	import { createTabs, melt } from '@melt-ui/svelte';
	import { onMount } from 'svelte';
	import { derived, writable, type Writable } from 'svelte/store';
	import SearchWorker from '$lib/search_worker?worker';
	import { type Stop } from '$lib/api';
	import type { BusStop } from '$lib/bus_api';
	import { stops as stop_store, bus_stops as bus_stop_store, monitored_routes } from '$lib/stores';
	import Trigger from '$lib/components/Stop/Trigger.svelte';
	import BusTrigger from '$lib/components/Stop/BusTrigger.svelte';
	import List from '$lib/components/List.svelte';
	// import { cubicInOut } from 'svelte/easing';
	// import { crossfade } from 'svelte/transition';

	const {
		elements: { root, list, content, trigger },
		states: { value }
	} = createTabs({
		defaultValue: 'Train'
	});

	// const triggers = ['Train', 'Bus'];

	export let title: string = 'Stops';
	export let stop_ids: Writable<string[]> = writable([]);
	export let bus_stop_ids: Writable<number[]> = writable([]);
	// show search bar on bottom
	export let show_search: boolean = false;
	// show ask for location button
	export let show_location: boolean = false;
	// set a max height for the list
	export let manage_height: boolean = true;

	const stops = derived([stop_ids, stop_store], ([$stop_ids, $stop_store]) => {
		// this preserves the order of stop_ids but its slower
		const st = show_location
			? ($stop_ids
					.map((id) => $stop_store.find((stop) => stop.id === id))
					.filter(Boolean) as Stop[])
			: $stop_store.filter((st) => $stop_ids.includes(st.id));
		return st;
	});

	const bus_stops = derived([bus_stop_ids, bus_stop_store], ([$bus_stop_ids, $bus_stop_store]) => {
		// this preserves the order of stop_ids but its slower
		const st = show_location
			? ($bus_stop_ids
					.map((id) => $bus_stop_store.find((stop) => stop.id === id))
					.filter(Boolean) as BusStop[])
			: $bus_stop_store.filter((st) => $bus_stop_ids.includes(st.id));

		// monitor routes so we get trip/times for them
		$monitored_routes = [
			...new Set([...st.map((s) => s.routes.map((r) => r.id)).flat(), ...$monitored_routes])
		].slice(0, 15);

		return st;
	});

	// from https://www.okupter.com/blog/svelte-debounce
	const debounce = (callback: Function, wait = 200) => {
		let timeout: ReturnType<typeof setTimeout>;

		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => callback(...args), wait);
		};
	};

	let search_el: HTMLInputElement;

	let search = 'loading';
	let search_term = '';
	let searchWorker: Worker;

	function clearSearch() {
		// reset stop ids
		$stop_ids = $stop_store.slice(0, 15).map((s) => s.id);
		$bus_stop_ids = $bus_stop_store.slice(0, 15).map((s) => s.id);
		// search_el.value = '';
		search_term = '';
	}

	let list_el: List;
	function searchStops(e: { target: { value: string } }) {
		// If search is empty, clear search and show all stops
		if (e.target.value === '') {
			clearSearch();
			return;
		}

		search_term = e.target.value;
		searchWorker.postMessage({
			type: 'search',
			payload: { search_term, search_type: $value }
		});
	}

	onMount(() => {
		if (show_search) {
			// create worker
			searchWorker = new SearchWorker();
			// listen for messages
			searchWorker.addEventListener('message', (e) => {
				const { type, payload } = e.data;

				if (type === 'ready') search = 'ready';

				if (type === 'results' && payload.results.length) {
					if (payload.search_type === 'Train') $stop_ids = payload.results;
					else if (payload.search_type === 'Bus') $bus_stop_ids = payload.results;

					if (payload.results && payload.results.length < 6) {
						list_el.scrollIntoView();
					}
				}
			});
			// initialize when the component mounts
			searchWorker.postMessage({ type: 'load' });

			// watch if they change modes, and rerun search if they do
			value.subscribe((value) => {
				if (search === 'ready' && search_term !== '') {
					searchWorker.postMessage({
						type: 'search',
						payload: { search_term, search_type: value }
					});
					// make sure there is a search el otherwise we get undefined error
				} else if (search_el) {
					clearSearch();
				}
			});
		}
	});
</script>

<List bind:show_search bind:manage_height bind:this={list_el} bind:title>
	<div slot="title">
		{#if show_location}
			<slot name="location" />
		{/if}
	</div>

	<div slot="train" class="divide-y divide-neutral-800">
		{#if $stops}
			{#each $stops as stop (stop?.id)}
				<Trigger {stop} />
			{/each}
		{/if}
	</div>

	<div slot="bus" class="divide-y divide-neutral-800">
		{#if $bus_stops}
			{#each $bus_stops as stop (stop?.id)}
				<BusTrigger {stop} />
			{/each}
		{/if}
	</div>

	<!-- TODO: prevent virtual keyboard from blocking results (use window.visualViewport.height to calculate max height of stops list or virtual keyboard api whenever that comes out) -->
	{#if show_search}
		<div class="relative">
			<input
				bind:this={search_el}
				bind:value={search_term}
				on:input={debounce(searchStops)}
				type="search"
				placeholder={search === 'ready' ? 'Search stops' : 'Loading search...'}
				class="search-stops text-indigo-200 max-w-[calc(100vw-10px)] pl-10 z-20 w-full h-12 rounded bg-neutral-900 shadow-2xl border-neutral-800/20 ring-1 ring-inset ring-neutral-700 placeholder:text-neutral-400 focus:ring-2 focus:ring-inset focus:ring-indigo-600"
			/>
			<button
				aria-label="Clear search"
				class="z-30 w-6 h-6 text-indigo-600 hover:text-indigo-700 active:text-indigo-700 absolute right-2 my-auto top-1/2 transform -translate-y-1/2"
				on:click={clearSearch}
			>
				<CircleX />
			</button>
		</div>
	{/if}
	<!-- </div> -->
</List>

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
