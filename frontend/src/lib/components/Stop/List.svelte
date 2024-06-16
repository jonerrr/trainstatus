<script lang="ts">
	import { BusFront, CircleX, TrainFront } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { derived } from 'svelte/store';
	import SearchWorker from '$lib/search_worker?worker';
	import { type Stop } from '$lib/api';
	import { stops as stop_store, bus_stops as bus_stop_store, monitored_routes } from '$lib/stores';
	import Trigger from '$lib/components/Stop/Trigger.svelte';
	import BusTrigger from '$lib/components/Stop/BusTrigger.svelte';
	import List from '$lib/components/List.svelte';
	import { createTabs, melt } from '@melt-ui/svelte';
	import type { BusStop } from '$lib/bus_api';
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
	export let stop_ids: string[] | null = [];
	export let bus_stop_ids: number[] | null = [];
	// show search bar on bottom
	export let show_search: boolean = false;
	// show ask for location button
	export let show_location: boolean = false;
	// set a max height for the list
	export let expand: boolean = true;

	$: stops = derived(stop_store, ($stop_store) => {
		if (!stop_ids) return $stop_store.slice(0, 15);
		// this preserves the order of stop_ids but its slower
		const st = show_location
			? (stop_ids.map((id) => $stop_store.find((stop) => stop.id === id)).filter(Boolean) as Stop[])
			: $stop_store.filter((st) => stop_ids!.includes(st.id));
		return st;
	});

	// TODO: not sure if we want to ignore shuttles
	$: bus_stops = derived(bus_stop_store, ($bus_stop_store) => {
		if (!bus_stop_ids) return $bus_stop_store.slice(0, 15);
		// this preserves the order of stop_ids but its slower
		const st = show_location
			? (bus_stop_ids
					.map((id) => $bus_stop_store.find((stop) => stop.id === id))
					.filter(Boolean) as BusStop[])
			: $bus_stop_store.filter((st) => bus_stop_ids!.includes(st.id));
		return st;
	});

	// TODO: make sure this wont go in some insane loop and crash the page
	$: $monitored_routes = [
		...new Set([...$monitored_routes, ...$bus_stops.map((s) => s.routes.map((r) => r.id)).flat()])
	];

	// from https://www.okupter.com/blog/svelte-debounce
	const debounce = (callback: Function, wait = 50) => {
		let timeout: ReturnType<typeof setTimeout>;

		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => callback(...args), wait);
		};
	};

	let list_el: List;
	function searchStops(e: any) {
		// If search is empty, clear search and show all stops
		if (e.target.value === '') {
			stop_ids = null;
			bus_stop_ids = null;
			return;
		}

		search_term = e.target.value;
		searchWorker.postMessage({
			type: 'search',
			payload: { search_term, search_type: $value === 'Train' ? 't' : 'b' }
		});

		// this is to make sure that the results are in view on mobile even when keyboard is open
		// list_el.scrollIntoView({ behavior: 'smooth' });
	}

	let search_el: HTMLInputElement;

	function clearSearch() {
		stop_ids = null;
		search_el.value = '';
	}

	let search = 'loading';
	let search_term = '';
	let searchWorker: Worker;

	onMount(() => {
		// create worker
		searchWorker = new SearchWorker();
		// listen for messages
		searchWorker.addEventListener('message', (e) => {
			const { type, payload } = e.data;

			if (type === 'ready') search = 'ready';

			if (type === 'results') {
				console.log(payload);
				if (payload.search_type === 't') stop_ids = payload.results;
				else if (payload.search_type === 'b') bus_stop_ids = payload.results;

				if (payload.results.length < 6) {
					list_el.scrollIntoView();
				}
			}
		});
		// initialize when the component mounts
		searchWorker.postMessage({ type: 'load' });
	});

	// calculate height of list
	// const item_heights: number[] = [];
	// $: min_h = item_heights.slice(0, 2).reduce((acc, cur) => acc + cur, 0);
	$: min_h = 50;
</script>

<List bind:expand bind:min_h bind:this={list_el}>
	<div
		use:melt={$root}
		class="flex border border-neutral-800 flex-col rounded-xl shadow-lg data-[orientation=vertical]:flex-row"
	>
		<div class="flex gap-2 pointer-events-auto pb-1">
			<div class="font-semibold text-lg text-indigo-300">{title}</div>

			{#if show_location}
				<slot name="location" />
			{/if}

			<div
				use:melt={$list}
				class="grid grid-cols-2 bg-neutral-900 rounded shrink-0 overflow-x-auto text-indigo-100 border border-neutral-500"
				aria-label="List"
			>
				<button
					use:melt={$trigger('Train')}
					class="trigger px-2 rounded-l relative border-neutral-400 border-r data-[state=active]:bg-indigo-800"
				>
					<TrainFront />
				</button>
				<button
					use:melt={$trigger('Bus')}
					class="px-2 trigger rounded-r relative border-neutral-400 border-l data-[state=active]:bg-indigo-800"
				>
					<BusFront />
				</button>
			</div>
		</div>
		<!-- TODO: use melt $content instead of if statement -->
		<div
			class={`flex flex-col gap-1 ${show_search ? 'max-h-[calc(100dvh-13rem)] overflow-auto' : 'max-h-[calc(100dvh-4rem)]'}`}
		>
			{#if $value === 'Train'}
				{#if $stops}
					<!-- TODO: figure out a way to make list length only 3 rows long (maybe get innerheight from trigger component and put in writable) -->
					{#each $stops as stop (stop?.id)}
						<Trigger {stop} />
					{/each}
				{/if}
			{:else if $value === 'Bus'}
				{#if $bus_stops}
					{#each $bus_stops as stop (stop?.id)}
						<BusTrigger {stop} />
					{/each}
				{/if}
			{/if}
		</div>

		<!-- TODO: prevent virtual keyboard from blocking results (use window.visualViewport.height to calculate max height of stops list or virtual keyboard api whenever that comes out) -->
		{#if show_search}
			<div class="relative">
				<input
					bind:this={search_el}
					on:input={debounce(searchStops)}
					type="search"
					placeholder="Search stops"
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
	</div>
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
