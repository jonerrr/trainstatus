<script lang="ts" generics="T, B">
	import { BusFront, TrainFront, AArrowUp, AArrowDown } from 'lucide-svelte';
	import type { Component, Snippet } from 'svelte';
	import { crossfade, slide } from 'svelte/transition';
	import { cubicInOut, quintOut } from 'svelte/easing';
	import { persisted_rune } from './util.svelte';
	import Icon from './Icon.svelte';
	import type { Stop } from './static';
	// import type { Stop } from './static';

	let {
		title,
		// bus_tab,
		// train_tab,
		button,
		bus_data,
		train_data,
		locate_button,
		search = false,
		min_items,
		class: class_name
	}: {
		title: string;
		locate_button?: Snippet;
		search?: boolean;
		button: Snippet<[T | B, boolean]>;
		// bus_tab: Snippet<[B]>;
		// train_tab: Snippet<[T]>;
		bus_data: B[];
		train_data: T[];
		min_items?: number;
		class?: string;
	} = $props();

	let list_div: HTMLDivElement;
	let list_height = $state(0);

	function item_heights() {
		const list_items = Array.from(list_div.querySelectorAll('#list-item')).slice(
			0,
			min_items
		) as HTMLDivElement[];

		list_height = list_items.reduce((h, e) => e.offsetHeight + h, 0);
	}

	$effect(() => {
		if (min_items) {
			// initial height check
			item_heights();

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
				// 		// Add your custom logic here
				// 	}
				// });
			});
			const config = { childList: true, subtree: true };
			observer.observe(list_div, config);
		}
	});
	$inspect(list_height);
	let tab = persisted_rune<'train' | 'bus'>(`${title.toLowerCase()}_tab`, 'train');
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

<!-- TODO: use calc() for getting height of list div -->
<div
	transition:slide={{ easing: quintOut, axis: 'y', duration: 200, delay: 200 }}
	class="flex flex-col text-neutral-200 relative w-full px-1 z-30"
>
	<div class="flex text-neutral-50 justify-between w-full z-30">
		<div class="flex gap-1 items-center font-bold text-lg">
			{title}

			<button
				aria-label="Change font size"
				class="text-white rounded p-1 active:bg-neutral-700 hover:bg-neutral-600"
				class:bg-neutral-700={large.value}
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
				class:text-neutral-100={tab.value === value}
				onclick={() => (tab.value = value)}
			>
				<Icon class="relative z-10" />

				{#if tab.value === value}
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
		{#if tab.value === 'train'}
			{#each train_data as d}
				{@render button(d, large.value)}
			{/each}
		{:else}
			{#each bus_data as d}
				{@render button(d, large.value)}
			{/each}
		{/if}
	</div>

	<!-- {#if search}
		{@render search()} -->
</div>
