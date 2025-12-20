<script lang="ts" generics="T extends string | number">
	import { Pin } from '@lucide/svelte';

	import type { PersistedRune } from './util.svelte';

	// TODO: not sure if i need to bind
	let {
		pin_rune = $bindable(),
		id = $bindable(),
		class: class_name,
		size
	}: {
		pin_rune: PersistedRune<T[]>;
		id: T;
		class?: string;
		size?: string;
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
		<Pin {size} fill="#fff" />
	{:else}
		<Pin {size} />
	{/if}
</button>

<!-- <style>
	@keyframes wiggle {
		0% {
			transform: rotate(0deg);
		}
		80% {
			transform: rotate(0deg);
		}
		85% {
			transform: rotate(10deg);
		}
		95% {
			transform: rotate(-10deg);
		}
		100% {
			transform: rotate(0deg);
		}
	}

	.wiggle {
		animation: wiggle 1s ease-in-out;
	}
</style> -->
