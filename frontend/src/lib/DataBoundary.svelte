<script lang="ts">
	import type { Snippet } from 'svelte';

	import ErrorView from '#lib/ErrorView.svelte';
	import { getTransitData } from '#lib/transit/store.svelte.js';

	let { children }: { children: Snippet } = $props();
	const transit = getTransitData();
	// The callback is only consumed by explicit recovery events, not rendering.
	let resetBoundary: (() => void) | undefined;
	function recover() {
		transit.retry();
		const reset = resetBoundary;
		resetBoundary = undefined;
		reset?.();
	}
	function check() {
		if (transit.status.error) throw transit.status.error;
	}
</script>

<svelte:window ononline={recover} />
<svelte:boundary
	onerror={(error, reset) => {
		resetBoundary = reset;
		console.error(error);
	}}
>
	{check()}
	{@render children()}
	{#snippet failed()}
		<div class="h-dvh overflow-auto bg-canvas">
			<ErrorView
				error={{ status: 500, message: 'Couldn’t load transit data. Please try again.' }}
				onretry={recover}
			/>
		</div>
	{/snippet}
</svelte:boundary>
