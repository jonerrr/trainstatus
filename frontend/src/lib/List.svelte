<script lang="ts">
	import { cubicInOut } from 'svelte/easing';
	import { SvelteMap } from 'svelte/reactivity';
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

	// Item heights cache using SvelteMap for reactivity
	const item_heights = new SvelteMap<string, number>();
	let item_offsets: Record<number, number> = {};

	// Reset offsets when items change
	$effect(() => {
		// maybe move this to derived
		items;
		item_offsets = {};
	});

	function getItemOffset(startIndex: number): number {
		if (item_offsets[startIndex] !== undefined) {
			return item_offsets[startIndex];
		}
		let offset = 0;
		for (let i = 0; i < startIndex; i++) {
			const itemId = items[i].id;
			let height = item_heights.get(itemId);
			if (!height) {
				height = height_calc(items[i]);
			}
			offset += height;
		}
		item_offsets[startIndex] = offset;
		return offset;
	}

	// Calculate start index based on scroll position
	function calculateStartIndex() {
		let start = 0;
		let position = 0;
		const scroll_top = scroll.y;
		while (start < items.length) {
			const item = items[start];
			const height = item_heights.get(item.id) ?? height_calc(item);
			if (position + height > scroll_top - overscan * height) break;
			position += height;
			start++;
		}
		return Math.max(0, start);
	}

	// Calculate end index based on viewport height
	function calculateEndIndex(start: number) {
		let end = start;
		let position = getItemOffset(start);
		const scroll_top = scroll.y;
		const viewport_height = viewport_size.height;
		while (end < items.length) {
			const item = items[end];
			const height = item_heights.get(item.id) ?? height_calc(item) ?? 50;
			position += height;
			if (position > scroll_top + viewport_height + overscan * height) break;
			end++;
		}
		return Math.min(end, items.length);
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
		let total = 0;
		for (let i = 0; i < total_items; i++) {
			const item = items[i];
			const height = item_heights.get(item.id) ?? height_calc(item);
			total += height;
		}
		return total;
	}

	const total_height = $derived.by(calculate_total_height);

	// Action to measure item height - returns cleanup function
	function measureHeight(node: HTMLElement, id: string) {
		// Measure immediately
		item_heights.set(id, node.offsetHeight);

		// Re-measure on resize
		const observer = new ResizeObserver(() => {
			item_heights.set(id, node.offsetHeight);
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
			<div class="rounded-full border border-neutral-700/50 bg-neutral-800/50 p-1 shadow-inner">
				<!-- TODO: doesn't need to be a snippet anymore -->
				{#snippet source_tab(source: Source)}
					{@const source_data = sources[source] ?? []}
					{#if source_data.length > 0}
						{@const Icon = source_icons[source]}
						<div transition:slide={{ axis: 'x', duration: 250 }}>
							<!-- 		class="relative flex items-center gap-2 rounded-full px-4 py-1 transition-all duration-200 {active_source.current ===
									source && 'font-medium text-neutral-100'} {active_source.current !== source &&
									'text-neutral-400'}" -->
							<button
								class={[
									'relative flex items-center gap-2 rounded-full px-4 py-1 transition-all duration-200',
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
										class="absolute inset-0 -z-10 rounded-full bg-neutral-700/50"
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
						class="relative list-item w-full rounded-sm border border-neutral-800/50 bg-neutral-950 will-change-transform"
					>
						<button
							class="flex w-full items-center justify-between p-2 transition-colors duration-200 hover:bg-neutral-800/50 active:bg-neutral-700/50"
							onclick={() => {
								pushState('', {
									// TODO: fix typing
									modal: { type, data: $state.snapshot(data), source: active_source }
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
