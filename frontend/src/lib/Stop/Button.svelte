<script lang="ts">
	import { crossfade, fade } from 'svelte/transition';
	import { page } from '$app/stores';
	import type {
		BusRouteStop,
		BusStopData,
		Route,
		Stop,
		TrainRouteStop,
		TrainStopData
	} from '$lib/static';
	import { type PersistedRune } from '$lib/util.svelte';
	import {
		stop_times as rt_stop_times,
		monitored_routes,
		type StopTime
	} from '$lib/stop_times.svelte';
	import { trips as rt_trips } from '$lib/trips.svelte';
	import Button from '$lib/Button.svelte';
	import BusArrow from './BusArrow.svelte';
	import Icon from '$lib/Icon.svelte';
	import { TripDirection } from '$lib/trips.svelte';
	import { quintOut } from 'svelte/easing';

	interface ButtonProps {
		stop: Stop<'train' | 'bus'>;
		pin_rune: PersistedRune<number[]>;
		large: boolean;
	}
	let { stop, pin_rune, large }: ButtonProps = $props();

	$effect.pre(() => {
		if (stop.type === 'bus') {
			for (const route of stop.routes as BusRouteStop[]) {
				if (!monitored_routes.includes(route.id)) {
					monitored_routes.push(route.id);
				}
			}
		}
	});

	let { stop_times } = $derived.by(() => {
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
					direction: trip?.direction,
					route_id: trip?.route_id
				};
			})
			// TODO: fix so we don't need to filter (maybe store trips in map)
			.filter((st) => st.direction !== undefined && st.eta > 0) as StopTime<
			number,
			TripDirection,
			string
		>[];
		// TODO: also get trips where current stop_id is this stop
		// const trips = rt_trips.trips.filter((t) => arrivals.some((a) => a.trip_id === t.id));
		return {
			stop_times
			// trips
		};
	});
</script>

{#snippet eta(n: number)}
	{@const eta = n.toFixed(0)}
	{#key eta}
		<span class="" in:fade={{ duration: 400 }}>
			{eta}m
		</span>
	{/key}
{/snippet}

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
			stop_times: StopTime<number, TripDirection, string>[],
			large: boolean
		)}
			<!-- <div class="flex flex-col mt-auto" style:width={large ? '' : '40%'}> -->
			<div class="table-row">
				<div
					class="text-neutral-200 font-semibold table-cell"
					class:text-xs={!large}
					class:text-wrap={!large}
					class:text-left={!large}
				>
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
							<div class="flex gap-1" class:text-xs={!large}>
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

		{@const data = stop.data as TrainStopData}
		{@const routes = stop.routes as TrainRouteStop[]}
		{@const base_routes = routes
			.filter((r) => r.type === 'full_time' || r.type === 'part_time')
			.map((r) => r.id)}
		{@const other_routes = stop_times.map((st) => st.route_id)}
		{@const all_routes = [...new Set([...base_routes, ...other_routes])]}
		{#key large}
			<div class="grid grid-cols-3 gap-1" in:fade={{ duration: 300 }}>
				<div class="flex items-center text-left">
					{#if large}
						{#each all_routes as route_id}
							<Icon
								height={large ? '1.5rem' : '1rem'}
								width={large ? '1.5rem' : '1rem'}
								express={false}
								link={false}
								route={$page.data.routes.find((r) => r.id === route_id) as Route}
							/>
						{/each}
					{/if}

					<div class="font-semibold my-auto">
						{stop.name}
					</div>
				</div>
				<!-- <div class="col-span-2 flex justify-between items-center"> -->
				{@render arrivals(
					data.north_headsign,
					all_routes,
					stop_times.filter((st) => st.direction === TripDirection.North),
					large
				)}
				{@render arrivals(
					data.south_headsign,
					all_routes,
					stop_times.filter((st) => st.direction === TripDirection.South),
					large
				)}
				<!-- </div> -->
			</div>
		{/key}
	{:else}
		<!-- TODO: make spacing consistent (use grid maybe idk) -->
		{@const data = stop.data as BusStopData}
		{@const stop_routes = stop.routes as BusRouteStop[]}

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
				{#each stop_routes as stop_route}
					{@const route = $page.data.routes.find((r) => r.id === stop_route.id) as Route}
					{@const route_stop_times = stop_times.filter((st) => st.route_id === stop_route.id)}
					<div class="flex gap-2 items-center text-xs text-wrap text-left rounded p-1">
						<Icon {route} link={false} express={false} />
						<div class="text-neutral-100 max-w-3/4">
							{stop_route.headsign}
						</div>

						<div class="flex gap-2 pr-1">
							{#if route_stop_times.length}
								{#each route_stop_times.slice(0, 2) as stop_time}
									{@render eta(stop_time.eta)}
								{/each}
							{:else}
								<div class="text-xs text-neutral-400">No trips</div>
							{/if}
						</div>

						<!-- <div class="">
							<BusArrivals route_id={route.id} stop_id={stop.id} />
						</div> -->
					</div>
				{/each}
			</div>
		</div>
	{/if}
</Button>
