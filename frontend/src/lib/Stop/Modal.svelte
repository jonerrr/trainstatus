<script lang="ts">
	import { page } from '$app/stores';
	import {
		stop_times as rt_stop_times,
		monitored_routes,
		type StopTime
	} from '$lib/stop_times.svelte';
	import {
		type Route,
		type Stop,
		type TrainStopData,
		is_train as is_train_stop,
		is_bus as is_bus_stop
	} from '$lib/static';
	import {
		trips as rt_trips,
		TripDirection,
		is_bus,
		is_train,
		type BusTripData,
		type TrainTripData,
		type Trip
	} from '$lib/trips.svelte';
	import { persisted_rune } from '$lib/util.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import Button from '$lib/Button.svelte';
	import BusCapacity from '$lib/BusCapacity.svelte';
	import { pushState } from '$app/navigation';
	import { onMount } from 'svelte';

	interface ModalProps {
		show_previous: boolean;
		time_format: 'time' | 'countdown';
		stop: Stop<'bus' | 'train'>;
	}

	// TODO: figure out why some stops randomly have the wrong trips showing (for example, a 5 train showing for 7 train grand central stop)

	let { stop, show_previous, time_format }: ModalProps = $props();

	onMount(() => {
		if (is_bus_stop(stop)) {
			const current_monitored_routes = monitored_routes.get('modal') || [];

			// const routes = stop.routes.map((r) => r.id);
			current_monitored_routes.push(...stop.routes.map((r) => r.id));
			// keep a max of 20 monitored routes for modal
			monitored_routes.set('modal', current_monitored_routes.slice(-20));
			console.log('modal monitoring route');
		}
	});

	interface StopTimeWithTrip extends StopTime<number> {
		trip: Trip<TrainTripData | BusTripData>;
		last_stop: string;
	}

	let stop_times: StopTimeWithTrip[] = $derived.by(() => {
		if (is_train_stop(stop)) {
			const stop_times = rt_stop_times.stop_times
				.filter((st) => st.stop_id === stop.id)
				.map((st) => {
					const trip = rt_trips.trips.get(st.trip_id);
					const last_st = rt_stop_times.stop_times.filter((st) => st.trip_id === trip?.id).pop();

					return {
						...st,
						eta: (st.arrival.getTime() - new Date().getTime()) / 1000 / 60,
						last_stop: trip ? $page.data.stops[last_st!.stop_id].name : 'unknown',
						trip
					};
				})
				// TODO: fix so we don't need to filter (maybe store trips in map)
				.filter((st) => st.trip !== undefined && st.eta >= 0) as StopTimeWithTrip[];
			// TODO: also get trips where current stop_id is this stop
			return stop_times;
		} else if (is_bus_stop(stop)) {
			const stop_times = rt_stop_times.stop_times
				.filter((st) => st.stop_id === stop.id)
				.map((st) => {
					const trip = rt_trips.trips.get(st.trip_id);
					// for bus, we get last stop from route headsign bc stop times doesn't include all of the stops

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
		} else {
			console.error('Invalid stop type', stop);
			return [];
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

<div class="flex gap-1 items-center p-1">
	<div class="flex gap-1" class:flex-col={stop.type === 'bus'}>
		{#each stop.routes as route}
			<Icon
				width={24}
				height={24}
				express={false}
				link={true}
				route={$page.data.routes[route.id] as Route}
			/>
		{/each}
	</div>

	<div class="font-medium text-lg">
		{stop.name}
	</div>
</div>

{#if is_train_stop(stop) && stop.data.transfers.length}
	<div class="flex gap-1 items-center pb-1 pl-1">
		<div>Transfers:</div>
		{#each stop.data.transfers as transfer}
			{@const transfer_stop = $page.data.stops[transfer] as Stop<'train'>}
			<button
				class="flex gap-1 rounded border border-neutral-400 shadow-2xl active:border-neutral-700 hover:border-neutral-700"
				onclick={() =>
					pushState('', { modal: 'stop', data: JSON.parse(JSON.stringify(transfer_stop)) })}
			>
				{#each transfer_stop.routes as route}
					<Icon
						width={24}
						height={24}
						express={false}
						link={false}
						route={$page.data.routes[route.id]}
					/>
				{/each}
			</button>
		{/each}
	</div>
{/if}

{#if !selected_stop_times.length}
	<div class="text-neutral-400 text-center font-semibold">No upcoming trips</div>
{/if}
<ModalList>
	{#each selected_stop_times as st}
		<Button state={{ modal: 'trip', data: st.trip }}>
			<div class="flex gap-2 items-center">
				<div class="flex flex-col items-center">
					{#if is_bus(stop, st.trip) && st.trip.data.passengers && st.trip.data.capacity}
						<BusCapacity passengers={st.trip.data.passengers} capacity={st.trip.data.capacity} />
					{/if}
					<!-- {st.trip.vehicle_id} -->
					<Icon
						width={20}
						height={20}
						express={is_train(stop, st.trip) && st.trip.data.express}
						link={false}
						route={$page.data.routes[st.trip.route_id] as Route}
					/>
				</div>
				<div class="flex flex-col" class:italic={is_train(stop, st.trip) && !st.trip.data.assigned}>
					<!-- if bus trip and theres a deviation more than 2 min -->
					{#if is_bus(stop, st.trip) && st.trip.data.deviation && Math.abs(st.trip.data.deviation) > 120}
						<div
							class={`text-xs ${st.trip.data.deviation > 0 ? 'text-red-400' : 'text-green-400'}`}
						>
							{(st.trip.data.deviation / 60).toFixed(0)}m
						</div>
					{/if}

					<div class="text-left">
						{#if time_format === 'time'}
							{st.arrival.toLocaleTimeString().replace(/AM|PM/, '')}
						{:else}
							{st.eta.toFixed(0)}m
						{/if}
					</div>

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
			class="p-2"
			class:bg-neutral-900={selected_direction.value === direction}
			class:text-neutral-400={selected_direction.value !== direction}
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
