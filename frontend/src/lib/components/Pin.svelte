<script lang="ts">
	import { createToggle, melt, createSync } from '@melt-ui/svelte';
	import type { Writable } from 'svelte/store';
	import { Pin } from 'lucide-svelte';

	// should probably use generic types
	export let item_id: string | number;
	export let store: Writable<string[] | number[]>;
	export let size: string = 'size-8';

	const {
		elements: { root },
		states
	} = createToggle();
	const sync = createSync(states);
	const pressed = states.pressed;
	$: sync.pressed($store.includes(item_id), (v) => {
		if (v) {
			$store = [...$store, item_id];
		} else {
			$store = $store.filter((id) => id !== item_id);
		}
	});
</script>

<button
	use:melt={$root}
	on:click|stopPropagation
	aria-label="Pin to home screen"
	class={`z-30 ${size} items-center justify-center rounded-md
text-base text-indigo-300 data-[state=on]:text-indigo-600
data-[disabled]:cursor-not-allowed`}
>
	{#if $pressed}
		<Pin fill="#4f46e5" />
	{:else}
		<Pin />
	{/if}
</button>
