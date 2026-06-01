<script lang="ts">
	import { browser } from '$app/environment';
	import type { Component } from 'svelte';

	let {
		...tripMarkerProps
	}: {
		source?: import('$lib/client').Source;
		refreshInterval?: number;
		enabled?: boolean;
	} = $props();

	let TripMarkers = $state<Component | null>(null);

	$effect(() => {
		if (!browser) return;

		let cancelled = false;
		import('./TripMarkers.svelte').then((module) => {
			if (!cancelled) TripMarkers = module.default;
		});

		return () => {
			cancelled = true;
		};
	});
</script>

{#if TripMarkers}
	<TripMarkers {...tripMarkerProps} />
{/if}
