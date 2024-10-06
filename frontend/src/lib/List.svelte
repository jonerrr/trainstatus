<script lang="ts" generics="T, B">
	import { BusFront, TrainFront, AArrowUp, AArrowDown } from 'lucide-svelte';
	import type { Snippet } from 'svelte';
	import { crossfade, slide } from 'svelte/transition';
	import { cubicInOut, quintOut } from 'svelte/easing';
	import { persisted_rune, type PersistedRune } from './util.svelte';
	// import type { Action } from 'svelte/action';

	let {
		title,
		button,
		bus_data,
		train_data,
		locate_button,
		selected_tab = $bindable(
			persisted_rune<'train' | 'bus'>(`${title.toLocaleLowerCase()}_tab`, 'train')
		),
		min_items,
		class: class_name
	}: {
		title: string;
		locate_button?: Snippet;
		selected_tab?: PersistedRune<'train' | 'bus'>;
		button: Snippet<[T | B, boolean]>;
		bus_data: B[];
		train_data: T[];
		min_items?: number;
		class?: string;
	} = $props();

	let list_height = $state(0);
	let list_div: HTMLDivElement | undefined = $state();

	// export function scrollIntoView() {
	// 	list_div.scrollIntoView({ behavior: 'smooth' });
	// }

	function item_heights() {
		const list_items = Array.from(list_div!.querySelectorAll('#list-item')).slice(
			0,
			min_items
		) as HTMLDivElement[];

		list_height = list_items.reduce((h, e) => e.offsetHeight + h, 0);
	}

	$effect(() => {
		if (list_div && (bus_data.length < 8 || train_data.length < 8)) {
			console.log('scrolling list into view');
			list_div.scrollIntoView({ behavior: 'smooth' });
		}
	});

	// const manage_scroll: Action<HTMLDivElement, [T[], B[]]> = (
	// 	node,
	// 	[train_data, bus_data]: [T[], B[]]
	// ) => {
	// 	if (bus_data.length < 8 || train_data.length < 8) {
	// 		console.log('scrolling list into view');
	// 		node.scrollIntoView({ behavior: 'smooth' });
	// 	}

	// 	// return {
	// 	// 	destroy() {
	// 	// 		// the node has been removed from the DOM
	// 	// 	}
	// 	// };
	// };

	$effect(() => {
		if (min_items && list_div) {
			// initial height check
			item_heights();

			// whenever list changes, recalculate height
			const observer = new MutationObserver(() => {
				// Callback function to handle mutations
				console.log('list mutation');
				item_heights();
				// mutations.forEach((mutation) => {
				// 	if (mutation.type === 'childList') {
				// 		console.log('Child nodes changed:', mutation.addedNodes);
				// 		// mutation.addedNodes.forEach((node) => {
				// 		//  if (node.id)
				// 		// })
				// 	}
				// });
			});
			const config = { childList: true, subtree: true };
			observer.observe(list_div, config);
		}
	});

	// const scroll_into_view: Action = (node) => {
	// 	// the node has been mounted in the DOM

	// 	node.scrollIntoView({ behavior: 'smooth' });
	// };

	let large = persisted_rune(`${title.toLowerCase()}_large`, false);

	const tab_icons = {
		train: TrainFront,
		bus: BusFront
	};

	const [send, receive] = crossfade({
		duration: 300,
		easing: cubicInOut
	});
</script>

<div
	transition:slide={{ easing: quintOut, axis: 'y', duration: 200, delay: 200 }}
	class="flex flex-col text-neutral-200 relative w-full px-1 z-30"
>
	<div class="flex text-neutral-50 justify-between w-full z-30">
		<div class="flex gap-1 items-center font-bold text-lg">
			{title}

			<button
				aria-label="Change font size"
				class="rounded p-1 active:bg-neutral-800 hover:bg-neutral-800"
				class:bg-neutral-800={large.value}
				class:text-neutral-300={!large.value}
				class:text-white={large.value}
				onclick={() => (large.value = !large.value)}
			>
				{#if large.value}
					<AArrowUp />
				{:else}
					<AArrowDown />
				{/if}
			</button>

			{#if locate_button}
				{@render locate_button()}
			{/if}
		</div>

		{#snippet tab_button(value: 'train' | 'bus')}
			{@const Icon = tab_icons[value]}
			<button
				class="p-1 px-2 rounded relative m-0.5 border-transparent"
				class:text-neutral-100={selected_tab.value === value}
				onclick={() => (selected_tab.value = value)}
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
			{@render tab_button('train')}
			{@render tab_button('bus')}
		</div>
	</div>

	<div
		bind:this={list_div}
		style:height={min_items ? `${list_height}px` : 'auto'}
		class={`flex border-y border-neutral-800 flex-col divide-y overflow-auto overscroll-none divide-neutral-800 text-base ${class_name ?? ''}`}
	>
		{#if selected_tab.value === 'train'}
			{#each train_data as d}
				{@render button(d, large.value)}
			{/each}
		{:else}
			{#each bus_data as d}
				{@render button(d, large.value)}
			{/each}
		{/if}
	</div>
</div>
