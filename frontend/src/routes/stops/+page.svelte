<script lang="ts">
	import { CircleX } from 'lucide-svelte';
	import { page } from '$app/stores';
	import { type Stop } from '$lib/static';
	import List from '$lib/List.svelte';
	import { persisted_rune, stop_pins_rune } from '$lib/util.svelte';
	import { StopSearch } from '$lib/search.svelte';

	let bus_stops = $state<Stop<'bus'>[]>($page.data.bus_stops);
	let train_stops = $state<Stop<'train'>[]>($page.data.train_stops);

	let selected_tab = $state(persisted_rune<'train' | 'bus'>('stops_tab', 'train'));

	const search = new StopSearch($page.data.bus_stops, $page.data.train_stops);

	// let search_el: HTMLInputElement;
	let search_input: string = $state('');
	// let search_term = $derived.by(debounce(() => search_input, 300));
	// $inspect(search_term);
	function clear_search() {
		// reset stop ids
		bus_stops = $page.data.bus_stops;
		train_stops = $page.data.train_stops;

		search_input = '';
	}

	let search_timeout: number;

	$effect(() => {
		selected_tab;
		search_input;
		clearTimeout(search_timeout);

		search_timeout = setTimeout(() => {
			if (search_input === '') {
				clear_search();
			} else {
				const results = search.search(search_input, selected_tab.value);
				// not sure if its safe to assume that the results are always the same type
				if (results.length) {
					if (selected_tab.value === 'train') train_stops = results as Stop<'train'>[];
					else if (selected_tab.value === 'bus') bus_stops = results as Stop<'bus'>[];
				}
			}
		}, 250);
	});
</script>

<svelte:head>
	<title>Stops</title>
</svelte:head>

<List
	title="Stops"
	type="stop"
	bus_data={bus_stops}
	train_data={train_stops}
	pin_rune={stop_pins_rune}
	monitor_routes
	class="max-h-[calc(100dvh-13.1rem)]"
	auto_scroll
/>

<div class="absolute bottom-1 w-full">
	<input
		name="search"
		bind:value={search_input}
		type="search"
		placeholder="Search stops"
		class="search-stops w-full h-12 text-neutral-200 pl-10 rounded bg-neutral-900 border-neutral-800 ring-1 ring-inset ring-neutral-600 focus:ring-neutral-400 focus:border-neutral-400 focus:ring-2 focus:ring-inset placeholder:text-neutral-400"
	/>
	<button
		aria-label="Clear search"
		class="z-30 w-6 h-6 text-neutral-200 hover:text-neutral-400 active:text-neutral-400 absolute right-2 my-auto top-1/2 transform -translate-y-1/2"
		onclick={clear_search}
	>
		<CircleX />
	</button>
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
