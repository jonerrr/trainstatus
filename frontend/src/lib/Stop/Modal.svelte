<script lang="ts">
	import { page } from '$app/stores';
	import {
		stop_times as rt_stop_times,
		monitored_routes,
		type StopTime
	} from '$lib/stop_times.svelte';
	import type { Route, Stop } from '$lib/static';
	import {
		trips as rt_trips,
		TripDirection,
		type BusTripData,
		type TrainTripData,
		type Trip
	} from '$lib/trips.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import Button from '$lib/Button.svelte';

	interface ModalProps {
		show_previous: boolean;
		stop: Stop<'bus' | 'train'>;
	}

	const { stop, show_previous = $bindable() }: ModalProps = $props();

	interface StopTimeWithTrip extends StopTime<number> {
		trip: Trip<TrainTripData | BusTripData>;
	}

	let stop_times: StopTimeWithTrip[] = $derived.by(() => {
		// get arrival for stop and add eta
		const stop_times = rt_stop_times.stop_times
			.filter((st) => st.stop_id === stop.id)
			.map((st) => {
				// const trip = rt_trips.trips.find((t) => t.id === st.trip_id);
				const trip = rt_trips.trips.get(st.trip_id);

				// if (!trip) {
				// 	$inspect(st);
				// }
				return {
					...st,
					eta: (st.arrival.getTime() - new Date().getTime()) / 1000 / 60,
					trip
				};
			})
			// TODO: fix so we don't need to filter (maybe store trips in map)
			.filter((st) => st.direction !== undefined && st.eta >= 0) as StopTimeWithTrip[];
		// TODO: also get trips where current stop_id is this stop
		return stop_times;
	});
</script>

<div class="flex gap-1 p-1">
	<!-- {#if large} -->
	{#each stop.routes as route}
		<Icon
			width="1.5rem"
			height="1.5rem"
			express={false}
			link={true}
			route={$page.data.routes.get(route.id) as Route}
		/>
	{/each}

	<div class="font-medium text-lg">
		{stop.name}
	</div>

	<ModalList>
		{#each stop_times as st}
			<Button state={{ dialog_type: 'trip', data: st.trip }}>
				<div class="flex items-center"></div>
			</Button>
		{/each}
	</ModalList>
</div>
