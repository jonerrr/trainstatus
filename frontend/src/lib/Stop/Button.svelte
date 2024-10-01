<script lang="ts">
	import type { Stop } from '$lib/static';
	import { persisted_rune, type PersistedRune } from '$lib/util.svelte';
	import Button from '$lib/Button.svelte';

	interface ButtonProps {
		stop: Stop<'train' | 'bus'>;
		pin_rune: PersistedRune<number[]>;
	}

	let { stop, pin_rune }: ButtonProps = $props();

	// const pins = persisted_rune<number[]>('stop_pins', []);
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
		<div class="w-[25%] grow-0 font-semibold text-indigo-200">
			{stop.name}
		</div>
	{:else}
		bus
	{/if}
</Button>
