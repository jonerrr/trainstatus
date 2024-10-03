<script lang="ts">
	import { page } from '$app/stores';
	import type { BusStopData, Route, Stop, TrainRouteStop, TrainStopData } from '$lib/static';
	import { type PersistedRune } from '$lib/util.svelte';
	import { stop_times as rt_stop_times, type StopTime } from '$lib/stop_times.svelte';
	import { trips as rt_trips } from '$lib/trips.svelte';
	import Button from '$lib/Button.svelte';
	import BusArrow from './BusArrow.svelte';
	import Icon from '$lib/Icon.svelte';
	import { TripDirection } from '$lib/trips.svelte';

	interface ButtonProps {
		stop: Stop<'train' | 'bus'>;
		pin_rune: PersistedRune<number[]>;
	}

	let { stop, pin_rune }: ButtonProps = $props();

	let { stop_times } = $derived.by(() => {
		// get arrival for stop and add eta
		const stop_times = rt_stop_times.stop_times
			.filter((st) => st.stop_id === stop.id)
			.map((st) => {
				const trip = rt_trips.trips.find((t) => t.id === st.trip_id);
				if (!trip) {
					$inspect(st);
				}
				return {
					...st,
					eta: (st.arrival.getTime() - new Date().getTime()) / 1000 / 60,
					direction: trip?.direction,
					route_id: trip?.route_id
				};
			})
			// TODO: fix so we don't need to filter (maybe store trips in map)
			.filter((st) => st.direction !== undefined) as StopTime<number, TripDirection, string>[];
		// TODO: also get trips where current stop_id is this stop
		// const trips = rt_trips.trips.filter((t) => arrivals.some((a) => a.trip_id === t.id));
		return {
			stop_times
			// trips
		};
	});

	// $inspect(arrivals, arrival_trips);
</script>

<Button
	state={{
		dialog_id: stop.id,
		dialog_type: 'stop',
		dialog_open: true,
		data: stop
	}}
	{pin_rune}
>
	{#if stop.type === 'train'}
		{#snippet arrivals(
			headsign: string,
			route_ids: string[],
			stop_times: StopTime<number, TripDirection, string>[]
		)}
			<div class="flex flex-col w-[30%] mt-auto">
				<div class="text-xs text-neutral-200 text-wrap text-left pb-1">
					{headsign}
				</div>
				<div class="flex flex-col gap-1">
					{#each route_ids as route_id}
						{@const route_stop_times = stop_times.filter((st) => st.route_id === route_id)}
						<div class="flex gap-1">
							<Icon
								express={false}
								link={false}
								route={$page.data.routes.find((r) => r.id === route_id) as Route}
							/>
							<!-- TODO: animation when eta changes -->
							<div class="flex text-xs">
								{#if route_stop_times.length}
									{#each route_stop_times.slice(0, 2) as stop_time}
										{stop_time.eta.toFixed(0)}m {' '}
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

		{@const data = stop.data as TrainStopData}
		{@const routes = stop.routes as TrainRouteStop[]}
		{@const base_routes = routes
			.filter((r) => r.type === 'full_time' || r.type === 'part_time')
			.map((r) => r.id)}
		{@const other_routes = stop_times.map((st) => st.route_id)}
		{@const all_routes = [...new Set([...base_routes, ...other_routes])]}

		<div class="w-[25%] grow-0 text-neutral-100">
			{stop.name}
		</div>

		{@render arrivals(
			data.north_headsign,
			all_routes,
			stop_times.filter((st) => st.direction === TripDirection.North)
		)}
		{@render arrivals(
			data.south_headsign,
			all_routes,
			stop_times.filter((st) => st.direction === TripDirection.South)
		)}
	{:else}
		<!-- TODO: make spacing consistent (use grid maybe idk) -->
		{@const data = stop.data as BusStopData}

		<div class="flex flex-col text-left text-xs">
			<div class="flex gap-2">
				<div>
					<BusArrow direction={data.direction} />
				</div>
				<div class="font-bold">
					{stop.name}
				</div>
				<div class="text-neutral-300 font-bold">
					#{stop.id}
				</div>
			</div>

			<div class="flex flex-col">
				<!-- {#each stop_routes as route}
					<div class="flex gap-2 items-center text-xs text-wrap text-left rounded p-1">
						<BusIcon {route} />
						<div class="text-neutral-100 max-w-[60%]">
							{stop.routes.find((r) => r.id === route.id)?.headsign}
						</div>

						<div class="">
							<BusArrivals route_id={route.id} stop_id={stop.id} />
						</div>
					</div>
				{/each} -->
			</div>
		</div>
	{/if}
</Button>
