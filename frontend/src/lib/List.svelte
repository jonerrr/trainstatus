<script lang="ts">
	import type { Snippet } from 'svelte';

	import { cubicInOut } from 'svelte/easing';
	import { crossfade, slide } from 'svelte/transition';

	import { browser } from '$app/environment';
	import { page } from '$app/state';

	import Pin from '$lib/Pin.svelte';
	import RouteButton from '$lib/Route/Button.svelte';
	import StopButton from '$lib/Stop/Button.svelte';
	import TripButton from '$lib/Trip/Button.svelte';
	import type { Route, Source, Stop, Trip } from '$lib/client';
	import type { Pins } from '$lib/pins.svelte';
	import { source_info } from '$lib/resources/index.svelte';
	import { LocalStorage } from '$lib/storage.svelte';
	import { open_modal } from '$lib/url_params.svelte';

	type ItemType = 'stop' | 'route' | 'trip';

	interface Props {
		// title of list
		title: string;
		// renders extra header content (like geolocate button)
		header_slot?: Snippet;
		// item type for rendering and modal
		type: ItemType;
		// data organized by source
		sources: Partial<Record<Source, (Stop | Route | Trip)[]>>;
		// persisted state for pinned items
		pins?: LocalStorage<Pins>;
		// persisted state for selected source tab
		selected_source?: LocalStorage<Source>;
		// items to show before the user scrolls (for pinned lists)
		items_before_scroll?: number;
		list_class?: string;
		container_class?: string;
		// scroll list into view if there are few items
		auto_scroll?: boolean;
		// height calculation function for virtualization
		height_calc: (item: any) => number;
		// minimum number of items to render during SSR
		ssr_min?: number;
		// extra items to render before and after visible items
		overscan?: number;
	}

	let {
		title,
		type,
		sources,
		pins,
		header_slot,
		selected_source = $bindable(
			new LocalStorage<Source>(
				`${title.toLocaleLowerCase()}Tab`,
				page.data.selected_sources[0] ?? 'mta_subway'
			)
		),
		height_calc,
		container_class,
		list_class,
		auto_scroll = false,
		items_before_scroll,
		ssr_min = 10,
		overscan = 5
	}: Props = $props();

	const source_entries = $derived(
		page.data.selected_sources.map((source) => ({
			source,
			data: sources[source] ?? []
		}))
	);

	// Get available sources (those with data)
	const available_sources = $derived(source_entries.filter((s) => s.data.length > 0));

	let active_source = $derived.by(() => {
		// if the selected source has no data, fall back to the first available source
		if ((sources[selected_source.current]?.length ?? 0) > 0) {
			return selected_source.current;
		}
		return available_sources[0]?.source;
	});

	// Get items for current source
	const items = $derived(sources[active_source] ?? []);

	const [send, receive] = crossfade({
		duration: 300,
		easing: cubicInOut
	});

	let viewport_el = $state<HTMLDivElement>();
	let viewport_height = $state(0);
	let scroll_top = $state(0);

	function reset_scroll() {
		if (viewport_el) {
			viewport_el.scrollTo(0, 0);
		}
	}

	// Auto-scroll for lists with few items
	$effect(() => {
		if (auto_scroll && viewport_el && items.length < 8) {
			viewport_el.scrollIntoView({ behavior: 'smooth' });
		}
	});

	// Reset scroll when source changes
	$effect(() => {
		// maybe move this to derived
		active_source;
		reset_scroll();
	});

	const derived_layout = $derived.by(() => {
		let running = 0;
		const offs = items.map((item) => {
			const top = running;
			running += height_calc(item);
			return top;
		});
		return { offsets: offs, total: running };
	});

	// O(1) lookup
	function getItemOffset(index: number): number {
		return derived_layout.offsets[index] ?? 0;
	}

	// Binary search for start index - O(log n) instead of O(n)
	function calculateStartIndex(offs = derived_layout.offsets) {
		if (items.length === 0 || scroll_top <= 0) return 0;

		let low = 0;
		let high = items.length - 1;

		while (low <= high) {
			const mid = Math.floor((low + high) / 2);
			const top = offs[mid] ?? 0;
			const height = height_calc(items[mid]);

			if (top + height < scroll_top) {
				low = mid + 1;
			} else if (top > scroll_top) {
				high = mid - 1;
			} else {
				return Math.max(0, mid - overscan);
			}
		}
		return Math.max(0, low - overscan);
	}

	// Calculate end index based on viewport height
	function calculateEndIndex(start: number, offs = derived_layout.offsets) {
		const max_position = scroll_top + viewport_height;

		for (let i = start; i < items.length; i++) {
			const top = offs[i] ?? 0;
			const height = height_calc(items[i]);

			if (top > max_position + overscan * height) {
				return i;
			}
		}
		return items.length;
	}

	// Derive visible items with virtualization
	const [visible_items, start_index] = $derived.by(() => {
		const { offsets } = derived_layout;
		const start = calculateStartIndex(offsets);
		const end = calculateEndIndex(start, offsets);

		const visible = items.slice(start, browser ? end : Math.min(ssr_min, items.length));

		return [
			visible.map((item, idx) => ({
				id: item.id,
				data: item,
				top: offsets[start + idx] ?? 0
			})),
			start
		];
	});

	// Calculate total height for the scroll container
	const total_height = $derived.by(() => {
		const { offsets, total } = derived_layout;
		const total_items = Math.min(items_before_scroll ?? items.length, items.length);

		if (total_items === 0) return 0;

		// No items_before_scroll cap — use the fully computed total directly
		if (!items_before_scroll || items_before_scroll >= items.length) {
			return total;
		}

		// Cap to items_before_scroll
		const last_idx = total_items - 1;
		return (offsets[last_idx] ?? 0) + height_calc(items[last_idx]);
	});
