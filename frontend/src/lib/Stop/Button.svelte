<script lang="ts">
	import type { Stop } from '$lib/static';
	import { type PersistedRune } from '$lib/util.svelte';
	import Button from '$lib/Button.svelte';

	interface ButtonProps {
		stop: Stop<'train' | 'bus'>;
		pin_rune: PersistedRune<number[]>;
	}

	let { stop, pin_rune }: ButtonProps = $props();
</script>

<Button
	state={{
		dialog_id: stop.id,
		dialog_type: 'stop',
		dialog_open: true
	}}
	{pin_rune}
>
	{#if stop.type === 'train'}
		<div class="w-[25%] grow-0 font-semibold text-neutral-200">
			{stop.name}
		</div>
	{:else}
		bus
	{/if}
</Button>
