<script lang="ts">
	import { page } from '$app/state';

	import Icon from '$lib/Icon.svelte';
	import { position_context } from '$lib/resources/positions.svelte';
	import { stop_time_context } from '$lib/resources/stop_times.svelte';
	import { current_time } from '$lib/url_params.svelte';

	import { ArrowBigRight } from '@lucide/svelte';
	import type { Trip } from '@trainstatus/client';

	interface Props {
		data: Trip;
	}
	let { data }: Props = $props();

	// TODO: maybe move this to List.svelte
	// onMount(() => {
	// 	if (is_bus_route(data.route, data)) {
	// 		monitored_bus_routes.add(data.route_id);
	// 	}
	// });

	const source_stop_times = $derived(stop_time_context.getSource(data.data.source));

	const stop_times = $derived(
		(source_stop_times?.value?.by_trip_id.get(data.id) ?? []).filter(
			(st) => st.arrival.getTime() > current_time.ms
		)
	);

	const position = $derived(
		position_context.getSource(data.data.source)?.value?.get(data.vehicle_id)
	);

	const route = $derived(page.data.routes_by_id?.[data.data.source]?.[data.route_id]);

	// TODO: create a single reusable last_stop function for trip button and modal
	const last_stop = $derived.by(() => {
		if (!stop_times.length) return 'Unknown';

		switch (data.data.source) {
			case 'mta_bus':
				// TODO: get actual last stop instead of headsign. I think the issue was that bus trips don't always include all stop times, so the last stop time might not be the actual last stop.
				// get stop in the direction of trip and get headsign
				const stop = page.data.stops_by_id?.[data.data.source]?.[stop_times[0].stop_id];
				const routeStop = stop?.routes.find((r) => r.route_id === data.route_id);
				// this shouldn't be necessary since we should only be looking at bus routes, but just in case (and also to satisfy type checker)
				return routeStop?.data.source === 'mta_bus' ? routeStop.data.headsign : 'Unknown';
			case 'mta_subway':
				const last_st = stop_times[stop_times.length - 1];
				return page.data.stops_by_id?.[data.data.source]?.[last_st.stop_id]?.name ?? 'Unknown';
			default:
				return 'Unknown';
		}
	});

	const current_stop = $derived.by(() => {
		// TODO: check that position was updated_at recently (like in the past 5 minutes) to prevent showing wrong stop due to stale position data.
		const target_stop_id = position?.stop_id || stop_times[0]?.stop_id;
		if (!stop_times.length) return 'Unknown';
		return page.data.stops_by_id?.[data.data.source]?.[target_stop_id]?.name ?? 'Unknown';
	});

	// const { current_status, current_stop } = $derived.by(() => {
	// 	if (!stop_times.length) return { current_status: 'Unknown', current_stop: 'Unknown' };

	// 	// check if trip has been updated in past 3 minutes
	// 	// TODO: check if stop_id has already passed as well
	// 	if (
	// 		data.updated_at.getTime() > current_time.ms - 3 * 60 * 1000 &&
	// 		data.data.status !== 'none' &&
	// 		data.data.stop_id
	// 	) {
	// 		let status_text = '';
	// 		switch (data.data.status) {
	// 			case 'incoming':
	// 				status_text = 'Arriving at:';
	// 				break;
	// 			case 'at_stop':
	// 				status_text = 'At stop:';
	// 				break;
	// 			case 'in_transit_to':
	// 				status_text = 'En route to:';
	// 				break;
	// 			// TODO: this is not how layovers work i think
	// 			case 'layover':
	// 				status_text = 'Layover at:';
	// 				break;
	// 		}

	// 		return {
	// 			current_status: status_text,
	// 			current_stop: page.data.stops_by_id[data.data.source][data.data.stop_id].name
	// 		};
	// 	}
	// 	return {
	// 		current_status: 'Next stop:',
	// 		current_stop: page.data.stops[stop_times[0].stop_id].name
	// 	};
	// });
</script>

<div class="flex flex-col items-center gap-1 text-left">
	<div class="flex max-w-[95%] items-center gap-1 self-start">
		{#if route}
			<Icon width={36} height={36} {route} link={false} />
		{/if}

		<ArrowBigRight />

		{last_stop}
	</div>

	<div class="flex gap-1 self-start">
		<!-- TODO: map specific status values or just convert to title case -->
		{#if position?.data && 'status' in position.data}
			<span>{(position.data as any).status}</span>
		{/if}
		{current_stop}
	</div>
</div>
