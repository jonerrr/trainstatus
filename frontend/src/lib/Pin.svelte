<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import type { Pins } from '#lib/pins.svelte.js';
	import type { LocalStorage } from '#lib/storage.svelte.js';

	import { Pin } from '@lucide/svelte';

	// TODO: maybe just take the localstorage.current instead of the entire localstorage object
	let {
		pins = $bindable(),
		id,
		source,
		class: className,
		size
	}: {
		pins: LocalStorage<Pins>;
		id: string;
		source: Source;
		class?: string;
		size?: string;
	} = $props();

	const isPinned = $derived(pins.current[source].includes(id));

	function togglePin() {
		if (isPinned) {
			pins.current = {
				...pins.current,
				[source]: pins.current[source].filter((pId) => pId !== id)
			};
		} else {
			pins.current = {
				...pins.current,
				[source]: [...pins.current[source], id]
			};
		}
	}
</script>

<button onclick={togglePin} aria-label="Pin to home screen" class={className}>
	{#if isPinned}
		<Pin {size} fill="#fff" />
	{:else}
		<Pin {size} />
	{/if}
</button>
