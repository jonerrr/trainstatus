<script lang="ts">
	import type { Snippet } from 'svelte';

	import Pin from '#lib/Pin.svelte';
	import { tripPinKey, type Pins } from '#lib/pins.svelte.js';
	import type { LocalStorage } from '#lib/storage.svelte.js';
	import { openModal, type ModalData } from '#lib/url-params.svelte.js';

	interface Props {
		state: ModalData;
		pins?: LocalStorage<Pins>;
		children: Snippet;
	}

	let { state: pState, pins = $bindable(), children }: Props = $props();

	const pinId = $derived(
		pState.type === 'trip' && pState.routeId ? tripPinKey(pState.routeId, pState.id) : pState.id
	);
</script>

<!-- currently only used in modals, not main list -->
<div class="relative list-item w-full">
	<button
		class="flex w-full items-center justify-between border-b border-border-subtle p-1 text-white transition-colors duration-200 last:border-b-0 hover:bg-surface-1 active:bg-surface-1"
		onclick={() => {
			openModal(pState);
		}}
	>
		{@render children()}
	</button>

	{#if pins}
		<Pin
			bind:pins
			id={pinId}
			source={pState.source}
			class="absolute top-[50%] right-0 z-20 -translate-y-1/2 transform rounded-none px-2 py-1 text-secondary hover:text-muted"
		/>
	{/if}
</div>
