<script lang="ts">
	import { page } from '$app/state';

	import FeatureSummary from '#lib/FeatureSummary.svelte';
	import { stop_time_context } from '#lib/resources/stop_times.svelte.js';
	import { trip_context } from '#lib/resources/trips.svelte.js';
	import { current_time } from '#lib/url_params.svelte.js';

	import type { MapTarget } from './interactions';
	import { presentFeature } from './presentation';

	let { target }: { target: MapTarget } = $props();
	const trips = trip_context.get();
	const times = stop_time_context.get();
	const trip = $derived(
		target.kind === 'trip' ? trips[target.source]?.current?.get(target.id) : undefined
	);
	const route = $derived(
		page.data.routes_by_id[target.source]?.[
			target.kind === 'route' ? target.id : (trip?.route_id ?? target.routeId ?? '')
		]
	);
	const feature = $derived(
		presentFeature(target, {
			route,
			trip,
			stop: page.data.stops_by_id[target.source]?.[target.id],
			stopTimes: times[target.source]?.current.by_trip_id.get(target.id),
			stops: page.data.stops_by_id[target.source],
			at: current_time.ms
		})
	);
</script>

<FeatureSummary {feature} />
