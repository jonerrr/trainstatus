<script lang="ts" generics="T extends string | number">
	import type { Snippet } from 'svelte';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { pushState } from '$app/navigation';
	import type { PersistedRune } from './util.svelte';
	import Pin from './Pin.svelte';

	let {
		state,
		// id,
		pin_rune = $bindable(),
		children
	}: {
		state: App.PageState;
		// id?: string;
		pin_rune?: PersistedRune<T[]>;
		children: Snippet;
	} = $props();
</script>

<!-- 	transition:slide={{ easing: quintOut, axis: 'y', duration: 100 }}
 -->
<!-- <div class="relative w-full list-item"> -->
<button
	class="hover:bg-neutral-900 active:bg-neutral-900 w-full flex justify-between items-center p-1 text-white"
	onclick={() => {
		pushState('', JSON.parse(JSON.stringify(state)));
	}}
>
	{@render children()}
</button>

{#if pin_rune}
	<Pin
		bind:pin_rune
		id={state.data.id}
		class="absolute z-20 right-0 py-1 px-2 rounded-md text-neutral-200 hover:text-neutral-400 top-[50%] transform -translate-y-1/2"
	/>
{/if}
<!-- </div> -->
