<script lang="ts">
	import { page } from '$app/state';
	import { pushState } from '$app/navigation';
	import { stop_times as rt_stop_times, type StopTime } from '$lib/stop_times.svelte';
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
	import { alerts } from '$lib/alerts.svelte';
	import { persisted_rune, current_time } from '$lib/util.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import Button from '$lib/Button.svelte';
	import BusCapacity from '$lib/BusCapacity.svelte';
	import BusArrow from './BusArrow.svelte';

	interface Props {
		show_previous: boolean;
		time_format: 'time' | 'countdown';
		stop: Stop<'bus' | 'train'>;
	}

	// TODO: figure out why some stops randomly have the wrong trips showing (for example, a 5 train showing for 7 train grand central stop)

	let { stop, show_previous, time_format }: Props = $props();

	interface StopTimeWithTrip extends StopTime<number> {
		trip: Trip<TripData>;
	}

	const { stop_times, active_routes } = $derived.by(() => {
		const now = current_time.ms;
		const stop_times: StopTimeWithTrip[] = [];
		const active_routes: Set<string> = new Set();

		for (const st of rt_stop_times.stop_times) {
			if (st.stop_id === stop.id) {
				const trip = rt_trips.trips.get(st.trip_id);
				if (trip) {
					const eta = (st.arrival.getTime() - now) / 60000;
					if (eta >= 0) {
						// TODO: add a way to disable eta if statement
						active_routes.add(trip.route_id);

						stop_times.push({ ...st, eta, trip });
					}
				}
			}
		}

		return { stop_times, active_routes };
	});
	// $inspect(active_routes);

	// $inspect(stop_times);
	let selected_direction = persisted_rune('direction', TripDirection.North);
	// if its a train, we only want to show stop times for the selected direction
	let selected_stop_times = $derived(
		stop.route_type === 'train'
			? stop_times.filter((st) => st.trip.direction === selected_direction.value)
			: stop_times
	);

	// only show routes that stop at this stop and sort by id length
	const main_rs = $derived(main_stop_routes(stop));
	const route_stops = $derived.by(() => {
		if (main_rs.length < 6) {
			return main_rs;
		} else {
			return main_rs.sort((a, b) => {
				const a_active = active_routes.has(a.id);
				const b_active = active_routes.has(b.id);

				if (a_active && !b_active) {
					return -1;
				} else if (!a_active && b_active) {
					return 1;
				} else {
					return a.id.length - b.id.length;
				}
			});

			// return main_rs.filter((route) => active_routes.has(route.id));
		}
	});

	// show indicator if there is an alert at the stop TODO: maybe make map of stop_id to alert
	const show_alert_icon = $derived.by(() => {
		return alerts.alerts.some((alert) => alert.entities.some((e) => e.stop_id === stop.id));
	});

	// $inspect(show_alert_icon);
</script>

<div class="flex gap-1 items-center p-1">
	<!-- grid gap-y-1 [grid-template-columns:repeat(auto-fit,minmax(4rem,1fr))] max-w-xs -->
	<!-- grid gap-1 grid-cols-5 grid-rows-3 grid-flow-col -->

	<div class="flex flex-wrap gap-1 max-h-36 max-w-40 md:max-w-xs items-center">
		{#if route_stops.length > 6}
			{#each route_stops.slice(0, 5) as route}
				<Icon
					width={36}
					height={36}
					express={false}
					link={true}
					route={page.data.routes[route.id] as Route}
				/>
			{/each}
			<!-- {#if route_stops.length > 5} -->
			<div class="font-semibold rounded bg-neutral-700 p-1">+{main_rs.length - 5}</div>
			<!-- {/if} -->
		{:else}
			{#each route_stops as route}
				<Icon
					width={36}
					height={36}
					express={false}
					link={true}
					route={page.data.routes[route.id] as Route}
				/>
			{/each}
		{/if}

		<!-- </div> -->
	</div>
	<div class="relative text-xl font-semibold flex gap-1 items-center">
		{#if is_bus_stop(stop)}
			<BusArrow direction={stop.data.direction} />
		{/if}
		{stop.name}

		{#if show_alert_icon}
			<!-- TODO: make less ugly -->
			<div class="absolute -top-1 -right-1 size-3 rounded-full bg-orange-400"></div>
		{/if}
	</div>
</div>

<!-- TODO: also show transfers for bus if multiple routes at bus stop -->
{#if is_train_stop(stop) && stop.data.transfers.length}
	<div class="flex gap-1 items-center pb-1 pl-1">
		<div>Transfers:</div>
		{#each stop.data.transfers as transfer}
			{@const transfer_stop = page.data.stops[transfer] as Stop<'train'>}
			<button
				class="flex rounded bg-neutral-800 shadow-2xl gap-1 p-1 items-center transition-colors duration-200 hover:bg-neutral-700 active:bg-neutral-900"
				onclick={() =>
					pushState('', { modal: 'stop', data: JSON.parse(JSON.stringify(transfer_stop)) })}
			>
				{#each main_stop_routes(transfer_stop) as route}
					<Icon
						width={24}
						height={24}
						express={false}
						link={false}
						route={page.data.routes[route.id]}
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
						route={page.data.routes[st.trip.route_id] as Route}
					/>
				</div>

				<div class="text-left">
					{#if is_train_stop(stop)}
						{@const last_stop_time = rt_stop_times.stop_times
							.filter((trip_st) => trip_st.trip_id === st.trip.id)
							.pop()!}
						{page.data.stops[last_stop_time.stop_id].name}
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
					<div class="text-xs {st.trip.data.deviation > 0 ? 'text-red-400' : 'text-green-400'}">
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
