<script lang="ts" generics="T, B">
	import { createVirtualizer, notUndefined } from '@tanstack/svelte-virtual';
	import { VirtualList } from 'svelte-virtuallists';
	import { BusFront, TrainFront } from 'lucide-svelte';
	import { type Snippet } from 'svelte';
	import { crossfade } from 'svelte/transition';
	import { cubicInOut } from 'svelte/easing';
	import { persisted_rune, type PersistedRune } from './util.svelte';
	import type { Stop } from './static';
	import { monitored_bus_routes } from './stop_times.svelte';
	import Button from './Button.svelte';
	import TripButton from './Trip/Button.svelte';
	import StopButton from './Stop/Button.svelte';
	import RouteButton from './Route/Button.svelte';

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

	interface ListItems {
		bus: HTMLDivElement[];
		train: HTMLDivElement[];
	}

	interface Props {
		// title of list
		title: string;
		// renders geolocate button for stops list
		locate_button?: Snippet;
		// current selected tab. Used for selecting correct search index
		selected_tab?: PersistedRune<'train' | 'bus'>;
		type: ItemType;
		pin_rune: PersistedRune<(number | string)[]>;
		bus_data: B[];
		train_data: T[];
		// control height of list by number of items
		min_items?: number;
		// watch for monitored routes changes
		monitor_routes?: boolean;
		class?: string;
		// scroll list into view if theres less than 8 items
		auto_scroll?: boolean;
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
		min_items,
		monitor_routes = false,
		class: class_name,
		auto_scroll = false
	}: Props = $props();

	let list_div = $state<HTMLDivElement | null>(null);

	let list_item_els = $state<ListItems>({
		bus: [],
		train: []
	});

	// if bus/train data don't have any items, switch tabs
	$effect(() => {
		if (!bus_data.length && train_data.length) selected_tab.value = 'train';
		if (!train_data.length && bus_data.length) selected_tab.value = 'bus';
	});

	// let offset = $state(0);

	const virtualizer = $derived.by(() => {
		console.log('creating virtualizer');
		return createVirtualizer<HTMLDivElement, HTMLDivElement>({
			count: selected_tab.value === 'train' ? train_data.length : bus_data.length,
			getScrollElement: () => list_div,
			estimateSize: () => item_components[type][1],
			paddingEnd: 5,
			overscan: 5,
			initialOffset: 100,
			debug: false
		});
	});
	const virtual_list_items = $derived($virtualizer.getVirtualItems());

	$inspect($virtualizer.scrollOffset);
	// TODO: fix list height calculation
	// const list_height = $derived(
	// 	min_items
	// 		? $virtualizer.measurementsCache.slice(0, min_items).reduce((h, e) => e.size, 5) + 'px'
	// 		: 'auto'
	// );

	// const max_height = $derived(
	// 	min_items ? virtual_list_items.slice(0, min_items).reduce((h, e) => e.size + h, 5) : 0
	// );
	// $inspect(max_height);

	// Ref: https://github.com/TanStack/virtual/issues/640#issuecomment-1885029911
	// Ref: https://github.com/TanStack/virtual/discussions/476#discussioncomment-4724139
	// let [virtualListBefore, virtualListAfter] = $derived(
	// 	virtual_list_items.length > 0
	// 		? [
	// 				notUndefined(virtual_list_items[0]).start - $virtualizer.options.scrollMargin,
	// 				$virtualizer.getTotalSize() -
	// 					notUndefined(virtual_list_items[virtual_list_items.length - 1]).end
	// 			]
	// 		: [0, 0]
	// );
	// $inspect($virtualizer.getVirtualItems());

	// https://github.com/TanStack/virtual/issues/866
	let mounted = $state(false);
	$effect(() => {
		if (!mounted && list_div !== null) {
			mounted = true;
			$virtualizer._willUpdate();
		}
	});

	$effect(() => {
		if (list_item_els[selected_tab.value].length) {
			list_item_els[selected_tab.value].forEach((el) => $virtualizer.measureElement(el));
		}
	});

	// probably could combine effects
	if (auto_scroll) {
		$effect(() => {
			if (list_div && (bus_data.length < 8 || train_data.length < 8)) {
				// console.log('scrolling list into view');
				list_div.scrollIntoView({ behavior: 'smooth' });
			}
		});
	}

	// $inspect(virtual_list_items);

	// we will monitor bus routes if it is a stop list
	// $effect(() => {
	// 	if (type === 'stop' && selected_tab.value === 'bus') {
	// 		console.log('monitoring bus routes');
	// 		const buses_shown = virtual_list_items.map((row) => bus_data[row.index]);
	// 		//@ts-expect-error
	// 		const bus_routes = buses_shown.flatMap((stop) => stop.routes.map((r) => r.id));
	// 		bus_routes.forEach((r) => monitored_bus_routes.add(r));
	// 	}
	// });
	// if (type === 'stop') {
	// const all_bus_routes = $derived(
	// 	bus_data
	// 		//@ts-expect-error
	// 		.flatMap((stop: Stop<'bus'>) => {
	// 			return stop.routes.map((r) => r.id);
	// 		})
	// );
	// const visible_bus_routes = $derived.by(() => {
	// 	if (selected_tab.value === 'bus') {
	// 		const bus_data_shown = virtual_list_items.map((row) => bus_data[row.index]);
	// 		//@ts-expect-error
	// 		return bus_data_shown.flatMap((stop) => stop.routes.map((r) => r.id));
	// 	}
	// 	return [];
	// });
	// $inspect(visible_bus_routes);
	// $effect(() => {
	// 	// console.log('adding routes');
	// 	visible_bus_routes.forEach((r) => monitored_bus_routes.add(r));
	// });
	// }
	// function get_items() {
	// 	const list_items = Array.from(list_div!.querySelectorAll('.list-item')) as HTMLDivElement[];
	// 	// start with 5 prevents scrollbars
	// 	list_height = list_items.slice(0, min_items).reduce((h, e) => e.offsetHeight + h, 5);
	// }
	// if (min_items) {
	// 	$effect(() => {
	// 		// initial height calculation
	// 		get_items();

	// 		// whenever list changes, recalculate height
	// 		const observer = new MutationObserver(() => {
	// 			// console.log('list mutation');
	// 			// if (min_items)
	// 			get_items();
	// 		});
	// 		observer.observe(list_div!, { childList: true, subtree: true, characterData: true });
	// 	});
	// }

	// let large = persisted_rune(`${title.toLowerCase()}_large`, false);

	const Item = item_components[type][0];

	const tab_icons = {
		train: TrainFront,
		bus: BusFront
	};

	const [send, receive] = crossfade({
		duration: 300,
		easing: cubicInOut
	});

	// $inspect(list_items);
