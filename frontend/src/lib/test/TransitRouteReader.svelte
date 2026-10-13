<script lang="ts">
	import type { Source } from '#lib/client/index.js';
	import type { TransitData } from '#lib/transit/store.svelte.js';

	let {
		data,
		source,
		route_id,
		at = null
	}: { data: TransitData; source: Source; route_id: string; at?: number | null } = $props();
	// The owner is fixed for this reader; source, route and time remain reactive.
	// svelte-ignore state_referenced_locally
	const resource = data.route(() => ({ source, route_id, at }));
</script>

<svelte:boundary>
	{const state = $derived(resource.current)}
	{#if state.loading}
		<p data-pending={route_id}>loading</p>
	{:else if state.error}
		<p data-failed={route_id}>{state.error.message}</p>
	{:else if state.value}
		<p data-route={route_id}>{[...state.value.trips.keys()].join(',')}</p>
	{/if}
</svelte:boundary>
