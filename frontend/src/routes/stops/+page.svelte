<script lang="ts">
	import { CircleX, Search } from 'lucide-svelte';
	import { page } from '$app/stores';
	import { calculate_stop_height, type Stop } from '$lib/static';
	import List from '$lib/List.svelte';
	import { persisted_rune, stop_pins_rune } from '$lib/util.svelte';
	import { StopSearch } from '$lib/search.svelte';

	let bus_stops = $state<Stop<'bus'>[]>($page.data.bus_stops);
	let train_stops = $state<Stop<'train'>[]>($page.data.train_stops);

	let selected_tab = $state(persisted_rune<'train' | 'bus'>('stops_tab', 'train'));

	const search = new StopSearch($page.data.bus_stops, $page.data.train_stops);

	// console.log(calculate_stop_height($page.data.train_stops[0]), 'sheight');

	// let search_el: HTMLInputElement;
	let search_input: string = $state('');
	// let search_term = $derived.by(debounce(() => search_input, 300));
	// $inspect(search_term);
	function clear_search() {
		// reset stop ids
		bus_stops = $page.data.bus_stops;
		// $page.data.bus_stops.sort((a, b) => b.name.length - a.name.length).slice(0, 200);

		train_stops = $page.data.train_stops;

		search_input = '';
	}

	let search_timeout: number;

	$effect(() => {
		selected_tab.value;
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
		}, 150);
	});
</script>

<svelte:head>
	<title>Stops</title>
</svelte:head>
<!-- TODO: fix searching and when items are shorter than viewport, a scrollbar shows up when it shouldn't (issue with calculating total_height before dom updates or something) -->
<div class="flex flex-col h-full">
	<List
		title="Stops"
		type="stop"
		bus_data={bus_stops}
		train_data={train_stops}
		pin_rune={stop_pins_rune}
		auto_scroll
		class="max-h-[calc(100dvh-13.5rem)] flex-grow"
		bind:selected_tab
		height_calc={calculate_stop_height}
	/>

	<div class="w-full">
		<div class="relative">
			<Search
				class="z-20 absolute left-3 top-1/2 -translate-y-1/2 w-5 h-5 text-neutral-400 group-focus-within:text-neutral-200 transition-colors duration-200"
			/>

			<input
				name="search"
				bind:value={search_input}
				type="search"
				placeholder="Search stops"
				class="w-full h-12 pl-10 pr-10
			   text-neutral-200
			   bg-neutral-900
			   backdrop-blur-sm
			   rounded
			   border border-neutral-800/50
			   shadow-lg shadow-black/10
			   ring-1 ring-inset ring-neutral-600/30
			   placeholder:text-neutral-500
			   focus:ring-2
			   focus:ring-neutral-500/50
			   focus:border-neutral-500/50
			   focus:bg-neutral-900"
			/>

			<button
				aria-label="Clear search"
				class="absolute right-3 top-1/2 -translate-y-1/2
			   w-6 h-6
			   text-neutral-400
			   hover:text-neutral-200
			   active:scale-95
			   transition-all duration-200"
				onclick={clear_search}
			>
				<CircleX />
			</button>
		</div>
	</div>
</div>

<style lang="postcss">
	/* .search-stops {
		background-image: url('/search.svg');
		background-position: 10px 10px;
		background-repeat: no-repeat;
	} */

	/* Remove default styles from search */
	input[type='search']::-webkit-search-decoration,
	input[type='search']::-webkit-search-cancel-button,
	input[type='search']::-webkit-search-results-button,
	input[type='search']::-webkit-search-results-decoration {
		-webkit-appearance: none;
	}
</style>
