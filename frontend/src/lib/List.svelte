<script lang="ts" module>
	import type { Source } from '#lib/client/index.js';

	/** A pinned trip row; `id` is its pin key. */
	export interface TripItem {
		id: string;
		source: Source;
		routeId: string;
		tripId: string;
	}
</script>

<script lang="ts" generics="T extends CatalogStop | CatalogRoute | TripItem">
	import type { Snippet } from 'svelte';

	import { cubicInOut } from 'svelte/easing';
	import { crossfade, slide } from 'svelte/transition';

	import { browser } from '$app/env';
	import { page } from '$app/state';

	import type { CatalogRoute, CatalogStop } from '#lib/client/index.js';
	import { getTransitData } from '#lib/data/store.svelte.js';
	import ListSkeleton from '#lib/ListSkeleton.svelte';
	import Pin from '#lib/Pin.svelte';
	import type { Pins } from '#lib/pins.svelte.js';
	import RouteButton from '#lib/route/Button.svelte';
	import { sourceIcons } from '#lib/source-icons.js';
	import { sourceConfig } from '#lib/sources.js';
	import StopButton from '#lib/stop/Button.svelte';
	import { LocalStorage } from '#lib/storage.svelte.js';
	import TripButton from '#lib/trip/Button.svelte';
	import { openModal } from '#lib/url-params.svelte.js';
	import { skeletonRowHeight } from '#lib/util.svelte.js';

	type ItemType = 'stop' | 'route' | 'trip';

	interface Props {
		// title of list
		title: string;
		// renders extra header content (like geolocate button)
		headerSlot?: Snippet;
		// item type for rendering and modal
		type: ItemType;
		// data organized by source
		sources: Partial<Record<Source, T[]>>;
		// persisted state for pinned items
		pins?: LocalStorage<Pins>;
		// persisted state for selected source tab
		selectedSource?: LocalStorage<Source>;
		// items to show before the user scrolls (for pinned lists)
		itemsBeforeScroll?: number;
		listClass?: string;
		containerClass?: string;
		// scroll list into view if there are few items
		autoScroll?: boolean;
		// height calculation function for virtualization
		heightCalc: (item: T) => number;
		// minimum number of items to render during SSR
		ssrMin?: number;
		// extra items to render before and after visible items
		overscan?: number;
		// some source's items are still loading
		loading?: boolean;
	}

	let {
		title,
		type,
		sources,
		pins,
		headerSlot,
		selectedSource = $bindable(
			new LocalStorage<Source>(
				`${title.toLowerCase().replace(/\s+([a-z])/g, (_, letter: string) => letter.toUpperCase())}Tab`,
				page.data.selectedSources[0] ?? 'mta_subway'
			)
		),
		heightCalc,
		containerClass,
		listClass,
		autoScroll = false,
		itemsBeforeScroll,
		ssrMin = 10,
		overscan = 5,
		loading
	}: Props = $props();

	const catalogs = getTransitData().catalogs(() => page.data.selectedSources);
	const pending = $derived(
		loading ?? catalogs.current.some((catalog) => catalog.state === 'loading')
	);

	const sourceEntries = $derived(
		page.data.selectedSources.map((source) => ({
			source,
			data: sources[source] ?? []
		}))
	);

	// Get available sources (those with data)
	const availableSources = $derived(sourceEntries.filter((s) => s.data.length > 0));

	let activeSource = $derived.by(() => {
		// if the selected source has no data, fall back to the first available source
		if ((sources[selectedSource.current]?.length ?? 0) > 0) {
			return selectedSource.current;
		}
		return availableSources[0]?.source;
	});

	// Get items for current source
	const items = $derived(sources[activeSource] ?? []);

	const [send, receive] = crossfade({
		duration: 300,
		easing: cubicInOut
	});

	let viewportEl = $state<HTMLDivElement>();
	let viewportHeight = $state(0);
	let scrollTop = $state(0);

	function resetScroll() {
		if (viewportEl) {
			viewportEl.scrollTo(0, 0);
		}
	}

	// Auto-scroll for lists with few items
	$effect(() => {
		if (autoScroll && viewportEl && items.length < 8) {
			viewportEl.scrollIntoView({ behavior: 'smooth' });
		}
	});

	// Reset scroll when source changes
	$effect(() => {
		// maybe move this to derived
		void activeSource;
		resetScroll();
	});

	const derivedLayout = $derived.by(() => {
		let running = 0;
		const offs = items.map((item) => {
			const top = running;
			running += heightCalc(item);
			return top;
		});
		return { offsets: offs, total: running };
	});

	// O(1) lookup
	function getItemOffset(index: number): number {
		return derivedLayout.offsets[index] ?? 0;
	}

	// Binary search for start index - O(log n) instead of O(n)
	function calculateStartIndex(offs = derivedLayout.offsets) {
		if (items.length === 0 || scrollTop <= 0) return 0;

		let low = 0;
		let high = items.length - 1;

		while (low <= high) {
			const mid = Math.floor((low + high) / 2);
			const top = offs[mid] ?? 0;
			const height = heightCalc(items[mid]);

			if (top + height < scrollTop) {
				low = mid + 1;
			} else if (top > scrollTop) {
				high = mid - 1;
			} else {
				return Math.max(0, mid - overscan);
			}
		}
		return Math.max(0, low - overscan);
	}

	// Calculate end index based on viewport height
	function calculateEndIndex(start: number, offs = derivedLayout.offsets) {
		const maxPosition = scrollTop + viewportHeight;

		for (let i = start; i < items.length; i++) {
			const top = offs[i] ?? 0;
			const height = heightCalc(items[i]);

			if (top > maxPosition + overscan * height) {
				return i;
			}
		}
		return items.length;
	}

	// Derive visible items with virtualization
	const [visibleItems, startIndex] = $derived.by(() => {
		const { offsets } = derivedLayout;
		const start = calculateStartIndex(offsets);
		const end = calculateEndIndex(start, offsets);

		const visible = items.slice(start, browser ? end : Math.min(ssrMin, items.length));

		return [
			visible.map((item, idx) => ({
				id: item.id,
				data: item,
				top: offsets[start + idx] ?? 0
			})),
			start
		];
	});

	function open(data: T) {
		if (type === 'trip') {
			const trip = data as TripItem;
			openModal({ type, source: trip.source, routeId: trip.routeId, id: trip.tripId });
		} else {
			const source = (data as CatalogStop | CatalogRoute).data.source;
			openModal({ type, source, id: data.id });
		}
	}

	// Calculate total height for the scroll container
	const totalHeight = $derived.by(() => {
		const { offsets, total } = derivedLayout;
		const totalItems = Math.min(itemsBeforeScroll ?? items.length, items.length);

		if (totalItems === 0) return 0;

		// No itemsBeforeScroll cap — use the fully computed total directly
		if (!itemsBeforeScroll || itemsBeforeScroll >= items.length) {
			return total;
		}

		// Cap to itemsBeforeScroll
		const lastIdx = totalItems - 1;
		return (offsets[lastIdx] ?? 0) + heightCalc(items[lastIdx]);
	});

	// Pin lists that show every row should size to those rows. The estimated
	// height is a few pixels short of the rendered border box, which otherwise
	// leaves a scrollbar on a single pin.
	const fitsWithoutScroll = $derived(
		itemsBeforeScroll !== undefined && items.length > 0 && items.length <= itemsBeforeScroll
	);

	const showingSkeleton = $derived(pending && !items.length);
	const skeletonSource = $derived(
		page.data.selectedSources.includes(selectedSource.current)
			? selectedSource.current
			: page.data.selectedSources[0]
	);

	// Pins live in LocalStorage and are unknown during SSR, so the space left for this list
	// can't be computed there. Render enough rows to cover any screen; the flex-sized
	// viewport clips the excess, which also accounts for pinned lists above it.
	const SKELETON_COVER_PX = 2400;
	const skeletonCount = $derived(
		itemsBeforeScroll ?? Math.ceil(SKELETON_COVER_PX / skeletonRowHeight(type, skeletonSource))
	);

	const previewHeight = $derived(
		itemsBeforeScroll !== undefined && !fitsWithoutScroll && !showingSkeleton
			? `${totalHeight}px`
			: undefined
	);
