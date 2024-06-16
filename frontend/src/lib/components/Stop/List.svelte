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
	export let expand: boolean = true;

	const stops = derived([stop_ids, stop_store], ([$stop_ids, $stop_store]) => {
		// this preserves the order of stop_ids but its slower
		console.log($stop_ids);
		const st = show_location
			? ($stop_ids
					.map((id) => $stop_store.find((stop) => stop.id === id))
					.filter(Boolean) as Stop[])
			: $stop_store.filter((st) => $stop_ids.includes(st.id));
		return st;
	});

	// TODO: not sure if we want to ignore shuttles
	const bus_stops = derived([bus_stop_ids, bus_stop_store], ([$bus_stop_ids, $bus_stop_store]) => {
		// this preserves the order of stop_ids but its slower
		const st = show_location
			? ($bus_stop_ids
					.map((id) => $bus_stop_store.find((stop) => stop.id === id))
					.filter(Boolean) as BusStop[])
			: $bus_stop_store.filter((st) => $bus_stop_ids.includes(st.id));

		$monitored_routes = [
			...new Set([...st.map((s) => s.routes.map((r) => r.id)).flat(), ...$monitored_routes])
		].slice(0, 15);

		return st;
	});

	// from https://www.okupter.com/blog/svelte-debounce
	const debounce = (callback: Function, wait = 50) => {
		let timeout: ReturnType<typeof setTimeout>;

		return (...args: any[]) => {
			clearTimeout(timeout);
			timeout = setTimeout(() => callback(...args), wait);
		};
	};

	let search_el: HTMLInputElement;

	function clearSearch() {
		// reset stop ids
		$stop_ids = $stop_store.slice(0, 15).map((s) => s.id);
		$bus_stop_ids = $bus_stop_store.slice(0, 15).map((s) => s.id);
		search_el.value = '';
	}

	let list_el: List;
	function searchStops(e: any) {
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
	});

	// calculate height of list
	// const item_heights: number[] = [];
	// $: min_h = item_heights.slice(0, 2).reduce((acc, cur) => acc + cur, 0);
	$: min_h = 50;
</script>

<List bind:expand bind:min_h bind:this={list_el}>
	<div use:melt={$root} class="flex border border-neutral-800 flex-col rounded-xl shadow-lg">
		<div class="flex pb-1 justify-between">
			<div class="flex gap-2">
				<div class="font-semibold text-lg text-indigo-300">{title}</div>

				{#if show_location}
					<slot name="location" />
				{/if}
			</div>

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
