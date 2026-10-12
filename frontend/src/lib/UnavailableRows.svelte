<script lang="ts">
	import { getTransitData } from '#lib/transit/store.svelte.js';

	interface Props {
		arrivals?: boolean;
		/** Last successful fetch of the data shown, epoch milliseconds. */
		fetched_at?: number | null;
		/** Resets the failed boundary showing this; called after a shared retry. */
		reset?: () => void;
	}

	let { arrivals = false, fetched_at = null, reset }: Props = $props();

	const transit = getTransitData();
	const retries = transit.status.retries;
	$effect(() => {
		if (transit.status.retries !== retries) reset?.();
	});

	const age = $derived.by(() => {
		if (fetched_at === null) return undefined;
		const minutes = Math.floor((Date.now() - fetched_at) / 60_000);
		return minutes < 1 ? 'just now' : `${minutes} min ago`;
	});
</script>

{#if arrivals}
	<p class="p-2 text-center text-neutral-400">
		Arrivals unavailable{#if age}, last fetched {age}{/if}
		{#if reset}
			<button class="ml-1 text-amber-200 underline" onclick={() => transit.retry()}>Retry</button>
		{/if}
	</p>
{:else}
	<span class="text-xs text-amber-200">Unavailable</span>
{/if}
