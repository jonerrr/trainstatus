<script lang="ts">
	import { fade } from 'svelte/transition';
	import { page } from '$app/stores';
	import {
		is_bus,
		is_train,
		type BusRouteStop,
		type BusStopData,
		type Route,
		type Stop
	} from '$lib/static';
	import { type PersistedRune } from '$lib/util.svelte';
	import { stop_times as rt_stop_times, type StopTime } from '$lib/stop_times.svelte';
	import { trips as rt_trips, TripDirection } from '$lib/trips.svelte';
	import Button from '$lib/Button.svelte';
	import BusArrow from './BusArrow.svelte';
	import Icon from '$lib/Icon.svelte';

	interface Props {
		data: Stop<'train' | 'bus'>;
	}
	let { data }: Props = $props();

	let stop_times = $derived(
		rt_stop_times.stop_times
			.filter((st) => st.stop_id === data.id)
			.map((st) => {
				// const trip = rt_trips.trips.find((t) => t.id === st.trip_id);
				const trip = rt_trips.trips.get(st.trip_id);

				// if (!trip) {
				// 	$inspect(st);
				// }
				return {
					...st,
					eta: (st.arrival.getTime() - new Date().getTime()) / 1000 / 60,
					direction: trip?.direction,
					route_id: trip?.route_id
				};
			})
			// TODO: prevent trips that don't exist from having stop times
			.filter((st) => st.direction !== undefined && st.eta >= 0) as StopTime<
			number,
			TripDirection,
			string
		>[]
	);

	// if its bus, show loading if its in trips but not in stop times
	// const loading = $derived(
	// 	() =>
	// 		is_bus(stop) &&
	// 		!stop.routes.every((r) => {
	// 			const trip = rt_trips.trips.values().find((t) => t.route_id === r.id);
	// 			return trip && stop_times.some((st) => st.trip_id === trip.id);
	// 		})
	// );
</script>

{#snippet eta(n: number)}
	{@const eta = parseInt(n.toFixed(0))}
	<!-- numberflow was causing a hydration mismatch error -->
	<!-- <NumberFlow value={eta} suffix="m" /> -->
	{#key eta}
		<span in:fade={{ duration: 300 }}>
			{eta}m
		</span>
	{/key}
{/snippet}

{#if is_train(data)}
	{#snippet arrivals(
		headsign: string,
		routes: Route[],
		stop_times: StopTime<number, TripDirection, string>[]
	)}
		<div class="flex flex-col mt-auto">
			<div class="font-semibold table-cell text-left max-w-[85%]">
				{headsign}
			</div>
			<div class="flex flex-col gap-1">
				{#each routes as route}
					{@const route_stop_times = stop_times.filter((st) => st.route_id === route.id)}
					<div class="flex gap-1 items-center">
						<Icon height={20} width={20} express={false} link={false} {route} />
						<div class="flex gap-1 items-center">
							{#if route_stop_times.length}
								{#each route_stop_times.slice(0, 2) as stop_time (stop_time.trip_id)}
									{@render eta(stop_time.eta)}
								{/each}
							{:else}
								<div class="text-neutral-400">No trips</div>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		</div>
	{/snippet}

	{@const routes = data.routes}
	{@const base_routes = routes
		.filter((r) => r.type === 'full_time' || r.type === 'part_time')
		.map((r) => r.id)}
	{@const other_routes = stop_times.map((st) => st.route_id)}
	{@const all_routes = [...new Set([...base_routes, ...other_routes])].map(
		(id) => $page.data.routes[id] as Route
	)}

	<div class="grid gap-1 w-full grid-cols-1" in:fade={{ duration: 300 }}>
		<div class="flex gap-1 items-center">
			{#each all_routes as route}
				<Icon height={24} width={24} express={false} link={false} {route} />
			{/each}

			<div class="font-medium my-auto text-left text-lg">
				{stop.name}
			</div>
		</div>
		<div class="grid grid-cols-2 gap-8">
			{@render arrivals(
				data.data.north_headsign,
				all_routes,
				stop_times.filter((st) => st.direction === TripDirection.North)
			)}
			{@render arrivals(
				data.data.south_headsign,
				all_routes,
				stop_times.filter((st) => st.direction === TripDirection.South)
			)}
		</div>
	</div>
{:else if is_bus(data)}
	<!-- TODO: make spacing consistent (use grid maybe idk) -->
	<!-- {@const data = stop.data as BusStopData}
	{@const stop_routes = stop.routes as BusRouteStop[]} -->

	<div class="flex flex-col text-white">
		<div class="flex gap-2 items-center">
			<div>
				<BusArrow direction={data.data.direction} />
			</div>
			<div class="font-bold">
				{stop.name}
			</div>
		</div>

		<div class="flex flex-col">
			{#each data.routes as stop_route}
				{@const route = $page.data.routes[stop_route.id] as Route}
				{@const route_stop_times = stop_times.filter((st) => st.route_id === stop_route.id)}

				<div class="flex gap-2 items-center text-wrap text-left rounded p-1">
					<Icon {route} link={false} express={false} />
					<div class="flex flex-col">
						<div class="">
							{stop_route.headsign}
						</div>
						<div class="flex gap-2 pr-1">
							{#if route_stop_times.length}
								{#each route_stop_times.slice(0, 2) as stop_time}
									{@render eta(stop_time.eta)}
								{/each}
								<!-- check if trips contains trip with route_id and that there are no stop times -->
								<!-- {:else if stop_times.length && !route_stop_times.length}
									<div class="text-neutral-400">Loading...</div> -->
							{:else}
								<div class="text-neutral-400">No trips</div>
							{/if}
						</div>
					</div>
				</div>
			{/each}
		</div>
	</div>

	<div class="text-neutral-300 self-start">
		#{data.id}
	</div>
{/if}
