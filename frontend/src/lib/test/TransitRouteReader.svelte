<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import type { TransitData } from '#lib/transit/store.svelte.js';

	let {
		data,
		source,
		route_id,
		at = undefined
	}: { data: TransitData; source: Source; route_id: string; at?: number } = $props();
	// The owner is fixed for this reader; source, route and time remain reactive.
	// svelte-ignore state_referenced_locally
	const resource = data.route(() => ({ source, route_id, at }));
</script>

<svelte:boundary>
	{const read = $derived(resource.current)}
	{#if read.state === 'loading'}
		<p data-pending={route_id}>loading</p>
	{:else if read.state === 'ready'}
		<p data-route={route_id}>{[...read.value.trips.keys()].join(',')}</p>
	{:else if read.state === 'missing'}
		<p data-missing={route_id}>Not found</p>
	{/if}
	{#if read.state === 'failed'}
		<p data-failed={route_id}>{read.error.message}</p>
	{/if}
</svelte:boundary>
