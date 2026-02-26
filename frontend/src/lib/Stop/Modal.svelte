<script lang="ts">
	import { page } from '$app/state';

	import Button from '$lib/Button.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import BusArrow from '$lib/Stop/BusArrow.svelte';
	import VehicleCapacity from '$lib/VehicleCapacity.svelte';
	import { alert_context } from '$lib/resources/alerts.svelte';
	import type { SourceMap, TypedVehiclePosition } from '$lib/resources/index.svelte';
	import { position_context } from '$lib/resources/positions.svelte';
	import { stop_time_context } from '$lib/resources/stop_times.svelte';
	import { trip_context } from '$lib/resources/trips.svelte';
	import { main_stop_routes } from '$lib/static';
	import { LocalStorage } from '$lib/storage.svelte';
	import { open_modal } from '$lib/url_params.svelte';
	import { current_time } from '$lib/util.svelte';

	import { CircleAlert } from '@lucide/svelte';
	import type { Stop, StopTime, Trip } from '@trainstatus/client';

	interface Props {
		show_previous: boolean;
		time_format: 'time' | 'countdown';
		stop: Stop;
	}

	interface StopTimeWithTrip extends StopTime {
		eta: number;
		trip: Trip;
	}

	let { stop, show_previous, time_format }: Props = $props();

	const trips = $derived(trip_context.getSource(stop.data.source));
	const stop_times = $derived(stop_time_context.getSource(stop.data.source));
	const positions = $derived(position_context.getSource(stop.data.source));
	const alerts = $derived(alert_context.getSource(stop.data.source));
	const routes = $derived(page.data.routes_by_id[stop.data.source]);

	const { stop_times_with_trip, active_routes } = $derived.by(() => {
		const now = current_time.ms;
		const active_routes = new Set<string>();

		const stop_times_with_trip = (stop_times.value?.by_stop_id.get(stop.id) ?? []).flatMap((st) => {
			if (!show_previous && st.arrival.getTime() <= now) return [];
			const trip = trips.value?.get(st.trip_id);
			if (!trip) return [];
			const eta = (st.arrival.getTime() - now) / 60000;
			active_routes.add(trip.route_id);
			return [{ ...st, eta, trip }] as StopTimeWithTrip[];
		});

		return { stop_times_with_trip, active_routes };
	});
	// $inspect(active_routes);

	// $inspect(stop_times);
	// TODO: generate defaults instead of hardcoding
	let selected_direction = new LocalStorage<SourceMap<number>>('direction', {
		mta_subway: 1,
		mta_bus: 0
	});
	// if its a train, we only want to show stop times for the selected direction
	// TODO: handle bus stops with opposite_stop_id once implemented
	const selected_stop_times = $derived(
		stop.data.source === 'mta_subway'
			? stop_times_with_trip.filter(
					(st) => st.trip.direction === selected_direction.current[stop.data.source]
				)
			: stop_times_with_trip
	);

	// Only show routes that stop at this stop and sort by id length
	const main_rs = $derived(main_stop_routes(stop));
	const route_stops = $derived.by(() => {
		if (main_rs.length < 6) return main_rs;
		return [...main_rs].sort((a, b) => {
			const a_active = active_routes.has(a.route_id);
			const b_active = active_routes.has(b.route_id);
			if (a_active && !b_active) return -1;
			if (!a_active && b_active) return 1;
			return a.route_id.length - b.route_id.length;
		});
	});

	// Show indicator if there is an alert at this stop
	const show_alert_icon = $derived(
		alerts.value?.alerts.some((alert) => alert.entities.some((e) => e.stop_id === stop.id))
	);
</script>

