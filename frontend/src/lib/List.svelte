<script lang="ts">
	import { BusFront, TrainFront } from 'lucide-svelte';
	import { tick, type Snippet } from 'svelte';
	import { crossfade } from 'svelte/transition';
	import { cubicInOut } from 'svelte/easing';
	import { pushState } from '$app/navigation';
	import { browser } from '$app/environment';
	import { item_heights, persisted_rune, type PersistedRune } from './util.svelte';
	import { monitored_bus_routes } from './stop_times.svelte';
	import TripButton from './Trip/Button.svelte';
	import StopButton from './Stop/Button.svelte';
	import RouteButton from './Route/Button.svelte';
	import Pin from './Pin.svelte';
	import type { Route, Stop } from './static';
	import type { BusTripData, TrainTripData, Trip } from './trips.svelte';

	// [ element, estimated size]
	interface ItemComponents {
		trip: [typeof TripButton, number];
		stop: [typeof StopButton, number];
		route: [typeof RouteButton, number];
	}

	const item_components: ItemComponents = {
		trip: [TripButton, 68],
		stop: [StopButton, 196],
		route: [RouteButton, 40]
	};

	type ItemType = keyof ItemComponents;

	type BusData = Stop<'bus'> | Trip<BusTripData, Route> | Route;
	type TrainData = Stop<'train'> | Trip<TrainTripData, Route> | Route;

	// TODO: simplify interfaces so generic is same for stop, trip, and route
	interface Props {
		// title of list
		title: string;
		// renders geolocate button for stops list
		locate_button?: Snippet;
		// current selected tab. Used for selecting correct search index
		selected_tab?: PersistedRune<'train' | 'bus'>;
		type: ItemType;
		pin_rune: PersistedRune<(number | string)[]>;
		bus_data: BusData[];
		train_data: TrainData[];
		// items to show before the user scrolls. Used on home page
		items_before_scroll?: number;
		class?: string;
		// scroll list into view if theres less than 8 items
		auto_scroll?: boolean;
		// height calculation function
		height_calc: (item: any) => number;
		// minimum number of items to render during SSR
		ssr_min?: number;
		// extra items to render before and after visible items
		overscan?: number;
		// css style for list
		style?: string;
		// current time gets passed to items
		// current_time?: number;
	}

	let {
		title,
		type,
		bus_data,
		train_data,
		pin_rune,
		locate_button,
		selected_tab = $bindable(
			persisted_rune<'train' | 'bus'>(`${title.toLocaleLowerCase()}_tab`, 'train')
		),
		height_calc,
		class: class_name,
		auto_scroll = false,
		items_before_scroll,
		ssr_min = 10,
		overscan = 5,
		style: style_
	}: Props = $props();

	// if bus/train data don't have any items, switch tabs
	$effect(() => {
		if (!bus_data.length && train_data.length) selected_tab.value = 'train';
		if (!train_data.length && bus_data.length) selected_tab.value = 'bus';
	});

	const items = $derived(selected_tab.value === 'train' ? train_data : bus_data);

	const Item = item_components[type][0];

	const tab_icons = {
		train: TrainFront,
		bus: BusFront
	};

	const [send, receive] = crossfade({
		duration: 300,
		easing: cubicInOut
	});

	let viewport_el = $state<HTMLDivElement>();

	// TODO: fix scroll reset on tab change
	function reset_scroll() {
		if (viewport_el) {
			viewport_el.scrollTop = 0;
		}
	}

	if (auto_scroll) {
		$effect(() => {
			if (viewport_el && items.length < 8) {
				// console.log('scrolling list into view');
				viewport_el.scrollIntoView({ behavior: 'smooth' });
			}
		});
	}

	let viewport_height = $state(0);
	let scroll_top = $state(0);
	// Caching item heights and offsets
	// let itemHeightsCache: { [key: string]: number } = {};
	let item_offsets: { [key: number]: number } = {};

	// either items before scroll or all items
	// const total_items = $derived(Math.min(items_before_scroll ?? items.length, items.length));
	// $inspect(total_items);

	// $effect(() => {
	// 	total_items;
	// 	console.log('resetting offsets');
	// 	itemOffsetsCache = {};
	// 	itemHeightsCache = {};
	// });

	// let total_height = $state(calculate_total_height());

	// $inspect(total_height);

	// $effect(() => {
	// 	items;
	// 	console.log('updating total height');
	// 	tick().then(() => {
	// 		setTimeout(() => {
	// 			total_height = calculate_total_height();
	// 		}, 500);

	// 		// total_height = calculate_total_height();
	// 	});
	// });

	$effect(() => {
		selected_tab.value;

		reset_scroll();
	});

	function getItemOffset(startIndex: number): number {
		if (item_offsets[startIndex] !== undefined) {
			return item_offsets[startIndex];
		}
		let offset = 0;
		for (let i = 0; i < startIndex; i++) {
			const itemId = items[i].id;
			let height = item_heights[itemId];
			if (!height) {
				height = item_heights[itemId] || height_calc(items[i]);
			}
			offset += height;
		}
		item_offsets[startIndex] = offset;
		return offset;
	}

	// Calculate start index
	function calculateStartIndex() {
		let start = 0;
		let position = 0;
		while (start < items.length) {
			const item = items[start];
			const height = item_heights[item.id] || height_calc(item);
			if (position + height > scroll_top - overscan * height) break;
			position += height;
			start++;
		}
		return Math.max(0, start);
	}

	// TODO: check for when height_calc is huge difference from actual height
	// Calculate end index
	function calculateEndIndex(start: number) {
		let end = start;
		let position = getItemOffset(start);
		while (end < items.length) {
			const item = items[end];
			const height = item_heights[item.id] || height_calc(item) || 50;
			position += height;
			if (position > scroll_top + viewport_height + overscan * height) break;
			end++;
		}
		return Math.min(end, items.length); // Clamp to items.length
	}

	const [visible_items, start, visible_bus_routes] = $derived.by(() => {
		const start = calculateStartIndex();
		const end = calculateEndIndex(start);

		const visible_items = items.slice(start, browser ? end : Math.min(ssr_min, items.length));

		let visible_bus_routes: string[] = [];

		// if user is looking at bus stops, we need to monitor the routes
		if (selected_tab.value === 'bus' && type === 'stop') {
			//@ts-expect-error
			visible_bus_routes = visible_items.flatMap((stop) => stop.routes.map((r) => r.id));
		}

		return [
			visible_items.map((item, idx) => ({
				id: item.id,
				data: item,
				top: getItemOffset(start + idx)
			})),
			start,
			visible_bus_routes
		];
	});

	$effect(() => {
		// visible_bus_routes;
		visible_bus_routes.forEach((r) => monitored_bus_routes.add(r));
	});

	function calculate_total_height() {
		const total_items = Math.min(items_before_scroll || items.length, items.length);

		// await tick();
		let total = 0;
		for (let i = 0; i < total_items; i++) {
			const item = items[i];
			let height = item_heights[item.id] || height_calc(item);
			// if (!height) {
			// 	height = item_heights[item.id] || height_calc(item);
			// 	item_heights[item.id] = height;
			// }
			total += height;
		}
		return total;
	}

	let total_height = $derived.by(calculate_total_height);
