<script lang="ts">
	import FeatureSummary from '#lib/FeatureSummary.svelte';
	import { getTransitData } from '#lib/transit/store.svelte.js';
	import { getCurrentTime } from '#lib/url_params.svelte.js';

	import type { MapTarget } from './interactions';
	import { presentFeature } from './presentation';

	const current_time = getCurrentTime();
	const transit = getTransitData();

	let { target }: { target: MapTarget } = $props();
	const catalog = $derived(transit.peekCatalog(target.source));
	const route_id = $derived(target.kind === 'route' ? target.id : target.routeId);
	// Hover summaries use loaded service only; opening the trip loads its route.
	const read = $derived(
		target.kind === 'trip' && target.routeId
			? transit.loadedTrip(target.source, target.routeId, target.id, current_time.value ?? null)
			: undefined
	);
	const feature = $derived(
		presentFeature(target, {
			route: route_id ? catalog?.routes_by_id.get(route_id) : undefined,
			trip: read?.trip,
			stop: catalog?.stops_by_id.get(target.id),
			calls: read?.calls,
			stops: catalog?.stops_by_id,
			at: current_time.ms
		})
	);
</script>

<FeatureSummary {feature} />
