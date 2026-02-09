<script lang="ts">
	import { Pin } from '@lucide/svelte';
	import type { Source } from '@trainstatus/client';
	import type { PersistedState } from 'runed';

	import { type Pins } from './stores.svelte';

	let {
		pins,
		id,
		source,
		class: class_name,
		size
	}: {
		pins: PersistedState<Pins>;
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
