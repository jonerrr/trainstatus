<script lang="ts">
	import { CircleX, Search } from 'lucide-svelte';
	import { page } from '$app/state';
	import { always_stop, calculate_stop_height, type Stop } from '$lib/static';
	import List from '$lib/List.svelte';
	import { debounce, persisted_rune, stop_pins_rune } from '$lib/util.svelte';
	import { StopSearch } from '$lib/search.svelte';

	interface StopObj {
		train: Stop<'train'>[];
		bus: Stop<'bus'>[];
	}

	const stops: StopObj = $state({
		bus: page.data.bus_stops,
		train: page.data.train_stops
	});

	let selected_tab = $state(persisted_rune<'train' | 'bus'>('stops_tab', 'train'));

	const search = new StopSearch(page.data.bus_stops, page.data.train_stops);

	// let search_el: HTMLInputElement;
	let search_input: string = $state('');
	// let search_term = $derived.by(debounce(() => search_input, 300));
	// $inspect(search_term);
	function clear_search() {
		// reset stop ids
		stops['bus'] = page.data.bus_stops;
		stops['train'] = page.data.train_stops;
		// bus_stops = page.data.bus_stops;
		// page.data.bus_stops.sort((a, b) => b.name.length - a.name.length).slice(0, 200);

		// train_stops = page.data.train_stops;

		search_input = '';
	}

	interface StopWithRouteSequence extends Stop<'train' | 'bus'> {
		route_stop_sequence: number;
	}

	$effect(() => {
		selected_tab.value;
		search_input;

		// TODO: figure out how to safely set the type of stops and remove ts-ignore-error
		debounce(() => {
			if (search_input === '') {
				clear_search();
			} else {
				// try searching for a stop id
				const as_stop_id = parseInt(search_input);
				// shortest stop id is 3
				if (search_input.length > 2 && !isNaN(as_stop_id)) {
					console.log('searching for stop id', as_stop_id);
					const stop = page.data.stops[as_stop_id];
					if (stop && stop.route_type === selected_tab.value) {
						//@ts-expect-error
						stops[selected_tab.value] = [stop];
					}
				} else {
					const search_route = page.data.routes[search_input.toUpperCase()];
					if (search_route && search_route.route_type === selected_tab.value) {
						console.log('searching for route id', search_route);

						// sort by route stop_sequence
						if (selected_tab.value === 'bus') {
							const new_stops: StopWithRouteSequence[] = [];
							for (const s of page.data.bus_stops) {
								const route = s.routes.find((r) => r.id === search_route.id);
								if (route) {
									new_stops.push({ ...s, route_stop_sequence: route.stop_sequence });
								}
							}

							// sort the new stops and convert back to Stop<'bus'> type
							stops['bus'] = new_stops
								.sort((a, b) => a.route_stop_sequence - b.route_stop_sequence)
								.map(({ route_stop_sequence, ...stop }) => stop) as Stop<'bus'>[];
						} else {
							const new_stops: StopWithRouteSequence[] = [];
							for (const s of page.data.train_stops) {
								const route = s.routes.find((r) => r.id === search_route.id);
								if (route && always_stop.includes(route.type ?? '')) {
									new_stops.push({ ...s, route_stop_sequence: route.stop_sequence });
								}
							}
							// sort the new stops and convert back to Stop<'train'> type
							stops['train'] = new_stops
								.sort((a, b) => a.route_stop_sequence - b.route_stop_sequence)
								.map(({ route_stop_sequence, ...stop }) => stop) as Stop<'train'>[];
						}
					} else {
						// search for stops
						const results = search.search(search_input, selected_tab.value);
						// not sure if its safe to assume that the results are always the same type
						if (results.length) {
							//@ts-expect-error
							stops[selected_tab.value] = results;
						}
					}
				}
			}
		}, 150)();
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
		bus_data={stops.bus}
		train_data={stops.train}
		pin_rune={stop_pins_rune}
		auto_scroll
		class="max-h-[calc(100dvh-13.5rem)] grow"
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
			   backdrop-blur-xs
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
