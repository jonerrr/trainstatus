<script lang="ts">
	import { getTransitData } from '#lib/data/store.svelte.js';
	import FeatureSummary from '#lib/FeatureSummary.svelte';
	import { getCurrentTime } from '#lib/url-params.svelte.js';

	import type { MapTarget } from './interactions';
	import { presentFeature } from './presentation';

	const currentTime = getCurrentTime();
	const transit = getTransitData();

	let { target }: { target: MapTarget } = $props();
	const catalog = $derived(transit.peekCatalog(target.source));
	const routeId = $derived(
		target.kind === 'route' ? target.id : target.kind === 'trip' ? target.routeId : undefined
	);
	// Hover summaries use loaded service only; opening the trip loads its route.
	const read = $derived(
		target.kind === 'trip'
			? transit.loadedTrip(target.source, target.routeId, target.id, currentTime.value)
			: undefined
	);
	const feature = $derived(
		presentFeature(target, {
			route: routeId ? catalog?.routesById.get(routeId) : undefined,
			trip: read?.trip,
			stop: catalog?.stopsById.get(target.id),
			calls: read?.calls,
			stops: catalog?.stopsById,
			at: currentTime.ms
		})
	);
</script>

<FeatureSummary {feature} />