</script>

<!-- TODO: back to top button in header -->
<!-- TODO: fix scroll warnings in console -->
<div class="relative z-30 flex w-full flex-col text-neutral-200 {container_class ?? ''}">
	<div
		class="sticky top-0 z-30 flex w-full items-center justify-between bg-neutral-900/95 shadow-lg shadow-black/10 backdrop-blur-xs"
	>
		<!-- TODO: remove either this header or the main website header -->
		<!-- it should be possible to combine the logo, tab selection, and settings button in one "row" -->
		<!-- plus, it already shows in the navbar what section is active (except home page lists) -->
		<h1 class="flex items-center gap-2 pl-2 text-xl font-bold h-10">
			<span>
				{title}
			</span>

			{#if header_slot}
				{@render header_slot()}
			{/if}
		</h1>

		{#if available_sources.length > 1}
			<div class="rounded-md border border-neutral-700/50 bg-neutral-800/50 p-1 shadow-inner">
				<div class="flex gap-1">
					{#each source_entries as { source } (source)}
						{@const source_data = sources[source] ?? []}
						{#if source_data.length > 0}
							<!-- {@const Icon = source_info[source].icon} -->
							<div transition:slide={{ axis: 'x', duration: 250 }}>
								<button
									class={[
										'relative flex items-center gap-2 rounded-md px-4 py-1 transition-all duration-200',
										{
											'font-medium text-neutral-100': active_source === source,
											'text-neutral-400': active_source !== source
										}
									]}
									onclick={() => {
										selected_source.current = source;
									}}
									aria-label={`Show ${source_info[source].name} items`}
								>
									<!-- TODO: improve icons (they are kinda ugly rn) -->
									<img alt="" src={source_info[source].icon} class="size-5" />
									<!-- <Icon class="h-4 w-4" /> -->
									<!-- TODO: only show text if theres enough room -->
									<!-- <span>{source_info[source].name}</span> -->

									{#if active_source === source}
										<div
											in:send={{ key: 'tab' }}
											out:receive={{ key: 'tab' }}
											class="absolute inset-0 -z-10 rounded-md bg-neutral-700/50"
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
	<div class="h-px bg-linear-to-r from-transparent via-neutral-700/50 to-transparent"></div>

	<div
		bind:this={viewport_el}
		bind:offsetHeight={viewport_height}
		onscroll={(e) => {
			scroll_top = e.currentTarget.scrollTop;
		}}
		style="-webkit-overflow-scrolling: touch;"
		class="relative flex-1 min-h-0 overflow-y-auto text-base {list_class ?? ''}"
	>
		<div style:height="{total_height}px" class="relative">
			<div
				class="will-change-transform"
				style:transform="translateY({getItemOffset(start_index)}px)"
			>
				{#each visible_items as { data, id } (id)}
					<div
						class="relative list-item w-full rounded-md border border-neutral-800/50 bg-neutral-950 will-change-transform"
					>
						<button
							class="flex w-full items-center justify-between p-2 transition-colors duration-200 hover:bg-neutral-800/50 active:bg-neutral-700/50"
							onclick={() => {
								open_modal({ type, ...data } as any);
							}}
						>
							{#if type === 'stop'}
								<StopButton data={data as Stop} />
							{:else if type === 'route'}
								<RouteButton data={data as Route} />
							{:else if type === 'trip'}
								<TripButton data={data as Trip} />
							{/if}
						</button>

						{#if pins}
							<!-- maybe do bind:pins -->
							<Pin
								{pins}
								id={data.id}
								source={active_source}
								class="absolute top-[50%] right-0 z-20 -translate-y-1/2 transform rounded-md px-2 py-1 text-neutral-200 hover:text-neutral-400"
							/>
						{/if}
					</div>
				{/each}
			</div>
		</div>
	</div>
</div>