</script>

<!-- 	transition:slide={{ easing: quintOut, axis: 'y', duration: 200, delay: 200 }} -->
<div class="flex flex-col text-neutral-200 relative w-full px-1 z-30">
	<div class="flex text-neutral-50 justify-between w-full z-30">
		<h1 class="flex gap-1 items-center font-bold text-lg">
			{title}

			{#if locate_button}
				{@render locate_button()}
			{/if}
		</h1>

		{#snippet tab_button(value: 'train' | 'bus', data: unknown[])}
			{@const Icon = tab_icons[value]}
			<button
				class="p-1 px-2 rounded relative m-0.5 border-transparent"
				class:text-neutral-100={selected_tab.value === value}
				onclick={() => (selected_tab.value = value)}
				disabled={!data.length}
				class:text-neutral-500={!data.length}
				aria-label={`Show ${value} stops`}
			>
				<Icon class="relative z-10" />

				{#if selected_tab.value === value}
					<div
						in:send={{ key: 'tab' }}
						out:receive={{ key: 'tab' }}
						class="absolute top-0 left-0 w-full h-full bg-neutral-800 rounded"
					></div>
				{/if}
			</button>
		{/snippet}

		<div
			class="grid grid-cols-2 bg-neutral-700 rounded text-neutral-300 border border-neutral-600 relative"
		>
			{@render tab_button('train', train_data)}
			{@render tab_button('bus', bus_data)}
		</div>
	</div>

	{#snippet list_item(data: T | B, idx: number, row_index: number)}
		<div
			bind:this={list_item_els[selected_tab.value][idx]}
			data-index={row_index}
			class="relative w-full"
		>
			<Button state={{ data, modal: type }} {pin_rune}>
				<Item {data} />
			</Button>
		</div>
	{/snippet}

	<div
		bind:this={list_div}
		class={`overflow-y-auto overscroll-contain text-base ${class_name ?? ''}`}
	>
		<div style="position: relative; height: {$virtualizer.getTotalSize()}px; width: 100%;">
			<!-- <div style="pdading-top: {virtualListBefore}; padding-bottom: {virtualListAfter}"> -->
			<div
				class="divide-y divide-neutral-800 border-y border-neutral-800"
				style="position: absolute; top: 0; left: 0; width: 100%; transform: translateY({virtual_list_items[0]
					? virtual_list_items[0].start
					: 0}px);"
			>
				<!-- {#if virtualListBefore > 0}
				<div style="height: {virtualListBefore}px"></div>
			{/if} -->

				{#if selected_tab.value === 'train'}
					{#each virtual_list_items as row, idx (row.index)}
						{@render list_item(train_data[row.index], idx, row.index)}
					{/each}
					<!-- {#each train_data as data, i}
						{@render list_item(data, i)}
					{/each} -->
				{:else}
					{#each virtual_list_items as row, idx (row.index)}
						{@render list_item(bus_data[row.index], idx, row.index)}
					{/each}
					<!-- {#each bus_data as data, i}
						{@render list_item(data, i)}
					{/each} -->
				{/if}
				<!-- {#if virtualListAfter > 0}
				<div style="height: {virtualListAfter}px"></div>
			{/if} -->
			</div>
		</div>
	</div>
</div>
