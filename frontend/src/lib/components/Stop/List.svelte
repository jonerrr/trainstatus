<script lang="ts">
	import { CircleX } from 'lucide-svelte';
	import { onMount } from 'svelte';
	import { derived } from 'svelte/store';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import SearchWorker from '$lib/search-worker?worker';
	import { type Stop } from '$lib/api';
	import { stops as stop_store } from '$lib/stores';

	import Trigger from '$lib/components/Stop/Trigger.svelte';

	export let title: string = 'Stops';
	export let stop_ids: string[] | null = [];
	// show search bar on bottom
	export let show_search: boolean = false;
	// show ask for location button
	export let show_location: boolean = false;

	$: stops = derived(stop_store, ($stop_store) => {
		if (!stop_ids) return $stop_store.slice(0, 15);
		// this preserves the order of stop_ids but its slower
		const st = show_location
			? (stop_ids.map((id) => $stop_store.find((stop) => stop.id === id)).filter(Boolean) as Stop[])
			: $stop_store.filter((st) => stop_ids!.includes(st.id));
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

	let list_el: HTMLDivElement;
	function searchStops(e: any) {
		// If search is empty, clear search and show all stops
		if (e.target.value === '') {
			stop_ids = null;
			return;
		}

		search_term = e.target.value;
		searchWorker.postMessage({ type: 'search', payload: { search_term } });

		// this is to make sure that the results are in view on mobile even when keyboard is open
		list_el.scrollIntoView({ behavior: 'smooth' });
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
			type === 'ready' && (search = 'ready');
			type === 'results' && (stop_ids = payload.results);
		});
		// initialize when the component mounts
		searchWorker.postMessage({ type: 'load', payload: { stops: $stops } });
	});

	// Prevent list from getting squished
	$: min_h = $stops.length ? 'min-h-[140px]' : 'min-h-[30px]';
</script>

<div
	bind:this={list_el}
	class={`relative text-indigo-200 bg-neutral-800/90 border border-neutral-700 p-1 ${min_h} overflow-auto`}
>
	<div class="flex gap-2 pointer-events-auto">
		<div class="font-semibold text-lg text-indigo-300">{title}</div>

		{#if show_location}
			<slot name="location" />
		{/if}
	</div>
	{#if $stops}
		<div
			class={`flex flex-col gap-1 ${show_search ? 'max-h-[calc(100dvh-13rem)] overflow-auto' : 'max-h-[calc(100dvh-4rem)]'}`}
		>
			{#each $stops as stop (stop?.id)}
				<div
					class="border-neutral-600 bg-neutral-700 rounded border shadow-2xl hover:bg-neutral-900 px-1"
					transition:slide={{ easing: quintOut, axis: 'y', duration: 100 }}
				>
					<Trigger {stop} />
				</div>
			{/each}
		</div>
	{/if}

	{#if show_search}
		<div class="relative">
			<input
				bind:this={search_el}
				on:input={debounce(searchStops)}
				type="search"
				placeholder="Search stops"
				class="search-stops text-indigo-200 max-w-[calc(100vw-10px)] md:max-w-[60%] pl-10 z-20 w-full h-12 rounded bg-neutral-900 shadow-2xl border-neutral-800/20 ring-1 ring-inset ring-neutral-700 placeholder:text-neutral-400 focus:ring-2 focus:ring-inset focus:ring-indigo-600"
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
