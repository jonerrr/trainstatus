<script lang="ts">
	import { page } from '$app/stores';
	import {
		stop_times as rt_stop_times,
		monitored_routes,
		type StopTime
	} from '$lib/stop_times.svelte';
	import type { Route, Stop, TrainStopData } from '$lib/static';
	import {
		trips as rt_trips,
		TripDirection,
		type BusTripData,
		type TrainTripData,
		type Trip
	} from '$lib/trips.svelte';
	import { persisted_rune } from '$lib/util.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import Button from '$lib/Button.svelte';
	import BusCapacity from '$lib/BusCapacity.svelte';

	interface ModalProps {
		show_previous: boolean;
		stop: Stop<'bus' | 'train'>;
	}

	// TODO: this probably doesn't needed to be binded
	let { stop = $bindable(), show_previous = $bindable() }: ModalProps = $props();

	interface StopTimeWithTrip extends StopTime<number> {
		trip: Trip<TrainTripData | BusTripData>;
		last_stop: string;
	}

	let stop_times: StopTimeWithTrip[] = $derived.by(() => {
		if (stop.type === 'train') {
			const stop_times = rt_stop_times.stop_times
				.filter((st) => st.stop_id === stop.id)
				.map((st) => {
					const trip = rt_trips.trips.get(st.trip_id);
					const last_st = rt_stop_times.stop_times.filter((st) => st.trip_id === trip?.id).pop();
					// if (!trip) {
					// 	$inspect(st);
					// }
					return {
						...st,
						eta: (st.arrival.getTime() - new Date().getTime()) / 1000 / 60,
						last_stop: trip ? $page.data.stop_map.get(last_st!.stop_id)?.name : 'unknown',
						trip
					};
				})
				// TODO: fix so we don't need to filter (maybe store trips in map)
				.filter((st) => st.trip !== undefined && st.eta >= 0) as StopTimeWithTrip[];
			// TODO: also get trips where current stop_id is this stop
			return stop_times;
		} else {
			const stop_times = rt_stop_times.stop_times
				.filter((st) => st.stop_id === stop.id)
				.map((st) => {
					const trip = rt_trips.trips.get(st.trip_id);
					// for bus, we get last stop from route headsing bc stop times doesn't include all of the stops

					return {
						...st,
						eta: (st.arrival.getTime() - new Date().getTime()) / 1000 / 60,
						last_stop: stop.routes.find((r) => r.id === trip?.route_id)?.headsign,
						trip
					};
				})
				// TODO: fix so we don't need to filter (maybe store trips in map)
				.filter((st) => st.trip !== undefined && st.eta >= 0) as StopTimeWithTrip[];
			// TODO: also get trips where current stop_id is this stop
			return stop_times;
		}
	});

	// $inspect(stop_times);
	let selected_direction = persisted_rune('direction', TripDirection.North);
	// if its a train, we only want to show stop times for the selected direction
	let selected_stop_times = $derived(
		stop.type === 'train'
			? stop_times.filter((st) => st.trip.direction === selected_direction.value)
			: stop_times
	);
</script>

<div class="flex gap-1">
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
</div>

<ModalList>
	{#each selected_stop_times as st}
		<Button state={{ modal: 'trip', data: st.trip }}>
			<div class="flex gap-2 items-center">
				<div class="flex flex-col">
					{#if stop.type === 'bus' && st.trip.data.passengers && st.trip.data.capacity}
						<BusCapacity passengers={st.trip.data.passengers} capacity={st.trip.data.capacity} />
					{/if}
					<Icon
						width="1.2rem"
						height="1.2rem"
						express={st.trip.data.express}
						link={false}
						route={$page.data.routes.get(st.trip.route_id) as Route}
					/>
				</div>
				<div class="flex flex-col" class:italic={stop.type === 'train' && !st.trip.data.assigned}>
					{#if stop.type === 'bus' && Math.abs(st.trip.data.deviation) > 120}
						<div
							class={`text-xs ${st.trip.data.deviation > 0 ? 'text-red-400' : 'text-green-400'}`}
						>
							{(st.trip.data.deviation / 60).toFixed(0)}m
						</div>
					{/if}

					{st.eta.toFixed(0)}m

					{#if st.trip.status === 'layover'}
						<div class="text-neutral-400 text-xs">+Layover</div>
					{/if}
				</div>

				<!-- {#if stops_away > 0}
								<div class="text-indigo-200 text-xs">
									{stops_away} stop{stops_away > 1 ? 's' : ''} away
								</div>
							{/if} -->
			</div>

			<div class="text-right pl-4">
				{st.last_stop}
			</div>
		</Button>
	{/each}
</ModalList>

{#if stop.type === 'train'}
	{@const stop_data = stop.data as TrainStopData}

	{#snippet direction_tab(direction: TripDirection, name: string)}
		<button
			class:bg-neutral-900={selected_direction.value === direction}
			onclick={() => {
				selected_direction.value = direction;
			}}
		>
			{name}
		</button>
	{/snippet}

	<div
		class="grid grid-cols-2 text-neutral-100 bg-neutral-800 border-neutral-700"
		aria-label="Trip information"
	>
		{@render direction_tab(TripDirection.North, stop_data.north_headsign)}
		{@render direction_tab(TripDirection.South, stop_data.south_headsign)}
	</div>
{/if}
