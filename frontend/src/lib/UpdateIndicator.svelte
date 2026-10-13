<script lang="ts">
	import { prefersReducedMotion } from 'svelte/motion';
	import { slide } from 'svelte/transition';

	import { getTransitData } from '#lib/transit/store.svelte.js';

	import { CircleAlert } from '@lucide/svelte';

	const data = getTransitData();
	const status = data.status;
	let { compact = false }: { compact?: boolean } = $props();
</script>

<span class="sr-only" role="status">{status.warning ? 'Can’t update data' : ''}</span>
{#if status.warning}
	<div
		class="overflow-hidden"
		transition:slide={{ duration: prefersReducedMotion.current ? 0 : 200 }}
	>
		<button
			type="button"
			class={[
				'update-control flex w-full items-center justify-center gap-2 bg-amber-950 text-xs text-amber-200',
				compact ? 'min-h-9 py-2' : 'h-5 md:h-auto md:min-h-16 md:flex-col md:gap-1 md:py-2'
			]}
			aria-label="Can’t update data. Retry updates"
			onclick={() => data.retry()}
		>
			<CircleAlert size={14} />
			<span>Can’t update data</span>
			<span class="underline">Retry</span>
		</button>
	</div>
{/if}
