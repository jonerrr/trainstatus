<script lang="ts">
	import type { Snippet } from 'svelte';

	// import { slide } from 'svelte/transition';
	// import { quintOut } from 'svelte/easing';

	import Pin from '$lib/Pin.svelte';
	import type { Pins } from '$lib/pins.svelte';
	import type { LocalStorage } from '$lib/storage.svelte';
	import { type ModalWithId, open_modal } from '$lib/url_params.svelte';

	interface Props {
		state: ModalWithId;
		// id?: string;
		pins?: LocalStorage<Pins>;
		children: Snippet;
	}

	let {
		state: pState,
		// id,
		pins = $bindable(),
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
			open_modal(pState);
		}}
	>
		{@render children()}
	</button>

	{#if pins}
		<Pin
			bind:pins
			id={pState.id}
			source={pState.data.source}
			class="absolute top-[50%] right-0 z-20 -translate-y-1/2 transform rounded-md px-2 py-1 text-neutral-200 hover:text-neutral-400"
		/>
	{/if}
</div>
