<script lang="ts">
	import { browser } from '$app/environment';
	import type { Component } from 'svelte';

	let {
		cursor = $bindable(),
		hoveredTripId = $bindable(null),
		hoveredObject = $bindable(null),
		hoverX = $bindable(0),
		hoverY = $bindable(0),
		...tripMarkerProps
	}: {
		sources?: import('$lib/client').Source[];
		refreshInterval?: number;
		enabled?: boolean;
		cursor?: 'default' | 'pointer' | undefined;
		hoveredTripId?: string | null;
		hoveredObject?: import('$lib/map/trajectoryArrow').ActiveVehicle | null;
		hoverX?: number;
		hoverY?: number;
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
	<TripMarkers
		{...tripMarkerProps}
		bind:cursor
		bind:hoveredTripId
		bind:hoveredObject
		bind:hoverX
		bind:hoverY
	/>
{/if}
