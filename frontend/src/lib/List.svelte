<script lang="ts">
	import { BusFront, TrainFront } from 'lucide-svelte';
	import type { Snippet } from 'svelte';
	import { persisted_rune } from './util.svelte';

	interface ListProps {
		title: string;
		locate_button?: Snippet;
		search?: Snippet;
		bus_tab: Snippet;
		train_tab: Snippet;
	}

	let { title, bus_tab, train_tab, locate_button, search }: ListProps = $props();

	let tab = persisted_rune(`${title.toLowerCase()}_tab`, 'Train');
</script>

<div class="flex flex-col text-indigo-200 relative w-full">
	<div class="flex text-indigo-300 fixed justify-between w-full z-30">
		<div class="flex gap-1 items-center font-bold text-lg">
			{title}
			{#if locate_button}
				{@render locate_button()}
			{/if}
		</div>

		<div class="grid grid-cols-2 bg-neutral-900 rounded text-indigo-100 border border-neutral-500">
			<button
				class="p-1 px-2 rounded-l relative border-2 border-transparent hover:text-indigo-400"
				class:bg-indigo-800={tab.value === 'Train'}
				class:border-indigo-500={tab.value === 'Train'}
				onclick={() => (tab.value = 'Train')}
			>
				<TrainFront />
			</button>
			<button
				class="p-1 px-2 rounded-r relative border-2 border-transparent hover:text-indigo-400"
				class:bg-indigo-800={tab.value === 'Bus'}
				class:border-indigo-500={tab.value === 'Bus'}
				onclick={() => (tab.value = 'Bus')}
			>
				<BusFront />
			</button>
		</div>
	</div>

	<div
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
