<script lang="ts" generics="T extends string | number">
	import { Pin } from 'lucide-svelte';
	import type { Snippet } from 'svelte';
	import { slide } from 'svelte/transition';
	import { quintOut } from 'svelte/easing';
	import { pushState } from '$app/navigation';
	import type { PersistedRune } from './util.svelte';

	// export let state: App.PageState;
	let {
		state,
		pin_rune = $bindable(),
		children
	}: {
		state: App.PageState<T>;
		pin_rune: PersistedRune<T[]>;
		children: Snippet;
	} = $props();
</script>

<div class="relative w-full">
	<button
		class="w-full flex justify-between items-center py-1 px-1 hover:bg-neutral-950"
		transition:slide={{ easing: quintOut, axis: 'y', duration: 100 }}
		onclick={() => {
			pushState('', state);
		}}
	>
		{@render children()}
	</button>

	<button
		onclick={() =>
			(pin_rune.value = pin_rune.value.includes(state.dialog_id)
				? pin_rune.value.filter((id) => id !== state.dialog_id)
				: [...pin_rune.value, state.dialog_id])}
		aria-label="Pin to home screen"
		class="absolute z-20 right-0 py-1 px-2 rounded-md text-indigo-300 hover:text-indigo-600 top-[50%] transform -translate-y-1/2"
	>
		{#if pin_rune.value.includes(state.dialog_id)}
			<Pin fill="#4f46e5" />
		{:else}
			<Pin />
		{/if}
	</button>
</div>
