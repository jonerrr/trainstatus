<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import type { TransitData } from '#lib/data/store.svelte.js';

	let {
		data,
		source,
		routeId,
		at = undefined
	}: { data: TransitData; source: Source; routeId: string; at?: number } = $props();
	// The owner is fixed for this reader; source, route and time remain reactive.
	// svelte-ignore state_referenced_locally
	const resource = data.route(() => ({ source, routeId, at }));
</script>

<svelte:boundary>
	{const read = $derived(resource.current)}
	{#if read.state === 'loading'}
		<p data-pending={routeId}>loading</p>
	{:else if read.state === 'ready'}
		<p data-route={routeId}>{[...read.value.trips.keys()].join(',')}</p>
	{:else if read.state === 'missing'}
		<p data-missing={routeId}>Not found</p>
	{/if}
	{#if read.state === 'failed'}
		<p data-failed={routeId}>{read.error.message}</p>
	{/if}
</svelte:boundary>