</script>

<!-- TODO: back to top button in header -->

<div class="flex flex-col text-neutral-200 relative w-full z-30">
	<div
		class="flex sticky top-0 bg-neutral-900/95 backdrop-blur-xs shadow-lg shadow-black/10 items-center justify-between w-full z-30"
	>
		<h1 class="flex gap-2 items-center font-bold text-xl pl-2">
			<span>
				{title}
			</span>

			{#if locate_button}
				{@render locate_button()}
			{/if}
		</h1>

		<div class="bg-neutral-800/50 rounded-full p-1 border border-neutral-700/50 shadow-inner">
			{#snippet tab_button(value: 'train' | 'bus', data: unknown[])}
				{@const Icon = tab_icons[value]}
				<button
					class="relative px-4 py-1 rounded-full transition-all duration-200 flex items-center gap-2"
					class:text-neutral-100={selected_tab.value === value && data.length}
					class:text-neutral-400={selected_tab.value !== value && data.length}
					class:text-neutral-500={!data.length}
					class:font-medium={selected_tab.value === value}
					class:opacity-40={!data.length}
					class:cursor-not-allowed={!data.length}
					onclick={() => (selected_tab.value = value)}
					disabled={!data.length}
					aria-label={`Show ${value} stops`}
				>
					<Icon class="w-4 h-4" />
					<span class="capitalize">{value}</span>

					{#if selected_tab.value === value && data.length}
						<div
							in:send={{ key: 'tab' }}
							out:receive={{ key: 'tab' }}
							class="absolute inset-0 bg-neutral-700/50 rounded-full -z-10"
						></div>
					{/if}
				</button>
			{/snippet}

			<div class="flex gap-1">
				{@render tab_button('train', train_data)}
				{@render tab_button('bus', bus_data)}
			</div>
		</div>
	</div>
	<div class="h-px bg-linear-to-r from-transparent via-neutral-700/50 to-transparent"></div>

	<div
		bind:this={viewport_el}
		bind:offsetHeight={viewport_height}
		onscroll={async (e) => {
			await tick();
			scroll_top = e.currentTarget.scrollTop;
		}}
		style="-webkit-overflow-scrolling: touch; {style_ ?? ''}"
		class="relative overflow-y-auto text-base {class_name ?? ''}"
	>
		<div style:height="{total_height}px" class="relative">
			<div class="will-change-transform" style:transform="translateY({getItemOffset(start)}px)">
				{#each visible_items as { data, id } (id)}
					<div
						bind:offsetHeight={item_heights[data.id]}
						class="relative w-full list-item will-change-transform bg-neutral-950 border border-neutral-800/50 rounded-sm"
					>
						<button
							class="w-full flex justify-between items-center p-2 hover:bg-neutral-800/50 active:bg-neutral-700/50 transition-colors duration-200"
							onclick={() => {
								pushState('', { modal: type, data: JSON.parse(JSON.stringify(data)) });
							}}
						>
							<Item {data} />
						</button>

						{#if pin_rune}
							<Pin
								bind:pin_rune
								id={data.id}
								class="absolute z-20 right-0 py-1 px-2 rounded-md text-neutral-200 hover:text-neutral-400 top-[50%] transform -translate-y-1/2"
							/>
						{/if}

						<!-- {#if i !== visible_items.length - 1}
							<div class="item-separator" />
						{/if} -->
					</div>
				{/each}
			</div>
		</div>
	</div>
</div>
