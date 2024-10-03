<script lang="ts" generics="T, B">
	import { BusFront, TrainFront } from 'lucide-svelte';
	import type { Snippet } from 'svelte';
	import { persisted_rune } from './util.svelte';
	// import type { Stop } from './static';

	// interface ListProps

	let {
		title,
		// bus_tab,
		// train_tab,
		button,
		bus_data,
		train_data,
		locate_button,
		search,
		min_items,
		class: class_name
	}: {
		title: string;
		locate_button?: Snippet;
		search?: Snippet;
		button: Snippet<[T | B]>;
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
				console.log('mutation');
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
	let tab = persisted_rune<'Train' | 'Bus'>(`${title.toLowerCase()}_tab`, 'Train');
</script>

<div class="flex flex-col text-neutral-200 relative w-full px-1 z-30">
	<div class="flex text-neutral-300 justify-between w-full z-30">
		<div class="flex gap-1 items-center font-bold text-lg">
			{title}
			{#if locate_button}
				{@render locate_button()}
			{/if}
		</div>

		<!-- TODO: make this a snippet -->
		<div class="grid grid-cols-2 bg-neutral-700 rounded text-neutral-300 border border-neutral-600">
			<button
				class="p-1 px-2 rounded-l relative m-0.5 border-transparent"
				class:bg-neutral-900={tab.value === 'Train'}
				class:text-neutral-100={tab.value === 'Train'}
				onclick={() => (tab.value = 'Train')}
			>
				<TrainFront />
			</button>
			<button
				class="p-1 px-2 rounded-r relative m-0.5 border-transparent"
				class:bg-neutral-900={tab.value === 'Bus'}
				class:text-neutral-100={tab.value === 'Train'}
				onclick={() => (tab.value = 'Bus')}
			>
				<BusFront />
			</button>
		</div>
	</div>

	<div
		bind:this={list_div}
		style={`height: ${min_items ? list_height : 'auto'}px;`}
		class={`flex flex-col divide-y overflow-auto overscroll-none divide-neutral-800 text-base ${class_name ?? ''}`}
	>
		{#if tab.value === 'Train'}
			{#each train_data as d}
				{@render button(d)}
			{/each}
		{:else}
			{#each bus_data as d}
				{@render button(d)}
			{/each}
		{/if}
	</div>

	<!-- {#if search}
		{@render search()} -->
</div>
