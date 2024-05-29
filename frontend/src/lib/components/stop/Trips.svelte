<script lang="ts">
	import type { Trip } from '$lib/api';
	import { stops } from '$lib/stores';
	import List from '$lib/components/List.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import TripPreview from '$lib/components/trip/Preview.svelte';

	export let trips: Trip[];
	// if (!trips.length) {
	// 	console.log('no trips');
	// }
</script>

<!-- TODO: fix max-h -->
<List loading={false} class="h-96">
	{#if trips.length}
		{#each trips as trip (trip.id)}
			<TripPreview {trip} />
		{/each}
	{:else}
		<div class="text-indigo-300 text-center">No trips found</div>
	{/if}
</List>
