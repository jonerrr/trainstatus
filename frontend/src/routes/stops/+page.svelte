<script lang="ts">
	import { page } from '$app/state';

	import type { CatalogStop, Source } from '#lib/client/index.js';
	import List from '#lib/List.svelte';
	import { stop_pins } from '#lib/pins.svelte.js';
	import { search_stops } from '#lib/search.js';
	import { LocalStorage } from '#lib/storage.svelte.js';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { calculate_stop_height, createThrottled } from '#lib/util.svelte.js';

	import { CircleX, Search } from '@lucide/svelte';

	let selected_source = $state(
		new LocalStorage<Source>('stopsTab', page.data.selected_sources[0] ?? 'mta_subway')
	);

	// Ensure selected source is still enabled
	$effect(() => {
		if (!page.data.selected_sources.includes(selected_source.current)) {
			selected_source.current = page.data.selected_sources[0] ?? 'mta_subway';
		}
	});

	const transit = getTransitData();
	const catalog_resources = transit.catalogs(() => page.data.selected_sources);
	const catalogs = $derived(catalog_resources.current);
	const all_stops = $derived(
		Object.fromEntries(
			catalogs.map(({ source, catalog }) => [source, catalog?.stops ?? []])
		) as Partial<Record<Source, readonly CatalogStop[]>>
	);

	const search_input = createThrottled('', 150);

	function matching_stops(term: string): readonly CatalogStop[] | undefined {
		const catalog = catalogs.find((c) => c.source === selected_source.current)?.catalog;
		if (!catalog || term === '') return undefined;

		// Route IDs take precedence when a stop shares the same ID.
		const id_check = term.toUpperCase();
		const route = catalog.routes_by_id.get(id_check);
		if (route) {
			// eslint-disable-next-line svelte/prefer-svelte-reactivity
			const sequences = new Map<string, number>();
			for (const s of catalog.stops) {
				const r = s.routes.find((r) => r.route_id === route.id);
				if (
					r &&
					(r.data.source !== 'mta_subway' ||
						['full_time', 'part_time', 'rush_hour'].includes(r.data.stop_type))
				) {
					sequences.set(s.id, r.stop_sequence);
				}
			}
			const stops = catalog.stops.filter((s) => sequences.has(s.id));
			return route.data.source === 'mta_subway'
				? stops.sort((a, b) => sequences.get(a.id)! - sequences.get(b.id)!)
				: stops;
		}
		const stop = catalog.stops_by_id.get(id_check);
		if (stop) return [stop];

		// TODO: no-match fallback is unfinished; see frontend/TESTING.md
		const results = search_stops(catalog, term);
		return results.length ? results : undefined;
	}

	const visible_stops = $derived.by(() => {
		const matches = matching_stops(search_input.current);
		return (matches ? { ...all_stops, [selected_source.current]: matches } : all_stops) as Partial<
			Record<Source, CatalogStop[]>
		>;
	});
	// $inspect(visible_stops);

	function clear_search() {
		search_input.set('');
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
				bind:value={search_input.get, search_input.set}
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
