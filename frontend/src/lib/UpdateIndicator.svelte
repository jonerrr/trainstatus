<script lang="ts">
	import { prefersReducedMotion } from 'svelte/motion';
	import { slide } from 'svelte/transition';

	import { getUpdateStatus } from '#lib/resources/status.svelte.js';

	import { CircleAlert } from '@lucide/svelte';

	const status = getUpdateStatus();
	let retrying = $state(false);
	const visible = $derived(status.failed);
	async function retry() {
		retrying = true;
		try {
			await status.retry();
		} finally {
			retrying = false;
		}
	}
</script>

<span class="sr-only" role="status">{visible ? 'Updates unavailable' : ''}</span>
{#if visible}
	<div
		class="overflow-hidden"
		inert={!visible}
		transition:slide={{ duration: prefersReducedMotion.current ? 0 : 200 }}
	>
		<button
			type="button"
			class="update-control flex h-5 w-full items-center justify-center gap-2 bg-amber-950 text-xs text-amber-200 md:h-auto md:min-h-16 md:flex-col md:gap-1 md:py-2"
			aria-label="Updates unavailable. Retry updates"
			disabled={retrying}
			onclick={retry}
		>
			<CircleAlert size={14} />
			<span>Updates unavailable</span>
			<span class="underline">{retrying ? 'Retrying…' : 'Retry'}</span>
		</button>
	</div>
{/if}
