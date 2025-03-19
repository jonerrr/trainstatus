<script lang="ts" generics="T extends string | number">
	import type { Snippet } from 'svelte';
	// import { slide } from 'svelte/transition';
	// import { quintOut } from 'svelte/easing';
	import { pushState } from '$app/navigation';
	import type { PersistedRune } from './util.svelte';
	import Pin from './Pin.svelte';

	interface Props {
		state: App.PageState;
		// id?: string;
		pin_rune?: PersistedRune<T[]>;
		children: Snippet;
	}

	let {
		state: pState,
		// id,
		pin_rune = $bindable(),
		children
	}: Props = $props();
</script>

<!-- 	transition:slide={{ easing: quintOut, axis: 'y', duration: 100 }}
 -->
<!-- currently only used in modals, not main list -->
<div class="relative list-item w-full">
	<button
		class="flex w-full items-center justify-between border-b border-neutral-800 p-1 text-white transition-colors duration-200 last:border-b-0 hover:bg-neutral-900 active:bg-neutral-900"
		onclick={() => {
			pushState('', $state.snapshot(pState));
		}}
	>
		{@render children()}
	</button>

	{#if pin_rune}
		<Pin
			bind:pin_rune
			id={pState.data.id}
			class="absolute top-[50%] right-0 z-20 -translate-y-1/2 transform rounded-md px-2 py-1 text-neutral-200 hover:text-neutral-400"
		/>
	{/if}
</div>
