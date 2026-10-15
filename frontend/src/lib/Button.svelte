<script lang="ts">
	import type { Snippet } from 'svelte';

	import Pin from '#lib/Pin.svelte';
	import { trip_pin_key, type Pins } from '#lib/pins.svelte.js';
	import type { LocalStorage } from '#lib/storage.svelte.js';
	import { open_modal, type ModalData } from '#lib/url_params.svelte.js';

	interface Props {
		state: ModalData;
		pins?: LocalStorage<Pins>;
		children: Snippet;
	}

	let { state: pState, pins = $bindable(), children }: Props = $props();

	const pin_id = $derived(
		pState.type === 'trip' && pState.route_id ? trip_pin_key(pState.route_id, pState.id) : pState.id
	);
</script>

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
			id={pin_id}
			source={pState.source}
			class="absolute top-[50%] right-0 z-20 -translate-y-1/2 transform rounded-md px-2 py-1 text-neutral-200 hover:text-neutral-400"
		/>
	{/if}
</div>
