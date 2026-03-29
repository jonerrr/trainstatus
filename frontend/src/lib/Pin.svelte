<script lang="ts">
	import type { Source } from '$lib/client';
	import type { Pins } from '$lib/pins.svelte';
	import type { LocalStorage } from '$lib/storage.svelte';

	import { Pin } from '@lucide/svelte';

	// TODO: maybe just take the localstorage.current instead of the entire localstorage object
	let {
		pins = $bindable(),
		id,
		source,
		class: class_name,
		size
	}: {
		pins: LocalStorage<Pins>;
		id: string;
		source: Source;
		class?: string;
		size?: string;
	} = $props();

	const is_pinned = $derived(pins.current[source].includes(id));

	function toggle_pin() {
		if (is_pinned) {
			pins.current = {
				...pins.current,
				[source]: pins.current[source].filter((p_id) => p_id !== id)
			};
		} else {
			pins.current = {
				...pins.current,
				[source]: [...pins.current[source], id]
			};
		}
	}
</script>

<button onclick={toggle_pin} aria-label="Pin to home screen" class={class_name}>
	{#if is_pinned}
		<Pin {size} fill="#fff" />
	{:else}
		<Pin {size} />
	{/if}
</button>
