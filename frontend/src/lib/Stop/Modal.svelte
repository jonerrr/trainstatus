<script lang="ts">
	import { page } from '$app/state';

	import Button from '$lib/Button.svelte';
	import Icon from '$lib/Icon.svelte';
	import ModalList from '$lib/ModalList.svelte';
	import Skeleton from '$lib/Skeleton.svelte';
	import BusArrow from '$lib/Stop/BusArrow.svelte';
	import Transfers from '$lib/Stop/Transfers.svelte';
	import VehicleCapacity from '$lib/VehicleCapacity.svelte';
	import type { Stop, StopTime, Trip } from '$lib/client';
	import { alert_context } from '$lib/resources/alerts.svelte';
	import {
		type SourceMap,
		type TypedVehiclePosition,
		source_info
	} from '$lib/resources/index.svelte';
	import { position_context } from '$lib/resources/positions.svelte';
	import { stop_time_context } from '$lib/resources/stop_times.svelte';
	import { trip_context } from '$lib/resources/trips.svelte';
	import { LocalStorage } from '$lib/storage.svelte';
	import { current_time } from '$lib/url_params.svelte';
	import { main_route_stops } from '$lib/util.svelte';

	import { CircleAlert } from '@lucide/svelte';

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
	const stop_times_store = $derived(stop_time_context.getSource(stop.data.source));
	const positions = $derived(position_context.getSource(stop.data.source));
	const alerts = $derived(alert_context.getSource(stop.data.source));
	const routes = $derived(page.data.routes_by_id[stop.data.source]);

	$effect(() => {
		if (stop_times_store && source_info[stop.data.source]?.monitor_routes) {
			for (const r of stop.routes) {
				stop_times_store.add_route(r.route_id);
			}
		}
	});

	const current_stop_times = $derived(stop_times_store?.current.by_stop_id.get(stop.id) ?? []);

	const st_loading = $derived(
		!stop_times_store || (stop_times_store.status !== 'ready' && current_stop_times.length === 0)
	);

	const { stop_times_with_trip, active_routes } = $derived.by(() => {
		const now = current_time.ms;
		const active_routes = new Set<string>();

		const stop_times_with_trip = current_stop_times.flatMap((st) => {
			if (!show_previous && st.arrival.getTime() <= now) return [];
			const trip = trips?.current?.get(st.trip_id);
			if (!trip) return [];
			const eta = (st.arrival.getTime() - now) / 60000;
			active_routes.add(trip.route_id);
			return [{ ...st, eta, trip }] as StopTimeWithTrip[];
		});

		return { stop_times_with_trip, active_routes };
	});

	let selected_direction = new LocalStorage<SourceMap<number>>('selectedDirection', {
		mta_subway: 1,
		mta_bus: 0,
		njt_bus: 0
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

	// if there are more than 6 routes, show the main ones first and sort the rest by active vs inactive and then id length
	// test lots of routes with http://localhost:5173/stops?s=400354
	const route_stops = $derived.by(() => {
		const main_rs = main_route_stops(stop.routes);
		if (stop.routes.length < 6) return main_rs;
		return main_rs.sort((a, b) => {
			const a_active = active_routes.has(a.route_id);
			const b_active = active_routes.has(b.route_id);
			if (a_active && !b_active) return -1;
			if (!a_active && b_active) return 1;
			return a.route_id.length - b.route_id.length;
		});
	});

	const show_alert_icon = $derived(
		alerts?.current?.alerts.some((alert) => alert.entities.some((e) => e.stop_id === stop.id))
	);
</script>

<div class="flex items-center gap-1 p-1">
	<div class="flex max-h-36 max-w-40 flex-wrap items-center gap-1 md:max-w-xs">
		{#if route_stops.length > 6}
			{#each route_stops.slice(0, 5) as route_stop}
				{@const route = routes?.[route_stop.route_id]}
				{#if route}
					<Icon width={36} height={36} link={true} {route} show_alerts />
				{/if}
			{/each}
			<div class="rounded-sm bg-neutral-700 p-1 font-semibold">+{stop.routes.length - 5}</div>
		{:else}
			{#each route_stops as route_stop}
				{@const route = routes?.[route_stop.route_id]}
				{#if route}
					<Icon width={36} height={36} link={true} {route} show_alerts />
				{/if}
			{/each}
		{/if}
	</div>
	<div class="flex items-center gap-1 text-xl font-semibold">
		{#if (stop.data.source === 'mta_bus' || stop.data.source === 'njt_bus') && 'direction' in stop.data}
			<BusArrow direction={(stop.data as any).direction} />
		{/if}
		{stop.name}

		{#if show_alert_icon}
			<CircleAlert size="1.5rem" class="text-red-800" />
		{/if}
	</div>
</div>

{#if stop.transfers.length}
	<Transfers stop_source={stop.data.source} transfers={stop.transfers} />
{/if}

{#if st_loading}
	<Skeleton lines={5} class="p-2" />
{:else if !selected_stop_times.length}
	<div class="text-center font-semibold text-neutral-400">No upcoming trips</div>
{:else}
	<ModalList>
		{#each selected_stop_times as st}
			{@const position = positions?.current?.get(st.trip.vehicle_id)}
			{@const route = routes?.[st.trip.route_id]}
			<Button state={{ type: 'trip', ...st.trip }}>
				<div class="flex items-center gap-1">
					<div class="flex flex-col items-center">
						{#if st.data.source === 'mta_bus'}
							<VehicleCapacity position={position as TypedVehiclePosition<'mta_bus'>} />
						{:else if st.data.source === 'njt_bus'}
							<VehicleCapacity position={position as TypedVehiclePosition<'njt_bus'>} />
						{/if}
						{#if route}
							<Icon width={20} height={20} link={false} {route} />
						{/if}
					</div>

					<div class="text-left" class:text-neutral-400={st.arrival.getTime() < current_time.ms}>
						{#if stop.data.source === 'mta_subway'}
							{@const trip_stop_times = stop_times_store?.current.by_trip_id.get(st.trip_id)}
							{@const last_stop_time = trip_stop_times?.[trip_stop_times.length - 1]}
							{#if last_stop_time}
								{page.data.stops_by_id['mta_subway']?.[last_stop_time.stop_id]?.name}
							{/if}
						{:else if stop.data.source === 'mta_bus' || stop.data.source === 'njt_bus'}
							{@const route_stop = stop.routes.find((r) => r.route_id === st.trip.route_id)}
							{#if route_stop?.data && 'headsign' in route_stop.data}
								{(route_stop.data as any).headsign}
							{/if}
						{/if}
					</div>
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
					<div>
						{#if time_format === 'time'}
							{st.arrival.toLocaleTimeString().replace(/AM|PM/, '')}
						{:else}
							{st.eta.toFixed(0)}m
						{/if}
					</div>
				</div>
			</Button>
		{/each}
	</ModalList>
{/if}

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
