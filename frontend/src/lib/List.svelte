<script lang="ts">
	import { BusFront, TrainFront } from 'lucide-svelte';
	import type { Snippet } from 'svelte';
	import { persisted_rune } from './util.svelte';
	import type { Stop } from './static';

	interface ListProps {
		title: string;
		locate_button?: Snippet;
		search?: Snippet;
		bus_tab: Snippet<[]>;
		train_tab: Snippet;
		min_items?: number;
	}

	let { title, bus_tab, train_tab, locate_button, search, min_items }: ListProps = $props();

	let list_div: HTMLDivElement;
	let list_height = $state(0);

	$effect(() => {
		if (min_items) {
			const observer = new MutationObserver((mutations) => {
				// Callback function to handle mutations
				console.log('mutation');
				const list_items = Array.from(list_div.querySelectorAll('#list-item')).slice(0, min_items);
				list_height = list_items.reduce((h, e) => e.offsetHeight + h, 0);

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

	let tab = persisted_rune(`${title.toLowerCase()}_tab`, 'Train');
</script>

<div class="flex flex-col text-neutral-200 relative w-full">
	<div class="flex text-neutral-300 fixed justify-between w-full z-30">
		<div class="flex gap-1 items-center font-bold text-lg">
			{title}
			{#if locate_button}
				{@render locate_button()}
			{/if}
		</div>

		<div class="grid grid-cols-2 bg-neutral-900 rounded text-indigo-100 border border-neutral-500">
			<button
				class="p-1 px-2 rounded-l relative border-2 border-transparent hover:text-neutral-400"
				class:bg-indigo-800={tab.value === 'Train'}
				class:border-indigo-500={tab.value === 'Train'}
				onclick={() => (tab.value = 'Train')}
			>
				<TrainFront />
			</button>
			<button
				class="p-1 px-2 rounded-r relative border-2 border-transparent hover:text-neutral-400"
				class:bg-indigo-800={tab.value === 'Bus'}
				class:border-indigo-500={tab.value === 'Bus'}
				onclick={() => (tab.value = 'Bus')}
			>
				<BusFront />
			</button>
		</div>
	</div>

	<div
		bind:this={list_div}
		style={`min-height: ${min_items ? list_height : 'auto'}px;`}
		class="flex flex-col divide-y overflow-auto overscroll-none divide-neutral-800 text-base mb-16 mt-8"
	>
		{#if tab.value === 'Train'}
			{@render train_tab()}
		{:else}
			{@render bus_tab()}
		{/if}
	</div>

	<!-- {#if search}
		{@render search()} -->
</div>
