<script lang="ts">
	import { page } from '$app/state';

	import List from '$lib/List.svelte';
	import { stop_pins } from '$lib/pins.svelte';
	import { StopSearch } from '$lib/search.svelte';
	import { LocalStorage } from '$lib/storage.svelte';
	import { calculate_stop_height } from '$lib/util.svelte';

	import { CircleX, Search } from '@lucide/svelte';
	import { type Source, type Stop } from '@trainstatus/client';
	import { Throttled } from 'runed';

	let selected_source = $state(
		new LocalStorage<Source>('stops_tab', page.data.selected_sources[0] ?? 'mta_subway')
	);

	// Ensure selected source is still enabled
	$effect(() => {
		if (!page.data.selected_sources.includes(selected_source.current)) {
			selected_source.current = page.data.selected_sources[0] ?? 'mta_subway';
		}
	});

	const stop_search = new StopSearch(page.data.stops);

	let search_input: string = $state('');
	const throttled_search_input = new Throttled(() => search_input, 150);

	const visible_stops = $derived.by(() => {
		// console.log('searching for', throttled_search_input.current);

		// default to all stops if search input empty
		if (throttled_search_input.current === '') {
			return page.data.stops;
		}

		// check if its a stop / route id
		// TODO: maybe add back id length check (since stop_id must have like at least 3 chars. but its different for each source, so maybe not worth it)
		const id_check = throttled_search_input.current.toUpperCase();

		const stop = page.data.stops_by_id[selected_source.current]?.[id_check];
		if (stop) {
			// TODO: maybe preserve the other source search results instead of implicitly resetting them
			return {
				...page.data.stops,
				[selected_source.current]: [stop]
			};
		}

		const route = page.data.routes_by_id[selected_source.current]?.[id_check];
		if (route) {
			const new_stops: Stop[] = [];
			// store sequences for sorting later
			const route_stop_sequences: Record<string, number> = {};

			for (const s of page.data.stops[selected_source.current] ?? []) {
				const r = s.routes.find((r) => r.route_id === route.id);

				if (
					r &&
					r.data.source === 'mta_subway' &&
					// TODO: maybe include other stop types
					['full_time', 'part_time', 'rush_hour'].includes(r.data.stop_type)
				) {
					new_stops.push(s);
					route_stop_sequences[s.id] = r.stop_sequence;
				}
			}
			// TODO: maybe add sorting by route stop sequence back
			if (new_stops.length) {
				return {
					...page.data.stops,
					[selected_source.current]: new_stops.sort(
						(a, b) => route_stop_sequences[a.id] - route_stop_sequences[b.id]
					)
				};
			}
		}

		const search_results = stop_search.query(
			throttled_search_input.current,
			selected_source.current
		);
		// TODO: maybe add some kind of "no results found" state when search_results is empty (and search input isn't empty)
		return {
			...page.data.stops,
			[selected_source.current]:
				search_results.length > 0
					? search_results
					: (page.data.stops[selected_source.current] ?? [])
		};
	}) as Partial<Record<Source, Stop[]>>;
	// $inspect(visible_stops);

	function clear_search() {
		search_input = '';
	}
</script>

<!-- TODO: fix space between navbar and search bar -->
<!-- TODO: fix searching and when items are shorter than viewport, a scrollbar shows up when it shouldn't (issue with calculating total_height before dom updates or something) -->
<!-- TODO: maybe show indicator when filtered for specific route / stop -->
<!-- TODO: maybe integrate the search with the List component (using attachments or something).-->
<div class="flex h-full flex-col">
	<List
		title="Stops"
		type="stop"
		sources={visible_stops}
		pins={stop_pins}
		auto_scroll
		container_class="flex-1 min-h-0"
		bind:selected_source
		height_calc={calculate_stop_height}
	/>

	<div class="w-full">
		<div class="relative">
			<Search
				class="absolute top-1/2 left-3 z-20 h-5 w-5 -translate-y-1/2 text-neutral-400 transition-colors duration-200 group-focus-within:text-neutral-200"
			/>
			<!-- TODO: maybe add autocomplete="off" -->
			<input
				name="search"
				bind:value={search_input}
				type="search"
				placeholder="Search stops by name, ID, or route..."
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
