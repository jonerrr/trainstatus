<script lang="ts">
	import { ArrowBigRight } from 'lucide-svelte';
	import { page } from '$app/stores';
	import {
		is_bus_route,
		is_train_route,
		type BusTripData,
		type TrainTripData,
		type Trip
	} from '$lib/trips.svelte';
	import type { PersistedRune } from '$lib/util.svelte';
	import { monitored_routes, stop_times as rt_stop_times } from '$lib/stop_times.svelte';
	import Button from '$lib/Button.svelte';
	import type { Route, Stop } from '$lib/static';
	import Icon from '$lib/Icon.svelte';
	import { onMount } from 'svelte';

	interface ButtonProps {
		trip: Trip<TrainTripData | BusTripData, Route>;
		pin_rune: PersistedRune<string[]>;
	}
	let { trip, pin_rune }: ButtonProps = $props();

	onMount(() => {
		if (is_bus_route(trip.route, trip)) {
			const current_monitored_routes = monitored_routes.get('pinned_trips') || [];
			current_monitored_routes.push(trip.route_id);
			monitored_routes.set('pinned_trips', current_monitored_routes);
		}
	});

	const stop_times = $derived(rt_stop_times.stop_times.filter((st) => st.trip_id === trip.id)!);

	const last_stop = $derived.by(() => {
		if (!stop_times.length) return 'Unknown';

		if (is_bus_route(trip.route, trip)) {
			// TODO: get actual last stop instead of headsign
			// get stop in the direction of trip and get headsign
			const stop = $page.data.stops[stop_times[0].stop_id] as Stop<'bus'>;
			return stop.routes.find((r) => r.id === trip.route_id)!.headsign;
		} else {
			const last_st = stop_times[stop_times.length - 1];
			return $page.data.stops[last_st.stop_id].name;
		}
	});

	const { current_status, current_stop } = $derived.by(() => {
		if (!stop_times.length) return { current_status: 'Unknown', current_stop: 'Unknown' };

		// check if trip has been updated in past 3 minutes
		if (
			trip.updated_at.getTime() > new Date().getTime() - 3 * 60 * 1000 &&
			trip.status !== 'none' &&
			trip.stop_id
		) {
			return {
				current_status: trip.status.toString(),
				current_stop: $page.data.stops[trip.stop_id].name
			};
		}
		return {
			current_status: 'Next stop:',
			current_stop: $page.data.stops[stop_times[0].stop_id].name
		};
	});
</script>

<Button
	state={{
		modal: 'trip',
		data: trip
	}}
	{pin_rune}
>
	<div class="flex flex-col gap-1 items-center text-left">
		<div class="flex gap-1 items-center self-start">
			<Icon
				width="2rem"
				height="2rem"
				route={trip.route}
				link={false}
				express={is_train_route(trip.route, trip) && trip.data.express}
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
</Button>
