<script lang="ts">
	import { ArrowBigRight } from 'lucide-svelte';
	import { page } from '$app/stores';
	import { onMount } from 'svelte';
	import {
		is_bus_route,
		is_train_route,
		type BusTripData,
		type TrainTripData,
		type Trip
	} from '$lib/trips.svelte';
	import { stop_times as rt_stop_times, monitored_bus_routes } from '$lib/stop_times.svelte';
	import type { Route, Stop } from '$lib/static';
	import Icon from '$lib/Icon.svelte';

	interface Props {
		data: Trip<TrainTripData | BusTripData, Route>;
	}
	let { data }: Props = $props();

	// TODO: maybe move this to List.svelte
	onMount(() => {
		if (is_bus_route(data.route, data)) {
			monitored_bus_routes.add(data.route_id);
		}
	});

	const stop_times = $derived(
		rt_stop_times.stop_times.filter((st) => st.trip_id === data.id && st.arrival > new Date())!
	);

	const last_stop = $derived.by(() => {
		if (!stop_times.length) return 'Unknown';

		if (is_bus_route(data.route, data)) {
			// TODO: get actual last stop instead of headsign
			// get stop in the direction of trip and get headsign
			const stop = $page.data.stops[stop_times[0].stop_id] as Stop<'bus'>;
			return stop.routes.find((r) => r.id === data.route_id)!.headsign;
		} else {
			const last_st = stop_times[stop_times.length - 1];
			return $page.data.stops[last_st.stop_id].name;
		}
	});

	const { current_status, current_stop } = $derived.by(() => {
		if (!stop_times.length) return { current_status: 'Unknown', current_stop: 'Unknown' };

		// check if trip has been updated in past 3 minutes
		if (
			data.updated_at.getTime() > new Date().getTime() - 3 * 60 * 1000 &&
			data.data.status !== 'none' &&
			data.data.stop_id
		) {
			// TODO: better stop status formatting
			return {
				current_status: data.data.status.toString(),
				current_stop: $page.data.stops[data.data.stop_id].name
			};
		}
		return {
			current_status: 'Next stop:',
			current_stop: $page.data.stops[stop_times[0].stop_id].name
		};
	});
</script>

<div class="flex flex-col gap-1 items-center text-left">
	<div class="flex gap-1 items-center self-start">
		<Icon
			width={32}
			height={32}
			route={data.route}
			link={false}
			express={is_train_route(data.route, data) && data.data.express}
		/>

		<ArrowBigRight />

		{last_stop}
	</div>

	<div class="flex self-start">
		<div>
			<span class="font-medium">{current_status}</span>
			{current_stop}
		</div>
	</div>
</div>
