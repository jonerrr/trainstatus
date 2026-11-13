<script lang="ts">
	import { afterNavigate } from '$app/navigation';
	import { page } from '$app/state';

	import type { CatalogStop, Source } from '#lib/client/index.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import List from '#lib/List.svelte';
	import { stopPins } from '#lib/pins.svelte.js';
	import { searchStops } from '#lib/search.js';
	import { LocalStorage } from '#lib/storage.svelte.js';
	import { calculateStopHeight, createThrottled } from '#lib/util.svelte.js';

	import { CircleX, Search } from '@lucide/svelte';

	let selectedSource = $state(
		new LocalStorage<Source>('stopsTab', page.data.selectedSources[0] ?? 'mta_subway')
	);

	// Ensure selected source is still enabled
	$effect(() => {
		if (!page.data.selectedSources.includes(selectedSource.current)) {
			selectedSource.current = page.data.selectedSources[0] ?? 'mta_subway';
		}
	});

	const transit = getTransitData();
	const catalogResources = transit.catalogs(() => page.data.selectedSources);
	const catalogs = $derived(catalogResources.current.filter((r) => r.state === 'ready'));
	const allStops = $derived(
		Object.fromEntries(catalogs.map(({ source, value }) => [source, value.stops])) as Partial<
			Record<Source, readonly CatalogStop[]>
		>
	);

	const searchInput = createThrottled('', 150);
	let searchElement: HTMLInputElement | undefined;

	afterNavigate(() => {
		// Avoid opening an on-screen keyboard on touch-first devices.
		if (window.matchMedia('(hover: hover) and (pointer: fine)').matches) {
			searchElement?.focus({ preventScroll: true });
		}
	});

	function matchingStops(term: string): readonly CatalogStop[] | undefined {
		const catalog = catalogs.find((c) => c.source === selectedSource.current)?.value;
		if (!catalog || term === '') return undefined;

		// Route IDs take precedence when a stop shares the same ID.
		const idCheck = term.toUpperCase();
		const route = catalog.routesById.get(idCheck);
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
		const stop = catalog.stopsById.get(idCheck);

		// TODO: no-match fallback is unfinished; see frontend/TESTING.md
		const results = searchStops(catalog, term);
		// An exact ID must not hide name matches such as "49 St".
		if (stop && !results.some((match) => match.id === stop.id)) return [...results, stop];
		return results.length ? results : undefined;
	}

	const visibleStops = $derived.by(() => {
		const matches = matchingStops(searchInput.current);
		return (matches ? { ...allStops, [selectedSource.current]: matches } : allStops) as Partial<
			Record<Source, CatalogStop[]>
		>;
	});
	// $inspect(visibleStops);

	function clearSearch() {
		searchInput.set('');
	}
</script>

<!-- TODO: fix space between navbar and search bar -->
<!-- TODO: fix searching and when items are shorter than viewport, a scrollbar shows up when it shouldn't (issue with calculating totalHeight before dom updates or something) -->
<!-- TODO: maybe show indicator when filtered for specific route / stop -->
<!-- TODO: maybe integrate the search with the List component (using attachments or something).-->
<div class="flex h-full flex-col">
	<List
		title="Stops"
		type="stop"
		sources={visibleStops}
		pins={stopPins}
		autoScroll
		containerClass="flex-1 min-h-0"
		bind:selectedSource
		heightCalc={calculateStopHeight}
	/>

	<div class="w-full">
		<div class="relative">
			<Search
				class="absolute top-1/2 left-3 z-20 h-5 w-5 -translate-y-1/2 text-muted transition-colors duration-200 group-focus-within:text-secondary"
			/>
			<!-- TODO: maybe add autocomplete="off" -->
			<input
				name="search"
				bind:this={searchElement}
				bind:value={searchInput.get, searchInput.set}
				type="search"
				placeholder="Search stops by name, ID, or route..."
				class="h-12 w-full rounded-none border
			   border-border-subtle/50
			   bg-surface-1
			   pr-10
			   pl-10
			   text-secondary shadow-lg
			   ring-1 shadow-black/10
			   ring-neutral-600/30 backdrop-blur-xs ring-inset
			   placeholder:text-disabled
			   focus:border-neutral-500/50
			   focus:bg-surface-1
			   focus:ring-2
			   focus:ring-neutral-500/50"
			/>

			<button
				aria-label="Clear search"
				class="absolute top-1/2 right-3 h-6
			   w-6 -translate-y-1/2
			   text-muted
			   transition-all
			   duration-200
			   hover:text-secondary active:scale-95"
				onclick={clearSearch}
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