</script>

<!-- TODO: back to top button in header -->
<!-- TODO: fix scroll warnings in console -->
<div class="relative z-30 flex w-full flex-col text-secondary {containerClass ?? ''}">
	<div
		class="sticky top-0 z-30 flex w-full items-center justify-between bg-surface-1/95 shadow-lg shadow-black/10 backdrop-blur-xs"
	>
		<!-- TODO: remove either this header or the main website header -->
		<!-- it should be possible to combine the logo, tab selection, and settings button in one "row" -->
		<!-- plus, it already shows in the navbar what section is active (except home page lists) -->
		<h1 class="flex h-10 items-center gap-2 pl-2 text-xl font-bold">
			<span>
				{title}
			</span>

			{#if headerSlot}
				{@render headerSlot()}
			{/if}
		</h1>

		{#if availableSources.length > 1}
			<div class="rounded-none border border-border-default/50 bg-surface-2/50 p-1 shadow-inner">
				<div class="flex gap-1">
					{#each sourceEntries as { source, data: sourceData } (source)}
						{const info = $derived(sourceConfig[source])}
						{#if sourceData.length > 0}
							<div transition:slide={{ axis: 'x', duration: 250 }}>
								<button
									class={[
										'relative flex items-center gap-2 rounded-none px-4 py-1 transition-all duration-200',
										{
											'font-medium text-accent': activeSource === source,
											'text-muted': activeSource !== source
										}
									]}
									onclick={() => {
										selectedSource.current = source;
									}}
									aria-label={`Show ${info.name} items`}
								>
									<!-- TODO: improve icons (they are kinda ugly rn) -->
									<enhanced:img alt="" src={sourceIcons[source]} sizes="20px" class="size-5" />
									<!-- <Icon class="h-4 w-4" /> -->
									<!-- TODO: only show text if theres enough room -->
									<!-- <span>{sources[source].name}</span> -->

									{#if activeSource === source}
										<div
											in:send={{ key: 'tab' }}
											out:receive={{ key: 'tab' }}
											class="absolute inset-0 -z-10 rounded-none bg-accent-bg"
										></div>
									{/if}
								</button>
							</div>
						{/if}
					{/each}
				</div>
			</div>
		{/if}
	</div>

	<div class="h-px bg-linear-to-r from-transparent via-border-default/50 to-transparent"></div>

	<div
		bind:this={viewportEl}
		bind:offsetHeight={viewportHeight}
		onscroll={(e) => {
			scrollTop = e.currentTarget.scrollTop;
		}}
		style:-webkit-overflow-scrolling="touch"
		style:height={previewHeight}
		class={[
			'relative text-base',
			showingSkeleton ? 'overflow-hidden' : 'overflow-y-auto',
			listClass,
			itemsBeforeScroll ? 'flex-none' : 'min-h-0 flex-1'
		]}
	>
		<div style:height={fitsWithoutScroll ? undefined : `${totalHeight}px`} class="relative">
			<div
				class="will-change-transform"
				style:transform="translateY({getItemOffset(startIndex)}px)"
			>
				{#each visibleItems as { data, id } (id)}
					<div
						class="relative list-item w-full rounded-none border border-border-subtle/50 bg-canvas will-change-transform"
					>
						<button
							class="flex w-full items-center justify-between p-2 transition-colors duration-200 hover:bg-hover/50 active:bg-border-default/50"
							onclick={() => open(data)}
						>
							{#if type === 'stop'}
								<StopButton data={data as CatalogStop} />
							{:else if type === 'route'}
								<RouteButton data={data as CatalogRoute} />
							{:else if type === 'trip'}
								{const trip = $derived(data as TripItem)}
								<TripButton source={trip.source} routeId={trip.routeId} tripId={trip.tripId} />
							{/if}
						</button>

						{#if pins}
							<!-- maybe do bind:pins -->
							<Pin
								{pins}
								id={data.id}
								source={activeSource}
								class="absolute top-[50%] right-0 z-20 -translate-y-1/2 transform rounded-none px-2 py-1 text-secondary hover:text-muted"
							/>
						{/if}
					</div>
				{/each}
			</div>
		</div>
		{#if showingSkeleton}
			<ListSkeleton {type} source={skeletonSource} count={skeletonCount} />
		{/if}
	</div>
</div>
