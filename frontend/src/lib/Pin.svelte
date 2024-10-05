<script lang="ts" generics="T extends string | number">
	import { Pin } from 'lucide-svelte';
	import type { PersistedRune } from './util.svelte';

	// TODO: not sure if i need to bind
	let {
		pin_rune = $bindable(),
		id = $bindable(),
		class: class_name
	}: {
		pin_rune: PersistedRune<T[]>;
		id: T;
		class?: string;
	} = $props();
</script>

<button
	onclick={() => {
		pin_rune.value = pin_rune.value.includes(id)
			? pin_rune.value.filter((p_id) => p_id !== id)
			: [...pin_rune.value, id];
	}}
	aria-label="Pin to home screen"
	class={class_name}
>
	{#if pin_rune.value.includes(id)}
		<Pin fill="#d4d4d4" />
	{:else}
		<Pin />
	{/if}
</button>
