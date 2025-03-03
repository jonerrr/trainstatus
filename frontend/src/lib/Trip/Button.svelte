<script lang="ts">
	import { ArrowBigRight } from 'lucide-svelte';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { is_bus_route, is_train_route, type Trip, type TripData } from '$lib/trips.svelte';
	import { stop_times as rt_stop_times, monitored_bus_routes } from '$lib/stop_times.svelte';
	import type { Route, Stop } from '$lib/static';
	import { current_time } from '$lib/util.svelte';
	import Icon from '$lib/Icon.svelte';

	interface Props {
		data: Trip<TripData, Route>;
	}
	let { data }: Props = $props();

	// TODO: maybe move this to List.svelte
	onMount(() => {
		if (is_bus_route(data.route, data)) {
			monitored_bus_routes.add(data.route_id);
		}
	});

	const stop_times = $derived(
		(rt_stop_times.by_trip_id[data.id] || []).filter((st) => st.arrival.getTime() > current_time.ms)
	);

	const last_stop = $derived.by(() => {
		if (!stop_times.length) return 'Unknown';

		if (is_bus_route(data.route, data)) {
			// TODO: get actual last stop instead of headsign
			// get stop in the direction of trip and get headsign
			const stop = page.data.stops[stop_times[0].stop_id] as Stop<'bus'>;
			return stop.routes.find((r) => r.id === data.route_id)!.headsign;
		} else {
			const last_st = stop_times[stop_times.length - 1];
			return page.data.stops[last_st.stop_id].name;
		}
	});

	const { current_status, current_stop } = $derived.by(() => {
		if (!stop_times.length) return { current_status: 'Unknown', current_stop: 'Unknown' };

		// check if trip has been updated in past 3 minutes
		// TODO: check if stop_id has already passed as well
		if (
			data.updated_at.getTime() > current_time.ms - 3 * 60 * 1000 &&
			data.data.status !== 'none' &&
			data.data.stop_id
		) {
			let status_text = '';
			switch (data.data.status) {
				case 'incoming':
					status_text = 'Arriving at:';
					break;
				case 'at_stop':
					status_text = 'At stop:';
					break;
				case 'in_transit_to':
					status_text = 'En route to:';
					break;
				// TODO: this is not how layovers work i think
				case 'layover':
					status_text = 'Layover at:';
					break;
			}

			return {
				current_status: status_text,
				current_stop: page.data.stops[data.data.stop_id].name
			};
		}
		return {
			current_status: 'Next stop:',
			current_stop: page.data.stops[stop_times[0].stop_id].name
		};
	});
</script>

<div class="flex flex-col gap-1 items-center text-left">
	<div class="flex gap-1 items-center self-start max-w-[95%]">
		<Icon
			width={36}
			height={36}
			route={data.route}
			link={false}
			express={is_train_route(data.route, data) && data.data.express}
		/>

		<ArrowBigRight />

		{last_stop}
	</div>

	<div class="flex self-start gap-1">
		<span>{current_status}</span>
		{current_stop}
	</div>
</div>
