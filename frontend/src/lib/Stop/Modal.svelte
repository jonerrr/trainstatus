<script lang="ts">
	import { page } from '$app/stores';
	import { pushState } from '$app/navigation';
	import {
		stop_times as rt_stop_times,
		monitored_bus_routes,
		type StopTime
	} from '$lib/stop_times.svelte';
	import {
		type Route,
		type Stop,
		type TrainStopData,
		is_train as is_train_stop,
		is_bus as is_bus_stop,
		main_stop_routes
	} from '$lib/static';
	import {
		trips as rt_trips,
		TripDirection,
		is_bus,
		is_train,
		type Trip,
		type TripData
	} from '$lib/trips.svelte';
	import { persisted_rune } from '$lib/util.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import Button from '$lib/Button.svelte';
	import BusCapacity from '$lib/BusCapacity.svelte';

	interface ModalProps {
		show_previous: boolean;
		time_format: 'time' | 'countdown';
		stop: Stop<'bus' | 'train'>;
	}

	// TODO: figure out why some stops randomly have the wrong trips showing (for example, a 5 train showing for 7 train grand central stop)

	let { stop, show_previous, time_format }: ModalProps = $props();

	interface StopTimeWithTrip extends StopTime<number> {
		trip: Trip<TripData>;
	}

	let stop_times: StopTimeWithTrip[] = $derived(
		rt_stop_times.stop_times
			.filter((st) => st.stop_id === stop.id && rt_trips.trips.has(st.trip_id))
			.map((st) => {
				const trip = rt_trips.trips.get(st.trip_id)!;

				return {
					...st,
					eta: (st.arrival.getTime() - new Date().getTime()) / 1000 / 60,
					trip
				};
			})
			.filter((st) => st.eta >= 0)
	);

	// $inspect(stop_times);
	let selected_direction = persisted_rune('direction', TripDirection.North);
	// if its a train, we only want to show stop times for the selected direction
	let selected_stop_times = $derived(
		stop.route_type === 'train'
			? stop_times.filter((st) => st.trip.direction === selected_direction.value)
			: stop_times
	);

	// only show routes that stop at this stop
	let route_stops = main_stop_routes(stop);
</script>

<div class="flex gap-1 items-center p-1">
	<div class="flex gap-1" class:flex-col={stop.route_type === 'bus'}>
		{#each route_stops as route}
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

<!-- TODO: also show transfers for bus if multiple routes at bus stop -->
{#if is_train_stop(stop) && stop.data.transfers.length}
	<div class="flex gap-1 items-center pb-1 pl-1">
		<div>Transfers:</div>
		{#each stop.data.transfers as transfer}
			{@const transfer_stop = $page.data.stops[transfer] as Stop<'train'>}
			<button
				class="flex rounded bg-neutral-800 shadow-2xl gap-1 p-1 items-center hover:bg-neutral-700 active:bg-neutral-900"
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
			<div class="flex items-center gap-1">
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

				<div class="text-left">
					{#if is_train_stop(stop)}
						{@const last_stop_time = rt_stop_times.stop_times
							.filter((trip_st) => trip_st.trip_id === st.trip.id)
							.pop()!}
						{$page.data.stops[last_stop_time.stop_id].name}
					{:else if is_bus_stop(stop)}
						{stop.routes.find((r) => r.id === st.trip.route_id)?.headsign}
					{/if}
				</div>

				<!-- {#if stops_away > 0}
								<div class="text-indigo-200 text-xs">
									{stops_away} stop{stops_away > 1 ? 's' : ''} away
								</div>
							{/if} -->
			</div>

			<div
				class="flex flex-col items-center"
				class:italic={is_train(stop, st.trip) && !st.trip.data.assigned}
			>
				<!-- if bus trip and theres a deviation more than 2 min -->
				{#if is_bus(stop, st.trip) && st.trip.data.deviation && Math.abs(st.trip.data.deviation) > 120}
					<div class={`text-xs ${st.trip.data.deviation > 0 ? 'text-red-400' : 'text-green-400'}`}>
						{(st.trip.data.deviation / 60).toFixed(0)}m
					</div>
				{/if}

				<div class="text-right">
					{#if time_format === 'time'}
						{st.arrival.toLocaleTimeString().replace(/AM|PM/, '')}
					{:else}
						{st.eta.toFixed(0)}m
					{/if}
				</div>

				{#if st.trip.data.status === 'layover'}
					<div class="text-neutral-400 text-xs">+Layover</div>
				{/if}
			</div>
		</Button>
	{/each}
</ModalList>

{#if stop.route_type === 'train'}
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
