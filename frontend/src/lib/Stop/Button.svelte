<script lang="ts">
	import { untrack } from 'svelte';

	import { SvelteMap, SvelteSet } from 'svelte/reactivity';

	import { page } from '$app/state';

	import Icon from '$lib/Icon.svelte';
	import BusArrow from '$lib/Stop/BusArrow.svelte';
	import { is_mta_bus, is_mta_subway, main_stop_routes } from '$lib/static';
	import { type StopTime, stop_times as rt_stop_times } from '$lib/stop_times.svelte';
	import { TripDirection, trips as rt_trips } from '$lib/trips.svelte';
	import { current_time } from '$lib/util.svelte';

	import type { Route, Stop } from '@trainstatus/client';

	interface Props {
		data: Stop;
	}
	let { data }: Props = $props();

	const source_routes = $derived(page.data.routes[data.data.source]);

	interface StopTimeData extends StopTime {
		eta: number;
		direction: TripDirection;
		route_id: string;
	}

	type StopTimeByRoute = Map<string, StopTimeData[]>;

	const nb_st_by_route = $state<StopTimeByRoute>(new SvelteMap());
	const sb_st_by_route = $state<StopTimeByRoute>(new SvelteMap());
	const active_routes = $state(new SvelteSet<Route>());

	$effect(() => {
		rt_stop_times?.by_stop_id;
		rt_trips.trips;
		data;

		// console.log('Triggering update for', data.name);
		untrack(() => updateRouteMaps());
	});

	const updateRouteMaps = () => {
		// Clear existing maps
		nb_st_by_route.clear();
		sb_st_by_route.clear();

		// Early exit if no data
		// if (!rt_stop_times?.stop_times?.length) {
		// 	return;
		// }
		const stop_times = rt_stop_times.by_stop_id[data.id] ?? [];

		for (const st of stop_times) {
			if (st.arrival.getTime() < current_time.ms) continue;
			const trip = rt_trips.trips.get(st.trip_id);
			if (!trip) continue;

			const route_id = trip.route_id;
			active_routes.add(page.data.routes[route_id]);

			const eta = (st.arrival.getTime() - current_time.ms) / 1000 / 60;

			const stopTimeData = {
				...st,
				eta,
				direction: trip.direction,
				route_id
			};

			if (is_mta_subway(data.data)) {
				const target_map = trip.direction === TripDirection.North ? nb_st_by_route : sb_st_by_route;

				if (!target_map.has(route_id)) {
					target_map.set(route_id, []);
				}

				const times = target_map.get(route_id)!;
				if (times.length < 2) {
					times.push(stopTimeData);
				}
			} else {
				if (!nb_st_by_route.has(route_id)) {
					nb_st_by_route.set(route_id, []);
				}

				const times = nb_st_by_route.get(route_id)!;
				if (times.length < 2) {
					times.push(stopTimeData);
				}
			}
		}
	};

	// TODO: add back current stop and active stop route combinations
	// TODO: fix 1 train showing up at dekalb ave for some reason
	// const current_stop_routes = $derived(main_stop_routes(data).map((r) => page.data.routes[r.id]));
	const default_stop_routes = $derived(data.routes.map((r) => source_routes[r.route_id]));
	// combine current stop routes with other active routes at stop
	// TODO: use nb_st_by_route and sb_st_by_route to get active routes instead of creating new set
	// const all_stop_routes = $derived([...new Set(current_stop_routes.concat(...active_routes))]);
	// const all_stop_routes = $derived(current_stop_routes);
	// TODO: fix all stop routes
	const all_stop_routes = $derived.by(() => {
		const routes: Route[] = [];
		// const route_ids = new Set<string>();

		// add main stop routes first
		for (const route of data.routes) {
			routes.push(source_routes[route.route_id]);
			// route_ids.add(route.id);
		}

		// add active routes next
		// for (const route of active_routes) {
		// 	if (!route_ids.has(route.id)) {
		// 		routes.push(route);
		// 		route_ids.add(route.id);
		// 	}
		// }

		return routes;
	});
</script>

<!-- eta used by bus and train -->
{#snippet eta(n: number)}
	{@const eta = parseInt(n.toFixed(0))}
	{#key eta}
		<span class="rounded-sm bg-neutral-800/70 px-1.5 py-0.5 text-sm font-medium">
			{eta}m
		</span>
	{/key}
{/snippet}

{#if data.data.source === 'mta_subway'}
	<div class="grid w-full grid-cols-1 gap-1">
		<div class="flex items-center gap-1">
			{#each default_stop_routes as route (route.id)}
				<Icon height={24} width={24} link={false} {route} />
			{/each}

			<div class="my-auto text-left text-lg font-medium">
				{data.name}
			</div>
		</div>
		<div class="grid grid-cols-2 gap-8">
			{@render arrivals(data.data.north_headsign, all_stop_routes, nb_st_by_route)}
			{@render arrivals(data.data.south_headsign, all_stop_routes, sb_st_by_route)}
		</div>
	</div>

	{#snippet arrivals(headsign: string, routes: Route[], stop_times: StopTimeByRoute)}
		<div class="mt-auto flex flex-col">
			<div class="table-cell max-w-[85%] text-left font-semibold">
				{headsign}
			</div>
			<div class="flex flex-col gap-1">
				{#each routes as route (route.id)}
					{@const route_stop_times = stop_times.get(route.id) ?? []}
					<!-- {@const route_stop_times = stop_times.filter((st) => st.route_id === route.id)} -->
					<div class="flex items-center gap-1">
						<Icon height={20} width={20} link={false} {route} />
						<div class="flex items-center gap-1">
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
{:else if data.data.source === 'mta_bus'}
	<!-- TODO: make spacing consistent (use grid maybe idk) -->
	<div class="flex flex-col text-white">
		<div class="flex items-center">
			<div>
				<BusArrow direction={data.data.direction} />
			</div>
			<div class="text-left text-lg font-medium">
				{data.name}
			</div>
		</div>

		<div class="flex flex-col">
			{#each data.routes as route_stop (route_stop.route_id)}
				{@const route = source_routes[route_stop.route_id]}
				<!-- {@const route_stop_times = stop_times.filter((st) => st.route_id === stop_route.id)} -->
				{@const route_stop_times = nb_st_by_route.get(route_stop.route_id) ?? []}

				<div class="flex items-center gap-2 rounded-sm p-1 text-left text-wrap">
					<Icon {route} link={false} />
					<div class="flex flex-col">
						<div>
							<!-- TODO: handle other sources -->
							{route_stop.data.source === 'mta_bus' && route_stop.data.headsign}
						</div>
						<div class="flex gap-2 pr-1">
							{#if rt_stop_times.updating_routes.has(route.id)}
								<div class="flex items-center gap-1 py-1">
									<div class="h-[1em] w-6 animate-pulse bg-neutral-700"></div>
									<div class="h-[1em] w-6 animate-pulse bg-neutral-700"></div>
								</div>
							{:else if route_stop_times.length}
								{#each route_stop_times.slice(0, 2) as stop_time (stop_time.trip_id)}
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

	<div class="self-start text-neutral-300">
		#{data.id}
	</div>
{/if}
