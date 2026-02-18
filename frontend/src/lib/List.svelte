<script lang="ts">
	import { cubicInOut } from 'svelte/easing';
	import { crossfade, slide } from 'svelte/transition';

	import { browser } from '$app/environment';
	import { pushState } from '$app/navigation';

	import { BusFront, TrainFront } from '@lucide/svelte';
	import type { Route, Source, Stop } from '@trainstatus/client';
	import { ElementSize, PersistedState, ScrollState } from 'runed';

	import Pin from './Pin.svelte';
	import RouteButton from './Route/Button.svelte';
	import StopButton from './Stop/Button.svelte';
	import type { Pins } from './stores.svelte';

	// TODO: probably put these mappings in sources.ts file
	// Source icons mapping
	const source_icons: Record<Source, typeof TrainFront> = {
		mta_subway: TrainFront,
		mta_bus: BusFront
	};

	// Source display names
	const source_names: Record<Source, string> = {
		mta_subway: 'Subway',
		mta_bus: 'Bus'
	};

	const source_order: Source[] = ['mta_subway', 'mta_bus'];

	// Estimated heights for virtualization
	const estimated_heights = {
		stop: 196,
		route: 40
	} as const;

	type ItemType = keyof typeof estimated_heights;

	interface Props {
		// title of list
		title: string;
		// renders extra header content (like geolocate button)
		header_slot?: import('svelte').Snippet;
		// item type for rendering and modal
		type: ItemType;
		// data organized by source
		sources: Record<Source, (Stop | Route)[]>;
		// persisted state for pinned items
		pins?: PersistedState<Pins>;
		// persisted state for selected source tab
		selected_source?: PersistedState<Source>;
		// items to show before the user scrolls (for pinned lists)
		items_before_scroll?: number;
		class?: string;
		// scroll list into view if there are few items
		auto_scroll?: boolean;
		// height calculation function for virtualization
		// eslint-disable-next-line @typescript-eslint/no-explicit-any
		height_calc: (item: any) => number;
		// minimum number of items to render during SSR
		ssr_min?: number;
		// extra items to render before and after visible items
		overscan?: number;
		// css style for list
		style?: string;
	}

	let {
		title,
		type,
		sources,
		pins,
		header_slot,
		selected_source = $bindable(
			new PersistedState<Source>(`${title.toLocaleLowerCase()}_tab`, 'mta_subway')
		),
		height_calc,
		class: class_name,
		auto_scroll = false,
		items_before_scroll,
		ssr_min = 10,
		overscan = 5,
		style: style_
	}: Props = $props();

	// Active source management - use provided or create internal state
	// let internal_source = $state<Source>('mta_subway');

	// TODO: update source when current one is empty (see previous implementation)
	// Initialize internal_source when sources becomes available
	// $effect(() => {
	// 	if (sources[0]?.source && internal_source === 'mta_subway') {
	// 		internal_source = sources[0].source;
	// 	}
	// });
	// TODO: fix this slop
	// const active_source = $derived({
	// 	get current() {
	// 		return selected_source?.current ?? internal_source;
	// 	},
	// 	set current(value: Source) {
	// 		if (selected_source) {
	// 			selected_source.current = value;
	// 		} else {
	// 			internal_source = value;
	// 		}
	// 	}
	// });
	// TODO: check here if source is empty and switch to first available (instead of using $effect)
	const active_source = $derived(selected_source.current);

	const source_entries = $derived.by(() =>
		source_order.map((source) => ({
			source,
			data: sources[source] ?? []
		}))
	);

	// Get available sources (those with data)
	const available_sources = $derived(source_entries.filter((s) => s.data.length > 0));

	// Auto-switch to first available source if current has no data
	// $effect(() => {
	// 	const current_has_data = sources.find((s) => s.source === active_source.current)?.data.length;
	// 	if (!current_has_data && available_sources.length > 0) {
	// 		active_source.current = available_sources[0].source;
	// 	}
	// });

	// Get items for current source
	const items = $derived(sources[active_source] ?? []);

	const [send, receive] = crossfade({
		duration: 300,
		easing: cubicInOut
	});

	// TODO: actually don't use runed utilities for this stuff
	// Viewport element and sizing using runed
	let viewport_el = $state<HTMLDivElement>();
	const viewport_size = new ElementSize(() => viewport_el);

	// Scroll state using runed ScrollState
	const scroll = new ScrollState({
		element: () => viewport_el,
		idle: 100
	});

	function reset_scroll() {
		if (viewport_el) {
			scroll.scrollTo(0, 0);
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

	// Plain Map - NOT reactive. The $effect below tracks `items` only.
	// Using SvelteMap here would cause the $effect to re-run O(n) times on initial render
	// (once per visible item measured), making the total work O(n²) with thousands of stops.
	// TODO: move to separate file so lists can share measured heights (for same items across sources)
	const item_heights = new Map<string, number>();

	// Prefix sum array for O(1) offset lookups
	let offsets = $state<number[]>([]);
	let measured_total_height = $state(0);

	// Rebuild offsets array when items or heights change (prefix sum calculation)
	$effect(() => {
		let running_offset = 0;
		offsets = items.map((item) => {
			const top = running_offset;
			const height = item_heights.get(item.id) ?? height_calc(item);
			running_offset += height;
			return top;
		});
		measured_total_height = running_offset;
	});

	// O(1) lookup instead of O(n) iteration
	function getItemOffset(index: number): number {
		return offsets[index] ?? 0;
	}

	// Binary search for start index - O(log n) instead of O(n)
	function calculateStartIndex() {
		const scroll_top = scroll.y;
		if (items.length === 0 || scroll_top <= 0) return 0;

		let low = 0;
		let high = items.length - 1;

		while (low <= high) {
			const mid = Math.floor((low + high) / 2);
			const top = offsets[mid] ?? 0;
			const height = item_heights.get(items[mid].id) ?? height_calc(items[mid]);

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
	function calculateEndIndex(start: number) {
		const scroll_top = scroll.y;
		const viewport_height = viewport_size.height;
		const max_position = scroll_top + viewport_height;

		for (let i = start; i < items.length; i++) {
			const top = offsets[i] ?? 0;
			const height = item_heights.get(items[i].id) ?? height_calc(items[i]);

			if (top > max_position + overscan * height) {
				return i;
			}
		}
		return items.length;
	}

	// Derive visible items with virtualization
	const [visible_items, start_index] = $derived.by(() => {
		const start = calculateStartIndex();
		const end = calculateEndIndex(start);

		const visible = items.slice(start, browser ? end : Math.min(ssr_min, items.length));

		return [
			visible.map((item, idx) => ({
				id: item.id,
				data: item,
				top: getItemOffset(start + idx)
			})),
			start
		];
	});

	// Calculate total height for the scroll container
	function calculate_total_height() {
		const total_items = Math.min(items_before_scroll ?? items.length, items.length);

		// Use measured height if available and all items are included
		if (
			measured_total_height > 0 &&
			(!items_before_scroll || items_before_scroll >= items.length)
		) {
			return measured_total_height;
		}

		// Otherwise calculate from offsets or estimates
		if (total_items === 0) return 0;
		if (offsets.length > total_items) {
			return (
				offsets[total_items - 1] +
				(item_heights.get(items[total_items - 1].id) ?? height_calc(items[total_items - 1]))
			);
		}

		// Fallback to manual calculation
		let total = 0;
		for (let i = 0; i < total_items; i++) {
			const item = items[i];
			const height = item_heights.get(item.id) ?? height_calc(item);
			total += height;
		}
		return total;
	}

	const total_height = $derived.by(calculate_total_height);
	// TODO: convert to attachment
	// Action to measure item height with scroll correction
	function measureHeight(node: HTMLElement, id: string) {
		// Find the index of this item
		const get_index = () => items.findIndex((item) => item.id === id);

		// Measure immediately and correct the estimated offset
		const initial_height = node.offsetHeight;
		const item = items.find((i) => i.id === id);
		const estimated = item ? height_calc(item) : initial_height;
		const init_delta = initial_height - estimated;
		item_heights.set(id, initial_height);

		// Apply the delta between actual and estimated height incrementally
		// (avoids a full O(n) offset rebuild for each measured item)
		if (init_delta !== 0) {
			const index = get_index();
			if (index >= 0) {
				measured_total_height += init_delta;
				for (let i = index + 1; i < offsets.length; i++) {
					offsets[i] += init_delta;
				}
			}
		}

		// Re-measure on resize with scroll correction
		const observer = new ResizeObserver(() => {
			const new_height = node.offsetHeight;
			const old_height = item_heights.get(id);

			if (new_height !== old_height && old_height !== undefined) {
				const delta = new_height - old_height;
				const index = get_index();

				// Update the height
				item_heights.set(id, new_height);

				// Update total height
				measured_total_height += delta;

				// Update all subsequent offsets
				for (let i = index + 1; i < offsets.length; i++) {
					offsets[i] += delta;
				}

				// Scroll correction: if item is above viewport, adjust scrollTop
				// to prevent visible content from jumping
				if (viewport_el && index >= 0) {
					const current_start = calculateStartIndex();
					if (index < current_start) {
						viewport_el.scrollTop += delta;
					}
				}
			} else if (old_height === undefined) {
				// First measurement
				item_heights.set(id, new_height);
			}
		});
		observer.observe(node);

		return {
			destroy() {
				observer.disconnect();
			}
		};
	}
</script>

<!-- TODO: back to top button in header -->
<!-- TODO: fix scroll warnings in console -->
<div class="relative z-30 flex w-full flex-col text-neutral-200">
	<div
		class="sticky top-0 z-30 flex w-full items-center justify-between bg-neutral-900/95 shadow-lg shadow-black/10 backdrop-blur-xs"
	>
		<h1 class="flex items-center gap-2 pl-2 text-xl font-bold">
			<span>
				{title}
			</span>

			{#if header_slot}
				{@render header_slot()}
			{/if}
		</h1>

		{#if available_sources.length > 1}
			<div class="rounded-md border border-neutral-700/50 bg-neutral-800/50 p-1 shadow-inner">
				<!-- TODO: doesn't need to be a snippet anymore -->
				{#snippet source_tab(source: Source)}
					{@const source_data = sources[source] ?? []}
					{#if source_data.length > 0}
						{@const Icon = source_icons[source]}
						<div transition:slide={{ axis: 'x', duration: 250 }}>
							<!-- 		class="relative flex items-center gap-2 rounded-md px-4 py-1 transition-all duration-200 {active_source.current ===
									source && 'font-medium text-neutral-100'} {active_source.current !== source &&
									'text-neutral-400'}" -->
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
								aria-label={`Show ${source_names[source]} items`}
							>
								<Icon class="h-4 w-4" />
								<span>{source_names[source]}</span>

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
				{/snippet}

				<div class="flex gap-1">
					{#each source_entries as { source } (source)}
						{@render source_tab(source)}
					{/each}
				</div>
			</div>
		{/if}
	</div>
	<div class="h-px bg-linear-to-r from-transparent via-neutral-700/50 to-transparent"></div>

	<div
		bind:this={viewport_el}
		style="-webkit-overflow-scrolling: touch; {style_ ?? ''}"
		class="relative overflow-y-auto text-base {class_name ?? ''}"
	>
		<div style:height="{total_height}px" class="relative">
			<div
				class="will-change-transform"
				style:transform="translateY({getItemOffset(start_index)}px)"
			>
				{#each visible_items as { data, id } (id)}
					<div
						use:measureHeight={id}
						class="relative list-item w-full rounded-md border border-neutral-800/50 bg-neutral-950 will-change-transform"
					>
						<button
							class="flex w-full items-center justify-between p-2 transition-colors duration-200 hover:bg-neutral-800/50 active:bg-neutral-700/50"
							onclick={() => {
								pushState('', {
									modal: { type, ...$state.snapshot(data) } as any
								});
							}}
						>
							{#if type === 'stop'}
								<StopButton data={data as Stop} />
							{:else if type === 'route'}
								<RouteButton data={data as Route} />
							{/if}
						</button>

						{#if pins}
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
