<script lang="ts">
	import { fade } from 'svelte/transition';
	import { page } from '$app/stores';
	import {
		is_bus,
		is_train,
		type BusRouteStop,
		type BusStopData,
		type Route,
		type Stop,
		type TrainRouteStop,
		type TrainStopData
	} from '$lib/static';
	import { type PersistedRune } from '$lib/util.svelte';
	import {
		stop_times as rt_stop_times,
		monitored_routes,
		type StopTime
	} from '$lib/stop_times.svelte';
	import { trips as rt_trips, TripDirection } from '$lib/trips.svelte';
	import Button from '$lib/Button.svelte';
	import BusArrow from './BusArrow.svelte';
	import Icon from '$lib/Icon.svelte';
	// import { quintOut } from 'svelte/easing';

	interface ButtonProps {
		stop: Stop<'train' | 'bus'>;
		pin_rune: PersistedRune<number[]>;
		large: boolean;
	}
	let { stop, pin_rune = $bindable(), large }: ButtonProps = $props();

	// if stop is a bus stop, add all routes to monitored_routes
	$effect(() => {
		if (is_bus(stop)) {
			for (const route of stop.routes) {
				// console.log('adding route', route.id);
				// monitored_routes.add(route.id);

				// if (monitored_routes.size > )
				if (!monitored_routes.includes(route.id)) {
					console.log('Adding route', route.id);
					// if (monitored_routes.length > 20) {
					// 	console.log('Removing oldest route');
					// 	monitored_routes.shift();
					// }
					monitored_routes.push(route.id);
				}
			}
		}
	});

	// TODO: figure out why it causes an infinite loop (seems to happen after clearing search with 20 routes)
	// $effect(() => {
	// 	if (is_bus(stop)) {
	// 		const to_monitor = stop.routes
	// 			.filter((route) => !monitored_routes.includes(route.id))
	// 			.map((r) => r.id);
	// 		if (to_monitor.length) {
	// 			// remove routes if we're monitoring more than 20
	// 			if (monitored_routes.length + to_monitor.length > 20) {
	// 				console.log('removing routes');
	// 				monitored_routes.splice(0, monitored_routes.length + to_monitor.length - 20);
	// 			}

	// 			console.log('Adding routes', to_monitor);
	// 			monitored_routes.push(...to_monitor);
	// 		}
	// 	}
	// });

	let stop_times = $derived(
		rt_stop_times.stop_times
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
			.filter((st) => st.direction !== undefined && st.eta >= 0) as StopTime<
			number,
			TripDirection,
			string
		>[]
	);
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
		modal: 'stop',
		data: stop
	}}
	bind:pin_rune
>
	{#if is_train(stop)}
		{#snippet arrivals(
			headsign: string,
			routes: Route[],
			stop_times: StopTime<number, TripDirection, string>[],
			large: boolean
		)}
			<!-- <div class="flex flex-col mt-auto" style:width={large ? '' : '40%'}> -->
			<div class="table-row" class:mt-auto={!large} class:my-auto={large}>
				<div
					class="text-neutral-200 font-semibold table-cell text-left"
					class:text-xs={!large}
					class:text-wrap={!large}
				>
					{headsign}
				</div>
				<div class="flex flex-col gap-1">
					{#each routes as route}
						{@const route_stop_times = stop_times.filter((st) => st.route_id === route.id)}
						<div class="flex gap-1">
							<Icon express={false} link={false} {route} />
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

		{@const data = stop.data}
		{@const routes = stop.routes}
		{@const base_routes = routes
			.filter((r) => r.type === 'full_time' || r.type === 'part_time')
			.map((r) => r.id)}
		{@const other_routes = stop_times.map((st) => st.route_id)}
		{@const all_routes = [...new Set([...base_routes, ...other_routes])].map(
			(id) => $page.data.routes[id] as Route
		)}

		{#key large}
			<div
				class="grid gap-1 w-full"
				class:grid-cols-1={large}
				class:grid-cols-3={!large}
				in:fade={{ duration: 300 }}
			>
				<div class="flex gap-1">
					{#if large}
						{#each all_routes as route}
							<Icon
								height={large ? '1.5rem' : '1rem'}
								width={large ? '1.5rem' : '1rem'}
								express={false}
								link={false}
								{route}
							/>
						{/each}
					{/if}

					<div class="font-medium my-auto text-left" class:text-lg={large}>
						{stop.name}
					</div>
				</div>
				<div class="grid grid-cols-2 gap-8">
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
				</div>
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
					{@const route = $page.data.routes[stop_route.id] as Route}
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
					</div>
				{/each}
			</div>
		</div>
	{/if}
</Button>
