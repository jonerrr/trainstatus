<script lang="ts">
	import type { ComponentProps } from 'svelte';

	import { browser } from '$app/environment';

	// TripMarkers pulls in deck.gl, so it is only ever loaded in the browser.
	// Referencing its type here (rather than the bare `Component`) keeps the
	// forwarded props type-checked against the real component.
	type TripMarkersComponent = typeof import('./TripMarkers.svelte').default;

	let { ...tripMarkerProps }: ComponentProps<TripMarkersComponent> = $props();

	let TripMarkers = $state<TripMarkersComponent | null>(null);

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
