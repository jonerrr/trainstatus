<script lang="ts" generics="T extends Stop | Route | Trip">
	import { untrack, type Snippet } from 'svelte';

	import { cubicInOut } from 'svelte/easing';
	import { crossfade, slide } from 'svelte/transition';

	import { browser } from '$app/env';
	import { page } from '$app/state';

	import type { Route, Source, Stop, Trip } from '#lib/client/index.js';
	import Pin from '#lib/Pin.svelte';
	import type { Pins } from '#lib/pins.svelte.js';
	import { source_info } from '#lib/resources/index.svelte.js';
	import { stop_time_context, type StopTimeResources } from '#lib/resources/stop_times.svelte.js';
	import RouteButton from '#lib/Route/Button.svelte';
	import StopButton from '#lib/Stop/Button.svelte';
	import { LocalStorage } from '#lib/storage.svelte.js';
	import TripButton from '#lib/Trip/Button.svelte';
	import { open_modal } from '#lib/url_params.svelte.js';

	type ItemType = 'stop' | 'route' | 'trip';

	interface Props {
		// title of list
		title: string;
		// renders extra header content (like geolocate button)
		header_slot?: Snippet;
		// item type for rendering and modal
		type: ItemType;
		// data organized by source
		sources: Partial<Record<Source, T[]>>;
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
		height_calc: (item: T) => number;
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
		void active_source;
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

	// The viewport owns subscriptions so replacing rows cannot temporarily release
	// routes that are still visible. Only changes to the route union reach the owner.
	let held_resource: StopTimeResources[Source];
	let held_routes = new Set<string>();
	$effect(() => {
		const source = active_source;
		const resource =
			source && type !== 'route' && source_info[source].monitor_routes
				? stop_time_context.getSource(source)
				: undefined;
		const routes = new Set(
			visible_items.flatMap(({ data }) =>
				type === 'stop'
					? (data as Stop).routes.map((route) => route.route_id)
					: type === 'trip'
						? [(data as Trip).route_id]
						: []
			)
		);
		untrack(() => {
			if (held_resource !== resource) {
				for (const route of held_routes) held_resource?.remove_route(route);
				held_routes = new Set();
				held_resource = resource;
			}
			for (const route of routes) {
				if (!held_routes.has(route)) void resource?.add_route(route).catch(() => {});
			}
			for (const route of held_routes) {
				if (!routes.has(route)) resource?.remove_route(route);
			}
			held_routes = routes;
		});
	});
	$effect(() => () => {
		for (const route of held_routes) held_resource?.remove_route(route);
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

	// Pin lists that show every row should size to those rows. The estimated
	// height is a few pixels short of the rendered border box, which otherwise
	// leaves a scrollbar on a single pin.
	const fits_without_scroll = $derived(
		items_before_scroll !== undefined && items.length > 0 && items.length <= items_before_scroll
	);

	const preview_height = $derived(
		items_before_scroll !== undefined && !fits_without_scroll ? `${total_height}px` : undefined
	);
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
		<h1 class="flex h-10 items-center gap-2 pl-2 text-xl font-bold">
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
					{#each source_entries as { source, data: source_data } (source)}
						{const info = $derived(source_info[source])}
						{#if source_data.length > 0}
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
									aria-label={`Show ${info.name} items`}
								>
									<!-- TODO: improve icons (they are kinda ugly rn) -->
									<enhanced:img alt="" src={info.icon} sizes="20px" class="size-5" />
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
		style:height={preview_height}
		class={[
			'relative overflow-y-auto text-base',
			list_class,
			items_before_scroll ? 'flex-none' : 'min-h-0 flex-1'
		]}
	>
		<div style:height={fits_without_scroll ? undefined : `${total_height}px`} class="relative">
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
								if (type === 'stop') open_modal({ ...(data as Stop), type });
								else if (type === 'route') open_modal({ ...(data as Route), type });
								else open_modal({ ...(data as Trip), type });
							}}
						>
							{#if type === 'stop'}
								<StopButton data={data as Stop} acquire_routes={false} />
							{:else if type === 'route'}
								<RouteButton data={data as Route} />
							{:else if type === 'trip'}
								<TripButton data={data as Trip} acquire_routes={false} />
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
