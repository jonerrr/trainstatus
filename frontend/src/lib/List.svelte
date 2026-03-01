<script lang="ts">
	import type { Snippet } from 'svelte';

	import type { Attachment } from 'svelte/attachments';
	import { cubicInOut } from 'svelte/easing';
	import { crossfade, slide } from 'svelte/transition';

	import { browser } from '$app/environment';

	import Pin from '$lib/Pin.svelte';
	import RouteButton from '$lib/Route/Button.svelte';
	import StopButton from '$lib/Stop/Button.svelte';
	import TripButton from '$lib/Trip/Button.svelte';
	import type { Pins } from '$lib/pins.svelte';
	import { default_sources, source_info } from '$lib/resources/index.svelte';
	import { LocalStorage } from '$lib/storage.svelte';
	import { open_modal } from '$lib/url_params.svelte';

	import type { Route, Source, Stop, Trip } from '@trainstatus/client';

	type ItemType = 'stop' | 'route' | 'trip';

	interface Props {
		// title of list
		title: string;
		// renders extra header content (like geolocate button)
		header_slot?: Snippet;
		// item type for rendering and modal
		type: ItemType;
		// data organized by source
		sources: Record<Source, (Stop | Route | Trip)[]>;
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
			new LocalStorage<Source>(`${title.toLocaleLowerCase()}_tab`, 'mta_subway')
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
		default_sources.map((source) => ({
			source,
			data: sources[source] ?? []
		}))
	);

	// Get available sources (those with data)
	const available_sources = $derived(source_entries.filter((s) => s.data.length > 0));

	let active_source = $derived.by(() => {
		// if the selected source has no data, fall back to the first available source
		if (sources[selected_source.current]?.length > 0) {
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

	// TODO: move to separate file so lists can share measured heights (for same items across sources)
	const item_heights = new Map<string, number>();

	// Single reactive signal. Incrementing this causes one O(N) derived rebuild instead of
	// N individual $state array mutations (which would each trigger fine-grained reactivity).
	let heights_version = $state(0);

	// Rebuild prefix sums in one pass whenever items or any measured height changes.
	// Replaces the $effect + mutable $state<number[]> approach which caused O(N²) overhead.
	const derived_layout = $derived.by(() => {
		heights_version; // subscribe to height changes
		let running = 0;
		const offs = items.map((item) => {
			const top = running;
			running += item_heights.get(item.id) ?? height_calc(item);
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
	function calculateEndIndex(start: number, offs = derived_layout.offsets) {
		const max_position = scroll_top + viewport_height;

		for (let i = start; i < items.length; i++) {
			const top = offs[i] ?? 0;
			const height = item_heights.get(items[i].id) ?? height_calc(items[i]);

			if (top > max_position + overscan * height) {
				return i;
			}
		}
		return items.length;
	}

	// Derive visible items with virtualization
	// NOTE: derived_layout is read here so visible_items recomputes whenever heights change.
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
		return (
			(offsets[last_idx] ?? 0) +
			(item_heights.get(items[last_idx].id) ?? height_calc(items[last_idx]))
		);
	});

	// Single shared ResizeObserver for all measured items.
	// One observer fires one callback with all changed entries batched together,
	// so we increment heights_version exactly once per animation frame
	let shared_ro: ResizeObserver | undefined;
	const observed_ids = new Map<Element, string>(); // element → item id

	function get_shared_ro(): ResizeObserver {
		if (shared_ro) return shared_ro;
		shared_ro = new ResizeObserver((entries) => {
			let scroll_delta = 0;
			let changed = false;

			// calculateStartIndex() uses derived_layout which is already up-to-date
			const current_start = calculateStartIndex();

			for (const entry of entries) {
				const id = observed_ids.get(entry.target);
				if (!id) continue;

				const new_height = (entry.target as HTMLElement).offsetHeight;
				const old_height = item_heights.get(id);

				if (new_height === old_height) continue;

				item_heights.set(id, new_height);
				changed = true;

				if (old_height !== undefined) {
					const delta = new_height - old_height;
					const index = items.findIndex((item) => item.id === id);
					// Accumulate scroll correction for items above the viewport
					if (index >= 0 && index < current_start) {
						scroll_delta += delta;
					}
				}
			}

			if (!changed) return;

			// ONE reactive update triggers ONE $derived.by rebuild
			heights_version++;

			// Apply scroll correction after the reactive update
			if (scroll_delta !== 0 && viewport_el) {
				viewport_el.scrollTop += scroll_delta;
			}
		});
		return shared_ro;
	}

	// measure item height on mount and watch for resizes
	function measure_height(id: string): Attachment<HTMLDivElement> {
		return (node) => {
			// Take initial measurement and store it immediately
			const initial_height = node.offsetHeight;
			const old_height = item_heights.get(id);
			item_heights.set(id, initial_height);

			// Only signal a reactive update when our measurement differs from the estimate
			if (old_height === undefined || initial_height !== old_height) {
				heights_version++;
			}

			// Register with the shared observer
			const ro = get_shared_ro();
			observed_ids.set(node, id);
			ro.observe(node);

			return () => {
				ro.unobserve(node);
				observed_ids.delete(node);
			};
		};
	}
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
			// TODO: why did the previous version have a tick here?
			// now (i think due to new svelte await handling), currentTarget is null if i do await tick beforehand.
			// maybe use an attachment that updates it or update it in the resize observer or some other callback
			// await tick();
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
						{@attach measure_height(id)}
						class="relative list-item w-full rounded-md border border-neutral-800/50 bg-neutral-950 will-change-transform"
					>
						<button
							class="flex w-full items-center justify-between p-2 transition-colors duration-200 hover:bg-neutral-800/50 active:bg-neutral-700/50"
							onclick={() => {
								open_modal({ type, ...data } as any);
								// pushState('', {
								// 	modal: { type, ...$state.snapshot(data) } as any
								// });
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