<div class="flex items-center gap-1 p-1">
	<!-- grid gap-y-1 [grid-template-columns:repeat(auto-fit,minmax(4rem,1fr))] max-w-xs -->
	<!-- grid gap-1 grid-cols-5 grid-rows-3 grid-flow-col -->
	<!-- TODO: use grid with auto-fit and minmax(min(100px)) or whatever -->
	<div class="flex max-h-36 max-w-40 flex-wrap items-center gap-1 md:max-w-xs">
		{#if route_stops.length > 6}
			{#each route_stops.slice(0, 5) as route_stop}
				<Icon width={36} height={36} link={true} route={routes[route_stop.route_id]} show_alerts />
			{/each}
			<!-- {#if route_stops.length > 5} -->
			<div class="rounded-sm bg-neutral-700 p-1 font-semibold">+{main_rs.length - 5}</div>
			<!-- {/if} -->
		{:else}
			{#each route_stops as route_stop}
				<Icon width={36} height={36} link={true} route={routes[route_stop.route_id]} show_alerts />
			{/each}
		{/if}

		<!-- </div> -->
	</div>
	<div class="flex items-center gap-1 text-xl font-semibold">
		{#if stop.data.source === 'mta_bus'}
			<BusArrow direction={stop.data.direction} />
		{/if}
		{stop.name}

		{#if show_alert_icon}
			<!-- TODO: make less ugly -->
			<CircleAlert size="1.5rem" class="text-red-800" />
			<!-- <div class="absolute -top-1 -right-1 size-3 rounded-full bg-orange-400"></div> -->
		{/if}
	</div>
</div>

<!-- TODO: also show transfers for bus if multiple routes at bus stop -->
{#if stop.data.source === 'mta_subway' && stop.transfers.length}
	<div class="flex items-center gap-1 pb-1 pl-1">
		<div>Transfers:</div>
		{#each stop.transfers as transfer}
			{@const transfer_stop = page.data.stops_by_id[transfer.to_stop_source][transfer.to_stop_id]}
			{#if transfer_stop}
				<button
					class="flex items-center gap-1 rounded-sm bg-neutral-800 p-1 shadow-2xl transition-colors duration-200 hover:bg-neutral-700 active:bg-neutral-900"
					onclick={() =>
						// pushState('', { modal: { type: 'stop', ...$state.snapshot(transfer_stop) } })}
						open_modal({ type: 'stop', ...$state.snapshot(transfer_stop) })}
				>
					{#each main_stop_routes(transfer_stop) as route_stop}
						<Icon
							width={24}
							height={24}
							link={false}
							route={page.data.routes_by_id[transfer.to_stop_source][route_stop.route_id]}
						/>
					{/each}
				</button>
			{/if}
		{/each}
	</div>
{/if}

{#if !selected_stop_times.length}
	<div class="text-center font-semibold text-neutral-400">No upcoming trips</div>
{/if}

<ModalList>
	{#each selected_stop_times as st}
		{@const position = positions.value?.get(st.trip.vehicle_id)}
		<Button state={{ type: 'trip', ...st.trip }}>
			<div class="flex items-center gap-1">
				<div class="flex flex-col items-center">
					<!-- TODO: figure out how to fix type safety so we don't need to check the source again -->
					<!-- {#if position?.data.source === 'mta_bus' && position.data.capacity && position.data.passengers}
						<BusCapacity passengers={position.data.passengers} capacity={position.data.capacity} />
					{/if} -->
					{#if st.data.source === 'mta_bus'}
						<VehicleCapacity position={position as TypedVehiclePosition<'mta_bus'>} />
					{/if}
					<!-- {st.trip.vehicle_id} -->
					<Icon width={20} height={20} link={false} route={routes[st.trip.route_id]} />
				</div>

				<div class="text-left" class:text-neutral-400={st.arrival.getTime() < current_time.ms}>
					{#if stop.data.source === 'mta_subway'}
						{@const trip_stop_times = stop_times.value?.by_trip_id.get(st.trip_id)}
						{@const last_stop_time = trip_stop_times?.[trip_stop_times.length - 1]}
						{#if last_stop_time}
							{page.data.stops_by_id['mta_subway'][last_stop_time.stop_id]?.name}
						{/if}
					{:else if stop.data.source === 'mta_bus'}
						{@const route_stop = stop.routes.find(
							(r) => r.route_id === st.trip.route_id && r.data.source === 'mta_bus'
						)}
						<!-- TODO: fix type inference. it should always be mta_bus -->
						{route_stop?.data.headsign}
					{/if}
				</div>

				<!-- {#if stops_away > 0}
								<div class="text-indigo-200 text-xs">
									{stops_away} stop{stops_away > 1 ? 's' : ''} away
								</div>
							{/if} -->
			</div>

			<div
				class={[
					'flex flex-col items-end',
					{
						italic: position?.data.source === 'mta_subway' && !position.data.assigned,
						'text-neutral-400': st.arrival.getTime() < current_time.ms
					}
				]}
			>
				<!-- if bus trip and theres a deviation more than 2 min -->
				<!-- TODO: Fix -->
				<!-- {#if st.trip.data.source === 'mta_bus' && st.trip.data. && Math.abs(st.trip.data.deviation) > 120}
					<div class="text-xs {st.trip.data.deviation > 0 ? 'text-red-400' : 'text-green-400'}">
						{st.trip.data.deviation > 0 ? '+' : ''}{(st.trip.data.deviation / 60).toFixed(0)}m
					</div>
				{/if} -->

				<div>
					{#if time_format === 'time'}
						{st.arrival.toLocaleTimeString().replace(/AM|PM/, '')}
					{:else}
						{st.eta.toFixed(0)}m
					{/if}
				</div>

				<!-- TODO: fix -->
				<!-- {#if st.trip.data.status === 'layover'}
					<div class="text-xs text-neutral-400">+Layover</div>
				{/if} -->
			</div>
		</Button>
	{/each}
</ModalList>

{#if stop.data.source === 'mta_subway'}
	{@const stop_data = stop.data}

	{#snippet direction_tab(direction: number, name: string)}
		<button
			class={[
				'p-2',
				{
					'bg-neutral-900': selected_direction.current[stop.data.source] === direction,
					'text-neutral-400': selected_direction.current[stop.data.source] !== direction
				}
			]}
			onclick={() => {
				selected_direction.current[stop.data.source] = direction;
			}}
		>
			{name}
		</button>
	{/snippet}

	<div
		class="grid grid-cols-2 border-neutral-700 bg-neutral-800 text-neutral-100"
		aria-label="Trip information"
	>
		{@render direction_tab(1, stop_data.north_headsign)}
		{@render direction_tab(3, stop_data.south_headsign)}
	</div>
{/if}
