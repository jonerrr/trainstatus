<script lang="ts">
	import { page } from '$app/state';

	import List from '$lib/List.svelte';
	import { StopSearch } from '$lib/search.svelte';
	import { calculate_stop_height } from '$lib/static';
	import { type Pins, stop_pins } from '$lib/stores.svelte';
	import { debounce } from '$lib/util.svelte';

	import { CircleX, Search } from '@lucide/svelte';
	import { type Source, type Stop } from '@trainstatus/client';
	import { PersistedState } from 'runed';

	// interface StopObj {
	// 	train: Stop<'train'>[];
	// 	bus: Stop<'bus'>[];
	// }

	// const stops: StopObj = $state({
	// 	bus: page.data.bus_stops,
	// 	train: page.data.train_stops
	// });

	// TODO: might need to use $state.snapshot on page.data.stops
	const stops = $state(page.data.stops);

	// let selected_tab = $state(persisted_rune<'train' | 'bus'>('stops_tab', 'train'));
	// TODO: determine default source from users preferences or something
	let selected_source = new PersistedState<Source>('stops_tab', 'mta_subway');

	const search = new StopSearch(page.data.stops);

	let search_input: string = $state('');
	function clear_search() {
		// reset stop ids
		// stops['bus'] = page.data.bus_stops;
		// stops['train'] = page.data.train_stops;
		Object.keys(stops).forEach((key) => {
			// @ts-ignore
			stops[key] = page.data.stops[key];
		});

		search_input = '';
	}

	interface StopWithRouteSequence extends Stop {
		route_stop_sequence: number;
	}

	// $effect(() => {
	// 	selected_source.current;
	// 	search_input;

	// 	// TODO: figure out how to safely set the type of stops and remove ts-ignore-error
	// 	// TODO: use debounce from runed lib
	// 	debounce(() => {
	// 		if (search_input === '') {
	// 			clear_search();
	// 		} else {
	// 			// try searching for a stop id
	// 			const as_stop_id = parseInt(search_input);
	// 			// TODO: double check if its safe to assume that route ids are always uppercase
	// 			const as_route =
	// 				page.data.routes_by_id[selected_source.current][search_input.toUpperCase()];
	// 			// shortest stop id is 3 (TODO: Double check this)
	// 			if (search_input.length > 2 && !isNaN(as_stop_id)) {
	// 				const stop = page.data.stops_by_id[selected_source.current][as_stop_id];
	// 				if (stop && stop.route_type === selected_tab.value) {
	// 					//@ts-expect-error
	// 					stops[selected_tab.value] = [stop];
	// 				}
	// 			} else if (as_route && as_route.route_type === selected_tab.value) {
	// 				const new_stops: StopWithRouteSequence[] = [];
	// 				// sort by route stop_sequence
	// 				switch (selected_tab.value) {
	// 					case 'bus':
	// 						for (const s of page.data.bus_stops) {
	// 							const route = s.routes.find((r) => r.id === as_route.id);
	// 							if (route) {
	// 								new_stops.push({ ...s, route_stop_sequence: route.stop_sequence });
	// 							}
	// 						}
	// 						break;
	// 					case 'train':
	// 						for (const s of page.data.train_stops) {
	// 							const route = s.routes.find((r) => r.id === as_route.id);
	// 							if (route && ['full_time', 'part_time', 'rush_hour'].includes(route.type ?? '')) {
	// 								new_stops.push({ ...s, route_stop_sequence: route.stop_sequence });
	// 							}
	// 						}
	// 						break;
	// 				}
	// 				if (new_stops.length) {
	// 					//@ts-expect-error
	// 					stops[selected_tab.value] = new_stops
	// 						.sort((a, b) => a.route_stop_sequence - b.route_stop_sequence)
	// 						.map(({ route_stop_sequence, ...stop }) => stop);
	// 				}
	// 			} else {
	// 				// search for stops
	// 				const results = search.search(search_input, selected_tab.value);
	// 				// not sure if its safe to assume that the results are always the same type
	// 				if (results.length) {
	// 					//@ts-expect-error
	// 					stops[selected_tab.value] = results;
	// 				}
	// 			}
	// 		}
	// 	}, 150)();
	// });
</script>

<svelte:head>
	<title>Stops | Train Status</title>
</svelte:head>
<!-- TODO: fix searching and when items are shorter than viewport, a scrollbar shows up when it shouldn't (issue with calculating total_height before dom updates or something) -->
<!-- TODO: maybe show indicator when filtered for specific route / stop -->
<div class="flex h-full flex-col">
	<List
		title="Stops"
		type="stop"
		sources={stops}
		pins={stop_pins}
		auto_scroll
		class="max-h-[calc(100dvh-13.5rem)] grow"
		bind:selected_source
		height_calc={calculate_stop_height}
	/>

	<div class="w-full">
		<div class="relative">
			<Search
				class="absolute top-1/2 left-3 z-20 h-5 w-5 -translate-y-1/2 text-neutral-400 transition-colors duration-200 group-focus-within:text-neutral-200"
			/>

			<input
				name="search"
				bind:value={search_input}
				type="search"
				placeholder="Search stops"
				class="h-12 w-full rounded border
			   border-neutral-800/50
			   bg-neutral-900
			   pr-10
			   pl-10
			   text-neutral-200 shadow-lg
			   ring-1 shadow-black/10
			   ring-neutral-600/30 backdrop-blur-xs ring-inset
			   placeholder:text-neutral-500
			   focus:border-neutral-500/50
			   focus:bg-neutral-900
			   focus:ring-2
			   focus:ring-neutral-500/50"
			/>

			<button
				aria-label="Clear search"
				class="absolute top-1/2 right-3 h-6
			   w-6 -translate-y-1/2
			   text-neutral-400
			   transition-all
			   duration-200
			   hover:text-neutral-200 active:scale-95"
				onclick={clear_search}
			>
				<CircleX />
			</button>
		</div>
	</div>
</div>

<style>
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
