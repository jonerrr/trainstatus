<script lang="ts" generics="T">
	import { BusFront, TrainFront, AArrowUp, AArrowDown } from 'lucide-svelte';
	import type { Component, Snippet } from 'svelte';
	import { crossfade, slide } from 'svelte/transition';
	import { cubicInOut, quintOut } from 'svelte/easing';
	import { persisted_rune } from './util.svelte';
	import Icon from './Icon.svelte';
	import type { Stop } from './static';
	// import type { Stop } from './static';

	let {
		children,
		class: class_name
	}: {
		children: Snippet<[]>;
		class?: string;
	} = $props();

	// const tab_icons = {
	// 	train: TrainFront,
	// 	bus: BusFront
	// };

	// const [send, receive] = crossfade({
	// 	duration: 300,
	// 	easing: cubicInOut
	// });
</script>

<!-- TODO: use calc() for getting height of list div -->
<!-- <div
	transition:slide={{ easing: quintOut, axis: 'y', duration: 200, delay: 200 }}
	class="flex flex-col text-neutral-200 relative w-full px-1 z-30"
> -->
<div
	transition:slide={{ easing: quintOut, axis: 'y', duration: 200, delay: 200 }}
	class={`flex flex-col max-h-[60dvh] border-y border-neutral-800 divide-y overflow-auto overscroll-none divide-neutral-800 text-base ${class_name ?? ''}`}
>
	{@render children()}
	<!-- {#each data as d}
		{@render button(d)}
	{/each} -->

	<!-- {#if tab.value === 'train'}
			{#each train_data as d}
				{@render button(d, large.value)}
			{/each}
		{:else}
			{#each bus_data as d}
				{@render button(d, large.value)}
			{/each}
		{/if} -->
</div>
<!-- </div> -->
